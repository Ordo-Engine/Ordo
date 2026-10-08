//! Decision table step
//!
//! A decision table maps input columns to output columns, one rule per row:
//!
//! ```json
//! {
//!   "id": "discount", "name": "Discount", "type": "decision_table",
//!   "hit_policy": "first",
//!   "inputs": ["customer.tier", "order.amount"],
//!   "outputs": ["discount", "reason"],
//!   "rules": [
//!     { "when": ["gold", ">= 1000"], "then": [0.15, "Gold, large order"] },
//!     { "when": [["silver", "bronze"], "100..999"], "then": [0.05, "= concat(\"Tier \", customer.tier)"] }
//!   ],
//!   "default": [0, "No discount"],
//!   "next_step": "price"
//! }
//! ```
//!
//! Outputs are stored as variables (`$discount`, `$reason`).
//!
//! Input cells:
//! - `"*"`, `"-"`, `""` or `null`: matches anything
//! - a number, boolean or plain string: equals that value
//! - an array: the input is one of the values
//! - `">= 1000"`, `"< 18"`, `"!= \"x\""`, `"== $limit"`: comparison with an expression
//! - `"in [...]"`, `"not in [...]"`: membership
//! - `"18..65"`: inclusive numeric range
//! - `"= <expression>"`: any boolean expression over the whole input
//!
//! Output cells are literal values, or `"= <expression>"` to compute one.
//!
//! Hit policies:
//! - `first`: the first matching row sets the outputs. With no match the
//!   `default` row is used; without a `default` the step fails rather than
//!   silently continuing.
//! - `collect`: every matching row is collected, in order, and each output
//!   becomes an array. With `aggregate` (`sum`, `count`, `min`, `max`) each
//!   output is reduced to one value instead.

use crate::context::Value;
use crate::error::{OrdoError, Result};
use crate::expr::{BinaryOp, Expr, ExprParser};
use serde::{Deserialize, Serialize};

/// How matching rows are combined
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitPolicy {
    /// First matching row wins
    #[default]
    First,
    /// All matching rows, in row order
    Collect,
}

/// Reduction applied to collected outputs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregate {
    Sum,
    Count,
    Min,
    Max,
}

/// One row of a decision table, as written
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRule {
    /// One cell per input column
    pub when: Vec<Value>,
    /// One cell per output column
    pub then: Vec<Value>,
}

/// Source form of a decision table step (what is read and written)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DecisionTableSource {
    #[serde(default)]
    hit_policy: HitPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregate: Option<Aggregate>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    rules: Vec<TableRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default: Option<Vec<Value>>,
    next_step: String,
}

/// A row with its cells compiled to expressions
#[derive(Debug, Clone)]
pub struct CompiledTableRow {
    /// `None` when every input cell is a wildcard
    pub condition: Option<Expr>,
    /// One expression per output column
    pub outputs: Vec<Expr>,
}

/// Decision table step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "DecisionTableSource", into = "DecisionTableSource")]
pub struct DecisionTable {
    /// How matching rows are combined
    pub hit_policy: HitPolicy,
    /// Reduction for `collect`
    pub aggregate: Option<Aggregate>,
    /// Input column expressions, as written
    pub inputs: Vec<String>,
    /// Output variable names
    pub outputs: Vec<String>,
    /// Rows, as written
    pub rules: Vec<TableRule>,
    /// Outputs used by `first` when no row matches
    pub default: Option<Vec<Value>>,
    /// Step to continue with
    pub next_step: String,
    /// Rows compiled to expressions, parallel to `rules`
    pub compiled_rules: Vec<CompiledTableRow>,
    /// Compiled `default` outputs
    pub compiled_default: Option<Vec<Expr>>,
}

impl TryFrom<DecisionTableSource> for DecisionTable {
    type Error = OrdoError;

