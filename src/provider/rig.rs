use crate::config::ProviderKind;
use crate::provider::{PromptRequest, ProviderAdapter, ProviderError};
use async_trait::async_trait;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::{anthropic, openai};
use serde_json::Value;

#[derive(Clone)]
pub struct RigAdapter;

impl RigAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProviderAdapter for RigAdapter {
    async fn invoke(&self, req: PromptRequest) -> Result<Value, ProviderError> {
        match req.provider.kind {
            ProviderKind::Openai => invoke_openai(req).await,
            ProviderKind::Anthropic => invoke_anthropic(req).await,
            ProviderKind::Ollama => Err(ProviderError::Call(
                "provider.kind=ollama is configured but Rig adapter is not implemented yet"
                    .to_string(),
            )),
        }
    }
}

async fn invoke_openai(req: PromptRequest) -> Result<Value, ProviderError> {
    if req.provider.base_url.is_some() {
        return Err(ProviderError::Call(
            "provider.base_url override is not implemented for Rig OpenAI client yet"
                .to_string(),
        ));
    }

    let client = openai::Client::new(&req.provider.api_key)
        .map_err(|e| ProviderError::Call(format!("openai client init failed: {e}")))?;
    let preamble = build_preamble(&req.instructions, &req.output_model_name, &req.output_contract_json);
    let user_prompt = build_user_prompt(&req.input_json);

    let agent = client.agent(&req.model).preamble(&preamble).build();
    let response = agent
        .prompt(&user_prompt)
        .await
        .map_err(|e| ProviderError::Call(format!("openai rig call failed: {e}")))?;

    parse_json_response(&response)
}

async fn invoke_anthropic(req: PromptRequest) -> Result<Value, ProviderError> {
    if req.provider.base_url.is_some() {
        return Err(ProviderError::Call(
            "provider.base_url override is not implemented for Rig Anthropic client yet"
                .to_string(),
        ));
    }

    let client = anthropic::Client::new(&req.provider.api_key)
        .map_err(|e| ProviderError::Call(format!("anthropic client init failed: {e}")))?;
    let preamble = build_preamble(&req.instructions, &req.output_model_name, &req.output_contract_json);
    let user_prompt = build_user_prompt(&req.input_json);

    let agent = client.agent(&req.model).preamble(&preamble).build();
    let response = agent
        .prompt(&user_prompt)
        .await
        .map_err(|e| ProviderError::Call(format!("anthropic rig call failed: {e}")))?;

    parse_json_response(&response)
}

fn build_preamble(instructions: &str, output_model_name: &str, output_contract_json: &Value) -> String {
    format!(
        "{instructions}\n\nReturn ONLY a single JSON object. No markdown, no code fences, no prose.\nThe object MUST satisfy output model: {output_model_name}.\nOutput contract (exact):\n{}",
        serde_json::to_string_pretty(output_contract_json)
            .unwrap_or_else(|_| output_contract_json.to_string())
    )
}

fn build_user_prompt(input_json: &Value) -> String {
    format!(
        "Input JSON:\n{}\n\nProduce the output JSON now.",
        serde_json::to_string_pretty(input_json).unwrap_or_else(|_| input_json.to_string())
    )
}

fn parse_json_response(raw: &str) -> Result<Value, ProviderError> {
    if let Ok(value) = serde_json::from_str::<Value>(raw)
        && value.is_object()
    {
        return Ok(value);
    }

    if let Some(extracted) = extract_fenced_json(raw)
        && let Ok(value) = serde_json::from_str::<Value>(&extracted)
        && value.is_object()
    {
        return Ok(value);
    }

    Err(ProviderError::Call(
        "model response is not a valid JSON object matching output contract".to_string(),
    ))
}

fn extract_fenced_json(raw: &str) -> Option<String> {
    let start = raw.find("```")?;
    let after = &raw[start + 3..];
    let after = if let Some(stripped) = after.strip_prefix("json") {
        stripped
    } else {
        after
    };
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}
