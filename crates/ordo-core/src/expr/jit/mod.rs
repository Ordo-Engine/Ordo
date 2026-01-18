//! JIT Compilation Module for Expressions
//!
//! This module provides Just-In-Time compilation of expressions to native
//! machine code using Cranelift with Schema-Aware direct field access.
//!
//! # Architecture
//!
//! ```text
//! Expression + Schema → SchemaJITCompiler → Native Code
//!                                              ↓
//!                                    Direct field access (ldr [ctx, #offset])
//! ```
//!
//! Performance: ~3-5ns per field access (12x faster than BytecodeVM)
//!
//! # Supported Operations
//!
//! - Arithmetic: +, -, *, /, %
//! - Comparison: ==, !=, <, <=, >, >=
//! - Logical: &&, ||, !
//! - Field access: direct memory offset
//! - Functions: abs, min, max, floor, ceil, round, sqrt
//!
//! # Usage
//!
//! ```ignore
//! use ordo_core::expr::jit::{SchemaJITCompiler, TypedContext};
//!
//! #[derive(TypedContext)]
//! #[repr(C)]
//! struct MyContext { amount: f64, score: i32 }
//!
//! let mut compiler = SchemaJITCompiler::new()?;
//! let compiled = compiler.compile_with_schema(&expr, hash, MyContext::schema())?;
//! let result = unsafe { compiled.call_typed(&ctx)? };
//! ```

mod schema_compiler;
mod schema_evaluator;
mod typed_context;

// Schema-Aware JIT exports
pub use schema_compiler::{
    SchemaCompiledFunction, SchemaJITCompiler, SchemaJITErrorCode, SchemaJITStats,
};
pub use schema_evaluator::{SchemaJITEvaluator, SchemaJITEvaluatorConfig, SchemaJITEvaluatorStats};
pub use typed_context::{DynamicTypedContext, FieldAccessInfo, TypedContext};