    fn try_from(src: DecisionTableSource) -> Result<Self> {
        if src.aggregate.is_some() && src.hit_policy != HitPolicy::Collect {
            return Err(table_error("`aggregate` requires hit_policy \"collect\""));
        }
        if src.default.is_some() && src.hit_policy != HitPolicy::First {
            return Err(table_error(
                "`default` is only used with hit_policy \"first\"",
            ));
        }
        if src.outputs.is_empty() {
            return Err(table_error("at least one output column is required"));
        }

        let columns = src
            .inputs
            .iter()
            .map(|input| {
                ExprParser::parse(input)
                    .map_err(|e| table_error(format!("input column `{input}`: {e}")))
            })
            .collect::<Result<Vec<_>>>()?;

        let compiled_rules = src
            .rules
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                let row = i + 1;
                if rule.when.len() != columns.len() {
                    return Err(table_error(format!(
                        "row {row}: `when` has {} cells, expected {} (one per input)",
                        rule.when.len(),
                        columns.len()
                    )));
                }
                let mut condition: Option<Expr> = None;
                for ((column, cell), input) in columns.iter().zip(&rule.when).zip(&src.inputs) {
                    let test = input_cell(column, cell)
                        .map_err(|e| table_error(format!("row {row}, input `{input}`: {e}")))?;
                    if let Some(test) = test {
                        condition = Some(match condition {
                            Some(prev) => Expr::binary(BinaryOp::And, prev, test),
                            None => test,
                        });
                    }
                }
                let outputs = output_cells(&rule.then, &src.outputs)
                    .map_err(|e| table_error(format!("row {row}: {e}")))?;
                Ok(CompiledTableRow { condition, outputs })
            })
            .collect::<Result<Vec<_>>>()?;

        let compiled_default = src
            .default
            .as_ref()
            .map(|cells| {
                output_cells(cells, &src.outputs).map_err(|e| table_error(format!("default: {e}")))
            })
            .transpose()?;

        Ok(Self {
            hit_policy: src.hit_policy,
            aggregate: src.aggregate,
            inputs: src.inputs,
            outputs: src.outputs,
            rules: src.rules,
            default: src.default,
            next_step: src.next_step,
            compiled_rules,
            compiled_default,
        })
    }
}

impl From<DecisionTable> for DecisionTableSource {
    fn from(table: DecisionTable) -> Self {
        Self {
            hit_policy: table.hit_policy,
            aggregate: table.aggregate,
            inputs: table.inputs,
            outputs: table.outputs,
            rules: table.rules,
            default: table.default,
            next_step: table.next_step,
        }
    }
}

impl DecisionTable {
    /// Every compiled expression in the table (conditions, outputs, default)
    pub fn expressions(&self) -> impl Iterator<Item = &Expr> {
        self.compiled_rules
            .iter()
            .flat_map(|row| row.condition.iter().chain(row.outputs.iter()))
            .chain(self.compiled_default.iter().flatten())
    }

    /// Rewrite the source text of every expression in the table (input
    /// columns and expression cells; literal cells are left alone) and
    /// recompile. Used to rewrite references, e.g. concept names to `$name`.
    pub fn map_expression_sources(&self, mut f: impl FnMut(&str) -> String) -> Result<Self> {
        let mut src = DecisionTableSource::from(self.clone());
        for input in &mut src.inputs {
            *input = f(input);
        }
        let rewrite = |cell: &mut Value,
                       is_expression: fn(&str) -> bool,
                       f: &mut dyn FnMut(&str) -> String| {
            if let Value::String(s) = cell {
                if is_expression(s) {
                    *cell = Value::string(f(s));
                }
            }
        };
        for rule in &mut src.rules {
            for cell in &mut rule.when {
                rewrite(cell, input_cell_is_expression, &mut f);
            }
            for cell in &mut rule.then {
                rewrite(cell, output_cell_is_expression, &mut f);
            }
        }
        for cell in src.default.iter_mut().flatten() {
            rewrite(cell, output_cell_is_expression, &mut f);
        }
        Self::try_from(src)
    }
}

/// Comparison prefixes of an input cell, longest first so ">=" is not read as ">".
const CELL_OPERATORS: [(&str, BinaryOp); 6] = [
    (">=", BinaryOp::Ge),
    ("<=", BinaryOp::Le),
    ("!=", BinaryOp::Ne),
    ("==", BinaryOp::Eq),
    (">", BinaryOp::Gt),
    ("<", BinaryOp::Lt),
];

/// Whether an input cell string contains an expression (rather than being
/// a wildcard, a range or a literal to compare with).
fn input_cell_is_expression(s: &str) -> bool {
    let s = s.trim();
    CELL_OPERATORS
        .iter()
        .any(|(prefix, _)| s.starts_with(prefix))
        || s.starts_with('=')
        || s.starts_with("in ")
        || s.starts_with("not in ")
}

fn output_cell_is_expression(s: &str) -> bool {
    s.trim_start().starts_with('=')
}

fn table_error(message: impl std::fmt::Display) -> OrdoError {
    OrdoError::parse_error(format!("decision table: {message}"))
}

/// Compile one input cell into a test against `column`.
fn input_cell(column: &Expr, cell: &Value) -> Result<Option<Expr>> {
    let compare = |op, rhs| Ok(Some(Expr::binary(op, column.clone(), rhs)));
    match cell {
        Value::Null => Ok(None),
        Value::Array(_) => compare(BinaryOp::In, Expr::Literal(cell.clone())),
        Value::String(s) => {
            let s = s.trim();
            if matches!(s, "" | "*" | "-") {
                return Ok(None);
            }
            for (prefix, op) in CELL_OPERATORS {
                if let Some(rest) = s.strip_prefix(prefix) {
                    return compare(op, ExprParser::parse(rest)?);
                }
            }
            if let Some(rest) = s.strip_prefix('=') {
                return Ok(Some(ExprParser::parse(rest)?));
            }
            if let Some(rest) = s.strip_prefix("not in ") {
                return compare(BinaryOp::NotIn, ExprParser::parse(rest)?);
            }
            if let Some(rest) = s.strip_prefix("in ") {
                return compare(BinaryOp::In, ExprParser::parse(rest)?);
            }
            if let Some((lo, hi)) = numeric_range(s) {
                let lower = Expr::binary(BinaryOp::Ge, column.clone(), Expr::literal(lo));
                let upper = Expr::binary(BinaryOp::Le, column.clone(), Expr::literal(hi));
                return Ok(Some(Expr::binary(BinaryOp::And, lower, upper)));
            }
            compare(BinaryOp::Eq, Expr::literal(Value::string(s)))
        }
        _ => compare(BinaryOp::Eq, Expr::Literal(cell.clone())),
    }
}

