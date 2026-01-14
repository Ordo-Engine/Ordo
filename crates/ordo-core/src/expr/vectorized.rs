//! Vectorized batch expression execution
//!
//! This module provides optimized batch execution of expressions over multiple
//! inputs. Instead of evaluating each input independently, it uses columnar
//! processing to improve cache utilization and enable SIMD-like optimizations.
//!
//! Key optimizations:
//! - Column-oriented data processing
//! - Batch memory allocation
//! - Reduced interpreter overhead per input
//! - Pre-compiled bytecode reuse

use super::ast::{BinaryOp, Expr};
use super::compiler::ExprCompiler;
use super::functions::FunctionRegistry;
use super::vm::BytecodeVM;
use super::vm::CompiledExpr;
use crate::context::{Context, Value};
use crate::error::Result;

/// Vectorized expression evaluator for batch processing
pub struct VectorizedEvaluator {
    /// Function registry (reserved for future custom function support)
    #[allow(dead_code)]
    functions: FunctionRegistry,
    /// Pre-compiled bytecode (optional)
    compiled: Option<CompiledExpr>,
    /// Bytecode VM for execution
    vm: BytecodeVM,
}

impl Default for VectorizedEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorizedEvaluator {
    /// Create a new vectorized evaluator
    pub fn new() -> Self {
        Self {
            functions: FunctionRegistry::new(),
            compiled: None,
            vm: BytecodeVM::new(),
        }
    }

    /// Pre-compile an expression for repeated batch evaluation
    pub fn compile(&mut self, expr: &Expr) {
        self.compiled = Some(ExprCompiler::new().compile(expr));
    }

    /// Clear the pre-compiled expression
    pub fn clear_compiled(&mut self) {
        self.compiled = None;
    }

    /// Evaluate an expression over a batch of contexts
    /// Returns results in the same order as inputs
    pub fn eval_batch(&mut self, expr: &Expr, contexts: &[Context]) -> Vec<Result<Value>> {
        if contexts.is_empty() {
            return Vec::new();
        }

        // Use pre-compiled bytecode if available, otherwise compile once
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        // Execute for each context
        contexts
            .iter()
            .map(|ctx| self.vm.execute(&compiled, ctx))
            .collect()
    }

    /// Evaluate an expression over a batch of JSON values
    /// Each value becomes the input context
    pub fn eval_batch_json(&mut self, expr: &Expr, inputs: &[Value]) -> Vec<Result<Value>> {
        // Convert values to contexts
        let contexts: Vec<Context> = inputs.iter().map(|v| Context::new(v.clone())).collect();

        self.eval_batch(expr, &contexts)
    }

    /// Optimized batch evaluation for simple comparison expressions
    /// Uses columnar processing for better performance
    pub fn eval_batch_compare(
        &self,
        field: &str,
        op: BinaryOp,
        threshold: &Value,
        contexts: &[Context],
    ) -> Vec<bool> {
        // Extract field values as a column
        let column: Vec<Option<&Value>> = contexts.iter().map(|ctx| ctx.get(field)).collect();

        // Vectorized comparison
        column
            .iter()
            .map(|value| match value {
                Some(v) => self.compare_values(v, op, threshold),
                None => false,
            })
            .collect()
    }

