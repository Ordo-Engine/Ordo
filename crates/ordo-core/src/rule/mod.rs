//! Rule module
//!
//! Provides rule models and step flow execution, including:
//! - RuleSet definition
//! - Step flow model (Decision Step, Action Step, Terminal Step)
//! - Condition and branch definitions

mod executor;
mod model;
mod step;

pub use executor::RuleExecutor;
pub use model::{RuleSet, RuleSetConfig};
pub use step::{Action, ActionKind, Branch, Condition, Step, StepKind, TerminalResult};
