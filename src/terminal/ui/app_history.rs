use super::{App, Command, DisplayMode, HistoryEntry, InputMode};

impl App {
    pub(super) fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_cursor {
            None => self.history.len() - 1,
            Some(i) => i.saturating_sub(1),
        };
        self.history_cursor = Some(idx);
        self.apply_history_index(idx);
    }

    pub(super) fn history_next(&mut self) {
        let Some(current) = self.history_cursor else {
            return;
        };
        if current + 1 < self.history.len() {
            let idx = current + 1;
            self.history_cursor = Some(idx);
            self.apply_history_index(idx);
        } else {
            self.history_cursor = None;
            self.clear_input_after_send();
        }
    }

    fn apply_history_index(&mut self, idx: usize) {
        let Some(entry) = self.history.get(idx).cloned() else {
            return;
        };
        match entry {
            HistoryEntry::Quick(text) => {
                self.mode = InputMode::Quick;
                self.input = text;
            }
            HistoryEntry::Form(values) => {
                if self.form_fields.is_empty() {
                    return;
                }
                self.mode = InputMode::Form;
                for (i, field) in self.form_fields.iter_mut().enumerate() {
                    field.value = values.get(i).cloned().unwrap_or_default();
                }
                self.form_index = 0;
            }
        }
    }

    pub(super) fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::DisplayYaml => {
                self.structured_output = true;
                self.display_mode = DisplayMode::PrettyYaml;
            }
            Command::DisplayPrettyJson => {
                self.structured_output = true;
                self.display_mode = DisplayMode::PrettyJson;
            }
            Command::DisplayRawJson => {
                self.structured_output = true;
                self.display_mode = DisplayMode::RawJson;
            }
            Command::DisplayQa => {
                self.structured_output = false;
                self.display_mode = DisplayMode::QaCompact;
            }
            Command::ToForm => {
                if self.form_spec.is_some() {
                    self.mode = InputMode::Form;
                }
            }
            Command::ToQuick => self.mode = InputMode::Quick,
            Command::ClearChat => self.turns.clear(),
            Command::HistoryPrev => self.history_prev(),
            Command::HistoryNext => self.history_next(),
            Command::HistoryClear => {
                self.history.clear();
                self.history_cursor = None;
                self.logs.push("History cleared".to_string());
            }
            Command::CopyLastJson => {
                if let Some(v) = &self.last_result {
                    self.copied_json =
                        Some(serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()));
                    self.logs
                        .push("Copied last result to session buffer".to_string());
                }
            }
            Command::Quit => self.should_quit = true,
        }
    }
}
