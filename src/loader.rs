use crate::config::{ComposeConfig, ModelSpec};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[path = "loader/env.rs"]
mod env;
#[path = "loader/paths.rs"]
mod paths;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse yaml in {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to parse json in {path}: {source}")]
    JsonParse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid config: {0}")]
    Validate(String),
    #[error("missing environment variable: {0}")]
    MissingEnv(String),
}

#[derive(Debug, Deserialize)]
struct SchemaFile {
    models: HashMap<String, ModelSpec>,
}

pub fn load_compose_config(path: &Path) -> Result<ComposeConfig, LoadError> {
    let _ = dotenvy::dotenv();

    let raw = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let mut raw_value: Value = serde_yaml::from_str(&raw).map_err(|source| LoadError::Parse {
        path: path.display().to_string(),
        source,
    })?;

    env::interpolate_env(&mut raw_value)?;

    let mut config: ComposeConfig =
        serde_json::from_value(raw_value).map_err(|source| LoadError::JsonParse {
            path: path.display().to_string(),
            source,
        })?;

    let schema_path = paths::resolve_schema_path(path, &config.schema.file);
    let schema_raw = std::fs::read_to_string(&schema_path).map_err(|source| LoadError::Read {
        path: schema_path.display().to_string(),
        source,
    })?;
    let mut schema_value: Value =
        serde_yaml::from_str(&schema_raw).map_err(|source| LoadError::Parse {
            path: schema_path.display().to_string(),
            source,
        })?;
    env::interpolate_env(&mut schema_value)?;
    let schema_file: SchemaFile =
        serde_json::from_value(schema_value).map_err(|source| LoadError::JsonParse {
            path: schema_path.display().to_string(),
            source,
        })?;
    config.schema.models = schema_file.models;

    config.validate().map_err(LoadError::Validate)?;
    Ok(config)
}
