//! Partial evaluator for filter compilation
//!
//! Substitutes known field values into expressions and simplifies them.
//! Unknown fields remain as symbolic variables for SQL/JSON generation.

use crate::context::Value;
use crate::expr::{Expr, ExprOptimizer};

/// Result of partially evaluating an expression
pub enum ExprClass {
    /// Expression evaluated to a constant true
    AlwaysTrue,
    /// Expression evaluated to a constant false
    AlwaysFalse,
    /// Expression still contains unknown fields
    Unknown(Expr),
}

/// Partial evaluator: substitutes known fields and simplifies via ExprOptimizer
pub struct PartialEvaluator {
    known: Value,
    optimizer: ExprOptimizer,
}

impl PartialEvaluator {
    pub fn new(known: Value) -> Self {
        Self {
            known,
            optimizer: ExprOptimizer::new(),
        }
    }

    /// Evaluate an expression with known fields substituted.
    /// Returns AlwaysTrue/AlwaysFalse if expression resolves to a constant,
    /// or Unknown(expr) with unresolved field references remaining.
    pub fn eval(&mut self, expr: &Expr) -> ExprClass {
        let substituted = self.substitute(expr);
        let optimized = self.optimizer.optimize(substituted);

        match &optimized {
            Expr::Literal(Value::Bool(true)) => ExprClass::AlwaysTrue,
            Expr::Literal(Value::Bool(false)) => ExprClass::AlwaysFalse,
            Expr::Literal(v) => {
                if v.is_truthy() {
                    ExprClass::AlwaysTrue
                } else {
                    ExprClass::AlwaysFalse
                }
            }
            _ => ExprClass::Unknown(optimized),
        }
    }

    /// Recursively substitute known fields into the expression tree
    fn substitute(&self, expr: &Expr) -> Expr {
        match expr {
            Expr::Field(path) => {
                if let Some(val) = self.lookup(path) {
                    Expr::Literal(val.clone())
                } else {
                    expr.clone()
                }
            }
            Expr::Exists(path) => {
                // If we can confirm existence from known_input, fold to true.
                // If absent from known_input, leave as Exists (it may be in DB).
                if self.lookup(path).is_some() {
                    Expr::Literal(Value::bool(true))
                } else {
                    expr.clone()
                }
            }
            Expr::Binary { op, left, right } => Expr::Binary {
                op: *op,
                left: Box::new(self.substitute(left)),
                right: Box::new(self.substitute(right)),
            },
            Expr::Unary { op, operand } => Expr::Unary {
                op: *op,
                operand: Box::new(self.substitute(operand)),
            },
            Expr::Call { name, args } => Expr::Call {
                name: name.clone(),
                args: args.iter().map(|a| self.substitute(a)).collect(),
            },
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => Expr::Conditional {
                condition: Box::new(self.substitute(condition)),
                then_branch: Box::new(self.substitute(then_branch)),
                else_branch: Box::new(self.substitute(else_branch)),
            },
            Expr::Array(elems) => Expr::Array(elems.iter().map(|e| self.substitute(e)).collect()),
            Expr::Coalesce(exprs) => {
                Expr::Coalesce(exprs.iter().map(|e| self.substitute(e)).collect())
            }
            Expr::Object(pairs) => Expr::Object(
                pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), self.substitute(v)))
                    .collect(),
            ),
            Expr::Literal(_) => expr.clone(),
        }
    }

    /// Look up a dot-separated field path in known_input.
    /// Returns None if the path does not exist or resolves to Null.
    fn lookup<'a>(&'a self, path: &str) -> Option<&'a Value> {
        let mut current = &self.known;
        for part in path.split('.') {
            match current {
                Value::Object(map) => {
                    current = map.get(part)?;
                }
                _ => return None,
            }
        }
        match current {
            Value::Null => None,
            other => Some(other),
        }
    }
}
