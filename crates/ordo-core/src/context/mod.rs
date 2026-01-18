//! Execution context module
//!
//! Provides context management during rule execution, including:
//! - Value type system (Value)
//! - Context storage (Context)
//! - Schema system for typed contexts (Schema)

mod schema;
mod store;
mod value;

pub use schema::{FieldSchema, FieldType, MessageSchema, ResolvedField, SchemaRegistry};
pub use store::Context;
pub use value::{IString, SmallArray, Value};
