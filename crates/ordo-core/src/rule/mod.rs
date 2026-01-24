//! Rule module
//!
//! Provides rule models and step flow execution, including:
//! - RuleSet definition
//! - Step flow model (Decision Step, Action Step, Terminal Step)
//! - Condition and branch definitions
//! - Metric sink abstraction for custom metrics

mod compiled;
mod compiled_executor;
mod compiler;
mod executor;
mod metrics;
mod model;
mod step;

pub use compiled::{
    get_enterprise_plugin,
    register_enterprise_plugin,
    CompiledAction,
    CompiledBranch,
    CompiledCondition,
    CompiledMetadata,
    CompiledOutput,
    CompiledRuleSet,
    CompiledStep,
    // Enterprise plugin system
    EnterprisePlugin,
    NoOpEnterprisePlugin,
    FIELD_MISSING_DEFAULT,
    FIELD_MISSING_LENIENT,
    FIELD_MISSING_STRICT,
};
pub use compiled_executor::CompiledRuleExecutor;
pub use compiler::RuleSetCompiler;
pub use executor::{BatchExecutionResult, ExecutionResult, RuleExecutor, SingleExecutionResult};
pub use metrics::{LoggingMetricSink, MetricSink, MetricType, NoOpMetricSink};
pub use model::{RuleSet, RuleSetConfig};
pub use step::{Action, ActionKind, Branch, Condition, Step, StepKind, TerminalResult};
