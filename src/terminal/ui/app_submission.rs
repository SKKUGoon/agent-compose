use super::formatting::parse_form_value;
use super::{
    App, ComposeRuntime, HistoryEntry, InputMode, RuntimeEvent, SubmittedInput, Turn, UiEvent,
    Value,
};
use serde_json::Map;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

impl App {
    pub(super) fn submit(
        &mut self,
        runtime: ComposeRuntime,
        model: Option<String>,
        tx: UnboundedSender<UiEvent>,
    ) {
        if self.running {
            self.logs
                .push("A run is already in progress. Wait for completion.".to_string());
            return;
        }

        let (submitted_input, payload) = match self.build_submission() {
            Ok(v) => v,
            Err(err) => {
                self.logs.push(format!("Input error: {err}"));
                return;
            }
        };

        self.push_submission_history();
        self.clear_input_after_send();
        self.history_cursor = None;
        self.esc_state = super::EscState::None;

        self.running = true;
        self.turns.push(Turn {
            submitted_input: submitted_input.clone(),
            answer: "Processing...".to_string(),
        });
        self.logs
            .push(format!("Submitted {} input", self.mode.label()));
        for entry in &mut self.chain {
            entry.status = "queued".to_string();
            entry.detail.clear();
            for child in &mut entry.children {
                child.status = "queued".to_string();
                child.detail.clear();
            }
        }

        let handle = tokio::spawn(async move {
            let (evt_tx, mut evt_rx) = unbounded_channel::<RuntimeEvent>();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                while let Some(evt) = evt_rx.recv().await {
                    let _ = tx_clone.send(UiEvent::Runtime(evt));
                }
            });

            let result = runtime
                .run_with_events(payload, model, Some(evt_tx))
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(UiEvent::Finished(result));
        });
        self.run_handle = Some(handle);
    }

    fn build_submission(&self) -> Result<(SubmittedInput, Value), String> {
        match self.mode {
            InputMode::Quick => {
                let raw = self.input.clone();
                if raw.trim().is_empty() {
                    return Err("empty prompt".to_string());
                }
                let parsed: Value =
                    serde_json::from_str(&raw).map_err(|e| format!("invalid JSON input: {e}"))?;
                if !parsed.is_object() {
                    return Err("raw input must be a JSON object".to_string());
                }
                Ok((SubmittedInput::Raw(raw), parsed))
            }
            InputMode::Form => {
                if self.form_fields.is_empty() {
                    return Err("form unavailable".to_string());
                }
                let mut obj = Map::new();
                for f in &self.form_fields {
                    if f.required && f.value.trim().is_empty() {
                        return Err(format!("{} is required", f.name));
                    }
                    let v = parse_form_value(&f.kind, f.value.trim())?;
                    obj.insert(f.name.clone(), v);
                }
                Ok((
                    SubmittedInput::Form {
                        fields: self.form_fields.clone(),
                        selected_index: self.form_index,
                    },
                    Value::Object(obj),
                ))
            }
        }
    }

    pub(super) fn push_submission_history(&mut self) {
        let entry = match self.mode {
            InputMode::Quick => HistoryEntry::Quick(self.input.trim().to_string()),
            InputMode::Form => {
                HistoryEntry::Form(self.form_fields.iter().map(|f| f.value.clone()).collect())
            }
        };
        if matches!(&entry, HistoryEntry::Quick(s) if s.is_empty()) {
            return;
        }
        self.history.push(entry);
        if self.history.len() > 200 {
            let drain = self.history.len().saturating_sub(200);
            self.history.drain(0..drain);
        }
    }

    pub(super) fn clear_input_after_send(&mut self) {
        match self.mode {
            InputMode::Quick => self.input.clear(),
            InputMode::Form => {
                for field in &mut self.form_fields {
                    field.value.clear();
                }
                self.form_index = 0;
            }
        }
    }
}
