use super::{ComposeRuntime, RuntimeError};
use crate::config::{ContextMode, SkipPolicy};
use crate::resolver::resolve_refs;
use serde_json::{json, Map, Value};

impl ComposeRuntime {
    pub(super) fn build_task_input(
        &self,
        task_name: &str,
        workflow_input: &Value,
        task_state: &Map<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let task = self
            .config
            .tasks
            .get(task_name)
            .ok_or_else(|| RuntimeError::Invalid(format!("missing task {task_name}")))?;
        let root_ctx = json!({"input": workflow_input, "tasks": task_state});
        let mut resolved = resolve_refs(&task.input, &root_ctx)?;
        if !resolved.is_object() {
            return Err(RuntimeError::Invalid(format!(
                "tasks.{task_name}.input must resolve to object"
            )));
        }

        if matches!(
            self.config.runtime.context_mode,
            ContextMode::MergedAndRefs | ContextMode::MergedOnly
        ) {
            let merged = self.merged_needs_context(task_name, task_state);
            if let Value::Object(ref mut obj) = resolved {
                for (k, v) in merged {
                    obj.entry(k).or_insert(v);
                }
            }
        }

        Ok(resolved)
    }

    fn merged_needs_context(
        &self,
        task_name: &str,
        task_state: &Map<String, Value>,
    ) -> Map<String, Value> {
        let mut merged = Map::new();
        let Some(task) = self.config.tasks.get(task_name) else {
            return merged;
        };

        for dep in &task.needs {
            let Some(dep_state) = task_state.get(dep).and_then(|v| v.as_object()) else {
                continue;
            };
            if dep_state.get("status").and_then(Value::as_str) != Some("completed") {
                continue;
            }

            let is_parallel = dep_state
                .get("parallel")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let output = dep_state.get("output").cloned().unwrap_or(Value::Null);
            if !is_parallel {
                if let Some(obj) = output.as_object() {
                    for (k, v) in obj {
                        merged.entry(k.clone()).or_insert(v.clone());
                    }
                }
            } else if let Some(obj) = output.as_object() {
                for (agent_id, agent_output) in obj {
                    merged
                        .entry(agent_id.clone())
                        .or_insert(agent_output.clone());
                    if let Some(inner) = agent_output.as_object() {
                        for (k, v) in inner {
                            merged.entry(k.clone()).or_insert(v.clone());
                        }
                    }
                }
            }
        }

        merged
    }

    pub(super) fn should_skip(
        &self,
        task_name: &str,
        task_state: &Map<String, Value>,
    ) -> Result<bool, RuntimeError> {
        if self.config.runtime.skip_policy != SkipPolicy::GatekeeperControlled {
            return Ok(false);
        }
        let gate_cfg = &self.config.runtime.gatekeeper;
        if !gate_cfg.skip_tasks.iter().any(|name| name == task_name) {
            return Ok(false);
        }

        let Some(gate) = task_state.get(&gate_cfg.task).and_then(Value::as_object) else {
            return Ok(false);
        };
        let Some(output) = gate.get("output").and_then(Value::as_object) else {
            return Ok(false);
        };

        let value = output
            .get(&gate_cfg.field)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(!value)
    }

    pub(super) fn mark_skipped(
        &self,
        task_name: &str,
        task_state: &mut Map<String, Value>,
    ) -> Result<(), RuntimeError> {
        let task = self
            .config
            .tasks
            .get(task_name)
            .ok_or_else(|| RuntimeError::Invalid(format!("unknown task {task_name}")))?;

        if let Some(agent_id) = &task.agent {
            let default_output = self.safe_default_output(agent_id);
            task_state.insert(
                task_name.to_string(),
                json!({"status": "skipped", "parallel": false, "output": default_output}),
            );
            return Ok(());
        }

        if let Some(agent_ids) = &task.agents {
            let mut merged = Map::new();
            for agent_id in agent_ids {
                merged.insert(agent_id.clone(), self.safe_default_output(agent_id));
            }
            let mut full = Map::new();
            full.insert("status".to_string(), Value::String("skipped".to_string()));
            full.insert("parallel".to_string(), Value::Bool(true));
            full.insert("output".to_string(), Value::Object(merged.clone()));
            for (k, v) in merged {
                full.insert(k, v);
            }
            task_state.insert(task_name.to_string(), Value::Object(full));
            return Ok(());
        }

        task_state.insert(
            task_name.to_string(),
            json!({"status": "skipped", "parallel": false, "output": {}}),
        );
        Ok(())
    }

    fn safe_default_output(&self, agent_id: &str) -> Value {
        if let Some(agent) = self.config.agents.get(agent_id)
            && let Ok(v) = self.schema.validate(&agent.output_model, json!({}))
        {
            return v;
        }
        json!({})
    }
}
