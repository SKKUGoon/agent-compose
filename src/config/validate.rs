use super::constants::SUPPORTED_STEPS;
use super::ComposeConfig;
use std::collections::{HashMap, HashSet, VecDeque};

impl ComposeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != "2" {
            return Err("version must be \"2\"".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if self.schema.file.trim().is_empty() {
            return Err("schema.file cannot be empty".to_string());
        }
        if self.schema.models.is_empty() {
            return Err("schema.models cannot be empty".to_string());
        }
        if self.chains.is_empty() {
            return Err("chains cannot be empty".to_string());
        }

        let mut serve_targets = HashSet::new();
        for (chain_id, chain) in &self.chains {
            if chain_id.trim().is_empty() {
                return Err("chain id cannot be empty".to_string());
            }
            let target = (chain.serve.host.clone(), chain.serve.port);
            if !serve_targets.insert(target.clone()) {
                return Err(format!(
                    "duplicate serve target {}:{} across chains",
                    target.0, target.1
                ));
            }
            self.validate_chain(chain_id)?;
        }

        Ok(())
    }

    fn validate_chain(&self, chain_id: &str) -> Result<(), String> {
        let chain = self
            .chains
            .get(chain_id)
            .ok_or_else(|| format!("unknown chain: {chain_id}"))?;

        if chain.provider.api_key.trim().is_empty() {
            return Err(format!(
                "chains.{chain_id}.provider.api_key cannot be empty"
            ));
        }
        if chain.serve.host.trim().is_empty() {
            return Err(format!("chains.{chain_id}.serve.host cannot be empty"));
        }

        if chain.agents.is_empty() {
            return Err(format!("chains.{chain_id}.agents cannot be empty"));
        }
        if chain.tasks.is_empty() {
            return Err(format!("chains.{chain_id}.tasks cannot be empty"));
        }

        for (agent_id, agent) in &chain.agents {
            if !self.schema.models.contains_key(&agent.input_model) {
                return Err(format!(
                    "chains.{chain_id}.agents.{agent_id}.input_model references unknown model: {}",
                    agent.input_model
                ));
            }
            if !self.schema.models.contains_key(&agent.output_model) {
                return Err(format!(
                    "chains.{chain_id}.agents.{agent_id}.output_model references unknown model: {}",
                    agent.output_model
                ));
            }
            if agent.instructions.trim().is_empty() {
                return Err(format!(
                    "chains.{chain_id}.agents.{agent_id}.instructions cannot be empty"
                ));
            }
        }

        if chain.provider.default_model.is_none()
            && chain.agents.values().any(|agent| agent.model.is_none())
        {
            return Err(format!(
                "chains.{chain_id}.provider.default_model required when an agent has no model override"
            ));
        }

        if chain.runtime.retry.contract_max_attempts == 0 {
            return Err(format!(
                "chains.{chain_id}.runtime.retry.contract_max_attempts must be >= 1"
            ));
        }

        if chain.runtime.skip_policy == super::SkipPolicy::GatekeeperControlled {
            if chain.runtime.gatekeeper.task.trim().is_empty() {
                return Err(format!(
                    "chains.{chain_id}.runtime.gatekeeper.task cannot be empty"
                ));
            }
            if chain.runtime.gatekeeper.field.trim().is_empty() {
                return Err(format!(
                    "chains.{chain_id}.runtime.gatekeeper.field cannot be empty"
                ));
            }
            if chain.runtime.gatekeeper.skip_tasks.is_empty() {
                return Err(format!(
                    "chains.{chain_id}.runtime.gatekeeper.skip_tasks cannot be empty"
                ));
            }
            for skip_task in &chain.runtime.gatekeeper.skip_tasks {
                if !chain.tasks.contains_key(skip_task) {
                    return Err(format!(
                        "chains.{chain_id}.runtime.gatekeeper.skip_tasks contains unknown task: {skip_task}"
                    ));
                }
            }
        }

        for (task_id, task) in &chain.tasks {
            for dep in &task.needs {
                if !chain.tasks.contains_key(dep) {
                    return Err(format!(
                        "chains.{chain_id}.tasks.{task_id}.needs contains unknown task: {dep}"
                    ));
                }
            }

            let mut choices = 0;
            if task.agent.is_some() {
                choices += 1;
            }
            if task.agents.as_ref().map(|v| !v.is_empty()).unwrap_or(false) {
                choices += 1;
            }
            if task.step.is_some() {
                choices += 1;
            }
            if task.python_step.is_some() {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id}.python_step is invalid; use Rust step"
                ));
            }
            if choices != 1 {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id} must define exactly one of agent, agents, step"
                ));
            }

            if let Some(agent_id) = &task.agent
                && !chain.agents.contains_key(agent_id)
            {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id}.agent references unknown agent: {agent_id}"
                ));
            }

            if let Some(agent_ids) = &task.agents {
                if agent_ids.is_empty() {
                    return Err(format!(
                        "chains.{chain_id}.tasks.{task_id}.agents cannot be empty"
                    ));
                }
                for agent_id in agent_ids {
                    if !chain.agents.contains_key(agent_id) {
                        return Err(format!(
                            "chains.{chain_id}.tasks.{task_id}.agents references unknown agent: {agent_id}"
                        ));
                    }
                }
            }

            if let Some(step) = &task.step
                && step.trim().is_empty()
            {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id}.step cannot be empty"
                ));
            }
            if let Some(step) = &task.step
                && !SUPPORTED_STEPS.contains(&step.as_str())
            {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id}.step unknown Rust step: {step}"
                ));
            }

            if !task.input.is_object() {
                return Err(format!(
                    "chains.{chain_id}.tasks.{task_id}.input must be an object"
                ));
            }
        }

        if chain.output.from_path.trim().is_empty() {
            return Err(format!("chains.{chain_id}.output.from cannot be empty"));
        }

        self.ensure_acyclic(chain_id)
    }

    pub fn chain_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.chains.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn topological_tasks(&self, chain_id: &str) -> Result<Vec<String>, String> {
        let chain = self
            .chains
            .get(chain_id)
            .ok_or_else(|| format!("unknown chain: {chain_id}"))?;

        let mut indegree: HashMap<String, usize> =
            chain.tasks.keys().map(|k| (k.clone(), 0usize)).collect();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for (task_id, task) in &chain.tasks {
            for dep in &task.needs {
                graph.entry(dep.clone()).or_default().push(task_id.clone());
                if let Some(count) = indegree.get_mut(task_id) {
                    *count += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = indegree
            .iter()
            .filter_map(|(k, v)| if *v == 0 { Some(k.clone()) } else { None })
            .collect();
        let mut out = Vec::new();

        while let Some(node) = queue.pop_front() {
            out.push(node.clone());
            if let Some(nexts) = graph.get(&node) {
                for next in nexts {
                    if let Some(count) = indegree.get_mut(next) {
                        *count -= 1;
                        if *count == 0 {
                            queue.push_back(next.clone());
                        }
                    }
                }
            }
        }

        if out.len() != chain.tasks.len() {
            return Err(format!("chains.{chain_id}.tasks graph contains a cycle"));
        }
        Ok(out)
    }

    fn ensure_acyclic(&self, chain_id: &str) -> Result<(), String> {
        let ordered = self.topological_tasks(chain_id)?;
        let chain = self
            .chains
            .get(chain_id)
            .ok_or_else(|| format!("unknown chain: {chain_id}"))?;
        if ordered.len() != chain.tasks.len() {
            return Err(format!("chains.{chain_id}.tasks graph contains a cycle"));
        }

        let model_names: HashSet<String> = self.schema.models.keys().cloned().collect();
        for (model_name, model) in &self.schema.models {
            if model.kind != "object" {
                return Err(format!("schema.models.{model_name}.type must be object"));
            }
            for (field_name, field) in &model.fields {
                if let Some(r) = &field.ref_model
                    && !model_names.contains(r)
                {
                    return Err(format!(
                        "schema.models.{model_name}.fields.{field_name} references unknown model: {r}"
                    ));
                }
            }
        }

        Ok(())
    }
}
