use crate::runtime::ComposeRuntime;
use crate::server;
use crate::terminal::ui;
#[path = "cli/args.rs"]
mod args;
#[path = "cli/chains.rs"]
mod chains;
#[path = "cli/interactive.rs"]
mod interactive;
#[path = "cli/remote.rs"]
mod remote;
#[path = "cli/serve_ctl.rs"]
mod serve_ctl;
use args::{Cli, Commands};
use chains::{resolve_chain, resolve_chain_optional};
use clap::Parser;
use dialoguer::{Select, theme::ColorfulTheme};

pub async fn run_cli() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            let options = ["run", "serve", "ps", "mcp", "mcp_spec"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select command")
                .items(&options)
                .default(0)
                .interact()
                .map_err(|e| e.to_string())?;

            match selection {
                0 => {
                    let config = "agent-compose.yaml".to_string();
                    let chain = resolve_chain(&config, None, true)?;
                    let runtime = ComposeRuntime::from_path_and_chain(&config, chain)
                        .map_err(|e| e.to_string())?;
                    ui::run_chat_tui(runtime, None, false).await
                }
                1 => {
                    let serve_options = ["start", "status", "stop", "logs"];
                    let serve_selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Serve action")
                        .items(&serve_options)
                        .default(0)
                        .interact()
                        .map_err(|e| e.to_string())?;
                    let config = "agent-compose.yaml".to_string();
                    let chain = resolve_chain_optional(&config, true)?;
                    match serve_selection {
                        0 => serve_ctl::handle_serve(args::ServeCommands::Start { config, chain }),
                        1 => serve_ctl::handle_serve(args::ServeCommands::Status { config, chain }),
                        2 => serve_ctl::handle_serve(args::ServeCommands::Stop { config, chain }),
                        3 => serve_ctl::handle_serve(args::ServeCommands::Logs { config, chain }),
                        _ => Ok(()),
                    }
                }
                2 => serve_ctl::show_ps("agent-compose.yaml".to_string(), None, false),
                3 => serve_ctl::show_mcp("agent-compose.yaml".to_string(), None, false),
                4 => serve_ctl::show_mcp_spec("agent-compose.yaml".to_string(), None, false, true),
                _ => Ok(()),
            }
        }
        Some(Commands::Run {
            config,
            chain,
            model,
            json,
            plain,
        }) => {
            let chain = resolve_chain(&config, chain, !plain)?;
            if plain {
                interactive::run_interactive(config, chain, model, json).await
            } else {
                let runtime =
                    ComposeRuntime::from_path_and_chain(config, chain).map_err(|e| e.to_string())?;
                ui::run_chat_tui(runtime, model, json).await
            }
        }
        Some(Commands::Serve { command }) => serve_ctl::handle_serve(command),
        Some(Commands::Call {
            text,
            config,
            chain,
            host,
            port,
            model,
            json,
        }) => remote::call_server(text, config, chain, host, port, model, json).await,
        Some(Commands::Ps {
            config,
            chain,
            json,
        }) => serve_ctl::show_ps(config, chain, json),
        Some(Commands::Mcp {
            config,
            chain,
            json,
        }) => serve_ctl::show_mcp(config, chain, json),
        Some(Commands::McpSpec {
            config,
            chain,
            all,
            pretty,
        }) => serve_ctl::show_mcp_spec(config, chain, all, pretty),
        Some(Commands::ServeWorker {
            config,
            chain,
            host,
            port,
        }) => server::serve(config, chain, host, port).await,
    }
}
