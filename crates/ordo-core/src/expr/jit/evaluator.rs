//! JIT-Enabled Expression Evaluator
//!
//! Wraps the standard evaluator with profiling and JIT compilation support.

use super::cache::{BackgroundJIT, DiskCache, JITCacheConfig};
use crate::context::{Context, Value};
use crate::error::Result;
use crate::expr::profiler::{hash_expr, JITDecision, Profiler, ProfilerConfig};
use crate::expr::{BytecodeVM, CompiledExpr, Evaluator, Expr, ExprCompiler, ExprOptimizer};
// Note: ExprCompiler is still used in eval_with_vm
use std::sync::Arc;
use std::time::Instant;

/// Configuration for JIT-enabled evaluator
#[derive(Debug, Clone)]
pub struct JITEvaluatorConfig {
    /// Profiler configuration
    pub profiler: ProfilerConfig,
    /// JIT cache configuration
    pub cache: JITCacheConfig,
    /// Whether to use the bytecode VM for non-JIT execution
    pub use_bytecode_vm: bool,
    /// Whether to apply constant folding optimization
    pub constant_folding: bool,
}

impl Default for JITEvaluatorConfig {
    fn default() -> Self {
        Self {
            profiler: ProfilerConfig::default(),
            cache: JITCacheConfig::default(),
            use_bytecode_vm: true,
            constant_folding: true,
        }
    }
}

/// JIT-enabled expression evaluator
///
/// This evaluator automatically profiles expression execution and triggers
/// background JIT compilation for hot expressions.
pub struct JITEvaluator {
    /// Standard evaluator (fallback)
    evaluator: Evaluator,
    /// Bytecode VM
    vm: BytecodeVM,
    /// Expression optimizer (behind Mutex for interior mutability)
    optimizer: parking_lot::Mutex<ExprOptimizer>,
    /// Profiler
    profiler: Arc<Profiler>,
    /// Background JIT system
    background_jit: Option<BackgroundJIT>,
    /// Disk cache
    disk_cache: DiskCache,
    /// Configuration
    config: JITEvaluatorConfig,
    /// Compiled expression cache (for bytecode VM)
    compiled_exprs: parking_lot::RwLock<hashbrown::HashMap<u64, CompiledExpr>>,
}

impl JITEvaluator {
    /// Create a new JIT-enabled evaluator
    pub fn new(config: JITEvaluatorConfig) -> Result<Self> {
        let background_jit = BackgroundJIT::new(config.cache.clone())?;
        let disk_cache = if config.cache.enable_disk_cache {
            DiskCache::new(config.cache.cache_dir.clone())
        } else {
            DiskCache::disabled()
        };

        Ok(Self {
            evaluator: Evaluator::new(),
            vm: BytecodeVM::new(),
            optimizer: parking_lot::Mutex::new(ExprOptimizer::new()),
            profiler: Arc::new(Profiler::with_config(config.profiler.clone())),
            background_jit: Some(background_jit),
            disk_cache,
            config,
            compiled_exprs: parking_lot::RwLock::new(hashbrown::HashMap::new()),
        })
    }

    /// Create a simple evaluator without JIT (for testing or low-traffic scenarios)
    pub fn simple() -> Self {
        Self {
            evaluator: Evaluator::new(),
            vm: BytecodeVM::new(),
            optimizer: parking_lot::Mutex::new(ExprOptimizer::new()),
            profiler: Arc::new(Profiler::new()),
            background_jit: None,
            disk_cache: DiskCache::disabled(),
            config: JITEvaluatorConfig::default(),
            compiled_exprs: parking_lot::RwLock::new(hashbrown::HashMap::new()),
        }
    }

    /// Evaluate an expression
    pub fn eval(&self, expr: &Expr, ctx: &Context) -> Result<Value> {
        let hash = self.hash_expr(expr);
        let start = Instant::now();

        // Try JIT-compiled version first
        if let Some(ref jit) = self.background_jit {
            if let Some(func_ptr) = jit.get(hash) {
                // Execute JIT-compiled function
                // Note: This path is currently not fully implemented
                // as we need FFI calling convention handling
                let _ = func_ptr;
                // For now, fall through to VM
            }
        }

        // Use bytecode VM if enabled
        let result = if self.config.use_bytecode_vm {
            self.eval_with_vm(expr, ctx, hash)?
        } else {
            self.evaluator.eval(expr, ctx)?
        };

        let duration = start.elapsed();

        // Profile the execution
        self.profiler.record_expr(hash, duration);

        // Check if should trigger JIT
        let decision = self.profiler.should_jit_expr(hash);
        if decision.should_jit {
            self.maybe_trigger_jit(expr, hash, decision);
        }

        Ok(result)
    }

    /// Evaluate an expression string
    pub fn eval_str(&self, expr_str: &str, ctx: &Context) -> Result<Value> {
        let expr = crate::expr::ExprParser::parse(expr_str)?;
        self.eval(&expr, ctx)
    }

    /// Evaluate with the bytecode VM
    fn eval_with_vm(&self, expr: &Expr, ctx: &Context, hash: u64) -> Result<Value> {
        // Check if we have a compiled version
        {
            let cache = self.compiled_exprs.read();
            if let Some(compiled) = cache.get(&hash) {
                return self.vm.execute(compiled, ctx);
            }
        }

        // Optimize and compile
        let optimized = if self.config.constant_folding {
            self.optimizer.lock().optimize(expr.clone())
        } else {
            expr.clone()
        };

        // Create a new compiler for each compilation (it consumes self)
        let compiler = ExprCompiler::new();
        let compiled = compiler.compile(&optimized);

        // Cache compiled expression
        {
            let mut cache = self.compiled_exprs.write();
            cache.insert(hash, compiled);
        }

        // Re-fetch and execute
        let cache = self.compiled_exprs.read();
        let compiled = cache.get(&hash).unwrap();
        self.vm.execute(compiled, ctx)
    }

