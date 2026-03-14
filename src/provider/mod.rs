use async_trait::async_trait;
pub mod rig;

use crate::config::ProviderConfig;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider call failed: {0}")]
    Call(String),
}

#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub provider: ProviderConfig,
    pub model: String,
    pub instructions: String,
    pub input_json: Value,
    pub output_model_name: String,
    pub output_contract_json: Value,
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn invoke(&self, req: PromptRequest) -> Result<Value, ProviderError>;
}

#[derive(Clone)]
pub struct ProviderRouter {
    provider: ProviderConfig,
    adapter: rig::RigAdapter,
}

impl ProviderRouter {
    pub fn new(provider: ProviderConfig) -> Self {
        Self {
            provider,
            adapter: rig::RigAdapter::new(),
        }
    }

    pub fn provider_config(&self) -> &ProviderConfig {
        &self.provider
    }

    pub async fn invoke(&self, req: PromptRequest) -> Result<Value, ProviderError> {
        self.adapter.invoke(req).await
    }
}
