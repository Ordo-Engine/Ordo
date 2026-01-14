//! Execution context module
//!
//! Provides context management during rule execution, including:
//! - Value type system (Value)
//! - Context storage (Context)

mod store;
mod value;

pub use store::Context;
pub use value::{IString, SmallArray, Value};
