use super::RuntimeError;
use serde_json::{json, Value};
use std::collections::HashSet;

pub fn build_pipeline_output(payload: Value) -> Result<Value, RuntimeError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| RuntimeError::Invalid("step payload must be object".to_string()))?;

    let assets = dedupe_tag_pair(obj.get("assets").cloned().unwrap_or_else(|| json!([])));
    let policies = dedupe_tag_pair(obj.get("policies").cloned().unwrap_or_else(|| json!([])));
    let events = dedupe_event_labels(obj.get("events").cloned().unwrap_or_else(|| json!([])));
    let countries = dedupe_countries(obj.get("countries").cloned().unwrap_or_else(|| json!([])));

    let legacy_theme = fallback_legacy_theme(&assets, &policies, &events);

    let summary = obj
        .get("perceived_news")
        .and_then(Value::as_str)
        .map(|s| trim_words(s, 40))
        .unwrap_or_default();

    Ok(json!({
        "passed_gatekeeper": obj.get("passed_gatekeeper").and_then(Value::as_bool).unwrap_or(false),
        "gatekeeper_reason": obj.get("gatekeeper_reason").and_then(Value::as_str).unwrap_or("unspecified"),
        "summary_distilled": summary,
        "countries": countries,
        "cities": obj.get("cities").cloned().unwrap_or_else(|| json!([])),
        "assets": assets,
        "policies": policies,
        "events": events,
        "legacy_theme": legacy_theme
    }))
}

fn trim_words(input: &str, max_words: usize) -> String {
    let words: Vec<_> = input.split_whitespace().collect();
    if words.len() <= max_words {
        return input.trim().to_string();
    }
    words[..max_words].join(" ")
}

fn dedupe_countries(v: Value) -> Value {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    if let Some(items) = v.as_array() {
        for item in items {
            if let Some(obj) = item.as_object() {
                let country = obj
                    .get("country")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if country.is_empty() {
                    continue;
                }
                let key = country.to_lowercase();
                if seen.insert(key) {
                    out.push(json!({"country": country, "note": obj.get("note").cloned().unwrap_or(Value::Null)}));
                }
            }
        }
    }
    Value::Array(out)
}

fn dedupe_tag_pair(v: Value) -> Value {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    if let Some(items) = v.as_array() {
        for item in items {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let Some(tag) = obj.get("tag").and_then(Value::as_object) else {
                continue;
            };
            let upper = tag
                .get("upper")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let lower = tag
                .get("lower")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if upper.is_empty() || lower.is_empty() {
                continue;
            }
            let key = format!("{upper}::{lower}");
            if seen.insert(key) {
                out.push(item.clone());
            }
        }
    }
    Value::Array(out)
}

fn dedupe_event_labels(v: Value) -> Value {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    if let Some(items) = v.as_array() {
        for item in items {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let Some(tag) = obj.get("tag").and_then(Value::as_object) else {
                continue;
            };
            let label = tag
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if label.is_empty() {
                continue;
            }
            if seen.insert(label) {
                out.push(item.clone());
            }
        }
    }
    Value::Array(out)
}

fn fallback_legacy_theme(assets: &Value, policies: &Value, events: &Value) -> Value {
    if let Some(first) = assets.as_array().and_then(|v| v.first())
        && let Some(tag) = first.get("tag").and_then(Value::as_object)
    {
        let upper = tag.get("upper").and_then(Value::as_str).unwrap_or("Other");
        let lower = tag.get("lower").and_then(Value::as_str).unwrap_or("other");
        return json!({"upper": format!("Asset({upper})"), "lower": lower});
    }
    if let Some(first) = policies.as_array().and_then(|v| v.first())
        && let Some(tag) = first.get("tag").and_then(Value::as_object)
    {
        let upper = tag.get("upper").and_then(Value::as_str).unwrap_or("Other");
        let lower = tag.get("lower").and_then(Value::as_str).unwrap_or("other");
        return json!({"upper": format!("Policy({upper})"), "lower": lower});
    }
    if let Some(first) = events.as_array().and_then(|v| v.first())
        && let Some(tag) = first.get("tag").and_then(Value::as_object)
    {
        let lower = tag.get("label").and_then(Value::as_str).unwrap_or("Other");
        return json!({"upper": "Event(Other)", "lower": lower});
    }
    json!({"upper": "Other", "lower": "other"})
}
