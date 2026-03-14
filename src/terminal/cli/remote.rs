use crate::runtime::ComposeRuntime;
use serde_json::{Value, json};

pub(super) async fn call_server(
    text: String,
    config: String,
    chain: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    model: Option<String>,
    as_json: bool,
) -> Result<(), String> {
    let (mut resolved_host, mut resolved_port) = if host.is_some() || port.is_some() {
        let chain = if let Some(chain) = chain.clone() {
            chain
        } else {
            let chains = ComposeRuntime::list_chains(&config).map_err(|e| e.to_string())?;
            if chains.len() != 1 {
                return Err(format!(
                    "multiple chains configured ({}), pass --chain when using host/port override",
                    chains.join(", ")
                ));
            }
            chains[0].clone()
        };
        ComposeRuntime::chain_serve_target(&config, &chain).map_err(|e| e.to_string())?
    } else {
        let chain = if let Some(chain) = chain {
            chain
        } else {
            let chains = ComposeRuntime::list_chains(&config).map_err(|e| e.to_string())?;
            if chains.len() != 1 {
                return Err(format!(
                    "multiple chains configured ({}), pass --chain or --host/--port",
                    chains.join(", ")
                ));
            }
            chains[0].clone()
        };
        ComposeRuntime::chain_serve_target(&config, &chain).map_err(|e| e.to_string())?
    };

    if let Some(host) = host {
        resolved_host = host;
    }
    if let Some(port) = port {
        resolved_port = port;
    }

    let url = format!("http://{resolved_host}:{resolved_port}/v1/infer");
    let body = json!({"text": text, "model": model});
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let value: Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("server error {status}: {value}"));
    }
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
        );
    } else {
        println!("{value}");
    }
    Ok(())
}
