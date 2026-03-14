use super::args::ServeCommands;
use crate::runtime::{ChainDescriptor, ComposeRuntime};
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
struct WorkerState {
    chain: String,
    config: String,
    host: String,
    port: u16,
    pid: u32,
    started_at_unix_ms: u128,
    log_path: String,
}

#[derive(Debug, Serialize)]
struct PsEntry {
    chain: String,
    host: String,
    port: u16,
    pid: Option<u32>,
    status: PsStatus,
    log_path: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum PsStatus {
    Running,
    Stale,
    Stopped,
}

impl PsStatus {
    fn as_machine(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stale => "stale",
            Self::Stopped => "stopped",
        }
    }

    fn as_title(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stale => "Stale",
            Self::Stopped => "Stopped",
        }
    }

    fn ansi_color_code(self) -> &'static str {
        match self {
            Self::Running => "32",
            Self::Stale => "33",
            Self::Stopped => "31",
        }
    }
}

pub(super) fn handle_serve(command: ServeCommands) -> Result<(), String> {
    match command {
        ServeCommands::Start { config, chain } => serve_start(config, chain),
        ServeCommands::Stop { config, chain } => serve_stop(config, chain),
        ServeCommands::Status { config, chain } => serve_status(config, chain),
        ServeCommands::Logs { config, chain } => serve_logs(config, chain),
    }
}

pub(super) fn show_ps(config: String, chain: Option<String>, as_json: bool) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    let mut rows = Vec::new();
    for desc in chains {
        rows.push(ps_entry_from_descriptor(&desc)?);
    }

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
        );
    } else {
        print_ps_table(&rows);
    }
    Ok(())
}

pub(super) fn show_mcp(config: String, chain: Option<String>, as_json: bool) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    let entries: Vec<_> = chains
        .into_iter()
        .map(|chain| {
            json!({
                "chain": chain.chain,
                "description": chain.description,
                "server_url": format!("http://{}:{}/rpc", chain.host, chain.port),
            })
        })
        .collect();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?
        );
    } else {
        for entry in entries {
            println!(
                "chain={} description={} server_url={}",
                entry["chain"].as_str().unwrap_or_default(),
                entry["description"].as_str().unwrap_or_default(),
                entry["server_url"].as_str().unwrap_or_default()
            );
        }
    }
    Ok(())
}

pub(super) fn show_mcp_spec(
    config: String,
    chain: Option<String>,
    all: bool,
    pretty: bool,
) -> Result<(), String> {
    let include_all = all || chain.is_none();
    let chains = if include_all {
        ComposeRuntime::chain_descriptors(&config).map_err(|e| e.to_string())?
    } else {
        selected_descriptors(&config, chain)?
    };
    let servers: Vec<_> = chains
        .into_iter()
        .map(|chain| {
            json!({
                "name": chain.chain,
                "description": chain.description,
                "transport": "http",
                "server_url": format!("http://{}:{}/rpc", chain.host, chain.port),
                "jsonrpc": "2.0",
                "methods": [
                    "initialize",
                    "ping",
                    "tools/list",
                    "tools/call"
                ],
                "tools": [
                    {
                        "name": "infer",
                        "description": "Run agent-compose chain inference",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": {"type": "object"},
                                "text": {"type": "string"},
                                "model": {"type": "string"}
                            },
                            "additionalProperties": false
                        },
                        "outputSchema": {
                            "type": "object"
                        }
                    }
                ]
            })
        })
        .collect();

    let payload = if include_all {
        json!({"servers": servers})
    } else {
        servers.into_iter().next().unwrap_or_else(|| json!({}))
    };

    if pretty {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "{}",
            serde_json::to_string(&payload).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

fn serve_start(config: String, chain: Option<String>) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    for desc in chains {
        let pid_path = pid_file(&desc.chain)?;
        if let Some(pid) = read_pid(&pid_path)? {
            if is_process_alive(pid) {
                println!(
                    "chain={} already running pid={} host={} port={}",
                    desc.chain, pid, desc.host, desc.port
                );
                continue;
            }
            cleanup_chain_state(&desc.chain)?;
        }

        let log_path = log_file(&desc.chain)?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?;
        let log_err = log.try_clone().map_err(|e| e.to_string())?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let child = Command::new(exe)
            .arg("_serve_worker")
            .arg("--config")
            .arg(&config)
            .arg("--chain")
            .arg(&desc.chain)
            .arg("--host")
            .arg(&desc.host)
            .arg("--port")
            .arg(desc.port.to_string())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|e| e.to_string())?;

        let state = WorkerState {
            chain: desc.chain.clone(),
            config: config.clone(),
            host: desc.host.clone(),
            port: desc.port,
            pid: child.id(),
            started_at_unix_ms: now_ms()?,
            log_path: log_path.display().to_string(),
        };
        fs::write(&pid_path, child.id().to_string()).map_err(|e| e.to_string())?;
        fs::write(
            state_file(&desc.chain)?,
            serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "started chain={} pid={} host={} port={} logs={}",
            desc.chain,
            child.id(),
            desc.host,
            desc.port,
            log_path.display()
        );
    }
    Ok(())
}