/// Parse `"lo..hi"` where both bounds are numbers.
fn numeric_range(s: &str) -> Option<(Value, Value)> {
    let (lo, hi) = s.split_once("..")?;
    let number = |part: &str| -> Option<Value> {
        let part = part.trim();
        part.parse::<i64>()
            .map(Value::int)
            .ok()
            .or_else(|| part.parse::<f64>().ok().map(Value::float))
    };
    Some((number(lo)?, number(hi)?))
}

fn output_cells(cells: &[Value], outputs: &[String]) -> Result<Vec<Expr>> {
    if cells.len() != outputs.len() {
        return Err(OrdoError::parse_error(format!(
            "has {} output cells, expected {} (one per output)",
            cells.len(),
            outputs.len()
        )));
    }
    cells
        .iter()
        .zip(outputs)
        .map(|(cell, name)| match cell {
            Value::String(s) if output_cell_is_expression(s) => {
                let source = s.trim_start()[1..].trim();
                ExprParser::parse(source).map_err(|e| {
                    OrdoError::parse_error(format!("output `{name}` expression `{source}`: {e}"))
                })
            }
            other => Ok(Expr::Literal(other.clone())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(json: &str) -> Result<DecisionTable> {
        serde_json::from_str::<DecisionTable>(json)
            .map_err(|e| OrdoError::parse_error(e.to_string()))
    }

    #[test]
    fn cells_compile_to_expressions() {
        let t = table(
            r#"{"inputs": ["tier", "amount"], "outputs": ["d"],
                "rules": [
                  {"when": ["gold", ">= 1000"], "then": [0.15]},
                  {"when": [["a", "b"], "10..20"], "then": ["= amount * 0.1"]},
                  {"when": ["*", null], "then": ["plain text"]}
                ],
                "next_step": "next"}"#,
        )
        .unwrap();
        assert_eq!(t.compiled_rules.len(), 3);
        assert!(t.compiled_rules[2].condition.is_none());
        assert_eq!(
            t.compiled_rules[2].outputs[0],
            Expr::literal(Value::string("plain text"))
        );
        assert!(matches!(
            t.compiled_rules[1].outputs[0],
            Expr::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn serializes_as_written() {
        let src = r#"{"hit_policy":"collect","aggregate":"sum","inputs":["x"],"outputs":["y"],"rules":[{"when":[">= 1"],"then":[1]}],"next_step":"n"}"#;
        let t = table(src).unwrap();
        assert_eq!(serde_json::to_string(&t).unwrap(), src);
    }

    #[test]
    fn map_expression_sources_skips_literal_cells() {
        let t = table(
            r#"{"inputs": ["score"], "outputs": ["d"],
                "rules": [{"when": ["> limit"], "then": ["= limit * 2"]},
                          {"when": ["limit"], "then": ["limit"]}],
                "next_step": "n"}"#,
        )
        .unwrap();
        let rewritten = t
            .map_expression_sources(|s| s.replace("limit", "$limit"))
            .unwrap();
        let json = serde_json::to_string(&rewritten.rules).unwrap();
        assert_eq!(
            json,
            r#"[{"when":["> $limit"],"then":["= $limit * 2"]},{"when":["limit"],"then":["limit"]}]"#
        );
        assert_eq!(
            rewritten.compiled_rules[0].condition,
            Some(Expr::binary(
                BinaryOp::Gt,
                Expr::field("score"),
                Expr::field("$limit")
            ))
        );
    }

    #[test]
    fn shape_errors_name_the_row() {
        let err = table(
            r#"{"inputs": ["a", "b"], "outputs": ["d"],
                "rules": [{"when": ["x"], "then": [1]}], "next_step": "n"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("row 1: `when` has 1 cells, expected 2"),
            "{err}"
        );

        let err = table(
            r#"{"inputs": ["a"], "outputs": ["d"], "aggregate": "sum",
                "rules": [], "next_step": "n"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("requires hit_policy \"collect\""), "{err}");

        let err = table(
            r#"{"inputs": ["a"], "outputs": ["d"],
                "rules": [{"when": [">= )"], "then": [1]}], "next_step": "n"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("row 1, input `a`"), "{err}");
    }
}
