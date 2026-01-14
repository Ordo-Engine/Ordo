//! Rule module
//!
//! Provides rule models and step flow execution, including:
//! - RuleSet definition
//! - Step flow model (Decision Step, Action Step, Terminal Step)
//! - Condition and branch definitions
//! - Metric sink abstraction for custom metrics

mod executor;
mod metrics;
mod model;
mod step;

pub use executor::{BatchExecutionResult, ExecutionResult, RuleExecutor, SingleExecutionResult};
pub use metrics::{LoggingMetricSink, MetricSink, MetricType, NoOpMetricSink};
pub use model::{RuleSet, RuleSetConfig};
pub use step::{Action, ActionKind, Branch, Condition, Step, StepKind, TerminalResult};