    /// Compare two values with the given operator
    fn compare_values(&self, left: &Value, op: BinaryOp, right: &Value) -> bool {
        match op {
            BinaryOp::Eq => left == right,
            BinaryOp::Ne => left != right,
            BinaryOp::Lt => left.compare(right) == Some(std::cmp::Ordering::Less),
            BinaryOp::Le => left
                .compare(right)
                .map(|o| o != std::cmp::Ordering::Greater)
                .unwrap_or(false),
            BinaryOp::Gt => left.compare(right) == Some(std::cmp::Ordering::Greater),
            BinaryOp::Ge => left
                .compare(right)
                .map(|o| o != std::cmp::Ordering::Less)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Optimized batch evaluation for logical AND of two comparisons
    /// Common pattern: field1 > threshold1 && field2 < threshold2
    #[allow(clippy::too_many_arguments)]
    pub fn eval_batch_and_compare(
        &self,
        field1: &str,
        op1: BinaryOp,
        threshold1: &Value,
        field2: &str,
        op2: BinaryOp,
        threshold2: &Value,
        contexts: &[Context],
    ) -> Vec<bool> {
        // Extract both columns
        let column1: Vec<Option<&Value>> = contexts.iter().map(|ctx| ctx.get(field1)).collect();
        let column2: Vec<Option<&Value>> = contexts.iter().map(|ctx| ctx.get(field2)).collect();

        // Vectorized AND comparison
        column1
            .iter()
            .zip(column2.iter())
            .map(|(v1, v2)| match (v1, v2) {
                (Some(val1), Some(val2)) => {
                    self.compare_values(val1, op1, threshold1)
                        && self.compare_values(val2, op2, threshold2)
                }
                _ => false,
            })
            .collect()
    }

    /// Batch evaluation with early termination on first success
    /// Useful for finding matching contexts
    pub fn find_first_match(&mut self, expr: &Expr, contexts: &[Context]) -> Option<usize> {
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        for (idx, ctx) in contexts.iter().enumerate() {
            if let Ok(result) = self.vm.execute(&compiled, ctx) {
                if result.is_truthy() {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Batch evaluation with early termination on first failure
    /// Useful for validation
    pub fn all_match(&mut self, expr: &Expr, contexts: &[Context]) -> bool {
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        for ctx in contexts {
            match self.vm.execute(&compiled, ctx) {
                Ok(result) if result.is_truthy() => continue,
                _ => return false,
            }
        }
        true
    }

    /// Count how many contexts match the expression
    pub fn count_matches(&mut self, expr: &Expr, contexts: &[Context]) -> usize {
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        contexts
            .iter()
            .filter(|ctx| {
                self.vm
                    .execute(&compiled, ctx)
                    .map(|v| v.is_truthy())
                    .unwrap_or(false)
            })
            .count()
    }

    /// Filter contexts that match the expression
    pub fn filter_matches<'a>(&mut self, expr: &Expr, contexts: &'a [Context]) -> Vec<&'a Context> {
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        contexts
            .iter()
            .filter(|ctx| {
                self.vm
                    .execute(&compiled, ctx)
                    .map(|v| v.is_truthy())
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Partition contexts into matching and non-matching groups
    pub fn partition<'a>(
        &mut self,
        expr: &Expr,
        contexts: &'a [Context],
    ) -> (Vec<&'a Context>, Vec<&'a Context>) {
        let compiled = match &self.compiled {
            Some(c) => c.clone(),
            None => ExprCompiler::new().compile(expr),
        };

        let mut matching = Vec::new();
        let mut non_matching = Vec::new();

        for ctx in contexts {
            let matches = self
                .vm
                .execute(&compiled, ctx)
                .map(|v| v.is_truthy())
                .unwrap_or(false);

            if matches {
                matching.push(ctx);
            } else {
                non_matching.push(ctx);
            }
        }

        (matching, non_matching)
    }
}

/// Statistics about vectorized batch execution
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// Total number of inputs processed
    pub total_inputs: usize,
    /// Number of successful evaluations
    pub successful: usize,
    /// Number of failed evaluations
    pub failed: usize,
    /// Number of truthy results
    pub truthy_count: usize,
}

impl BatchStats {
    /// Create stats from batch results
    pub fn from_results(results: &[Result<Value>]) -> Self {
        let mut stats = Self {
            total_inputs: results.len(),
            ..Default::default()
        };

        for result in results {
            match result {
                Ok(v) => {
                    stats.successful += 1;
                    if v.is_truthy() {
                        stats.truthy_count += 1;
                    }
                }
                Err(_) => stats.failed += 1,
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_contexts(jsons: &[&str]) -> Vec<Context> {
        jsons
            .iter()
            .map(|json| Context::from_json(json).unwrap())
            .collect()
    }

    #[test]
    fn test_eval_batch_simple() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18));

        let contexts = make_contexts(&[
            r#"{"age": 25}"#,
            r#"{"age": 15}"#,
            r#"{"age": 30}"#,
            r#"{"age": 18}"#,
        ]);

        let results = evaluator.eval_batch(&expr, &contexts);

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].as_ref().unwrap(), &Value::bool(true));
        assert_eq!(results[1].as_ref().unwrap(), &Value::bool(false));
        assert_eq!(results[2].as_ref().unwrap(), &Value::bool(true));
        assert_eq!(results[3].as_ref().unwrap(), &Value::bool(false));
    }

    #[test]
    fn test_eval_batch_with_precompile() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("value"), Expr::literal(50));

        // Pre-compile
        evaluator.compile(&expr);

        let contexts =
            make_contexts(&[r#"{"value": 100}"#, r#"{"value": 25}"#, r#"{"value": 75}"#]);

        let results = evaluator.eval_batch(&expr, &contexts);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), &Value::bool(true));
        assert_eq!(results[1].as_ref().unwrap(), &Value::bool(false));
        assert_eq!(results[2].as_ref().unwrap(), &Value::bool(true));
    }

    #[test]
    fn test_eval_batch_compare() {
        let evaluator = VectorizedEvaluator::new();
        let contexts = make_contexts(&[
            r#"{"score": 90}"#,
            r#"{"score": 60}"#,
            r#"{"score": 85}"#,
            r#"{"score": 45}"#,
        ]);

        let results =
            evaluator.eval_batch_compare("score", BinaryOp::Ge, &Value::int(70), &contexts);

        assert_eq!(results, vec![true, false, true, false]);
    }

    #[test]
    fn test_eval_batch_and_compare() {
        let evaluator = VectorizedEvaluator::new();
        let contexts = make_contexts(&[
            r#"{"age": 25, "score": 90}"#,
            r#"{"age": 15, "score": 90}"#,
            r#"{"age": 25, "score": 50}"#,
            r#"{"age": 15, "score": 50}"#,
        ]);

        let results = evaluator.eval_batch_and_compare(
            "age",
            BinaryOp::Ge,
            &Value::int(18),
            "score",
            BinaryOp::Ge,
            &Value::int(70),
            &contexts,
        );

        assert_eq!(results, vec![true, false, false, false]);
    }

    #[test]
    fn test_find_first_match() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("value"), Expr::literal(100));

