use super::RuntimeError;
use serde_json::Value;

pub fn build_pipeline_output(payload: Value) -> Result<Value, RuntimeError> {
    payload
        .as_object()
        .ok_or_else(|| RuntimeError::Invalid("step payload must be object".to_string()))?;
    Ok(payload)
}
