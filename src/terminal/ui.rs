use crate::runtime::{ComposeRuntime, FormSpec, RuntimeEvent};
#[path = "ui/constants.rs"]
mod constants;
#[path = "ui/formatting.rs"]
mod formatting;
#[path = "ui/input.rs"]
mod input;
#[path = "ui/render.rs"]
mod render;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

use constants::ESC_CONFIRM_MS;
use formatting::{parse_form_value, value_to_string};

#[derive(Clone, Copy)]
struct PaletteItem {
    label: &'static str,
    command: Option<Command>,
}

const PALETTE_ITEMS: &[PaletteItem] = &[
    PaletteItem {
        label: "Display",
        command: None,
    },
    PaletteItem {
        label: "  Pretty YAML",
        command: Some(Command::DisplayYaml),
    },
    PaletteItem {
        label: "  Pretty JSON",
        command: Some(Command::DisplayPrettyJson),
    },
    PaletteItem {
        label: "  Raw JSON",
        command: Some(Command::DisplayRawJson),
    },
    PaletteItem {
        label: "  Compact Q/A",
        command: Some(Command::DisplayQa),
    },
    PaletteItem {
        label: "Input",
        command: None,
    },
    PaletteItem {
        label: "  Form Mode",
        command: Some(Command::ToForm),
    },
    PaletteItem {
        label: "  Raw Mode",
        command: Some(Command::ToQuick),
    },
    PaletteItem {
        label: "Run",
        command: None,
    },
    PaletteItem {
        label: "  Clear Chat",
        command: Some(Command::ClearChat),
    },
    PaletteItem {
        label: "History",
        command: None,
    },
    PaletteItem {
        label: "  Previous Input",
        command: Some(Command::HistoryPrev),
    },
    PaletteItem {
        label: "  Next Input",
        command: Some(Command::HistoryNext),
    },
    PaletteItem {
        label: "  Clear History",
        command: Some(Command::HistoryClear),
    },
    PaletteItem {
        label: "Session",
        command: None,
    },
    PaletteItem {
        label: "  Copy Last Result JSON",
        command: Some(Command::CopyLastJson),
    },
    PaletteItem {
        label: "  Quit",
        command: Some(Command::Quit),
    },
];

enum UiEvent {
    Runtime(RuntimeEvent),
    Finished(Result<Value, String>),
}

#[derive(Clone)]
struct Turn {
    submitted_input: SubmittedInput,
    answer: String,
}

#[derive(Clone)]
enum SubmittedInput {
    Raw(String),
    Form {
        fields: Vec<FormFieldState>,
        selected_index: usize,
    },
}

#[derive(Clone)]
struct ChainEntry {
    task: String,
    label: String,
    status: String,
    detail: String,
    children: Vec<ChainChild>,
}

#[derive(Clone)]
struct ChainChild {
    agent: String,
    status: String,
    detail: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Quick,
    Form,
}

