//! Studio expression types (mirrors the TypeScript Expr model)

use serde::{Deserialize, Serialize};

/// Studio expression — mirrors the frontend `Expr` union type.
///
/// JSON uses `{ "type": "literal", "value": 42, "valueType": "number" }` etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StudioExpr {
    Literal {
        value: serde_json::Value,
        #[serde(rename = "valueType", default)]
        value_type: Option<String>,
    },
    Variable {
        path: String,
    },
    Binary {
        op: String,
        left: Box<StudioExpr>,
        right: Box<StudioExpr>,
    },
    Unary {
        op: String,
        operand: Box<StudioExpr>,
    },
    Member {
        object: Box<StudioExpr>,
        property: String,
    },
    Array {
        elements: Vec<StudioExpr>,
    },
    Object {
        entries: Vec<(String, StudioExpr)>,
    },
    Function {
        name: String,
        args: Vec<StudioExpr>,
    },
}

/// Convert a `StudioExpr` to an expression string that ordo-core can parse.
pub fn expr_to_string(expr: &StudioExpr) -> String {
    match expr {
        StudioExpr::Literal { value, .. } => json_value_to_literal(value),
        StudioExpr::Variable { path } => {
            // "$.user.age" → "user.age"  (context field)
            // "$result"    → "$result"   (variable, keep $ prefix)
            if let Some(stripped) = path.strip_prefix("$.") {
                stripped.to_string()
            } else {
                path.clone()
            }
        }
        StudioExpr::Binary { op, left, right } => {
            let op_str = binary_op_display(op);
            format!(
                "({} {} {})",
                expr_to_string(left),
                op_str,
                expr_to_string(right)
            )
        }
        StudioExpr::Unary { op, operand } => {
            let op_str = match op.as_str() {
                "not" | "!" => "!",
                "neg" | "-" => "-",
                other => other,
            };
            format!("{}({})", op_str, expr_to_string(operand))
        }
        StudioExpr::Function { name, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_string).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        StudioExpr::Member { object, property } => {
            format!("{}.{}", expr_to_string(object), property)
        }
        StudioExpr::Array { elements } => {
            let items: Vec<String> = elements.iter().map(expr_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        StudioExpr::Object { .. } => "null".to_string(),
    }
}

fn json_value_to_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_value_to_literal).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(_) => "null".to_string(),
    }
}

/// Map a studio operator string to an engine-parseable operator string.
///
/// Returns `None` for unknown operators (caller should handle the raw string).
pub(crate) fn map_binary_op(op: &str) -> Option<&'static str> {
    match op {
        "add" | "+" => Some("+"),
        "sub" | "-" => Some("-"),
        "mul" | "*" => Some("*"),
        "div" | "/" => Some("/"),
        "mod" | "%" => Some("%"),
        "eq" | "==" => Some("=="),
        "neq" | "ne" | "!=" => Some("!="),
        "gt" | ">" => Some(">"),
        "gte" | ">=" => Some(">="),
        "lt" | "<" => Some("<"),
        "lte" | "<=" => Some("<="),
        "and" | "&&" => Some("&&"),
        "or" | "||" => Some("||"),
        "in" => Some("in"),
        "not_in" => Some("not in"),
        "contains" => Some("contains"),
        _ => None,
    }
}

/// Map a studio binary op string to its display form (falls back to the raw string).
pub(crate) fn binary_op_display(op: &str) -> String {
    map_binary_op(op)
        .map(str::to_string)
        .unwrap_or_else(|| op.to_string())
}