        let contexts = make_contexts(&[
            r#"{"value": 50}"#,
            r#"{"value": 75}"#,
            r#"{"value": 150}"#,
            r#"{"value": 200}"#,
        ]);

        let result = evaluator.find_first_match(&expr, &contexts);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_all_match() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("value"), Expr::literal(0));

        let contexts_all_match =
            make_contexts(&[r#"{"value": 10}"#, r#"{"value": 20}"#, r#"{"value": 30}"#]);

        let contexts_some_fail =
            make_contexts(&[r#"{"value": 10}"#, r#"{"value": -5}"#, r#"{"value": 30}"#]);

        assert!(evaluator.all_match(&expr, &contexts_all_match));
        assert!(!evaluator.all_match(&expr, &contexts_some_fail));
    }

    #[test]
    fn test_count_matches() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Ge, Expr::field("score"), Expr::literal(60));

        let contexts = make_contexts(&[
            r#"{"score": 90}"#,
            r#"{"score": 55}"#,
            r#"{"score": 75}"#,
            r#"{"score": 40}"#,
            r#"{"score": 60}"#,
        ]);

        let count = evaluator.count_matches(&expr, &contexts);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_filter_matches() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Eq, Expr::field("status"), Expr::literal("active"));

        let contexts = make_contexts(&[
            r#"{"status": "active", "id": 1}"#,
            r#"{"status": "inactive", "id": 2}"#,
            r#"{"status": "active", "id": 3}"#,
        ]);

        let matches = evaluator.filter_matches(&expr, &contexts);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_partition() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Ge, Expr::field("age"), Expr::literal(18));

        let contexts = make_contexts(&[
            r#"{"age": 25}"#,
            r#"{"age": 15}"#,
            r#"{"age": 30}"#,
            r#"{"age": 10}"#,
        ]);

        let (adults, minors) = evaluator.partition(&expr, &contexts);
        assert_eq!(adults.len(), 2);
        assert_eq!(minors.len(), 2);
    }

    #[test]
    fn test_batch_stats() {
        let mut evaluator = VectorizedEvaluator::new();
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("value"), Expr::literal(50));

        let contexts =
            make_contexts(&[r#"{"value": 100}"#, r#"{"value": 25}"#, r#"{"value": 75}"#]);

        let results = evaluator.eval_batch(&expr, &contexts);
        let stats = BatchStats::from_results(&results);

        assert_eq!(stats.total_inputs, 3);
        assert_eq!(stats.successful, 3);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.truthy_count, 2);
    }

    #[test]
    fn test_complex_expression_batch() {
        let mut evaluator = VectorizedEvaluator::new();

        // (age >= 18 && status == "active") || vip == true
        let expr = Expr::binary(
            BinaryOp::Or,
            Expr::binary(
                BinaryOp::And,
                Expr::binary(BinaryOp::Ge, Expr::field("age"), Expr::literal(18)),
                Expr::binary(BinaryOp::Eq, Expr::field("status"), Expr::literal("active")),
            ),
            Expr::binary(BinaryOp::Eq, Expr::field("vip"), Expr::literal(true)),
        );

        let contexts = make_contexts(&[
            r#"{"age": 25, "status": "active", "vip": false}"#, // true (age && status)
            r#"{"age": 15, "status": "active", "vip": false}"#, // false
            r#"{"age": 15, "status": "inactive", "vip": true}"#, // true (vip)
            r#"{"age": 25, "status": "inactive", "vip": false}"#, // false
        ]);

        let results = evaluator.eval_batch(&expr, &contexts);

        assert_eq!(results[0].as_ref().unwrap(), &Value::bool(true));
        assert_eq!(results[1].as_ref().unwrap(), &Value::bool(false));
        assert_eq!(results[2].as_ref().unwrap(), &Value::bool(true));
        assert_eq!(results[3].as_ref().unwrap(), &Value::bool(false));
    }
}
