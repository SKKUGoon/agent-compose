use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    TaskStarted {
        task: String,
    },
    TaskCompleted {
        task: String,
    },
    TaskSkipped {
        task: String,
    },
    AgentStarted {
        task: String,
        agent: String,
        model: String,
    },
    AgentCompleted {
        task: String,
        agent: String,
    },
    AgentRetrying {
        task: String,
        agent: String,
        attempt: u8,
        reason: String,
    },
    StepStarted {
        task: String,
        step: String,
    },
    StepCompleted {
        task: String,
        step: String,
    },
}

pub(super) fn emit(tx: &Option<UnboundedSender<RuntimeEvent>>, event: RuntimeEvent) {
    if let Some(sender) = tx {
        let _ = sender.send(event);
    }
}