impl InputMode {
    fn label(&self) -> &'static str {
        match self {
            InputMode::Quick => "raw",
            InputMode::Form => "form",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DisplayMode {
    PrettyYaml,
    PrettyJson,
    RawJson,
    QaCompact,
}

impl DisplayMode {
    fn label(&self) -> &'static str {
        match self {
            DisplayMode::PrettyYaml => "yaml",
            DisplayMode::PrettyJson => "pretty-json",
            DisplayMode::RawJson => "raw-json",
            DisplayMode::QaCompact => "qa",
        }
    }
}

#[derive(Clone)]
struct FormFieldState {
    name: String,
    kind: String,
    required: bool,
    value: String,
}

#[derive(Clone)]
enum HistoryEntry {
    Quick(String),
    Form(Vec<String>),
}

#[derive(Clone, Copy)]
enum Command {
    DisplayYaml,
    DisplayPrettyJson,
    DisplayRawJson,
    DisplayQa,
    ToForm,
    ToQuick,
    ClearChat,
    HistoryPrev,
    HistoryNext,
    HistoryClear,
    CopyLastJson,
    Quit,
}

enum EscState {
    None,
    ConfirmInterrupt { until: Instant },
}

struct App {
    input: String,
    turns: Vec<Turn>,
    logs: Vec<String>,
    chain: Vec<ChainEntry>,
    chain_index: HashMap<String, usize>,
    running: bool,
    should_quit: bool,
    chat_scroll: u16,
    form_scroll: u16,
    model_hint: String,
    last_result: Option<Value>,
    mode: InputMode,
    form_spec: Option<FormSpec>,
    form_fields: Vec<FormFieldState>,
    form_index: usize,
    show_palette: bool,
    palette_pos: usize,
    copied_json: Option<String>,
    history: Vec<HistoryEntry>,
    history_cursor: Option<usize>,
    run_handle: Option<JoinHandle<()>>,
    esc_state: EscState,
    status_message: Option<String>,
    status_message_until: Option<Instant>,
    structured_output: bool,
    display_mode: DisplayMode,
    current_task: Option<String>,
    current_agent: Option<String>,
}

impl App {
    fn new(runtime: &ComposeRuntime, model: Option<String>, json_mode: bool) -> Self {
        let mut chain = Vec::new();
        let mut chain_index = HashMap::new();
        for task in runtime.task_order() {
            let label = runtime.chain_label(&task).unwrap_or(task.clone());
            let children = runtime
                .parallel_agents_for_task(&task)
                .unwrap_or_default()
                .into_iter()
                .map(|agent| ChainChild {
                    agent,
                    status: "queued".to_string(),
                    detail: String::new(),
                })
                .collect();
            chain_index.insert(task.clone(), chain.len());
            chain.push(ChainEntry {
                task,
                label,
                status: "queued".to_string(),
                detail: String::new(),
                children,
            });
        }

        let form_spec = runtime.default_form_spec();
        let mut form_fields = Vec::new();
        if let Some(spec) = &form_spec {
            let mut required_fields = Vec::new();
            let mut optional_fields = Vec::new();
            for field in &spec.fields {
                let value = field
                    .default_value
                    .as_ref()
                    .map(value_to_string)
                    .unwrap_or_default();
                let next = FormFieldState {
                    name: field.name.clone(),
                    kind: field.kind.clone(),
                    required: field.required,
                    value,
                };
                if next.required {
                    required_fields.push(next);
                } else {
                    optional_fields.push(next);
                }
            }
            form_fields.extend(required_fields);
            form_fields.extend(optional_fields);
        }

        let mode = if form_spec.is_some() {
            InputMode::Form
        } else {
            InputMode::Quick
        };

        let structured_output = json_mode || runtime.prefers_structured_output();
        let display_mode = if structured_output {
            DisplayMode::PrettyYaml
        } else {
            DisplayMode::QaCompact
        };

        Self {
            input: String::new(),
            turns: Vec::new(),
            logs: vec!["Ready. Press Ctrl+P for commands.".to_string()],
            chain,
            chain_index,
            running: false,
            should_quit: false,
            chat_scroll: 0,
            form_scroll: 0,
            model_hint: model.unwrap_or_else(|| "default".to_string()),
            last_result: None,
            mode,
            form_spec,
            form_fields,
            form_index: 0,
            show_palette: false,
            palette_pos: input::first_selectable_palette_index(),
            copied_json: None,
            history: Vec::new(),
            history_cursor: None,
            run_handle: None,
            esc_state: EscState::None,
            status_message: None,
            status_message_until: None,
            structured_output,
            display_mode,
            current_task: None,
            current_agent: None,
        }
    }

    fn submit(&mut self, runtime: ComposeRuntime, model: Option<String>, tx: UnboundedSender<UiEvent>) {
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
        self.esc_state = EscState::None;

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

    fn push_submission_history(&mut self) {
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

    fn clear_input_after_send(&mut self) {
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

    fn apply_runtime_event(&mut self, event: RuntimeEvent) {
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
                self.logs
                    .push(format!("Retrying {task}/{agent} attempt #{attempt}: {reason}"));
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

    fn mark_run_failed(&mut self, err: &str) {
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

    fn history_prev(&mut self) {
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

    fn history_next(&mut self) {
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

    fn execute_command(&mut self, cmd: Command) {
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
                    self.copied_json = Some(
                        serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
                    );
                    self.logs.push("Copied last result to session buffer".to_string());
                }
            }
            Command::Quit => self.should_quit = true,
        }
    }

    fn tick(&mut self) {
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

    fn handle_esc(&mut self) {
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
            return;
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

pub async fn run_chat_tui(
    runtime: ComposeRuntime,
    model: Option<String>,
    json_mode: bool,
) -> Result<(), String> {
    enable_raw_mode().map_err(|e| e.to_string())?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(|e| e.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;

    let result = input::run_tui_loop(&mut terminal, runtime, model, json_mode).await;

    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| e.to_string())?;
    terminal.show_cursor().map_err(|e| e.to_string())?;

    result
}
