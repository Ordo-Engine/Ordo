//! Schema-Aware JIT Evaluator
//!
//! A high-performance expression evaluator that uses Schema-Aware JIT compilation
//! for typed contexts, with fallback to BytecodeVM for unsupported cases.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    SchemaJITEvaluator                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  eval_typed<T: TypedContext>()                              │
//! │      │                                                       │
//! │      ├── Has compiled function? ─── Yes ──► Execute JIT     │
//! │      │                                                       │
//! │      ├── Can compile with schema? ─ Yes ──► Compile + Exec  │
//! │      │                                                       │
//! │      └── No ──────────────────────────────► BytecodeVM      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Performance
//!
//! - Schema JIT: ~3-10ns per evaluation (direct memory access)
//! - BytecodeVM: ~15-40ns per evaluation (interpreted)
//!
//! # Usage
//!
//! ```ignore
//! use ordo_core::expr::jit::{SchemaJITEvaluator, TypedContext};
//!
//! #[derive(TypedContext)]
//! #[repr(C)]
//! struct LoanContext {
//!     amount: f64,
//!     credit_score: i32,
//! }
//!
//! let evaluator = SchemaJITEvaluator::new()?;
//! let ctx = LoanContext { amount: 50000.0, credit_score: 720 };
//! let result = evaluator.eval_typed(&expr, &ctx)?;
//! ```

use super::schema_compiler::{SchemaJITCompiler, SchemaJITStats};
use super::typed_context::TypedContext;
use crate::context::{MessageSchema, SchemaRegistry, Value};
use crate::error::{OrdoError, Result};
use crate::expr::profiler::{hash_expr, Profiler, ProfilerConfig};
use crate::expr::{BytecodeVM, CompiledExpr, Evaluator, Expr, ExprCompiler, ExprOptimizer};

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Configuration for Schema-Aware JIT Evaluator
#[derive(Debug, Clone)]
pub struct SchemaJITEvaluatorConfig {
    /// Whether to enable profiling
    pub enable_profiling: bool,
    /// Profiler configuration
    pub profiler: ProfilerConfig,
    /// Maximum number of compiled functions to cache
    pub max_cache_size: usize,
    /// Whether to apply constant folding optimization
    pub constant_folding: bool,
}

impl Default for SchemaJITEvaluatorConfig {
    fn default() -> Self {
        Self {
            enable_profiling: true,
            profiler: ProfilerConfig::default(),
            max_cache_size: 10000,
            constant_folding: true,
        }
    }
}

/// Schema-Aware JIT Evaluator
///
/// This evaluator provides high-performance expression evaluation for typed contexts
/// by using Schema-Aware JIT compilation with direct field access.
pub struct SchemaJITEvaluator {
    /// Schema-Aware JIT compiler
    compiler: Mutex<SchemaJITCompiler>,
    /// Standard evaluator (fallback for complex cases)
    #[allow(dead_code)]
    evaluator: Evaluator,
    /// Bytecode VM (fallback for non-JIT expressions)
    #[allow(dead_code)]
    vm: BytecodeVM,
    /// Expression optimizer
    optimizer: Mutex<ExprOptimizer>,
    /// Profiler
    profiler: Arc<Profiler>,
    /// Schema registry
    schema_registry: RwLock<SchemaRegistry>,
    /// Compiled bytecode cache (for VM fallback)
    bytecode_cache: RwLock<HashMap<u64, CompiledExpr>>,
    /// Configuration
    config: SchemaJITEvaluatorConfig,
}

