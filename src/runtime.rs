use crate::config::{ChainConfig, ModelSpec};
use crate::loader::{LoadError, load_compose_config};
use crate::provider::{ProviderError, ProviderRouter};
use crate::resolver::ResolveError;
use crate::schema::{SchemaError, SchemaRegistry};
#[path = "runtime/constants.rs"]
mod constants;
#[path = "runtime/context.rs"]
mod context;
#[path = "runtime/events.rs"]
mod events;
#[path = "runtime/execution.rs"]
mod execution;
#[path = "runtime/form.rs"]
mod form;
#[path = "runtime/steps.rs"]
mod steps;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use thiserror::Error;

pub use events::RuntimeEvent;
pub use form::FormSpec;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone)]
pub struct ComposeRuntime {
    _config_path: PathBuf,
    _chain_id: String,
    config: ChainConfig,
    models: HashMap<String, ModelSpec>,
    order: Vec<String>,
    schema: SchemaRegistry,
    providers: ProviderRouter,
}

impl ComposeRuntime {
    pub fn from_path_and_chain(
        path: impl AsRef<Path>,
        chain_id: impl Into<String>,
    ) -> Result<Self, RuntimeError> {
        let config_path = path.as_ref().to_path_buf();
        let config = load_compose_config(&config_path)?;
        let chain_id = chain_id.into();
        let order = config
            .topological_tasks(&chain_id)
            .map_err(RuntimeError::Invalid)?;
        let chain = config
            .chains
            .get(&chain_id)
            .cloned()
            .ok_or_else(|| RuntimeError::Invalid(format!("unknown chain: {chain_id}")))?;
        let schema = SchemaRegistry::new(config.schema.models.clone())?;
        let providers = ProviderRouter::new(chain.provider.clone());
        Ok(Self {
            _config_path: config_path,
            _chain_id: chain_id,
            config: chain,
            models: config.schema.models.clone(),
            order,
            schema,
            providers,
        })
    }

    pub fn list_chains(path: impl AsRef<Path>) -> Result<Vec<String>, RuntimeError> {
        let config_path = path.as_ref().to_path_buf();
        let config = load_compose_config(&config_path)?;
        Ok(config.chain_ids())
    }

    pub fn chain_serve_target(
        path: impl AsRef<Path>,
        chain_id: &str,
    ) -> Result<(String, u16), RuntimeError> {
        let config_path = path.as_ref().to_path_buf();
        let config = load_compose_config(&config_path)?;
        let chain = config
            .chains
            .get(chain_id)
            .ok_or_else(|| RuntimeError::Invalid(format!("unknown chain: {chain_id}")))?;
        Ok((chain.serve.host.clone(), chain.serve.port))
    }

    pub fn chain_descriptors(path: impl AsRef<Path>) -> Result<Vec<ChainDescriptor>, RuntimeError> {
        let config_path = path.as_ref().to_path_buf();
        let config = load_compose_config(&config_path)?;
        let mut out = Vec::new();
        for chain_id in config.chain_ids() {
            let Some(chain) = config.chains.get(&chain_id) else {
                continue;
            };
            out.push(ChainDescriptor {
                chain: chain_id.clone(),
                description: chain
                    .serve
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("{chain_id} chain")),
                host: chain.serve.host.clone(),
                port: chain.serve.port,
            });
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct ChainDescriptor {
    pub chain: String,
    pub description: String,
    pub host: String,
    pub port: u16,
}
