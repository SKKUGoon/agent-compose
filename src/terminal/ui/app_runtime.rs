use super::{App, RuntimeEvent};

impl App {
    pub(super) fn apply_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::TaskStarted { task } => {
                self.set_task(&task, "running", "");
                self.current_task = Some(task.clone());
                self.logs.push(format!("Task started: {task}"));
            }
            RuntimeEvent::TaskCompleted { task } => {
                self.set_task(&task, "done", "");
                self.complete_children_for_task(&task);
                if self.current_task.as_deref() == Some(task.as_str()) {
                    self.current_task = None;
                    self.current_agent = None;
                }
                self.logs.push(format!("Task completed: {task}"));
            }
            RuntimeEvent::TaskSkipped { task } => {
                self.set_task(&task, "skipped", "gatekeeper skipped");
                self.mark_children_for_task(&task, "skipped", "gatekeeper skipped");
                if self.current_task.as_deref() == Some(task.as_str()) {
                    self.current_task = None;
                    self.current_agent = None;
                }
                self.logs.push(format!("Task skipped: {task}"));
            }
            RuntimeEvent::AgentStarted { task, agent, model } => {
                self.set_task(&task, "running", &format!("{agent} ({model})"));
                self.set_agent_status(&task, &agent, "running", &model);
                self.current_task = Some(task);
                self.current_agent = Some(agent);
            }
            RuntimeEvent::AgentCompleted { task, agent } => {
                self.set_task(&task, "running", "");
                self.set_agent_status(&task, &agent, "done", "");
                if self.current_agent.as_deref() == Some(agent.as_str()) {
                    self.current_agent = None;
                }
            }
            RuntimeEvent::AgentRetrying {
                task,
                agent,
                attempt,
                reason,
            } => {
                self.set_task(&task, "retrying", &format!("{agent} retry #{attempt}"));
                self.set_agent_status(&task, &agent, "retrying", &format!("retry #{attempt}"));
                self.logs.push(format!(
                    "Retrying {task}/{agent} attempt #{attempt}: {reason}"
                ));
            }
            RuntimeEvent::StepStarted { task, step } => {
                self.set_task(&task, "running", &format!("step {step}"));
                self.current_task = Some(task);
                self.current_agent = Some(format!("step:{step}"));
            }
            RuntimeEvent::StepCompleted { task, step } => {
                self.set_task(&task, "done", &format!("step {step}"));
                self.current_task = None;
                self.current_agent = None;
            }
        }
        if self.logs.len() > 500 {
            let drain = self.logs.len().saturating_sub(500);
            self.logs.drain(0..drain);
        }
    }

    pub(super) fn mark_run_failed(&mut self, err: &str) {
        if let Some(task) = self.current_task.clone() {
            self.set_task(&task, "failed", err);
            if let Some(agent) = self.current_agent.clone() {
                self.set_agent_status(&task, &agent, "failed", err);
            } else {
                self.mark_running_children_failed(&task, err);
            }
        } else if let Some(entry) = self.chain.iter_mut().find(|e| e.status == "running") {
            entry.status = "failed".to_string();
            entry.detail = err.to_string();
            for child in &mut entry.children {
                if child.status == "running" {
                    child.status = "failed".to_string();
                    child.detail = err.to_string();
                }
            }
        }

        for entry in &mut self.chain {
            if entry.status == "queued" {
                entry.status = "aborted".to_string();
                entry.detail = "stopped after failure".to_string();
            }
            for child in &mut entry.children {
                if child.status == "queued" {
                    child.status = "aborted".to_string();
                    child.detail = "stopped after failure".to_string();
                }
            }
        }

        self.current_task = None;
        self.current_agent = None;
    }

    fn set_task(&mut self, task: &str, status: &str, detail: &str) {
        if let Some(idx) = self.chain_index.get(task).copied()
            && let Some(entry) = self.chain.get_mut(idx)
        {
            entry.status = status.to_string();
            if !detail.is_empty() {
                entry.detail = detail.to_string();
            } else {
                entry.detail.clear();
            }
        }
    }

    fn set_agent_status(&mut self, task: &str, agent: &str, status: &str, detail: &str) {
        if let Some(idx) = self.chain_index.get(task).copied()
            && let Some(entry) = self.chain.get_mut(idx)
        {
            if let Some(child) = entry.children.iter_mut().find(|c| c.agent == agent) {
                child.status = status.to_string();
                if !detail.is_empty() {
                    child.detail = detail.to_string();
                } else {
                    child.detail.clear();
                }
            }
            if !entry.children.is_empty() {
                if entry.children.iter().any(|c| c.status == "failed") {
                    entry.status = "failed".to_string();
                } else if entry.children.iter().any(|c| c.status == "running") {
                    entry.status = "running".to_string();
                } else if entry.children.iter().all(|c| c.status == "done") {
                    entry.status = "done".to_string();
                } else if entry.children.iter().all(|c| c.status == "aborted") {
                    entry.status = "aborted".to_string();
                }
            }
        }
    }

    fn complete_children_for_task(&mut self, task: &str) {
        if let Some(idx) = self.chain_index.get(task).copied()
            && let Some(entry) = self.chain.get_mut(idx)
        {
            for child in &mut entry.children {
                if child.status == "running" || child.status == "queued" {
                    child.status = "done".to_string();
                    child.detail.clear();
                }
            }
        }
    }

    fn mark_children_for_task(&mut self, task: &str, status: &str, detail: &str) {
        if let Some(idx) = self.chain_index.get(task).copied()
            && let Some(entry) = self.chain.get_mut(idx)
        {
            for child in &mut entry.children {
                child.status = status.to_string();
                child.detail = detail.to_string();
            }
        }
    }

    fn mark_running_children_failed(&mut self, task: &str, detail: &str) {
        if let Some(idx) = self.chain_index.get(task).copied()
            && let Some(entry) = self.chain.get_mut(idx)
        {
            for child in &mut entry.children {
                if child.status == "running" {
                    child.status = "failed".to_string();
                    child.detail = detail.to_string();
                }
            }
        }
    }
}
