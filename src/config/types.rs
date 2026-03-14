use super::constants;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct ComposeConfig {
    pub version: String,
    pub name: String,
    pub schema: SchemaConfig,
    pub chains: HashMap<String, ChainConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChainConfig {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    pub agents: HashMap<String, AgentConfig>,
    pub tasks: HashMap<String, TaskConfig>,
    pub output: OutputConfig,
    #[serde(default)]
    pub serve: ServeConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub api_key: String,
    pub default_model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Openai,
    Anthropic,
    Ollama,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeConfig {
    #[serde(default = "default_context_mode")]
    pub context_mode: ContextMode,
    #[serde(default = "default_skip_policy")]
    pub skip_policy: SkipPolicy,
    #[serde(default)]
    pub gatekeeper: GatekeeperControl,
    #[serde(default)]
    pub retry: RetryConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            context_mode: default_context_mode(),
            skip_policy: default_skip_policy(),
            gatekeeper: GatekeeperControl::default(),
            retry: RetryConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryConfig {
    #[serde(default = "default_contract_max_attempts")]
    pub contract_max_attempts: u8,
    #[serde(default = "default_contract_backoff_ms")]
    pub contract_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            contract_max_attempts: default_contract_max_attempts(),
            contract_backoff_ms: default_contract_backoff_ms(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextMode {
    MergedAndRefs,
    RefsOnly,
    MergedOnly,
}

fn default_context_mode() -> ContextMode {
    ContextMode::MergedAndRefs
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipPolicy {
    None,
    GatekeeperControlled,
}

fn default_skip_policy() -> SkipPolicy {
    SkipPolicy::None
}

#[derive(Debug, Deserialize, Clone)]
pub struct GatekeeperControl {
    #[serde(default = "default_gate_task")]
    pub task: String,
    #[serde(default = "default_gate_field")]
    pub field: String,
    #[serde(default)]
    pub skip_tasks: Vec<String>,
}

impl Default for GatekeeperControl {
    fn default() -> Self {
        Self {
            task: default_gate_task(),
            field: default_gate_field(),
            skip_tasks: Vec::new(),
        }
    }
}

fn default_gate_task() -> String {
    constants::DEFAULT_GATEKEEPER_TASK.to_string()
}

fn default_gate_field() -> String {
    constants::DEFAULT_GATEKEEPER_FIELD.to_string()
}

fn default_contract_max_attempts() -> u8 {
    constants::DEFAULT_CONTRACT_MAX_ATTEMPTS
}

fn default_contract_backoff_ms() -> u64 {
    constants::DEFAULT_CONTRACT_BACKOFF_MS
}

#[derive(Debug, Deserialize, Clone)]
pub struct SchemaConfig {
    pub file: String,
    #[serde(default)]
    pub models: HashMap<String, ModelSpec>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub fields: HashMap<String, FieldSpec>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FieldSpec {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "$ref")]
    pub ref_model: Option<String>,
    pub required: Option<bool>,
    pub nullable: Option<bool>,
    pub default: Option<Value>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<Value>>,
    pub items: Option<Box<FieldSpec>>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    pub instructions: String,
    pub model: Option<String>,
    pub input_model: String,
    pub output_model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TaskConfig {
    #[serde(default)]
    pub needs: Vec<String>,
    pub agent: Option<String>,
    pub agents: Option<Vec<String>>,
    pub step: Option<String>,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    #[serde(rename = "from")]
    pub from_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServeConfig {
    #[serde(default = "default_serve_host")]
    pub host: String,
    #[serde(default = "default_serve_port")]
    pub port: u16,
    pub description: Option<String>,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            host: default_serve_host(),
            port: default_serve_port(),
            description: None,
        }
    }
}

fn default_serve_host() -> String {
    "127.0.0.1".to_string()
}

fn default_serve_port() -> u16 {
    8787
}