fn serve_stop(config: String, chain: Option<String>) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    for desc in chains {
        let pid_path = pid_file(&desc.chain)?;
        let Some(pid) = read_pid(&pid_path)? else {
            println!("chain={} not running", desc.chain);
            cleanup_chain_state(&desc.chain)?;
            continue;
        };

        if is_process_alive(pid) {
            #[cfg(unix)]
            {
                let status = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .status()
                    .map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err(format!("failed to stop chain={} pid={pid}", desc.chain));
                }
            }
        }

        cleanup_chain_state(&desc.chain)?;
        println!("stopped chain={} pid={pid}", desc.chain);
    }
    Ok(())
}

fn serve_status(config: String, chain: Option<String>) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    for desc in chains {
        let entry = ps_entry_from_descriptor(&desc)?;
        println!(
            "chain={} status={} pid={} host={} port={}",
            entry.chain,
            entry.status.as_machine(),
            entry
                .pid
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry.host,
            entry.port
        );
    }
    Ok(())
}

fn serve_logs(config: String, chain: Option<String>) -> Result<(), String> {
    let chains = selected_descriptors(&config, chain)?;
    for desc in chains {
        let path = log_file(&desc.chain)?;
        println!("chain={} logs={}", desc.chain, path.display());
    }
    Ok(())
}

fn selected_descriptors(
    config: &str,
    chain: Option<String>,
) -> Result<Vec<ChainDescriptor>, String> {
    let mut all = ComposeRuntime::chain_descriptors(config).map_err(|e| e.to_string())?;
    if let Some(chain) = chain {
        all.retain(|d| d.chain == chain);
        if all.is_empty() {
            return Err(format!("unknown chain: {chain}"));
        }
    }
    Ok(all)
}

fn state_dir() -> Result<PathBuf, String> {
    let path = PathBuf::from(".agent-compose");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

fn pid_file(chain: &str) -> Result<PathBuf, String> {
    Ok(state_dir()?.join(format!("{chain}.pid")))
}

fn state_file(chain: &str) -> Result<PathBuf, String> {
    Ok(state_dir()?.join(format!("{chain}.state.json")))
}

fn log_file(chain: &str) -> Result<PathBuf, String> {
    Ok(state_dir()?.join(format!("{chain}.log")))
}

fn read_pid(path: &PathBuf) -> Result<Option<u32>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(raw.trim().parse::<u32>().ok())
}

fn now_ms() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis())
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();
        return status.map(|s| s.success()).unwrap_or(false);
    }

    #[allow(unreachable_code)]
    false
}

