use crate::config::{FieldSpec, ModelSpec};
use serde_json::{Map, Value};

pub(super) fn model_to_value(model: &ModelSpec) -> Value {
    let mut fields = Map::new();
    for (name, spec) in &model.fields {
        fields.insert(name.clone(), field_to_value(spec));
    }

    Value::Object(
        [
            ("type".to_string(), Value::String(model.kind.clone())),
            ("fields".to_string(), Value::Object(fields)),
        ]
        .into_iter()
        .collect(),
    )
}

fn field_to_value(spec: &FieldSpec) -> Value {
    let mut out = Map::new();

    if let Some(kind) = &spec.kind {
        out.insert("type".to_string(), Value::String(kind.clone()));
    }
    if let Some(r) = &spec.ref_model {
        out.insert("$ref".to_string(), Value::String(r.clone()));
    }
    if let Some(required) = spec.required {
        out.insert("required".to_string(), Value::Bool(required));
    }
    if let Some(nullable) = spec.nullable {
        out.insert("nullable".to_string(), Value::Bool(nullable));
    }
    if let Some(default) = &spec.default {
        out.insert("default".to_string(), default.clone());
    }
    if let Some(enum_values) = &spec.enum_values {
        out.insert("enum".to_string(), Value::Array(enum_values.clone()));
    }
    if let Some(min) = spec.min_length {
        out.insert(
            "min_length".to_string(),
            Value::Number(serde_json::Number::from(min)),
        );
    }
    if let Some(max) = spec.max_length {
        out.insert(
            "max_length".to_string(),
            Value::Number(serde_json::Number::from(max)),
        );
    }
    if let Some(items) = &spec.items {
        out.insert("items".to_string(), field_to_value(items));
    }

    Value::Object(out)
}
