//! MongoDB `$match` predicate generator for filter compilation
//!
//! Converts filter paths into a MongoDB aggregation pipeline `$match` stage.
//! Conditions within a path are ANDed; multiple paths are ORed via `$or`.
//!
//! # Format
//!
//! ```json
//! { "$or": [
//!   { "owner_id": "alice" },
//!   { "visibility": "public" }
//! ]}
//! ```
//!
//! An always-matches result is an empty document `{}` (no filter).
//! A never-matches result is `{ "$expr": false }`.

use std::collections::HashMap;

use serde_json::{json, Map, Value as JsonValue};

use crate::context::Value;
use crate::expr::{BinaryOp, Expr, UnaryOp};

use super::path_collector::FilterPath;

/// Convert filter paths to a MongoDB `$match` predicate.
pub fn to_mongo(paths: &[FilterPath], mapping: &HashMap<String, String>) -> JsonValue {
    if paths.is_empty() {
        return json!({ "$expr": false });
    }

    if paths.iter().any(|p| p.conditions.is_empty()) {
        return json!({});
    }

    if paths.len() == 1 {
        return path_to_mongo(&paths[0], mapping);
    }

    let clauses: Vec<JsonValue> = paths.iter().map(|p| path_to_mongo(p, mapping)).collect();
    json!({ "$or": clauses })
}

fn path_to_mongo(path: &FilterPath, mapping: &HashMap<String, String>) -> JsonValue {
    match path.conditions.len() {
        0 => json!({}),
        1 => expr_to_mongo(&path.conditions[0], mapping),
        _ => {
            let clauses: Vec<JsonValue> = path
                .conditions
                .iter()
                .map(|c| expr_to_mongo(c, mapping))
                .collect();
            json!({ "$and": clauses })
        }
    }
}

fn expr_to_mongo(expr: &Expr, mapping: &HashMap<String, String>) -> JsonValue {
    match expr {
        Expr::Binary { op, left, right } => binary_to_mongo(*op, left, right, mapping),
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => not_to_mongo(operand, mapping),
        Expr::Call { name, args } => call_to_mongo(name, args, mapping),
        _ => json!({ "$expr": false }),
    }
}

fn binary_to_mongo(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    mapping: &HashMap<String, String>,
) -> JsonValue {
    match op {
        BinaryOp::Eq => {
            if let Expr::Field(path) = left {
                return obj1(resolve_col(path, mapping), literal_or_null(right));
            }
            if let Expr::Field(path) = right {
                return obj1(resolve_col(path, mapping), literal_or_null(left));
            }
            json!({ "$expr": false })
        }
        BinaryOp::Ne => {
            if let Expr::Field(path) = left {
                return obj1(
                    resolve_col(path, mapping),
                    op_obj("$ne", literal_or_null(right)),
                );
            }
            if let Expr::Field(path) = right {
                return obj1(
                    resolve_col(path, mapping),
                    op_obj("$ne", literal_or_null(left)),
                );
            }
            json!({ "$expr": false })
        }
        BinaryOp::Lt => cmp_to_mongo("$lt", "$gt", left, right, mapping),
        BinaryOp::Le => cmp_to_mongo("$lte", "$gte", left, right, mapping),
        BinaryOp::Gt => cmp_to_mongo("$gt", "$lt", left, right, mapping),
        BinaryOp::Ge => cmp_to_mongo("$gte", "$lte", left, right, mapping),
        BinaryOp::And => {
            json!({ "$and": [expr_to_mongo(left, mapping), expr_to_mongo(right, mapping)] })
        }
        BinaryOp::Or => {
            json!({ "$or": [expr_to_mongo(left, mapping), expr_to_mongo(right, mapping)] })
        }
        BinaryOp::In => {
            if let (Expr::Field(path), Expr::Literal(Value::Array(arr))) = (left, right) {
                let col = resolve_col(path, mapping);
                let values: Vec<JsonValue> = arr.iter().map(value_to_json).collect();
                return obj1(col, op_obj("$in", JsonValue::Array(values)));
            }
            json!({ "$expr": false })
        }
        BinaryOp::NotIn => {
            if let (Expr::Field(path), Expr::Literal(Value::Array(arr))) = (left, right) {
                let col = resolve_col(path, mapping);
                let values: Vec<JsonValue> = arr.iter().map(value_to_json).collect();
                return obj1(col, op_obj("$nin", JsonValue::Array(values)));
            }
            json!({ "$expr": false })
        }
        BinaryOp::Contains => {
            if let (Expr::Field(path), Expr::Literal(Value::String(s))) = (left, right) {
                let col = resolve_col(path, mapping);
                return obj1(col, op_obj("$regex", JsonValue::String(regex_escape(s))));
            }
            json!({ "$expr": false })
        }
        _ => json!({ "$expr": false }),
    }
}