fn cleanup_chain_state(chain: &str) -> Result<(), String> {
    let _ = fs::remove_file(pid_file(chain)?);
    let _ = fs::remove_file(state_file(chain)?);
    Ok(())
}

fn ps_entry_from_descriptor(desc: &ChainDescriptor) -> Result<PsEntry, String> {
    let pid = read_pid(&pid_file(&desc.chain)?)?;
    let (pid, status) = match pid {
        Some(pid) if is_process_alive(pid) => (Some(pid), PsStatus::Running),
        Some(_) => {
            cleanup_chain_state(&desc.chain)?;
            (None, PsStatus::Stale)
        }
        None => (None, PsStatus::Stopped),
    };

    Ok(PsEntry {
        chain: desc.chain.clone(),
        host: desc.host.clone(),
        port: desc.port,
        pid,
        status,
        log_path: log_file(&desc.chain)?.display().to_string(),
    })
}

fn print_ps_table(rows: &[PsEntry]) {
    let chain_header = "CHAIN";
    let status_header = "STATUS";
    let endpoint_header = "ENDPOINT";
    let spacing = 2usize;
    let status_width = status_header.len().max(
        rows.iter()
            .map(|r| r.status.as_title().len())
            .max()
            .unwrap_or(0),
    );

    let terminal_width = terminal_width().unwrap_or(100);
    let mut available = terminal_width.saturating_sub(status_width + spacing * 2);
    let chain_min = chain_header.len();
    let endpoint_min = endpoint_header.len();

    let chain_len_max = rows
        .iter()
        .map(|r| r.chain.len())
        .max()
        .unwrap_or(chain_header.len())
        .max(chain_header.len());

    if available < chain_min + endpoint_min {
        available = chain_min + endpoint_min;
    }

    let mut chain_width = (available * 35) / 100;
    chain_width = chain_width.max(chain_min).min(available - endpoint_min);
    chain_width = chain_width.min(chain_len_max.max(chain_min));
    let endpoint_width = available.saturating_sub(chain_width);

    let status_colored = color_enabled();

    println!(
        "{}{}{}{}{}",
        pad_or_truncate(chain_header, chain_width),
        " ".repeat(spacing),
        pad_or_truncate(status_header, status_width),
        " ".repeat(spacing),
        pad_or_truncate(endpoint_header, endpoint_width)
    );

    for row in rows {
        let endpoint = format!("{}:{}", row.host, row.port);
        let chain_cell = pad_or_truncate(&row.chain, chain_width);
        let status_cell = pad_or_truncate(row.status.as_title(), status_width);
        let endpoint_cell = pad_or_truncate(&endpoint, endpoint_width);
        let status_cell = colorize_status(&status_cell, row.status, status_colored);
        println!(
            "{}{}{}{}{}",
            chain_cell,
            " ".repeat(spacing),
            status_cell,
            " ".repeat(spacing),
            endpoint_cell
        );
    }
}

fn terminal_width() -> Option<usize> {
    if let Ok(columns) = std::env::var("COLUMNS") {
        if let Ok(parsed) = columns.parse::<usize>() {
            if parsed > 0 {
                return Some(parsed);
            }
        }
    }
    if io::stdout().is_terminal() {
        return crossterm::terminal::size().ok().map(|(w, _)| w as usize);
    }
    None
}

fn color_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn colorize_status(text: &str, status: PsStatus, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    format!("\x1b[{}m{text}\x1b[0m", status.ansi_color_code())
}

fn pad_or_truncate(value: &str, width: usize) -> String {
    let truncated = truncate_with_ellipsis(value, width);
    if truncated.len() >= width {
        return truncated;
    }
    format!("{truncated:<width$}")
}

fn truncate_with_ellipsis(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let total_chars = value.chars().count();
    if total_chars <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut out = String::with_capacity(width);
    for ch in value.chars().take(width - 3) {
        out.push(ch);
    }
    out.push_str("...");
    out
}
