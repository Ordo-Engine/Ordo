//! Debug types for VM visualization

use ordo_core::prelude::Value;
use serde::{Deserialize, Serialize};

/// Trace level for debug execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceLevel {
    /// No tracing - just return result
    None,
    /// Minimal tracing - only final result and basic stats
    #[default]
    Minimal,
    /// Standard tracing - record each instruction execution
    Standard,
    /// Full tracing - record all register states at each step
    Full,
}

/// Debug execution request (for existing ruleset by name)
#[derive(Debug, Deserialize)]
pub struct DebugExecuteRequest {
    /// Input data for rule execution
    pub input: Value,
    /// Trace level
    #[serde(default)]
    pub trace_level: TraceLevel,
    /// Breakpoints (step IDs or instruction indices)
    #[serde(default)]
    pub breakpoints: Vec<String>,
}

/// Debug execution request for inline ruleset (no upload required)
#[derive(Debug, Deserialize)]
pub struct DebugExecuteInlineRequest {
    /// Complete RuleSet definition
    pub ruleset: ordo_core::prelude::RuleSet,
    /// Input data for rule execution
    pub input: Value,
    /// Trace level
    #[serde(default)]
    pub trace_level: TraceLevel,
    /// Breakpoints (step IDs or instruction indices)
    #[serde(default)]
    pub breakpoints: Vec<String>,
}

/// Debug execution response
#[derive(Debug, Serialize)]
pub struct DebugExecuteResponse {
    /// Execution result
    pub result: ExecutionResultInfo,
    /// VM execution trace (if trace_level >= Standard)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_trace: Option<VMTrace>,
    /// Expression evaluation traces
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expr_traces: Vec<ExprTrace>,
    /// Rule execution trace
    pub rule_trace: RuleTrace,
}

/// Execution result info
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResultInfo {
    pub code: String,
    pub message: String,
    pub output: Value,
    pub duration_us: u64,
}

/// VM execution trace
#[derive(Debug, Clone, Serialize)]
pub struct VMTrace {
    /// List of instructions (human-readable)
    pub instructions: Vec<String>,
    /// Constants pool
    pub constants: Vec<String>,
    /// Fields pool
    pub fields: Vec<String>,
    /// Functions pool
    pub functions: Vec<String>,
    /// Execution snapshots
    pub snapshots: Vec<VMSnapshot>,
    /// Total instructions executed
    pub total_instructions: usize,
    /// Total execution time in nanoseconds
    pub total_duration_ns: u64,
}

/// VM state snapshot at a point in execution
#[derive(Debug, Clone, Serialize)]
pub struct VMSnapshot {
    /// Instruction pointer
    pub ip: usize,
    /// Current instruction (human-readable)
    pub instruction: String,
    /// Register states (only non-null registers)
    pub registers: Vec<RegisterValue>,
    /// Duration of this instruction in nanoseconds
    pub duration_ns: u64,
}

/// Register value with type info
#[derive(Debug, Clone, Serialize)]
pub struct RegisterValue {
    /// Register index
    pub index: u8,
    /// Value (JSON representation)
    pub value: Value,
    /// Type name
    pub type_name: String,
}

/// Expression evaluation trace
#[derive(Debug, Clone, Serialize)]
pub struct ExprTrace {
    /// Original expression string
    pub expression: String,
    /// AST representation
    pub ast: ASTNode,
    /// Compiled bytecode info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<BytecodeInfo>,
    /// Evaluation steps
    pub eval_steps: Vec<EvalStep>,
    /// Final result
    pub result: Value,
}

