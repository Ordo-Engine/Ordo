//! Input contract for a RuleSet
//!
//! A ruleset can declare the input fields it expects. Before execution the
//! input is checked against the declaration: missing required fields and type
//! mismatches are rejected with one error listing every problem, and missing
//! optional fields with a default are filled in. Without this, a missing field
//! silently makes a condition false (e.g. `age < 18` passes when `age` is not
//! sent), which is fail-open for hard business rules.
//!
//! The shape matches the Studio editor's `SchemaField`, so a schema authored
//! there is enforced by the engine as-is.

use crate::context::Value;
use crate::error::{OrdoError, Result};
use serde::{Deserialize, Serialize};

/// Declared type of an input field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputFieldType {
    String,
    /// Integer or float
    Number,
    /// Exact decimal (money). Numbers and numeric strings are converted to a
    /// decimal value before execution; send amounts as strings to keep more
    /// than ~15 significant digits.
    Decimal,
    Boolean,
    Array,
    Object,
    /// No type check (also used for unrecognized type names)
    #[default]
    #[serde(other)]
    Any,
}

/// One declared input field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputField {
    /// Field name (one path segment; nest with `fields`)
    pub name: String,

    /// Expected type
    #[serde(rename = "type", default)]
    pub field_type: InputFieldType,

    /// Must be present and non-null
    #[serde(default)]
    pub required: bool,

    /// Description (documentation only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Value used when the field is absent (or null)
    #[serde(
        default,
        rename = "defaultValue",
        alias = "default_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_value: Option<Value>,

    /// Nested fields, for `object`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<InputField>,

    /// Element schema, for `array`
    #[serde(
        default,
        rename = "itemType",
        alias = "item_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub item_type: Option<Box<InputField>>,
}

/// Validate `input` against `schema`, filling in defaults.
///
/// Returns `OrdoError::InvalidInput` listing every violation.
pub fn apply_input_schema(schema: &[InputField], input: &mut Value) -> Result<()> {
    if schema.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();
    match input {
        Value::Object(_) => check_fields(schema, input, "", &mut errors),
        Value::Null => {
            *input = Value::object(std::collections::HashMap::new());
            check_fields(schema, input, "", &mut errors);
        }
        other => errors.push(format!("input: expected object, got {}", other.type_name())),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(OrdoError::InvalidInput { errors })
    }
}

fn check_fields(fields: &[InputField], obj: &mut Value, prefix: &str, errors: &mut Vec<String>) {
    let Value::Object(map) = obj else {
        return;
    };
    for field in fields {
        let path = if prefix.is_empty() {
            field.name.clone()
        } else {
            format!("{prefix}.{}", field.name)
        };
        let absent = matches!(map.get(field.name.as_str()), None | Some(Value::Null));
        if absent {
            if let Some(default) = &field.default_value {
                map.insert(field.name.as_str().into(), default.clone());
            } else if field.required {
                errors.push(format!("{path}: required field is missing"));
                continue;
            } else {
                continue;
            }
        }
        if let Some(value) = map.get_mut(field.name.as_str()) {
            check_value(field, value, &path, errors);
        }
    }
}

