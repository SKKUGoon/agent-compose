use super::constants::BUILD_PIPELINE_OUTPUT_STEP;
use super::events::emit;
use super::{ComposeRuntime, RuntimeError, RuntimeEvent, steps};
use crate::config::AgentConfig;
use crate::provider::PromptRequest;
use crate::resolver::resolve_refs;
use futures::future::join_all;
use serde_json::{Map, Value, json};
use tokio::time::{Duration, sleep};
use tokio::sync::mpsc::UnboundedSender;

impl ComposeRuntime {
    pub async fn run(
        &self,
        workflow_input: Value,
        model_override: Option<String>,
    ) -> Result<Value, RuntimeError> {
        self.run_with_events(workflow_input, model_override, None).await
    }

    pub async fn run_with_events(
        &self,
        workflow_input: Value,
        model_override: Option<String>,
        event_tx: Option<UnboundedSender<RuntimeEvent>>,
    ) -> Result<Value, RuntimeError> {
        let mut task_state = Map::new();

        for task_name in &self.order {
            emit(
                &event_tx,
                RuntimeEvent::TaskStarted {
                    task: task_name.clone(),
                },
            );

            let task = self
                .config
                .tasks
                .get(task_name)
                .ok_or_else(|| RuntimeError::Invalid(format!("missing task {task_name}")))?;

            if self.should_skip(task_name, &task_state)? {
                self.mark_skipped(task_name, &mut task_state)?;
                emit(
                    &event_tx,
                    RuntimeEvent::TaskSkipped {
                        task: task_name.clone(),
                    },
                );
                continue;
            }

            let payload = self.build_task_input(task_name, &workflow_input, &task_state)?;

            if let Some(agent_id) = &task.agent {
                let output = self
                    .run_agent(
                        task_name,
                        agent_id,
                        payload,
                        model_override.clone(),
                        &event_tx,
                    )
                    .await?;
                task_state.insert(
                    task_name.clone(),
                    json!({"status": "completed", "parallel": false, "output": output}),
                );
                emit(
                    &event_tx,
                    RuntimeEvent::TaskCompleted {
                        task: task_name.clone(),
                    },
                );
                continue;
            }

            if let Some(agent_ids) = &task.agents {
                let futures = agent_ids.iter().map(|agent_id| {
                    self.run_agent(
                        task_name,
                        agent_id,
                        payload.clone(),
                        model_override.clone(),
                        &event_tx,
                    )
                });
                let outputs = join_all(futures).await;
                let mut merged = Map::new();
                for (idx, res) in outputs.into_iter().enumerate() {
                    let out = res?;
                    let agent_id = &agent_ids[idx];
                    merged.insert(agent_id.clone(), out);
                }
                let mut obj = Map::new();
                obj.insert("status".to_string(), Value::String("completed".to_string()));
                obj.insert("parallel".to_string(), Value::Bool(true));
                obj.insert("output".to_string(), Value::Object(merged.clone()));
                for (k, v) in merged {
                    obj.insert(k, v);
                }
                task_state.insert(task_name.clone(), Value::Object(obj));
                emit(
                    &event_tx,
                    RuntimeEvent::TaskCompleted {
                        task: task_name.clone(),
                    },
                );
                continue;
            }

            if let Some(step_name) = &task.step {
                emit(
                    &event_tx,
                    RuntimeEvent::StepStarted {
                        task: task_name.clone(),
                        step: step_name.clone(),
                    },
                );
                let output = self.run_step(step_name, payload)?;
                task_state.insert(
                    task_name.clone(),
                    json!({"status": "completed", "parallel": false, "output": output}),
                );
                emit(
                    &event_tx,
                    RuntimeEvent::StepCompleted {
                        task: task_name.clone(),
                        step: step_name.clone(),
                    },
                );
                emit(
                    &event_tx,
                    RuntimeEvent::TaskCompleted {
                        task: task_name.clone(),
                    },
                );
                continue;
            }
        }

        let ctx = json!({"input": workflow_input, "tasks": task_state});
        let final_expr = format!("${{{{ {} }}}}", self.config.output.from_path);
        let final_value = resolve_refs(&Value::String(final_expr), &ctx)?;
        if !final_value.is_object() {
            return Err(RuntimeError::Invalid(
                "workflow output must resolve to object".to_string(),
            ));
        }
        Ok(final_value)
    }

