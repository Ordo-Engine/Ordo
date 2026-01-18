//! Expression module
//!
//! Provides expression parsing and evaluation, including:
//! - Expression AST (Expr)
//! - Expression parser
//! - Expression evaluator
//! - Built-in functions
//! - Expression optimizer (constant folding, dead code elimination)
//! - High-performance bytecode compiler and VM with superinstructions
//! - Vectorized batch execution
//! - JIT compilation for hot expressions

mod ast;
mod compiler;
mod eval;
mod functions;
pub mod jit;
mod optimizer;
mod parser;
mod profiler;
mod vectorized;
mod vm;

pub use ast::{BinaryOp, Expr, UnaryOp};
pub use compiler::ExprCompiler;
pub use eval::Evaluator;
pub use functions::FunctionRegistry;

// Schema-Aware JIT exports
pub use jit::{
    DynamicTypedContext, FieldAccessInfo, SchemaCompiledFunction, SchemaJITCompiler,
    SchemaJITErrorCode, SchemaJITEvaluator, SchemaJITEvaluatorConfig, SchemaJITEvaluatorStats,
    SchemaJITStats, TypedContext,
};
pub use optimizer::{ExprOptimizer, OptimizationStats};
pub use parser::ExprParser;
pub use profiler::{
    hash_expr, ExprProfile, JITDecision, JITPriority, Profiler, ProfilerConfig, ProfilerStats,
    RulePathProfile,
};
pub use vectorized::{BatchStats, VectorizedEvaluator};
pub use vm::{
    BytecodeVM, CompiledExpr, CompiledExprStats, Instruction, Opcode, RegisterValue, TraceLevel,
    VMExecutionTrace, VMSnapshot,
};
