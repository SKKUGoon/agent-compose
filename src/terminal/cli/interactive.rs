use crate::io::workflow_input_from_text;
use crate::runtime::ComposeRuntime;
use serde_json::Value;
use std::io::{self, Write};

pub(super) async fn run_interactive(
    config: String,
    chain: String,
    model: Option<String>,
    mut json_mode: bool,
) -> Result<(), String> {
    let runtime = ComposeRuntime::from_path_and_chain(config, chain).map_err(|e| e.to_string())?;
    println!("agent-compose interactive mode. Type /quit to exit.");
    println!("Commands: /json on|off");
    loop {
        print!("You> ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        let bytes = io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
        if bytes == 0 {
            println!();
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input == "/quit" {
            break;
        }
        if input.starts_with("/json") {
            let parts: Vec<_> = input.split_whitespace().collect();
            if parts.len() == 2 {
                json_mode = parts[1] == "on";
                println!("json mode: {}", if json_mode { "on" } else { "off" });
            } else {
                println!("usage: /json on|off");
            }
            continue;
        }

        let payload = workflow_input_from_text(input);
        let result = runtime
            .run(payload, model.clone())
            .await
            .map_err(|e| e.to_string())?;
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        } else {
            let verdict = if result
                .get("passed_gatekeeper")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "pass"
            } else {
                "blocked"
            };
            let reason = result
                .get("gatekeeper_reason")
                .and_then(Value::as_str)
                .unwrap_or("");
            println!("Agent> gatekeeper={verdict} reason={reason}");
            if let Some(summary) = result.get("summary_distilled").and_then(Value::as_str)
                && !summary.is_empty()
            {
                println!("Agent> {summary}");
            }
        }
    }
    Ok(())
}
