//! SQL WHERE clause generator for filter compilation

use std::collections::HashMap;

use crate::context::Value;
use crate::error::{OrdoError, Result};
use crate::expr::{BinaryOp, Expr, UnaryOp};

use super::path_collector::FilterPath;

/// Convert collected paths to a SQL WHERE clause.
///
/// Multiple paths are combined with OR; conditions within a single path with AND.
pub fn to_sql(paths: &[FilterPath], mapping: &HashMap<String, String>) -> Result<String> {
    if paths.is_empty() {
        return Ok("FALSE".to_string());
    }

    // Any path with no conditions means "always matches"
    if paths.iter().any(|p| p.conditions.is_empty()) {
        return Ok("TRUE".to_string());
    }

    let clauses: Result<Vec<String>> = paths.iter().map(|p| path_to_sql(p, mapping)).collect();
    let clauses = clauses?;

    if clauses.len() == 1 {
        Ok(clauses.into_iter().next().unwrap())
    } else {
        let parts: Vec<String> = clauses.into_iter().map(|c| format!("({})", c)).collect();
        Ok(parts.join(" OR "))
    }
}

fn path_to_sql(path: &FilterPath, mapping: &HashMap<String, String>) -> Result<String> {
    match path.conditions.len() {
        0 => Ok("TRUE".to_string()),
        1 => expr_to_sql(&path.conditions[0], mapping),
        _ => {
            let parts: Result<Vec<String>> = path
                .conditions
                .iter()
                .map(|c| expr_to_sql(c, mapping))
                .collect();
            Ok(parts?.join(" AND "))
        }
    }
}

fn expr_to_sql(expr: &Expr, mapping: &HashMap<String, String>) -> Result<String> {
    match expr {
        Expr::Field(path) => {
            let col = mapping
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.replace('.', "_"));
            Ok(col)
        }
        Expr::Literal(val) => Ok(value_to_sql(val)),
        Expr::Binary { op, left, right } => binary_to_sql(*op, left, right, mapping),
        Expr::Unary { op, operand } => unary_to_sql(*op, operand, mapping),
        Expr::Call { name, args } => call_to_sql(name, args, mapping),
        Expr::Array(elems) => {
            let parts: Result<Vec<String>> =
                elems.iter().map(|e| expr_to_sql(e, mapping)).collect();
            Ok(format!("({})", parts?.join(", ")))
        }
        other => Err(OrdoError::parse_error(format!(
            "Cannot convert expression to SQL: {:?}",
            other
        ))),
    }
}

fn binary_to_sql(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    mapping: &HashMap<String, String>,
) -> Result<String> {
    let l = || expr_to_sql(left, mapping);
    let r = || expr_to_sql(right, mapping);

    match op {
        BinaryOp::Eq => {
            if matches!(right, Expr::Literal(Value::Null)) {
                Ok(format!("{} IS NULL", l()?))
            } else if matches!(left, Expr::Literal(Value::Null)) {
                Ok(format!("{} IS NULL", r()?))
            } else {
                Ok(format!("{} = {}", l()?, r()?))
            }
        }
        BinaryOp::Ne => {
            if matches!(right, Expr::Literal(Value::Null)) {
                Ok(format!("{} IS NOT NULL", l()?))
            } else if matches!(left, Expr::Literal(Value::Null)) {
                Ok(format!("{} IS NOT NULL", r()?))
            } else {
                Ok(format!("{} != {}", l()?, r()?))
            }
        }
        BinaryOp::Lt => Ok(format!("{} < {}", l()?, r()?)),
        BinaryOp::Le => Ok(format!("{} <= {}", l()?, r()?)),
        BinaryOp::Gt => Ok(format!("{} > {}", l()?, r()?)),
        BinaryOp::Ge => Ok(format!("{} >= {}", l()?, r()?)),
        BinaryOp::And => Ok(format!("({} AND {})", l()?, r()?)),
        BinaryOp::Or => Ok(format!("({} OR {})", l()?, r()?)),
        BinaryOp::In => Ok(format!("{} IN {}", l()?, r()?)),
        BinaryOp::NotIn => Ok(format!("{} NOT IN {}", l()?, r()?)),
        BinaryOp::Contains => {
            if let Expr::Literal(Value::String(s)) = right {
                Ok(format!(
                    "{} LIKE '%{}%' ESCAPE '!'",
                    l()?,
                    escape_like_pattern(s)
                ))
            } else {
                Err(OrdoError::parse_error(
                    "SQL LIKE requires a string literal for 'contains'",
                ))
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            Err(OrdoError::parse_error(format!(
                "Arithmetic operator {:?} is not supported in SQL filter generation",
                op
            )))
        }
    }
}

fn unary_to_sql(op: UnaryOp, operand: &Expr, mapping: &HashMap<String, String>) -> Result<String> {
    match op {
        UnaryOp::Not => {
            // Special case: NOT(is_null(field)) → field IS NOT NULL
            if let Expr::Call { name, args } = operand {
                if name == "is_null" && args.len() == 1 {
                    return Ok(format!("{} IS NOT NULL", expr_to_sql(&args[0], mapping)?));
                }
            }
            Ok(format!("NOT ({})", expr_to_sql(operand, mapping)?))
        }
        UnaryOp::Neg => Err(OrdoError::parse_error(
            "Unary negation is not supported in SQL filter generation",
        )),
    }
}

fn call_to_sql(name: &str, args: &[Expr], mapping: &HashMap<String, String>) -> Result<String> {
    match (name, args) {
        ("is_null", [field]) => Ok(format!("{} IS NULL", expr_to_sql(field, mapping)?)),
        ("starts_with", [field, Expr::Literal(Value::String(s))]) => Ok(format!(
            "{} LIKE '{}%' ESCAPE '!'",
            expr_to_sql(field, mapping)?,
            escape_like_pattern(s)
        )),
        ("ends_with", [field, Expr::Literal(Value::String(s))]) => Ok(format!(
            "{} LIKE '%{}' ESCAPE '!'",
            expr_to_sql(field, mapping)?,
            escape_like_pattern(s)
        )),
        _ => Err(OrdoError::parse_error(format!(
            "Function '{}' is not supported in SQL filter generation",
            name
        ))),
    }
}

fn value_to_sql(val: &Value) -> String {
    match val {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => format!("'{}'", escape_sql(s)),
        Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(value_to_sql).collect();
            format!("({})", parts.join(", "))
        }
        _ => "NULL".to_string(),
    }
}

fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Escape a string for use inside a SQL LIKE pattern with `ESCAPE '!'`.
///
/// Escapes:
/// - `!` → `!!`  (the escape character itself)
/// - `%` → `!%`  (SQL wildcard: any sequence of characters)
/// - `_` → `!_`  (SQL wildcard: any single character)
/// - `'` → `''`  (SQL string delimiter)
fn escape_like_pattern(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '!' => out.push_str("!!"),
            '%' => out.push_str("!%"),
            '_' => out.push_str("!_"),
            '\'' => out.push_str("''"),
            other => out.push(other),
        }
    }
    out
}
