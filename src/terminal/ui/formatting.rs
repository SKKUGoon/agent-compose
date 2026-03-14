use super::DisplayMode;
use serde_json::Value;

pub fn format_answer(value: &Value, structured_output: bool, display_mode: DisplayMode) -> String {
    if !structured_output || display_mode == DisplayMode::QaCompact {
        return render_answer(value);
    }

    match display_mode {
        DisplayMode::PrettyYaml => serde_yaml::to_string(value)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }),
        DisplayMode::PrettyJson => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        DisplayMode::RawJson => value.to_string(),
        DisplayMode::QaCompact => render_answer(value),
    }
}

pub fn parse_form_value(kind: &str, input: &str) -> Result<Value, String> {
    if input.is_empty() {
        return Ok(Value::String(String::new()));
    }
    if kind == "string" || kind.starts_with("ref:") {
        return Ok(Value::String(input.to_string()));
    }
    if kind == "boolean" {
        return match input {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("boolean must be true/false".to_string()),
        };
    }
    if kind == "integer" {
        let parsed: i64 = input
            .parse()
            .map_err(|_| "invalid integer value".to_string())?;
        return Ok(Value::Number(parsed.into()));
    }
    if kind == "number" {
        let parsed: f64 = input
            .parse()
            .map_err(|_| "invalid number value".to_string())?;
        return serde_json::Number::from_f64(parsed)
            .map(Value::Number)
            .ok_or_else(|| "invalid floating number".to_string());
    }
    if kind == "array" || kind == "object" {
        let parsed: Value =
            serde_json::from_str(input).map_err(|_| format!("invalid JSON for {kind}"))?;
        return Ok(parsed);
    }
    Ok(Value::String(input.to_string()))
}

pub fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn render_answer(value: &Value) -> String {
    let gate = value
        .get("passed_gatekeeper")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reason = value
        .get("gatekeeper_reason")
        .and_then(Value::as_str)
        .unwrap_or("unspecified");
    let summary = value
        .get("summary_distilled")
        .and_then(Value::as_str)
        .unwrap_or("");
    if summary.is_empty() {
        format!(
            "gatekeeper={} reason={reason}",
            if gate { "pass" } else { "blocked" }
        )
    } else {
        format!(
            "gatekeeper={} reason={reason}\n{summary}",
            if gate { "pass" } else { "blocked" }
        )
    }
}