    async fn run_agent(
        &self,
        task_name: &str,
        agent_id: &str,
        payload: Value,
        model_override: Option<String>,
        event_tx: &Option<UnboundedSender<RuntimeEvent>>,
    ) -> Result<Value, RuntimeError> {
        let spec = self
            .config
            .agents
            .get(agent_id)
            .ok_or_else(|| RuntimeError::Invalid(format!("unknown agent {agent_id}")))?;

        let validated_input = self.schema.validate(&spec.input_model, payload)?;
        let output_contract = self.schema.output_contract(&spec.output_model)?;
        let model = self.resolve_model(spec, model_override)?;
        let max_attempts = self.config.runtime.retry.contract_max_attempts.max(1);
        let backoff_ms = self.config.runtime.retry.contract_backoff_ms;

        emit(
            event_tx,
            RuntimeEvent::AgentStarted {
                task: task_name.to_string(),
                agent: agent_id.to_string(),
                model: model.clone(),
            },
        );

        let mut last_contract_error: Option<String> = None;

        for attempt in 1..=max_attempts {
            let instructions = if let Some(err) = &last_contract_error {
                format!(
                    "{}\n\nPrevious attempt failed schema contract ({err}). Return ONLY corrected JSON that matches output model {} exactly.",
                    spec.instructions, spec.output_model
                )
            } else {
                spec.instructions.clone()
            };

            let raw = self
                .providers
                .invoke(PromptRequest {
                    provider: self.providers.provider_config().clone(),
                    model: model.clone(),
                    instructions,
                    input_json: validated_input.clone(),
                    output_model_name: spec.output_model.clone(),
                    output_contract_json: output_contract.clone(),
                })
                .await?;

            let contract_result = if !raw.is_object() {
                Err(format!("agent {agent_id} output must be JSON object"))
            } else {
                self.schema
                    .validate(&spec.output_model, raw)
                    .map_err(|err| format!("agent {agent_id} output failed contract {}: {err}", spec.output_model))
            };

            match contract_result {
                Ok(validated) => {
                    emit(
                        event_tx,
                        RuntimeEvent::AgentCompleted {
                            task: task_name.to_string(),
                            agent: agent_id.to_string(),
                        },
                    );
                    return Ok(validated);
                }
                Err(err) => {
                    if attempt < max_attempts {
                        emit(
                            event_tx,
                            RuntimeEvent::AgentRetrying {
                                task: task_name.to_string(),
                                agent: agent_id.to_string(),
                                attempt: attempt + 1,
                                reason: err.clone(),
                            },
                        );
                        last_contract_error = Some(err);
                        if backoff_ms > 0 {
                            sleep(Duration::from_millis(backoff_ms)).await;
                        }
                        continue;
                    }
                    return Err(RuntimeError::Invalid(err));
                }
            }
        }

        Err(RuntimeError::Invalid(format!(
            "agent {agent_id} output failed contract {}",
            spec.output_model
        )))
    }

    fn resolve_model(
        &self,
        spec: &AgentConfig,
        model_override: Option<String>,
    ) -> Result<String, RuntimeError> {
        if let Some(model) = model_override {
            return Ok(model);
        }
        if let Some(model) = &spec.model {
            return Ok(model.clone());
        }
        let provider = self.providers.provider_config();
        if let Some(model) = &provider.default_model {
            return Ok(model.clone());
        }
        Err(RuntimeError::Invalid("no model configured".to_string()))
    }

    fn run_step(&self, step_name: &str, payload: Value) -> Result<Value, RuntimeError> {
        match step_name {
            BUILD_PIPELINE_OUTPUT_STEP => steps::build_pipeline_output(payload),
            _ => Err(RuntimeError::Invalid(format!(
                "unsupported step: {step_name}"
            ))),
        }
    }
}