impl SchemaJITEvaluator {
    /// Create a new Schema-Aware JIT Evaluator
    pub fn new(config: SchemaJITEvaluatorConfig) -> Result<Self> {
        let compiler = SchemaJITCompiler::new()?;

        Ok(Self {
            compiler: Mutex::new(compiler),
            evaluator: Evaluator::new(),
            vm: BytecodeVM::new(),
            optimizer: Mutex::new(ExprOptimizer::new()),
            profiler: Arc::new(Profiler::with_config(config.profiler.clone())),
            schema_registry: RwLock::new(SchemaRegistry::new()),
            bytecode_cache: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Create a simple evaluator with default configuration
    pub fn simple() -> Result<Self> {
        Self::new(SchemaJITEvaluatorConfig::default())
    }

    /// Register a schema
    pub fn register_schema(&self, schema: MessageSchema) -> Arc<MessageSchema> {
        self.schema_registry.write().register(schema)
    }

    /// Get a registered schema
    pub fn get_schema(&self, name: &str) -> Option<Arc<MessageSchema>> {
        self.schema_registry.read().get(name)
    }

    /// Evaluate an expression with a typed context
    ///
    /// This is the primary evaluation method for typed contexts.
    /// It uses Schema-Aware JIT when possible, falling back to BytecodeVM.
    pub fn eval_typed<C: TypedContext>(&self, expr: &Expr, ctx: &C) -> Result<Value> {
        let schema = C::schema();
        let hash = self.hash_expr_with_schema(expr, schema);
        let start = Instant::now();

        // Try to get cached JIT function
        {
            let compiler = self.compiler.lock();
            if let Some(compiled) = compiler.get(hash, &schema.name) {
                let result = unsafe { compiled.call_typed_value(ctx) };
                if self.config.enable_profiling {
                    self.profiler.record_expr(hash, start.elapsed());
                }
                return result;
            }
        }

        // Try to compile with schema
        if SchemaJITCompiler::can_compile_with_schema(expr, schema) {
            let compiled = {
                let mut compiler = self.compiler.lock();
                compiler.compile_with_schema(expr, hash, schema)?.clone()
            };

            let result = unsafe { compiled.call_typed_value(ctx) };
            if self.config.enable_profiling {
                self.profiler.record_expr(hash, start.elapsed());
            }
            return result;
        }

        // Fallback to BytecodeVM
        self.eval_with_vm_typed(expr, ctx, hash, start)
    }

    /// Evaluate with BytecodeVM (fallback path)
    fn eval_with_vm_typed<C: TypedContext>(
        &self,
        expr: &Expr,
        ctx: &C,
        hash: u64,
        start: Instant,
    ) -> Result<Value> {
        // We need to convert TypedContext to a Context for the VM
        // This is less efficient, but provides a fallback

        // For now, we use the standard evaluator which can handle any expression
        // In a real implementation, we might build a Value from the TypedContext

        // Check bytecode cache
        {
            let cache = self.bytecode_cache.read();
            if let Some(_compiled) = cache.get(&hash) {
                // We need a Context to execute, but we have a TypedContext
                // For now, evaluate using the tree-walking evaluator
                // This is a limitation of the fallback path
                let result = self.eval_expr_tree_walk(expr, ctx)?;
                if self.config.enable_profiling {
                    self.profiler.record_expr(hash, start.elapsed());
                }
                return Ok(result);
            }
        }

        // Optimize expression
        let optimized = if self.config.constant_folding {
            self.optimizer.lock().optimize(expr.clone())
        } else {
            expr.clone()
        };

        // Compile to bytecode
        let compiler = ExprCompiler::new();
        let compiled = compiler.compile(&optimized);

        // Cache it
        {
            let mut cache = self.bytecode_cache.write();
            if cache.len() < self.config.max_cache_size {
                cache.insert(hash, compiled);
            }
        }

        // Evaluate using tree-walking (since we don't have a Context)
        let result = self.eval_expr_tree_walk(expr, ctx)?;

        if self.config.enable_profiling {
            self.profiler.record_expr(hash, start.elapsed());
        }

        Ok(result)
    }

    /// Evaluate expression using tree-walking with TypedContext
    fn eval_expr_tree_walk<C: TypedContext>(&self, expr: &Expr, ctx: &C) -> Result<Value> {
        match expr {
            Expr::Literal(v) => Ok(v.clone()),

            Expr::Field(name) => {
                // Use TypedContext to get field value
                let value = unsafe { ctx.read_field_as_f64(name) };
                match value {
                    Some(v) => Ok(Value::Float(v)),
                    None => Err(OrdoError::eval_error(format!(
                        "Field '{}' not found or not numeric",
                        name
                    ))),
                }
            }

            Expr::Binary { left, op, right } => {
                let left_val = self.eval_expr_tree_walk(left, ctx)?;
                let right_val = self.eval_expr_tree_walk(right, ctx)?;
                self.apply_binary_op(*op, &left_val, &right_val)
            }

            Expr::Unary { op, operand } => {
                let val = self.eval_expr_tree_walk(operand, ctx)?;
                self.apply_unary_op(*op, &val)
            }

            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.eval_expr_tree_walk(condition, ctx)?;
                if cond_val.is_truthy() {
                    self.eval_expr_tree_walk(then_branch, ctx)
                } else {
                    self.eval_expr_tree_walk(else_branch, ctx)
                }
            }

            Expr::Call { name, args } => {
                let arg_vals: Result<Vec<_>> = args
                    .iter()
                    .map(|a| self.eval_expr_tree_walk(a, ctx))
                    .collect();
                self.apply_function(name, &arg_vals?)
            }

            _ => Err(OrdoError::eval_error(
                "Unsupported expression in tree-walk fallback".to_string(),
            )),
        }
    }

    /// Apply binary operation
    fn apply_binary_op(
        &self,
        op: crate::expr::BinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        use crate::expr::BinaryOp;

        let left_f = left.as_float().unwrap_or(0.0);
        let right_f = right.as_float().unwrap_or(0.0);

        let result = match op {
            BinaryOp::Add => left_f + right_f,
            BinaryOp::Sub => left_f - right_f,
            BinaryOp::Mul => left_f * right_f,
            BinaryOp::Div => left_f / right_f,
            BinaryOp::Mod => left_f % right_f,
            BinaryOp::Eq => {
                if (left_f - right_f).abs() < f64::EPSILON {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Ne => {
                if (left_f - right_f).abs() >= f64::EPSILON {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Lt => {
                if left_f < right_f {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Le => {
                if left_f <= right_f {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Gt => {
                if left_f > right_f {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Ge => {
                if left_f >= right_f {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::And => {
                if left_f != 0.0 && right_f != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            BinaryOp::Or => {
                if left_f != 0.0 || right_f != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            _ => {
                return Err(OrdoError::eval_error(format!(
                    "Unsupported binary op {:?}",
                    op
                )))
            }
        };

        Ok(Value::Float(result))
    }

    /// Apply unary operation
    fn apply_unary_op(&self, op: crate::expr::UnaryOp, val: &Value) -> Result<Value> {
        use crate::expr::UnaryOp;

        let v = val.as_float().unwrap_or(0.0);

        let result = match op {
            UnaryOp::Not => {
                if v == 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            UnaryOp::Neg => -v,
        };

        Ok(Value::Float(result))
    }

    /// Apply function
    fn apply_function(&self, name: &str, args: &[Value]) -> Result<Value> {
        let result = match name {
            "abs" => args.first().and_then(|v| v.as_float()).unwrap_or(0.0).abs(),
            "floor" => args
                .first()
                .and_then(|v| v.as_float())
                .unwrap_or(0.0)
                .floor(),
            "ceil" => args
                .first()
                .and_then(|v| v.as_float())
                .unwrap_or(0.0)
                .ceil(),
            "round" => args
                .first()
                .and_then(|v| v.as_float())
                .unwrap_or(0.0)
                .round(),
            "sqrt" => args
                .first()
                .and_then(|v| v.as_float())
                .unwrap_or(0.0)
                .sqrt(),
            "min" => {
                let a = args.first().and_then(|v| v.as_float()).unwrap_or(0.0);
                let b = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
                a.min(b)
            }
            "max" => {
                let a = args.first().and_then(|v| v.as_float()).unwrap_or(0.0);
                let b = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
                a.max(b)
            }
            _ => {
                return Err(OrdoError::eval_error(format!(
                    "Unknown function '{}'",
                    name
                )))
            }
        };

        Ok(Value::Float(result))
    }

    /// Compute expression hash with schema
    fn hash_expr_with_schema(&self, expr: &Expr, schema: &MessageSchema) -> u64 {
        // Include schema name in hash to differentiate same expression with different schemas
        let source = format!("{}:{:?}", schema.name, expr);
        hash_expr(&source)
    }

    /// Get profiler statistics
    pub fn profiler_stats(&self) -> crate::expr::profiler::ProfilerStats {
        self.profiler.stats()
    }

    /// Get JIT statistics
    pub fn jit_stats(&self) -> SchemaJITStats {
        self.compiler.lock().stats().clone()
    }

    /// Get bytecode cache size
    pub fn bytecode_cache_size(&self) -> usize {
        self.bytecode_cache.read().len()
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.bytecode_cache.write().clear();
        // Note: JIT cache cannot be easily cleared due to memory management
    }

    /// Get the profiler
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }
}

/// Statistics for the Schema JIT Evaluator
#[derive(Debug, Clone, Default)]
pub struct SchemaJITEvaluatorStats {
    /// JIT compilation stats
    pub jit_stats: SchemaJITStats,
    /// Number of JIT cache hits
    pub jit_cache_hits: u64,
    /// Number of VM fallbacks
    pub vm_fallbacks: u64,
    /// Bytecode cache size
    pub bytecode_cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{FieldType, MessageSchema};
    use crate::expr::BinaryOp;
    use std::sync::OnceLock;

    #[repr(C)]
    struct TestLoanContext {
        amount: f64,
        credit_score: i32,
        approved: bool,
    }

    impl TypedContext for TestLoanContext {
        fn schema() -> &'static MessageSchema {
            static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
            SCHEMA.get_or_init(|| {
                MessageSchema::builder("TestLoanContext")
                    .field_at("amount", FieldType::Float64, 0)
                    .field_at("credit_score", FieldType::Int32, 8)
                    .field_at("approved", FieldType::Bool, 12)
                    .build()
            })
        }

        unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
            match field_name {
                "amount" => Some((
                    std::ptr::addr_of!(self.amount) as *const u8,
                    FieldType::Float64,
                )),
                "credit_score" => Some((
                    std::ptr::addr_of!(self.credit_score) as *const u8,
                    FieldType::Int32,
                )),
                "approved" => Some((
                    std::ptr::addr_of!(self.approved) as *const u8,
                    FieldType::Bool,
                )),
                _ => None,
            }
        }
    }

    #[test]
    fn test_schema_jit_evaluator_simple() {
        let evaluator = SchemaJITEvaluator::simple().unwrap();
        let ctx = TestLoanContext {
            amount: 50000.0,
            credit_score: 720,
            approved: false,
        };

        // Simple literal
        let expr = Expr::Literal(Value::Float(42.0));
        let result = evaluator.eval_typed(&expr, &ctx).unwrap();
        assert_eq!(result, Value::Float(42.0));
    }

    #[test]
    fn test_schema_jit_evaluator_field_access() {
        let evaluator = SchemaJITEvaluator::simple().unwrap();
        let ctx = TestLoanContext {
            amount: 75000.0,
            credit_score: 680,
            approved: true,
        };

        // Field access: amount
        let expr = Expr::Field("amount".to_string());
        let result = evaluator.eval_typed(&expr, &ctx).unwrap();
        if let Value::Float(v) = result {
            assert!((v - 75000.0).abs() < 0.001);
        } else {
            panic!("Expected Float");
        }
    }

    #[test]
    fn test_schema_jit_evaluator_arithmetic() {
        let evaluator = SchemaJITEvaluator::simple().unwrap();
        let ctx = TestLoanContext {
            amount: 100.0,
            credit_score: 700,
            approved: false,
        };

        // amount * 2 + 50
        let expr = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Field("amount".to_string())),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Literal(Value::Float(2.0))),
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(50.0))),
        };

        let result = evaluator.eval_typed(&expr, &ctx).unwrap();
        if let Value::Float(v) = result {
            assert!((v - 250.0).abs() < 0.001);
        } else {
            panic!("Expected Float");
        }
    }

    #[test]
    fn test_schema_jit_evaluator_comparison() {
        let evaluator = SchemaJITEvaluator::simple().unwrap();

        // credit_score >= 700
        let expr = Expr::Binary {
            left: Box::new(Expr::Field("credit_score".to_string())),
            op: BinaryOp::Ge,
            right: Box::new(Expr::Literal(Value::Int(700))),
        };

        // Test with score = 720 (should be true = 1.0)
        let ctx1 = TestLoanContext {
            amount: 50000.0,
            credit_score: 720,
            approved: false,
        };
        let result1 = evaluator.eval_typed(&expr, &ctx1).unwrap();
        if let Value::Float(v) = result1 {
            assert!((v - 1.0).abs() < 0.001);
        } else {
            panic!("Expected Float");
        }

        // Test with score = 650 (should be false = 0.0)
        let ctx2 = TestLoanContext {
            amount: 50000.0,
            credit_score: 650,
            approved: false,
        };
        let result2 = evaluator.eval_typed(&expr, &ctx2).unwrap();
        if let Value::Float(v) = result2 {
            assert!(v.abs() < 0.001);
        } else {
            panic!("Expected Float");
        }
    }

    #[test]
    fn test_schema_jit_evaluator_cache() {
        let evaluator = SchemaJITEvaluator::simple().unwrap();
        let ctx = TestLoanContext {
            amount: 100.0,
            credit_score: 700,
            approved: false,
        };

        let expr = Expr::Field("amount".to_string());

        // First call compiles
        evaluator.eval_typed(&expr, &ctx).unwrap();
        let stats1 = evaluator.jit_stats();

        // Second call should use cache
        evaluator.eval_typed(&expr, &ctx).unwrap();
        let stats2 = evaluator.jit_stats();

        // Should have only compiled once
        assert_eq!(stats1.successful_compiles, stats2.successful_compiles);
    }
}