/// Comparison operator, handling field on either side.
/// If the field is on the right, the operator is flipped.
fn cmp_to_mongo(
    op_field_left: &str,
    op_field_right: &str,
    left: &Expr,
    right: &Expr,
    mapping: &HashMap<String, String>,
) -> JsonValue {
    if let Expr::Field(path) = left {
        return obj1(
            resolve_col(path, mapping),
            op_obj(op_field_left, literal_or_null(right)),
        );
    }
    if let Expr::Field(path) = right {
        return obj1(
            resolve_col(path, mapping),
            op_obj(op_field_right, literal_or_null(left)),
        );
    }
    json!({ "$expr": false })
}

fn not_to_mongo(operand: &Expr, mapping: &HashMap<String, String>) -> JsonValue {
    // NOT(is_null(field)) → { field: { $ne: null, $exists: true } }
    if let Expr::Call { name, args } = operand {
        if name == "is_null" {
            if let [Expr::Field(path)] = args.as_slice() {
                let col = resolve_col(path, mapping);
                let mut inner = Map::new();
                inner.insert("$ne".to_string(), JsonValue::Null);
                inner.insert("$exists".to_string(), JsonValue::Bool(true));
                return obj1(col, JsonValue::Object(inner));
            }
        }
    }
    // General NOT → $nor
    json!({ "$nor": [expr_to_mongo(operand, mapping)] })
}

fn call_to_mongo(name: &str, args: &[Expr], mapping: &HashMap<String, String>) -> JsonValue {
    match (name, args) {
        ("is_null", [Expr::Field(path)]) => obj1(resolve_col(path, mapping), JsonValue::Null),
        ("starts_with", [Expr::Field(path), Expr::Literal(Value::String(s))]) => {
            let col = resolve_col(path, mapping);
            let pattern = format!("^{}", regex_escape(s));
            obj1(col, op_obj("$regex", JsonValue::String(pattern)))
        }
        ("ends_with", [Expr::Field(path), Expr::Literal(Value::String(s))]) => {
            let col = resolve_col(path, mapping);
            let pattern = format!("{}$", regex_escape(s));
            obj1(col, op_obj("$regex", JsonValue::String(pattern)))
        }
        _ => json!({ "$expr": false }),
    }
}

// --- helpers ---

fn resolve_col(path: &str, mapping: &HashMap<String, String>) -> String {
    mapping
        .get(path)
        .cloned()
        .unwrap_or_else(|| path.replace('.', "_"))
}

/// Build `{ key: val }` with a dynamic key.
fn obj1(key: String, val: JsonValue) -> JsonValue {
    let mut map = Map::new();
    map.insert(key, val);
    JsonValue::Object(map)
}

/// Build `{ "$op": val }`.
fn op_obj(op: &str, val: JsonValue) -> JsonValue {
    let mut map = Map::new();
    map.insert(op.to_string(), val);
    JsonValue::Object(map)
}

/// Extract the JSON value from a literal expression, or `null` for non-literals.
fn literal_or_null(expr: &Expr) -> JsonValue {
    if let Expr::Literal(val) = expr {
        value_to_json(val)
    } else {
        JsonValue::Null
    }
}

