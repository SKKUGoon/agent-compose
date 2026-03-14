use super::LoadError;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn env_regex() -> &'static Regex {
    static ENV_REGEX: OnceLock<Regex> = OnceLock::new();
    ENV_REGEX.get_or_init(|| {
        Regex::new(r"\$\{env:([A-Za-z_][A-Za-z0-9_]*)(:-([^}]*))?\}")
            .unwrap_or_else(|err| panic!("invalid env interpolation regex: {err}"))
    })
}

pub(super) fn interpolate_env(value: &mut Value) -> Result<(), LoadError> {
    match value {
        Value::Object(map) => {
            for v in map.values_mut() {
                interpolate_env(v)?;
            }
        }
        Value::Array(items) => {
            for v in items {
                interpolate_env(v)?;
            }
        }
        Value::String(s) => {
            *s = interpolate_env_string(s)?;
        }
        _ => {}
    }
    Ok(())
}

fn interpolate_env_string(input: &str) -> Result<String, LoadError> {
    let re = env_regex();

    let mut out = String::new();
    let mut last = 0;

    for caps in re.captures_iter(input) {
        let Some(whole) = caps.get(0) else {
            continue;
        };
        out.push_str(&input[last..whole.start()]);

        let Some(key) = caps.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let fallback = caps.get(3).map(|m| m.as_str());

        match std::env::var(key) {
            Ok(v) => out.push_str(&v),
            Err(_) => {
                if let Some(default) = fallback {
                    out.push_str(default);
                } else {
                    return Err(LoadError::MissingEnv(key.to_string()));
                }
            }
        }

        last = whole.end();
    }

    if last == 0 {
        return Ok(input.to_string());
    }

    out.push_str(&input[last..]);
    Ok(out)
}
