use crate::runtime::ComposeRuntime;
use dialoguer::{theme::ColorfulTheme, Select};

pub(super) fn resolve_chain(
    config: &str,
    chain: Option<String>,
    interactive: bool,
) -> Result<String, String> {
    if let Some(chain) = chain {
        return Ok(chain);
    }
    let chains = ComposeRuntime::list_chains(config).map_err(|e| e.to_string())?;
    if chains.is_empty() {
        return Err("no chains configured".to_string());
    }
    if chains.len() == 1 {
        return Ok(chains[0].clone());
    }
    if !interactive {
        return Err(format!(
            "multiple chains configured ({}), pass --chain",
            chains.join(", ")
        ));
    }
    pick_chain(&chains)
}

pub(super) fn resolve_chain_optional(
    config: &str,
    interactive: bool,
) -> Result<Option<String>, String> {
    let chains = ComposeRuntime::list_chains(config).map_err(|e| e.to_string())?;
    if chains.len() <= 1 {
        return Ok(chains.first().cloned());
    }
    if !interactive {
        return Ok(None);
    }

    let mut options = vec!["all chains".to_string()];
    options.extend(chains.iter().cloned());
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select chain")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;
    if selection == 0 {
        return Ok(None);
    }
    Ok(options.get(selection).cloned())
}

pub(super) fn resolve_chain_for_remote(
    config: &str,
    chain: Option<String>,
    host_or_port_override: bool,
) -> Result<String, String> {
    if let Some(chain) = chain {
        return Ok(chain);
    }

    let chains = ComposeRuntime::list_chains(config).map_err(|e| e.to_string())?;
    if chains.len() == 1 {
        return Ok(chains[0].clone());
    }

    if host_or_port_override {
        return Err(format!(
            "multiple chains configured ({}), pass --chain when using host/port override",
            chains.join(", ")
        ));
    }

    Err(format!(
        "multiple chains configured ({}), pass --chain or --host/--port",
        chains.join(", ")
    ))
}

fn pick_chain(chains: &[String]) -> Result<String, String> {
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select chain")
        .items(chains)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;
    chains
        .get(selection)
        .cloned()
        .ok_or_else(|| "invalid chain selection".to_string())
}