/// AST node for visualization
#[derive(Debug, Clone, Serialize)]
pub struct ASTNode {
    /// Node type (literal, binary, unary, field, call, etc.)
    pub node_type: String,
    /// Display label
    pub label: String,
    /// Child nodes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ASTNode>,
    /// Value (for literals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Compiled bytecode info
#[derive(Debug, Clone, Serialize)]
pub struct BytecodeInfo {
    /// Number of instructions
    pub instruction_count: usize,
    /// Number of constants
    pub constant_count: usize,
    /// Number of fields
    pub field_count: usize,
    /// Number of functions
    pub function_count: usize,
    /// Instructions (human-readable)
    pub instructions: Vec<String>,
}

/// Single evaluation step
#[derive(Debug, Clone, Serialize)]
pub struct EvalStep {
    /// Step number
    pub step: usize,
    /// Description of what happened
    pub description: String,
    /// Intermediate result
    pub result: Value,
}

/// Rule execution trace
#[derive(Debug, Clone, Serialize)]
pub struct RuleTrace {
    /// Execution path (step IDs)
    pub path: Vec<String>,
    /// Step execution details
    pub steps: Vec<StepTraceInfo>,
    /// Variables at end of execution
    pub variables: serde_json::Map<String, serde_json::Value>,
}

/// Step trace info
#[derive(Debug, Clone, Serialize)]
pub struct StepTraceInfo {
    /// Step ID
    pub id: String,
    /// Step name
    pub name: String,
    /// Step type (decision, action, terminal)
    pub step_type: String,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Branch taken (for decision steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_taken: Option<String>,
    /// Condition result (for decision steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_result: Option<bool>,
}

/// Debug expression evaluation request
#[derive(Debug, Deserialize)]
pub struct DebugEvalRequest {
    /// Expression to evaluate
    pub expression: String,
    /// Context data
    #[serde(default)]
    pub context: Value,
    /// Trace level
    #[serde(default)]
    pub trace_level: TraceLevel,
}

/// Debug expression evaluation response
#[derive(Debug, Serialize)]
pub struct DebugEvalResponse {
    /// Evaluation result
    pub result: Value,
    /// AST representation
    pub ast: ASTNode,
    /// Compiled bytecode info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode: Option<BytecodeInfo>,
    /// Evaluation steps
    pub eval_steps: Vec<EvalStep>,
    /// Parsing duration in nanoseconds
    pub parse_duration_ns: u64,
    /// Compilation duration in nanoseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_duration_ns: Option<u64>,
    /// Execution duration in nanoseconds
    pub eval_duration_ns: u64,
}

/// Debug control command
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DebugCommand {
    /// Step to next instruction
    StepInto,
    /// Step over function calls
    StepOver,
    /// Continue execution until breakpoint or end
    Continue,
    /// Pause execution
    Pause,
    /// Stop and terminate session
    Stop,
    /// Set breakpoint
    SetBreakpoint { location: String },
    /// Remove breakpoint
    RemoveBreakpoint { location: String },
}

/// Debug control response
#[derive(Debug, Serialize)]
pub struct DebugControlResponse {
    /// Whether command was accepted
    pub success: bool,
    /// Current session state
    pub state: SessionState,
    /// Message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Debug session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Session created, waiting to start
    Created,
    /// Execution in progress
    Running,
    /// Paused at breakpoint or step
    Paused,
    /// Execution completed
    Completed,
    /// Session terminated
    Terminated,
}

/// SSE event types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebugEvent {
    /// Session state changed
    StateChange { state: SessionState },
    /// VM state update
    VMState {
        ip: usize,
        instruction: String,
        registers: Vec<RegisterValue>,
    },
    /// Breakpoint hit
    BreakpointHit { ip: usize, reason: String },
    /// Execution completed
    ExecutionComplete {
        result: ExecutionResultInfo,
        total_instructions: usize,
    },
    /// Error occurred
    Error { message: String },
    /// Heartbeat (keep-alive)
    Heartbeat { timestamp: u64 },
}

/// Debug session info (for listing)
#[derive(Debug, Clone, Serialize)]
pub struct DebugSessionInfo {
    /// Session ID
    pub id: String,
    /// Ruleset name being debugged
    pub ruleset_name: String,
    /// Current state
    pub state: SessionState,
    /// Created timestamp
    pub created_at: String,
    /// Number of breakpoints
    pub breakpoint_count: usize,
}

/// Create debug session request
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Ruleset name to debug
    pub ruleset_name: String,
    /// Input data
    pub input: Value,
    /// Initial breakpoints
    #[serde(default)]
    pub breakpoints: Vec<String>,
    /// Trace level
    #[serde(default)]
    pub trace_level: TraceLevel,
}

/// Create debug session response
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    /// Session ID
    pub session_id: String,
    /// Initial state
    pub state: SessionState,
    /// SSE stream URL
    pub stream_url: String,
}
