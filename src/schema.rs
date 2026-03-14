use crate::config::{FieldSpec, ModelSpec};
use serde_json::{Map, Value};
use std::collections::HashMap;
use thiserror::Error;

#[path = "schema/contract.rs"]
mod contract;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("unknown schema model: {0}")]
    UnknownModel(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    models: HashMap<String, ModelSpec>,
}

impl SchemaRegistry {
    pub fn new(models: HashMap<String, ModelSpec>) -> Result<Self, SchemaError> {
        if models.is_empty() {
            return Err(SchemaError::Invalid(
                "schema.models cannot be empty".to_string(),
            ));
        }
        Ok(Self { models })
    }

    pub fn validate(&self, model_name: &str, value: Value) -> Result<Value, SchemaError> {
        let model = self
            .models
            .get(model_name)
            .ok_or_else(|| SchemaError::UnknownModel(model_name.to_string()))?;
        self.validate_model(model_name, model, value)
    }

    pub fn output_contract(&self, model_name: &str) -> Result<Value, SchemaError> {
        let _ = self
            .models
            .get(model_name)
            .ok_or_else(|| SchemaError::UnknownModel(model_name.to_string()))?;

        let mut models_obj = Map::new();
        for (name, model) in &self.models {
            models_obj.insert(name.clone(), contract::model_to_value(model));
        }

        Ok(Value::Object(
            [
                (
                    "output_model".to_string(),
                    Value::String(model_name.to_string()),
                ),
                ("models".to_string(), Value::Object(models_obj)),
            ]
            .into_iter()
            .collect(),
        ))
    }

    fn validate_model(
        &self,
        model_name: &str,
        model: &ModelSpec,
        value: Value,
    ) -> Result<Value, SchemaError> {
        if model.kind != "object" {
            return Err(SchemaError::Invalid(format!(
                "model {model_name} is not type object"
            )));
        }
        let obj = value
            .as_object()
            .ok_or_else(|| SchemaError::Invalid(format!("{model_name} expects object payload")))?;

        let mut out = Map::new();

        for (field_name, field_spec) in &model.fields {
            let input = obj.get(field_name).cloned();
            let val = self.validate_field(model_name, field_name, field_spec, input)?;
            if !val.is_null()
                || field_spec.required.unwrap_or(false)
                || field_spec.default.is_some()
            {
                out.insert(field_name.clone(), val);
            }
        }

        Ok(Value::Object(out))
    }

    fn validate_field(
        &self,
        model_name: &str,
        field_name: &str,
        spec: &FieldSpec,
        input: Option<Value>,
    ) -> Result<Value, SchemaError> {
        let required = spec.required.unwrap_or(false);
        let nullable = spec.nullable.unwrap_or(false);

        let value = match input {
            Some(v) => v,
            None => {
                if let Some(default) = &spec.default {
                    return Ok(default.clone());
                }
                if required {
                    return Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} is required"
                    )));
                }
                return Ok(Value::Null);
            }
        };

        if value.is_null() {
            if nullable || !required {
                return Ok(Value::Null);
            }
            return Err(SchemaError::Invalid(format!(
                "{model_name}.{field_name} cannot be null"
            )));
        }

        if let Some(allowed) = &spec.enum_values
            && !allowed.contains(&value)
        {
            return Err(SchemaError::Invalid(format!(
                "{model_name}.{field_name} contains disallowed enum value"
            )));
        }

        if let Some(ref_model) = &spec.ref_model {
            return self.validate(ref_model, value);
        }

        let kind = spec.kind.as_deref().ok_or_else(|| {
            SchemaError::Invalid(format!("{model_name}.{field_name} missing type or $ref"))
        })?;

        match kind {
            "string" => {
                let s = value.as_str().ok_or_else(|| {
                    SchemaError::Invalid(format!("{model_name}.{field_name} must be string"))
                })?;
                if let Some(min) = spec.min_length
                    && s.chars().count() < min
                {
                    return Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} shorter than min_length"
                    )));
                }
                if let Some(max) = spec.max_length
                    && s.chars().count() > max
                {
                    return Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} longer than max_length"
                    )));
                }
                Ok(Value::String(s.to_string()))
            }
            "boolean" => {
                if value.is_boolean() {
                    Ok(value)
                } else {
                    Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} must be boolean"
                    )))
                }
            }
            "integer" => {
                if value.as_i64().is_some() {
                    Ok(value)
                } else {
                    Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} must be integer"
                    )))
                }
            }
            "number" => {
                if value.is_number() {
                    Ok(value)
                } else {
                    Err(SchemaError::Invalid(format!(
                        "{model_name}.{field_name} must be number"
                    )))
                }
            }
            "array" => {
                let items = value.as_array().ok_or_else(|| {
                    SchemaError::Invalid(format!("{model_name}.{field_name} must be array"))
                })?;
                let item_spec = spec.items.as_ref().ok_or_else(|| {
                    SchemaError::Invalid(format!("{model_name}.{field_name} missing items"))
                })?;
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let validated =
                        self.validate_field(model_name, field_name, item_spec, Some(item.clone()))?;
                    out.push(validated);
                }
                Ok(Value::Array(out))
            }
            _ => Err(SchemaError::Invalid(format!(
                "unsupported type {kind} at {model_name}.{field_name}"
            ))),
        }
    }
}
