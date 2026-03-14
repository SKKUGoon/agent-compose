use crate::runtime::{FormSpec, RuntimeEvent};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tokio::task::JoinHandle;

#[derive(Clone, Copy)]
pub(super) struct PaletteItem {
    pub label: &'static str,
    pub command: Option<Command>,
}

#[derive(Clone, Copy)]
pub(super) enum Command {
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

pub(super) const PALETTE_ITEMS: &[PaletteItem] = &[
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
        label: "  Quick Mode",
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

pub(super) enum UiEvent {
    Runtime(RuntimeEvent),
    Finished(Result<Value, String>),
}

#[derive(Clone)]
pub(super) struct Turn {
    pub question: String,
    pub answer: String,
}

#[derive(Clone)]
pub(super) struct ChainEntry {
    pub task: String,
    pub label: String,
    pub status: String,
    pub detail: String,
    pub children: Vec<ChainChild>,
}

#[derive(Clone)]
pub(super) struct ChainChild {
    pub agent: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InputMode {
    Quick,
    Form,
}

impl InputMode {
    pub(super) fn label(&self) -> &'static str {
        match self {
            InputMode::Quick => "quick",
            InputMode::Form => "form",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DisplayMode {
    PrettyYaml,
    PrettyJson,
    RawJson,
    QaCompact,
}

impl DisplayMode {
    pub(super) fn label(&self) -> &'static str {
        match self {
            DisplayMode::PrettyYaml => "yaml",
            DisplayMode::PrettyJson => "pretty-json",
            DisplayMode::RawJson => "raw-json",
            DisplayMode::QaCompact => "qa",
        }
    }
}

#[derive(Clone)]
pub(super) struct FormFieldState {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub value: String,
}

#[derive(Clone)]
pub(super) enum HistoryEntry {
    Quick(String),
    Form(Vec<String>),
}

pub(super) enum EscState {
    None,
    ConfirmInterrupt { until: Instant },
}

pub(super) struct App {
    pub input: String,
    pub turns: Vec<Turn>,
    pub logs: Vec<String>,
    pub chain: Vec<ChainEntry>,
    pub chain_index: HashMap<String, usize>,
    pub running: bool,
    pub should_quit: bool,
    pub chat_scroll: u16,
    pub model_hint: String,
    pub last_result: Option<Value>,
    pub mode: InputMode,
    pub form_spec: Option<FormSpec>,
    pub form_fields: Vec<FormFieldState>,
    pub form_index: usize,
    pub show_palette: bool,
    pub palette_pos: usize,
    pub copied_json: Option<String>,
    pub history: Vec<HistoryEntry>,
    pub history_cursor: Option<usize>,
    pub run_handle: Option<JoinHandle<()>>,
    pub esc_state: EscState,
    pub status_message: Option<String>,
    pub status_message_until: Option<Instant>,
    pub structured_output: bool,
    pub display_mode: DisplayMode,
    pub current_task: Option<String>,
    pub current_agent: Option<String>,
}
