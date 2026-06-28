//! JSON predicate generator for filter compilation
//!
//! Produces a framework-agnostic JSON predicate tree that can be consumed
//! by frontend clients or translated to any query language.
//!
//! # Format
//!
//! ```json
//! { "type": "or", "conditions": [
//!   { "type": "eq", "field": "owner_id", "value": "alice" },
//!   { "type": "eq", "field": "visibility", "value": "public" }
//! ]}
//! ```

use std::collections::HashMap;

use serde_json::{json, Value as JsonValue};

use crate::context::Value;
use crate::expr::{BinaryOp, Expr, UnaryOp};

use super::path_collector::FilterPath;

/// Convert filter paths to a JSON predicate value.
pub fn to_json(paths: &[FilterPath], mapping: &HashMap<String, String>) -> JsonValue {
    if paths.is_empty() {
        return json!({ "type": "never" });
    }

    if paths.iter().any(|p| p.conditions.is_empty()) {
        return json!({ "type": "always" });
    }

    if paths.len() == 1 {
        return path_to_json(&paths[0], mapping);
    }

    let conditions: Vec<JsonValue> = paths.iter().map(|p| path_to_json(p, mapping)).collect();
    json!({ "type": "or", "conditions": conditions })
}

fn path_to_json(path: &FilterPath, mapping: &HashMap<String, String>) -> JsonValue {
    match path.conditions.len() {
        0 => json!({ "type": "always" }),
        1 => expr_to_json(&path.conditions[0], mapping),
        _ => {
            let conditions: Vec<JsonValue> = path
                .conditions
                .iter()
                .map(|c| expr_to_json(c, mapping))
                .collect();
            json!({ "type": "and", "conditions": conditions })
        }
    }
}

fn expr_to_json(expr: &Expr, mapping: &HashMap<String, String>) -> JsonValue {
    match expr {
        Expr::Field(path) => {
            let col = mapping.get(path).cloned().unwrap_or_else(|| path.clone());
            json!({ "type": "field", "name": col })
        }
        Expr::Literal(val) => json!({ "type": "literal", "value": value_to_json(val) }),
        Expr::Binary { op, left, right } => binary_to_json(*op, left, right, mapping),
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => {
            // NOT(is_null(field)) → { type: "not_null", field: "..." }
            if let Expr::Call { name, args } = operand.as_ref() {
                if name == "is_null" && args.len() == 1 {
                    let col = resolve_field(&args[0], mapping);
                    return json!({ "type": "not_null", "field": col });
                }
            }
            json!({ "type": "not", "condition": expr_to_json(operand, mapping) })
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => {
            json!({ "type": "neg", "value": expr_to_json(operand, mapping) })
        }
        Expr::Call { name, args } => call_to_json(name, args, mapping),
        Expr::Array(elems) => {
            let items: Vec<JsonValue> = elems.iter().map(|e| expr_to_json(e, mapping)).collect();
            json!({ "type": "array", "items": items })
        }
        _ => json!({ "type": "unsupported" }),
    }
}

fn binary_to_json(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    mapping: &HashMap<String, String>,
) -> JsonValue {
    let op_str = match op {
        BinaryOp::Eq => "eq",
        BinaryOp::Ne => "ne",
        BinaryOp::Lt => "lt",
        BinaryOp::Le => "le",
        BinaryOp::Gt => "gt",
        BinaryOp::Ge => "ge",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::In => "in",
        BinaryOp::NotIn => "not_in",
        BinaryOp::Contains => "contains",
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "mod",
    };

    match op {
        BinaryOp::And | BinaryOp::Or => {
            json!({
                "type": op_str,
                "conditions": [expr_to_json(left, mapping), expr_to_json(right, mapping)]
            })
        }
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            // Flatten field == literal into a compact form
            if let (Expr::Field(path), Expr::Literal(val)) = (left, right) {
                let col = mapping.get(path).cloned().unwrap_or_else(|| path.clone());
                return json!({ "type": op_str, "field": col, "value": value_to_json(val) });
            }
            json!({
                "type": op_str,
                "left": expr_to_json(left, mapping),
                "right": expr_to_json(right, mapping)
            })
        }
        BinaryOp::In | BinaryOp::NotIn => {
            if let (Expr::Field(path), Expr::Literal(Value::Array(arr))) = (left, right) {
                let col = mapping.get(path).cloned().unwrap_or_else(|| path.clone());
                let values: Vec<JsonValue> = arr.iter().map(value_to_json).collect();
                return json!({ "type": op_str, "field": col, "values": values });
            }
            json!({
                "type": op_str,
                "left": expr_to_json(left, mapping),
                "right": expr_to_json(right, mapping)
            })
        }
        BinaryOp::Contains => {
            if let (Expr::Field(path), Expr::Literal(val)) = (left, right) {
                let col = mapping.get(path).cloned().unwrap_or_else(|| path.clone());
                return json!({ "type": "contains", "field": col, "value": value_to_json(val) });
            }
            json!({
                "type": "contains",
                "left": expr_to_json(left, mapping),
                "right": expr_to_json(right, mapping)
            })
        }
        _ => json!({
            "type": op_str,
            "left": expr_to_json(left, mapping),
            "right": expr_to_json(right, mapping)
        }),
    }
}

fn call_to_json(name: &str, args: &[Expr], mapping: &HashMap<String, String>) -> JsonValue {
    match (name, args) {
        ("is_null", [field]) => {
            json!({ "type": "is_null", "field": resolve_field(field, mapping) })
        }
        ("starts_with", [field, value]) => json!({
            "type": "starts_with",
            "field": resolve_field(field, mapping),
            "value": expr_to_json(value, mapping)
        }),
        ("ends_with", [field, value]) => json!({
            "type": "ends_with",
            "field": resolve_field(field, mapping),
            "value": expr_to_json(value, mapping)
        }),
        _ => {
            let args_json: Vec<JsonValue> = args.iter().map(|a| expr_to_json(a, mapping)).collect();
            json!({ "type": "call", "name": name, "args": args_json })
        }
    }
}

fn resolve_field(expr: &Expr, mapping: &HashMap<String, String>) -> String {
    if let Expr::Field(path) = expr {
        mapping.get(path).cloned().unwrap_or_else(|| path.clone())
    } else {
        "unknown".to_string()
    }
}

fn value_to_json(val: &Value) -> JsonValue {
    match val {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::Int(n) => JsonValue::Number((*n).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::String(s) => JsonValue::String(s.to_string()),
        Value::Array(arr) => JsonValue::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(map) => {
            let obj: serde_json::Map<String, JsonValue> = map
                .iter()
                .map(|(k, v)| (k.to_string(), value_to_json(v)))
                .collect();
            JsonValue::Object(obj)
        }
    }
}
