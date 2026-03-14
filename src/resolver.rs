use regex::Regex;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("unknown reference path: {0}")]
    UnknownPath(String),
}

pub fn resolve_refs(value: &Value, context: &Value) -> Result<Value, ResolveError> {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_refs(v, context)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for v in items {
                out.push(resolve_refs(v, context)?);
            }
            Ok(Value::Array(out))
        }
        Value::String(s) => resolve_string(s, context),
        _ => Ok(value.clone()),
    }
}

fn resolve_string(input: &str, context: &Value) -> Result<Value, ResolveError> {
    let full_re = Regex::new(r"^\$\{\{\s*([^}]+?)\s*\}\}$").expect("valid regex");
    let partial_re = Regex::new(r"\$\{\{\s*([^}]+?)\s*\}\}").expect("valid regex");

    if let Some(caps) = full_re.captures(input)
        && let Some(expr) = caps.get(1)
    {
        return deep_get(context, expr.as_str());
    }

    let mut out = String::new();
    let mut last_end = 0;
    for cap in partial_re.captures_iter(input) {
        let matched = cap.get(0).expect("matched by regex");
        let expr = cap.get(1).expect("captured expression").as_str();
        out.push_str(&input[last_end..matched.start()]);
        let resolved = deep_get(context, expr)?;
        match resolved {
            Value::Object(_) | Value::Array(_) => {
                out.push_str(&serde_json::to_string(&resolved).unwrap_or_default())
            }
            Value::String(s) => out.push_str(&s),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::Bool(b) => out.push_str(if b { "true" } else { "false" }),
            Value::Null => out.push_str("null"),
        }
        last_end = matched.end();
    }
    if last_end == 0 {
        return Ok(Value::String(input.to_string()));
    }
    out.push_str(&input[last_end..]);
    Ok(Value::String(out))
}

fn deep_get(context: &Value, path: &str) -> Result<Value, ResolveError> {
    let mut current = context;
    for segment in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map
                    .get(segment)
                    .ok_or_else(|| ResolveError::UnknownPath(path.to_string()))?;
            }
            _ => return Err(ResolveError::UnknownPath(path.to_string())),
        }
    }
    Ok(current.clone())
}
