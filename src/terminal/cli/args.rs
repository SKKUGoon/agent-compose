use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-compose")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub(super) enum Commands {
    Run {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = false)]
        plain: bool,
    },
    Serve {
        #[command(subcommand)]
        command: ServeCommands,
    },
    Call {
        text: String,
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Ps {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Mcp {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(name = "mcp_spec")]
    McpSpec {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        pretty: bool,
    },
    #[command(name = "_serve_worker")]
    ServeWorker {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
    },
}

#[derive(Subcommand)]
pub(super) enum ServeCommands {
    Start {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
    },
    Stop {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
    },
    Status {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
    },
    Logs {
        #[arg(long, default_value = "agent-compose.yaml")]
        config: String,
        #[arg(long)]
        chain: Option<String>,
    },
}
