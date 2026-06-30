//! Studio condition types (mirrors the TypeScript Condition model)

use serde::{Deserialize, Serialize};

use super::expr::{expr_to_string, StudioExpr};

/// Studio condition — mirrors the frontend `Condition` union type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StudioCondition {
    Simple {
        left: StudioExpr,
        operator: String,
        right: StudioExpr,
    },
    Logical {
        operator: String,
        conditions: Vec<StudioCondition>,
    },
    Not {
        condition: Box<StudioCondition>,
    },
    Expression {
        expression: String,
    },
    Constant {
        value: bool,
    },
}

/// Convert a `StudioCondition` to an expression string ordo-core can parse.
pub fn condition_to_expr_string(cond: &StudioCondition) -> String {
    match cond {
        StudioCondition::Simple {
            left,
            operator,
            right,
        } => simple_condition_to_string(left, operator, right),
        StudioCondition::Logical {
            operator,
            conditions,
        } => {
            if conditions.is_empty() {
                return "true".to_string();
            }
            let op = match operator.as_str() {
                "and" | "&&" => "&&",
                "or" | "||" => "||",
                other => other,
            };
            let parts: Vec<String> = conditions.iter().map(condition_to_expr_string).collect();
            format!("({})", parts.join(&format!(" {} ", op)))
        }
        StudioCondition::Not { condition } => {
            format!("!({})", condition_to_expr_string(condition))
        }
        StudioCondition::Expression { expression } => expression.clone(),
        StudioCondition::Constant { value } => value.to_string(),
    }
}

fn simple_condition_to_string(left: &StudioExpr, operator: &str, right: &StudioExpr) -> String {
    let l = expr_to_string(left);
    let r = expr_to_string(right);

    match operator {
        "eq" => format!("{} == {}", l, r),
        "neq" | "ne" => format!("{} != {}", l, r),
        "gt" => format!("{} > {}", l, r),
        "gte" => format!("{} >= {}", l, r),
        "lt" => format!("{} < {}", l, r),
        "lte" => format!("{} <= {}", l, r),
        "in" => format!("{} in {}", l, r),
        "not_in" => format!("{} not in {}", l, r),
        "contains" => format!("{} contains {}", l, r),
        "not_contains" => format!("!({} contains {})", l, r),
        "is_null" => format!("{} == null", l),
        "is_not_null" => format!("{} != null", l),
        "is_empty" => format!("{} == \"\"", l),
        "is_not_empty" => format!("{} != \"\"", l),
        "starts_with" => format!("starts_with({}, {})", l, r),
        "ends_with" => format!("ends_with({}, {})", l, r),
        "regex" => format!("regex_match({}, {})", l, r),
        other => format!("{} {} {}", l, other, r),
    }
}
