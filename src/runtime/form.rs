use super::ComposeRuntime;
use crate::config::FieldSpec;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct FormFieldHint {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct FormSpec {
    pub model: String,
    pub fields: Vec<FormFieldHint>,
}

impl ComposeRuntime {
    pub fn task_order(&self) -> Vec<String> {
        self.order.clone()
    }

    pub fn chain_label(&self, task_name: &str) -> Option<String> {
        let task = self.config.tasks.get(task_name)?;
        if let Some(agent) = &task.agent {
            return Some(format!("{task_name}: {agent}"));
        }
        if let Some(agents) = &task.agents {
            return Some(format!("{task_name}: [{}]", agents.join(", ")));
        }
        if let Some(step) = &task.step {
            return Some(format!("{task_name}: step({step})"));
        }
        Some(task_name.to_string())
    }

    pub fn parallel_agents_for_task(&self, task_name: &str) -> Option<Vec<String>> {
        let task = self.config.tasks.get(task_name)?;
        let agents = task.agents.clone()?;
        if agents.len() > 1 {
            Some(agents)
        } else {
            None
        }
    }

    pub fn default_form_spec(&self) -> Option<FormSpec> {
        let first = self.order.first()?;
        let task = self.config.tasks.get(first)?;
        let agent_id = task.agent.as_ref()?;
        let agent = self.config.agents.get(agent_id)?;
        let model_name = agent.input_model.clone();
        let model = self.models.get(&model_name)?;

        let mut fields = Vec::new();
        for (name, spec) in &model.fields {
            fields.push(FormFieldHint {
                name: name.clone(),
                kind: field_kind(spec),
                required: spec.required.unwrap_or(false),
                default_value: spec.default.clone(),
            });
        }

        fields.sort_by(|a, b| a.name.cmp(&b.name));
        Some(FormSpec {
            model: model_name,
            fields,
        })
    }

    pub fn prefers_structured_output(&self) -> bool {
        let path = self.config.output.from_path.as_str();
        if !path.starts_with("tasks.") {
            return false;
        }
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 3 {
            return false;
        }
        let task_name = parts[1];
        let Some(task) = self.config.tasks.get(task_name) else {
            return false;
        };
        task.agent.is_some() || task.agents.is_some() || task.step.is_some()
    }
}

fn field_kind(spec: &FieldSpec) -> String {
    if let Some(r) = &spec.ref_model {
        return format!("ref:{r}");
    }
    spec.kind.clone().unwrap_or_else(|| "unknown".to_string())
}