    /// Maybe trigger JIT compilation
    fn maybe_trigger_jit(&self, expr: &Expr, hash: u64, decision: JITDecision) {
        if let Some(ref jit) = self.background_jit {
            if let Some(priority) = decision.priority {
                // Only submit JIT-compilable expressions
                if super::compiler::JITCompiler::is_jit_compilable(expr)
                    && jit.submit(expr.clone(), hash, priority)
                {
                    self.profiler.mark_expr_jit_triggered(hash);

                    // Record in disk cache for warm-up
                    let source = format!("{:?}", expr);
                    self.disk_cache.record_compilation(
                        hash,
                        source,
                        decision.avg_duration_ns / 1000,
                    );

                    tracing::debug!(
                        hash = hash,
                        priority = ?priority,
                        score = decision.hot_score,
                        "Triggered JIT compilation"
                    );
                }
            }
        }
    }

    /// Hash an expression
    fn hash_expr(&self, expr: &Expr) -> u64 {
        // Use the expression's string representation as hash input
        // This ensures the same expression always gets the same hash
        let source = format!("{:?}", expr);
        hash_expr(&source)
    }

    /// Get profiler statistics
    pub fn profiler_stats(&self) -> crate::expr::profiler::ProfilerStats {
        self.profiler.stats()
    }

    /// Get JIT cache statistics
    pub fn jit_stats(&self) -> Option<super::cache::JITCacheStats> {
        self.background_jit.as_ref().map(|jit| jit.stats())
    }

    /// Get disk cache statistics
    pub fn disk_cache_len(&self) -> usize {
        self.disk_cache.len()
    }

    /// Warm up the JIT cache from disk
    pub fn warm_up(&self, limit: usize) {
        let hot_entries = self.disk_cache.get_hot_expressions(limit);

        tracing::info!(
            entries = hot_entries.len(),
            "Warming up JIT cache from disk"
        );

        // Note: We don't have the expressions stored on disk, only metadata
        // In a full implementation, we would need to store expression ASTs
        // For now, we just log that warm-up was requested
        for entry in hot_entries {
            tracing::debug!(
                hash = entry.hash,
                access_count = entry.access_count,
                source = %entry.source,
                "Would warm up expression"
            );
        }
    }

    /// Save disk cache
    pub fn save_cache(&self) -> std::io::Result<()> {
        self.disk_cache.save_index()
    }

    /// Shutdown the JIT system gracefully
    pub fn shutdown(&mut self) {
        if let Some(ref mut jit) = self.background_jit {
            jit.shutdown();
        }

        // Save disk cache
        if let Err(e) = self.save_cache() {
            tracing::warn!(error = %e, "Failed to save JIT disk cache");
        }
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        if let Some(ref jit) = self.background_jit {
            jit.clear();
        }
        self.disk_cache.clear();
        self.compiled_exprs.write().clear();
    }

    /// Get the number of compiled bytecode expressions
    pub fn compiled_count(&self) -> usize {
        self.compiled_exprs.read().len()
    }

    /// Get the profiler
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }
}

impl Drop for JITEvaluator {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Value;
    use crate::expr::BinaryOp;

    #[test]
    fn test_jit_evaluator_simple() {
        let evaluator = JITEvaluator::simple();
        let ctx = Context::new(Value::Null);

        // Simple literal
        let expr = Expr::Literal(Value::Float(42.0));
        let result = evaluator.eval(&expr, &ctx).unwrap();
        assert_eq!(result, Value::Float(42.0));
    }

    #[test]
    fn test_jit_evaluator_arithmetic() {
        let evaluator = JITEvaluator::simple();
        let ctx = Context::new(Value::Null);

        // 1 + 2
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(1.0))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(2.0))),
        };

        let result = evaluator.eval(&expr, &ctx).unwrap();
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn test_jit_evaluator_profiling() {
        let evaluator = JITEvaluator::simple();
        let ctx = Context::new(Value::Null);
        let expr = Expr::Literal(Value::Float(42.0));

        // Execute multiple times
        for _ in 0..10 {
            evaluator.eval(&expr, &ctx).unwrap();
        }

        let stats = evaluator.profiler_stats();
        assert_eq!(stats.total_expr_executions, 10);
    }

    #[test]
    fn test_jit_evaluator_with_config() {
        let config = JITEvaluatorConfig {
            use_bytecode_vm: true,
            constant_folding: true,
            ..Default::default()
        };

        let evaluator = JITEvaluator::new(config).unwrap();

        // Create context with a value field
        let mut obj = hashbrown::HashMap::new();
        obj.insert("value".into(), Value::Int(100));
        let ctx = Context::new(Value::Object(obj));

        let expr = Expr::Field("value".to_string());
        let result = evaluator.eval(&expr, &ctx).unwrap();
        assert_eq!(result, Value::Int(100));
    }

    #[test]
    fn test_jit_evaluator_compiled_cache() {
        let evaluator = JITEvaluator::simple();
        let ctx = Context::new(Value::Null);

        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(5.0))),
            op: BinaryOp::Mul,
            right: Box::new(Expr::Literal(Value::Float(3.0))),
        };

        // First execution compiles
        evaluator.eval(&expr, &ctx).unwrap();
        assert_eq!(evaluator.compiled_count(), 1);

        // Second execution uses cache
        evaluator.eval(&expr, &ctx).unwrap();
        assert_eq!(evaluator.compiled_count(), 1); // Still 1
    }
}
