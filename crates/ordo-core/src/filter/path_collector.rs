//! Rule graph traversal for filter compilation
//!
//! Collects all paths through the rule graph that lead to target result codes.
//! Each path is a sequence of conditions that must all hold (AND).
//! Multiple paths are combined with OR in the final filter.

use super::partial_eval::{ExprClass, PartialEvaluator};
use crate::error::Result;
use crate::expr::{Expr, ExprParser, UnaryOp};
use crate::rule::{Condition, RuleSet, StepKind};

/// Hard limit on traversal depth to prevent infinite loops in cyclic graphs
const MAX_DEPTH: usize = 50;

/// A single execution path that leads to a target result
#[derive(Debug)]
pub struct FilterPath {
    /// Conditions accumulated along this path (ANDed together)
    pub conditions: Vec<Expr>,
    /// Result code at the terminal step
    pub result_code: String,
}

/// Collect all paths that reach any of `target_results`, up to `max_paths`.
///
/// Returns `(paths, truncated)`. `truncated` is true when the path limit was
/// reached before the full graph was explored. In that case the caller should
/// treat the result as `always_matches` to avoid false negatives (hiding rows
/// that the rule engine would have accepted).
///
/// A `max_paths` of 0 means no limit.
pub fn collect_paths(
    ruleset: &RuleSet,
    evaluator: &mut PartialEvaluator,
    target_results: &[String],
    max_paths: usize,
) -> Result<(Vec<FilterPath>, bool)> {
    let entry = ruleset.config.entry_step.clone();
    let mut paths = Vec::new();
    let mut conditions: Vec<Expr> = Vec::new();
    let mut truncated = false;

    collect_recursive(
        ruleset,
        evaluator,
        &entry,
        &mut conditions,
        target_results,
        max_paths,
        0,
        &mut paths,
        &mut truncated,
    )?;

    Ok((paths, truncated))
}

#[allow(clippy::too_many_arguments)]
fn collect_recursive(
    ruleset: &RuleSet,
    evaluator: &mut PartialEvaluator,
    step_id: &str,
    conditions: &mut Vec<Expr>,
    target_results: &[String],
    max_paths: usize,
    depth: usize,
    paths: &mut Vec<FilterPath>,
    truncated: &mut bool,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    if max_paths > 0 && paths.len() >= max_paths {
        *truncated = true;
        return Ok(());
    }

    let step = match ruleset.get_step(step_id) {
        Some(s) => s,
        None => return Ok(()), // dangling reference — skip silently
    };

    match &step.kind {
        StepKind::Terminal { result } => {
            if target_results.contains(&result.code) {
                paths.push(FilterPath {
                    conditions: conditions.clone(),
                    result_code: result.code.clone(),
                });
            }
        }

        StepKind::Action { next_step, .. } => {
            // V1: transparent — variable mutations not tracked
            collect_recursive(
                ruleset,
                evaluator,
                next_step,
                conditions,
                target_results,
                max_paths,
                depth + 1,
                paths,
                truncated,
            )?;
        }

        StepKind::Decision {
            branches,
            default_next,
        } => {
            // Track negations of taken branches for the default path
            let mut negations: Vec<Expr> = Vec::new();

            for branch in branches {
                if max_paths > 0 && paths.len() >= max_paths {
                    *truncated = true;
                    break;
                }

                let (expr_opt, always_true, always_false) =
                    evaluate_condition(&branch.condition, evaluator);

                if always_false {
                    // Branch never taken — accumulate its negation if we have an expr
                    if let Some(e) = expr_opt {
                        negations.push(negate(e));
                    }
                    continue;
                }

                if always_true {
                    // Branch always taken — recurse with no extra condition, done
                    collect_recursive(
                        ruleset,
                        evaluator,
                        &branch.next_step,
                        conditions,
                        target_results,
                        max_paths,
                        depth + 1,
                        paths,
                        truncated,
                    )?;
                    return Ok(()); // subsequent branches are dead code
                }

                // Unknown condition — recurse with this condition added
                let cond_expr = expr_opt.unwrap();
                conditions.push(cond_expr.clone());
                collect_recursive(
                    ruleset,
                    evaluator,
                    &branch.next_step,
                    conditions,
                    target_results,
                    max_paths,
                    depth + 1,
                    paths,
                    truncated,
                )?;
                conditions.pop();

                // For subsequent branches / default: this condition must be false
                negations.push(negate(cond_expr));
            }

            // Follow the default path with all branch-condition negations
            if let Some(default) = default_next.as_deref() {
                if max_paths == 0 || paths.len() < max_paths {
                    let neg_count = negations.len();
                    conditions.extend(negations);
                    collect_recursive(
                        ruleset,
                        evaluator,
                        default,
                        conditions,
                        target_results,
                        max_paths,
                        depth + 1,
                        paths,
                        truncated,
                    )?;
                    conditions.truncate(conditions.len() - neg_count);
                } else {
                    *truncated = true;
                }
            }
        }
    }

    Ok(())
}

/// Evaluate a branch condition against known inputs.
/// Returns `(expr, always_true, always_false)`.
fn evaluate_condition(
    condition: &Condition,
    evaluator: &mut PartialEvaluator,
) -> (Option<Expr>, bool, bool) {
    match condition {
        Condition::Always => (None, true, false),
        Condition::Expression(expr) => classify(evaluator.eval(expr)),
        Condition::ExpressionString(s) => match ExprParser::parse(s) {
            Ok(expr) => classify(evaluator.eval(&expr)),
            Err(_) => (None, false, true), // parse error → treat as never
        },
    }
}

fn classify(class: ExprClass) -> (Option<Expr>, bool, bool) {
    match class {
        ExprClass::AlwaysTrue => (None, true, false),
        ExprClass::AlwaysFalse => (None, false, true),
        ExprClass::Unknown(e) => (Some(e), false, false),
    }
}

fn negate(expr: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Not,
        operand: Box::new(expr),
    }
}
