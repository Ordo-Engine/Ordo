//! JIT Compilation Module for Expressions
//!
//! This module provides Just-In-Time compilation of expressions to native
//! machine code using Cranelift.
//!
//! # Architecture
//!
//! ```text
//! Expression → JITCompiler → Native Code → CompiledFunction
//!                               ↓
//!                          JITCache (L1 Memory + L2 Disk)
//! ```
//!
//! # Supported Operations
//!
//! - Arithmetic: +, -, *, /, %
//! - Comparison: ==, !=, <, <=, >, >=
//! - Logical: &&, ||, !
//! - Functions: abs, min, max, floor, ceil, round
//!
//! # Not Yet Supported
//!
//! - Field access (requires runtime context lookup)
//! - String operations
//! - Array/Object operations
//! - Complex function calls

mod cache;
mod compiler;
mod evaluator;

pub use cache::{
    BackgroundJIT, CacheEntryMetadata, DiskCache, DiskCacheIndex, JITCacheConfig, JITCacheStats,
    JITTask, MemoryCache,
};
pub use compiler::{CompiledFunction, JITCompiler, JITErrorCode, JITStats};
pub use evaluator::{JITEvaluator, JITEvaluatorConfig};
