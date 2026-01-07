//! Expression module
//!
//! Provides expression parsing and evaluation, including:
//! - Expression AST (Expr)
//! - Expression parser
//! - Expression evaluator
//! - Built-in functions

mod ast;
mod eval;
mod functions;
mod parser;

pub use ast::{BinaryOp, Expr, UnaryOp};
pub use eval::Evaluator;
pub use functions::FunctionRegistry;
pub use parser::ExprParser;
