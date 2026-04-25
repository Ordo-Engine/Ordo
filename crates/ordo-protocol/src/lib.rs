//! ordo-protocol — Studio ↔ engine protocol conversion layer.
//!
//! The frontend (Studio) speaks a camelCase JSON format with steps as arrays,
//! structured Condition/Expr objects, and editor metadata like positions and groups.
//! The engine (ordo-core) speaks a snake_case format with steps as a HashMap,
//! and conditions as expression strings or pre-compiled `Expr` ASTs.
//!
//! This crate owns all conversion between the two formats so the frontend can
//! send its natural format and the backend handles the rest.

pub mod convert;
pub mod types;

pub use convert::ConvertError;
pub use types::*;
