use crate::runtime::{ComposeRuntime, FormSpec, RuntimeEvent};
#[path = "ui/constants.rs"]
mod constants;
#[path = "ui/formatting.rs"]
mod formatting;
#[path = "ui/input.rs"]
mod input;
#[path = "ui/app_history.rs"]
mod app_history;
#[path = "ui/app_runtime.rs"]
mod app_runtime;
#[path = "ui/app_session.rs"]
mod app_session;
#[path = "ui/app_submission.rs"]
mod app_submission;
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
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Stdout};
use std::time::Instant;
use tokio::task::JoinHandle;

use formatting::value_to_string;

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