/// Escape regex metacharacters in a string literal for use in `$regex`.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '.' | '^' | '$' | '*' | '+' | '?' | '{' | '}' | '[' | ']' | '\\' | '|' | '(' | ')'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Value;
    use crate::expr::{BinaryOp, Expr};
    use crate::filter::path_collector::FilterPath;

    fn field(s: &str) -> Expr {
        Expr::Field(s.to_string())
    }

    fn lit_str(s: &str) -> Expr {
        Expr::Literal(Value::String(s.to_string().into()))
    }

    fn lit_int(n: i64) -> Expr {
        Expr::Literal(Value::Int(n))
    }

    fn lit_arr(vals: Vec<&str>) -> Expr {
        Expr::Literal(Value::Array(
            vals.into_iter()
                .map(|s| Value::String(s.to_string().into()))
                .collect(),
        ))
    }

    fn path_with(conditions: Vec<Expr>) -> FilterPath {
        FilterPath {
            conditions,
            result_code: "OK".to_string(),
        }
    }

    fn eq_expr(l: Expr, r: Expr) -> Expr {
        Expr::Binary {
            op: BinaryOp::Eq,
            left: Box::new(l),
            right: Box::new(r),
        }
    }

    #[test]
    fn test_mongo_never_matches() {
        let result = to_mongo(&[], &HashMap::new());
        assert_eq!(result, json!({ "$expr": false }));
    }

    #[test]
    fn test_mongo_always_matches() {
        let result = to_mongo(&[path_with(vec![])], &HashMap::new());
        assert_eq!(result, json!({}));
    }

    #[test]
    fn test_mongo_single_eq() {
        let paths = vec![path_with(vec![eq_expr(field("owner"), lit_str("alice"))])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "owner": "alice" }));
    }

    #[test]
    fn test_mongo_field_mapping() {
        let mut mapping = HashMap::new();
        mapping.insert("resource.owner".to_string(), "owner_id".to_string());

        let paths = vec![path_with(vec![eq_expr(
            field("resource.owner"),
            lit_str("alice"),
        )])];
        let result = to_mongo(&paths, &mapping);
        assert_eq!(result, json!({ "owner_id": "alice" }));
    }

    #[test]
    fn test_mongo_multi_path_or() {
        let paths = vec![
            path_with(vec![eq_expr(field("owner"), lit_str("alice"))]),
            path_with(vec![eq_expr(field("visibility"), lit_str("public"))]),
        ];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(
            result,
            json!({ "$or": [{ "owner": "alice" }, { "visibility": "public" }] })
        );
    }

    #[test]
    fn test_mongo_and_within_path() {
        let cond1 = eq_expr(field("visibility"), lit_str("public"));
        let cond2 = eq_expr(field("status"), lit_str("published"));
        let paths = vec![path_with(vec![cond1, cond2])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(
            result,
            json!({ "$and": [{ "visibility": "public" }, { "status": "published" }] })
        );
    }

    #[test]
    fn test_mongo_in_operator() {
        let cond = Expr::Binary {
            op: BinaryOp::In,
            left: Box::new(field("status")),
            right: Box::new(lit_arr(vec!["active", "pending"])),
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(
            result,
            json!({ "status": { "$in": ["active", "pending"] } })
        );
    }

    #[test]
    fn test_mongo_gt_operator() {
        let cond = Expr::Binary {
            op: BinaryOp::Gt,
            left: Box::new(field("age")),
            right: Box::new(lit_int(18)),
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "age": { "$gt": 18 } }));
    }

    #[test]
    fn test_mongo_contains() {
        let cond = Expr::Binary {
            op: BinaryOp::Contains,
            left: Box::new(field("title")),
            right: Box::new(lit_str("rust")),
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "title": { "$regex": "rust" } }));
    }

    #[test]
    fn test_mongo_contains_escapes_regex() {
        let cond = Expr::Binary {
            op: BinaryOp::Contains,
            left: Box::new(field("name")),
            right: Box::new(lit_str("100%")),
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        // % is not a regex metachar but . and others should be escaped
        assert_eq!(result, json!({ "name": { "$regex": "100%" } }));
    }

    #[test]
    fn test_mongo_is_null() {
        let cond = Expr::Call {
            name: "is_null".to_string(),
            args: vec![field("deleted_at")],
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "deleted_at": null }));
    }

    #[test]
    fn test_mongo_not_null() {
        let cond = Expr::Unary {
            op: crate::expr::UnaryOp::Not,
            operand: Box::new(Expr::Call {
                name: "is_null".to_string(),
                args: vec![field("email")],
            }),
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "email": { "$ne": null, "$exists": true } }));
    }

    #[test]
    fn test_mongo_starts_with() {
        let cond = Expr::Call {
            name: "starts_with".to_string(),
            args: vec![field("code"), lit_str("PRJ")],
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "code": { "$regex": "^PRJ" } }));
    }

    #[test]
    fn test_mongo_ends_with() {
        let cond = Expr::Call {
            name: "ends_with".to_string(),
            args: vec![field("filename"), lit_str(".rs")],
        };
        let paths = vec![path_with(vec![cond])];
        let result = to_mongo(&paths, &HashMap::new());
        assert_eq!(result, json!({ "filename": { "$regex": "\\.rs$" } }));
    }
}
