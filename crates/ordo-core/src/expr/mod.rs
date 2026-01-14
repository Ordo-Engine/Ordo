//! Expression module
//!
//! Provides expression parsing and evaluation, including:
//! - Expression AST (Expr)
//! - Expression parser
//! - Expression evaluator
//! - Built-in functions
//! - Expression optimizer (constant folding, dead code elimination)
//! - Bytecode compiler and VM
//! - Vectorized batch execution

mod ast;
mod bytecode;
mod compiler;
mod eval;
mod functions;
mod optimizer;
mod parser;
mod vectorized;
mod vm;

pub use ast::{BinaryOp, Expr, UnaryOp};
pub use bytecode::{CompiledExpr, CompiledExprStats, Opcode};
pub use compiler::ExprCompiler;
pub use eval::Evaluator;
pub use functions::FunctionRegistry;
pub use optimizer::{ExprOptimizer, OptimizationStats};
pub use parser::ExprParser;
pub use vectorized::{BatchStats, VectorizedEvaluator};
pub use vm::BytecodeVM;
