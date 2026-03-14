use super::constants::ESC_CONFIRM_MS;
use super::{App, EscState, Instant};
use std::time::Duration;

impl App {
    pub(super) fn tick(&mut self) {
        if let Some(until) = self.status_message_until
            && Instant::now() >= until
        {
            self.status_message = None;
            self.status_message_until = None;
        }

        match self.esc_state {
            EscState::ConfirmInterrupt { until } => {
                if Instant::now() >= until {
                    self.esc_state = EscState::None;
                }
            }
            EscState::None => {}
        }
    }

    fn show_flash(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_message_until = Some(Instant::now() + Duration::from_millis(1500));
    }

    pub(super) fn handle_esc(&mut self) {
        if self.show_palette {
            self.show_palette = false;
            self.esc_state = EscState::None;
            return;
        }

        if self.running {
            match self.esc_state {
                EscState::ConfirmInterrupt { until } if Instant::now() <= until => {
                    self.interrupt_run();
                    self.esc_state = EscState::None;
                }
                _ => {
                    self.esc_state = EscState::ConfirmInterrupt {
                        until: Instant::now() + Duration::from_millis(ESC_CONFIRM_MS),
                    };
                    self.show_flash("Press Esc again to interrupt current run");
                }
            }
        }
    }

    fn interrupt_run(&mut self) {
        if let Some(handle) = self.run_handle.take() {
            handle.abort();
        }
        self.running = false;
        if let Some(last) = self.turns.last_mut() {
            last.answer = "Interrupted by user.".to_string();
        }
        self.logs.push("Run cancelled by user".to_string());
        for entry in &mut self.chain {
            if entry.status == "running" {
                entry.status = "skipped".to_string();
                entry.detail = "interrupted".to_string();
            } else if entry.status == "queued" {
                entry.status = "aborted".to_string();
                entry.detail = "interrupted".to_string();
            }
            for child in &mut entry.children {
                if child.status == "running" {
                    child.status = "skipped".to_string();
                    child.detail = "interrupted".to_string();
                } else if child.status == "queued" {
                    child.status = "aborted".to_string();
                    child.detail = "interrupted".to_string();
                }
            }
        }
        self.current_task = None;
        self.current_agent = None;
        self.show_flash("INTERRUPTED");
    }
}