fn check_value(field: &InputField, value: &mut Value, path: &str, errors: &mut Vec<String>) {
    let ok = match field.field_type {
        InputFieldType::Any => true,
        InputFieldType::String => matches!(value, Value::String(_)),
        InputFieldType::Number => matches!(value, Value::Int(_) | Value::Float(_)),
        InputFieldType::Decimal => {
            let converted = match &*value {
                Value::String(s) => crate::expr::parse_decimal(s.trim()),
                other => crate::context::to_decimal(other),
            };
            match converted {
                Some(d) => {
                    *value = Value::Decimal(d);
                    true
                }
                None => false,
            }
        }
        InputFieldType::Boolean => matches!(value, Value::Bool(_)),
        InputFieldType::Array => matches!(value, Value::Array(_)),
        InputFieldType::Object => matches!(value, Value::Object(_)),
    };
    if !ok {
        let field_type = match field.field_type {
            InputFieldType::String => "string",
            InputFieldType::Number => "number",
            InputFieldType::Decimal => "decimal",
            InputFieldType::Boolean => "boolean",
            InputFieldType::Array => "array",
            InputFieldType::Object => "object",
            InputFieldType::Any => "any",
        };
        errors.push(format!(
            "{path}: expected {field_type}, got {}",
            value.type_name()
        ));
        return;
    }
    if !field.fields.is_empty() && matches!(value, Value::Object(_)) {
        check_fields(&field.fields, value, path, errors);
    }
    if let (Some(item), Value::Array(items)) = (&field.item_type, value) {
        for (i, element) in items.iter_mut().enumerate() {
            let element_path = format!("{path}[{i}]");
            if matches!(element, Value::Null) {
                if item.required {
                    errors.push(format!("{element_path}: required element is null"));
                }
                continue;
            }
            check_value(item, element, &element_path, errors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(json: &str) -> Vec<InputField> {
        serde_json::from_str(json).unwrap()
    }

    fn input(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    fn errors(result: Result<()>) -> Vec<String> {
        match result {
            Err(OrdoError::InvalidInput { errors }) => errors,
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn accepts_studio_shape_and_reports_every_violation() {
        let schema = schema(
            r#"[
              {"name": "age", "type": "number", "required": true},
              {"name": "country", "type": "string", "defaultValue": "CN"},
              {"name": "applicant", "type": "object", "required": true, "fields": [
                {"name": "score", "type": "number", "required": true}
              ]},
              {"name": "items", "type": "array", "itemType": {"name": "item", "type": "object", "fields": [
                {"name": "price", "type": "number", "required": true}
              ]}}
            ]"#,
        );

        let mut ok =
            input(r#"{"age": 30, "applicant": {"score": 700.0}, "items": [{"price": 1}]}"#);
        apply_input_schema(&schema, &mut ok).unwrap();
        assert_eq!(ok.get_path("country"), Some(&Value::string("CN")));

        let mut bad = input(r#"{"applicant": {"score": "700"}, "items": [{"price": 1}, {}]}"#);
        let errs = errors(apply_input_schema(&schema, &mut bad));
        assert_eq!(
            errs,
            vec![
                "age: required field is missing",
                "applicant.score: expected number, got string",
                "items[1].price: required field is missing",
            ]
        );
    }

    #[test]
    fn decimal_fields_are_converted_exactly() {
        let schema = schema(
            r#"[{"name": "price", "type": "decimal", "required": true},
                {"name": "fee", "type": "decimal"},
                {"name": "rate", "type": "decimal"}]"#,
        );
        let mut ok = input(r#"{"price": 0.1, "fee": "12345678901234567.89", "rate": 3}"#);
        apply_input_schema(&schema, &mut ok).unwrap();
        assert_eq!(
            ok.get_path("price"),
            Some(&Value::decimal("0.1".parse().unwrap()))
        );
        assert_eq!(
            ok.get_path("fee"),
            Some(&Value::decimal("12345678901234567.89".parse().unwrap()))
        );
        assert!(ok.get_path("rate").unwrap().is_decimal());

        let errs = errors(apply_input_schema(
            &schema,
            &mut input(r#"{"price": "abc"}"#),
        ));
        assert_eq!(errs, vec!["price: expected decimal, got string"]);
    }

    #[test]
    fn null_counts_as_missing() {
        let schema = schema(r#"[{"name": "age", "type": "number", "required": true}]"#);
        let errs = errors(apply_input_schema(&schema, &mut input(r#"{"age": null}"#)));
        assert_eq!(errs, vec!["age: required field is missing"]);
    }

    #[test]
    fn empty_schema_accepts_anything() {
        apply_input_schema(&[], &mut Value::int(1)).unwrap();
    }
}
