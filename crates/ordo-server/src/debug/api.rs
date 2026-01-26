//! Debug API handlers
//!
//! These endpoints are only available when debug mode is enabled.

use std::convert::Infallible;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::stream::Stream;
use ordo_core::{
    context::Context,
    expr::{BytecodeVM, Expr, ExprCompiler, ExprParser, TraceLevel as CoreTraceLevel},
    prelude::{ExecutionResult, Value},
};

use crate::error::ApiError;
use crate::AppState;

use super::types::*;

type ApiResult<T> = std::result::Result<T, ApiError>;

// ==================== Debug Execute ====================

/// Execute a ruleset with full debug tracing
pub async fn debug_execute_ruleset(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(request): Json<DebugExecuteRequest>,
) -> ApiResult<Json<DebugExecuteResponse>> {
    let start = Instant::now();

    // Get ruleset
    let ruleset = {
        let store = state.store.read().await;
        store
            .get(&name)
            .ok_or_else(|| ApiError::not_found(format!("RuleSet '{}' not found", name)))?
    };

    // Execute with tracing
    let result = state.executor.execute(&ruleset, request.input.clone());

    let duration_us = start.elapsed().as_micros() as u64;

    match result {
        Ok(exec_result) => {
            // Build rule trace from execution trace
            let rule_trace = build_rule_trace(&exec_result);

            // Build VM trace if requested
            let vm_trace = if request.trace_level >= TraceLevel::Standard {
                // For now, return a placeholder - full VM tracing requires core changes
                Some(VMTrace {
                    instructions: vec!["(VM tracing requires core integration)".to_string()],
                    constants: vec![],
                    fields: vec![],
                    functions: vec![],
                    snapshots: vec![],
                    total_instructions: 0,
                    total_duration_ns: duration_us * 1000,
                })
            } else {
                None
            };

            Ok(Json(DebugExecuteResponse {
                result: ExecutionResultInfo {
                    code: exec_result.code.clone(),
                    message: exec_result.message.clone(),
                    output: exec_result.output.clone(),
                    duration_us,
                },
                vm_trace,
                // Expression traces require additional tracing in the core executor
                // to capture individual expression evaluations during rule execution
                expr_traces: vec![],
                rule_trace,
            }))
        }
        Err(e) => Err(ApiError::internal(format!("Execution error: {}", e))),
    }
}

fn build_rule_trace(result: &ExecutionResult) -> RuleTrace {
    let mut path = Vec::new();
    let mut steps = Vec::new();
    let mut all_variables = serde_json::Map::new();

    if let Some(trace) = &result.trace {
        for step in &trace.steps {
            path.push(step.step_id.clone());

            // Determine step type from terminal flag and next_step
            let step_type = if step.is_terminal {
                "terminal".to_string()
            } else if step.next_step.is_some() {
                // If it has a single next step, likely an action
                "action".to_string()
            } else {
                // Could be decision (has branches) or other
                "decision".to_string()
            };

            steps.push(StepTraceInfo {
                id: step.step_id.clone(),
                name: step.step_name.clone(),
                step_type,
                duration_us: step.duration_us,
                branch_taken: step.next_step.clone(),
                condition_result: None,
            });

            // Collect variables from this step
            if let Some(vars) = &step.variables_snapshot {
                for (k, v) in vars {
                    // Convert ordo_core::Value to serde_json::Value
                    if let Ok(json_val) = serde_json::to_value(v) {
                        all_variables.insert(k.clone(), json_val);
                    }
                }
            }
        }
    }

    RuleTrace {
        path,
        steps,
        variables: all_variables,
    }
}

fn build_rule_trace_with_types(
    result: &ExecutionResult,
    ruleset: &ordo_core::prelude::RuleSet,
) -> RuleTrace {
    let mut path = Vec::new();
    let mut steps = Vec::new();
    let mut all_variables = serde_json::Map::new();

    if let Some(trace) = &result.trace {
        for step in &trace.steps {
            path.push(step.step_id.clone());

            // Get actual step type from ruleset
            let step_type = ruleset
                .steps
                .get(&step.step_id)
                .map(|s| match s.kind {
                    ordo_core::prelude::StepKind::Decision { .. } => "decision",
                    ordo_core::prelude::StepKind::Action { .. } => "action",
                    ordo_core::prelude::StepKind::Terminal { .. } => "terminal",
                })
                .unwrap_or("unknown")
                .to_string();

            steps.push(StepTraceInfo {
                id: step.step_id.clone(),
                name: step.step_name.clone(),
                step_type,
                duration_us: step.duration_us,
                branch_taken: step.next_step.clone(),
                condition_result: None,
            });

            // Collect variables from this step
            if let Some(vars) = &step.variables_snapshot {
                for (k, v) in vars {
                    // Convert ordo_core::Value to serde_json::Value
                    if let Ok(json_val) = serde_json::to_value(v) {
                        all_variables.insert(k.clone(), json_val);
                    }
                }
            }
        }
    }

    RuleTrace {
        path,
        steps,
        variables: all_variables,
    }
}

// ==================== Debug Execute Inline ====================

/// Execute an inline ruleset with full debug tracing (no upload required)
pub async fn debug_execute_inline(
    State(state): State<AppState>,
    Json(request): Json<DebugExecuteInlineRequest>,
) -> ApiResult<Json<DebugExecuteResponse>> {
    let start = Instant::now();

    // Validate and compile the ruleset
    let mut ruleset = request.ruleset;
    ruleset
        .compile()
        .map_err(|e| ApiError::bad_request(format!("RuleSet compilation error: {}", e)))?;

    // Validate the ruleset
    ruleset.validate().map_err(|errors| {
        ApiError::bad_request(format!("RuleSet validation errors: {:?}", errors))
    })?;

    // Execute with tracing
    let result = state.executor.execute(&ruleset, request.input.clone());

    let duration_us = start.elapsed().as_micros() as u64;

    match result {
        Ok(exec_result) => {
            // Build rule trace from execution trace with step type info
            let rule_trace = build_rule_trace_with_types(&exec_result, &ruleset);

            // Build VM trace if requested
            let vm_trace = if request.trace_level >= TraceLevel::Standard {
                // For now, return a placeholder - full VM tracing requires core changes
                Some(VMTrace {
                    instructions: vec!["(VM tracing requires core integration)".to_string()],
                    constants: vec![],
                    fields: vec![],
                    functions: vec![],
                    snapshots: vec![],
                    total_instructions: 0,
                    total_duration_ns: duration_us * 1000,
                })
            } else {
                None
            };

            Ok(Json(DebugExecuteResponse {
                result: ExecutionResultInfo {
                    code: exec_result.code.clone(),
                    message: exec_result.message.clone(),
                    output: exec_result.output.clone(),
                    duration_us,
                },
                vm_trace,
                // Expression traces require additional tracing in the core executor
                // to capture individual expression evaluations during rule execution
                expr_traces: vec![],
                rule_trace,
            }))
        }
        Err(e) => Err(ApiError::internal(format!("Execution error: {}", e))),
    }
}

// ==================== Debug Eval ====================

/// Convert API TraceLevel to core TraceLevel
fn to_core_trace_level(level: TraceLevel) -> CoreTraceLevel {
    match level {
        TraceLevel::None => CoreTraceLevel::None,
        TraceLevel::Minimal => CoreTraceLevel::Minimal,
        TraceLevel::Standard => CoreTraceLevel::Standard,
        TraceLevel::Full => CoreTraceLevel::Full,
    }
}

/// Evaluate an expression with full debug info
pub async fn debug_eval_expression(
    State(_state): State<AppState>,
    Json(request): Json<DebugEvalRequest>,
) -> ApiResult<Json<DebugEvalResponse>> {
    let parse_start = Instant::now();

    // Parse expression
    let expr = ExprParser::parse(&request.expression)
        .map_err(|e| ApiError::bad_request(format!("Parse error: {}", e)))?;

    let parse_duration_ns = parse_start.elapsed().as_nanos() as u64;

    // Build AST node
    let ast = expr_to_ast_node(&expr);

    // Compile to bytecode
    let compile_start = Instant::now();
    let compiled = ExprCompiler::new().compile(&expr);
    let compile_duration_ns = compile_start.elapsed().as_nanos() as u64;

    // Build bytecode info
    let bytecode = if request.trace_level >= TraceLevel::Standard {
        Some(BytecodeInfo {
            instruction_count: compiled.instructions.len(),
            constant_count: compiled.constants.len(),
            field_count: compiled.fields.len(),
            function_count: compiled.functions.len(),
            instructions: compiled
                .instructions
                .iter()
                .enumerate()
                .map(|(i, inst)| format!("{:3}: {:?}", i, inst))
                .collect(),
        })
    } else {
        None
    };

    // Create context
    let ctx = if request.context == Value::Null {
        Context::default()
    } else {
        Context::new(request.context.clone())
    };

    // Evaluate with tracing
    let vm = BytecodeVM::new();
    let core_trace_level = to_core_trace_level(request.trace_level);

    let (result, vm_trace) = vm
        .execute_with_trace(&compiled, &ctx, core_trace_level)
        .map_err(|e| ApiError::bad_request(format!("Evaluation error: {}", e)))?;

    // Build eval steps from VM trace
    let eval_steps: Vec<EvalStep> = vm_trace
        .snapshots
        .iter()
        .enumerate()
        .map(|(i, snapshot)| EvalStep {
            step: i + 1,
            description: snapshot.instruction.clone(),
            result: snapshot
                .registers
                .first()
                .map(|r| r.value.clone())
                .unwrap_or(Value::Null),
        })
        .collect();

    // If no steps, add a summary step
    let eval_steps = if eval_steps.is_empty() {
        vec![EvalStep {
            step: 1,
            description: format!("Evaluated: {}", request.expression),
            result: result.clone(),
        }]
    } else {
        eval_steps
    };

    Ok(Json(DebugEvalResponse {
        result,
        ast,
        bytecode,
        eval_steps,
        parse_duration_ns,
        compile_duration_ns: Some(compile_duration_ns),
        eval_duration_ns: vm_trace.total_duration_ns,
    }))
}

fn expr_to_ast_node(expr: &Expr) -> ASTNode {
    match expr {
        Expr::Literal(v) => ASTNode {
            node_type: "literal".to_string(),
            label: format!("{}", v),
            children: vec![],
            value: Some(format!("{:?}", v)),
        },
        Expr::Field(path) => ASTNode {
            node_type: "field".to_string(),
            label: format!("${}", path),
            children: vec![],
            value: None,
        },
        Expr::Binary { op, left, right } => ASTNode {
            node_type: "binary".to_string(),
            label: format!("{:?}", op),
            children: vec![expr_to_ast_node(left), expr_to_ast_node(right)],
            value: None,
        },
        Expr::Unary { op, operand } => ASTNode {
            node_type: "unary".to_string(),
            label: format!("{:?}", op),
            children: vec![expr_to_ast_node(operand)],
            value: None,
        },
        Expr::Call { name, args } => ASTNode {
            node_type: "call".to_string(),
            label: format!("{}()", name),
            children: args.iter().map(expr_to_ast_node).collect(),
            value: None,
        },
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => ASTNode {
            node_type: "conditional".to_string(),
            label: "if-then-else".to_string(),
            children: vec![
                expr_to_ast_node(condition),
                expr_to_ast_node(then_branch),
                expr_to_ast_node(else_branch),
            ],
            value: None,
        },
        Expr::Array(elements) => ASTNode {
            node_type: "array".to_string(),
            label: format!("[{}]", elements.len()),
            children: elements.iter().map(expr_to_ast_node).collect(),
            value: None,
        },
        Expr::Object(pairs) => ASTNode {
            node_type: "object".to_string(),
            label: format!("{{{}}}", pairs.len()),
            children: pairs
                .iter()
                .map(|(k, v)| ASTNode {
                    node_type: "pair".to_string(),
                    label: k.clone(),
                    children: vec![expr_to_ast_node(v)],
                    value: None,
                })
                .collect(),
            value: None,
        },
        Expr::Exists(path) => ASTNode {
            node_type: "exists".to_string(),
            label: format!("exists({})", path),
            children: vec![],
            value: None,
        },
        Expr::Coalesce(exprs) => ASTNode {
            node_type: "coalesce".to_string(),
            label: "??".to_string(),
            children: exprs.iter().map(expr_to_ast_node).collect(),
            value: None,
        },
    }
}

// ==================== Session Management ====================

/// List all debug sessions
pub async fn list_debug_sessions(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<DebugSessionInfo>>> {
    Ok(Json(state.debug_sessions.list_sessions()))
}

/// Create a new debug session
pub async fn create_debug_session(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> ApiResult<Json<CreateSessionResponse>> {
    // Verify ruleset exists
    {
        let store = state.store.read().await;
        if !store.exists(&request.ruleset_name) {
            return Err(ApiError::not_found(format!(
                "RuleSet '{}' not found",
                request.ruleset_name
            )));
        }
    }

    let session_id = state.debug_sessions.create_session(
        request.ruleset_name,
        request.input,
        request.trace_level,
        request.breakpoints,
    );

    Ok(Json(CreateSessionResponse {
        session_id: session_id.clone(),
        state: SessionState::Created,
        stream_url: format!("/api/v1/debug/stream/{}", session_id),
    }))
}

/// Get debug session info
pub async fn get_debug_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<DebugSessionInfo>> {
    let session = state
        .debug_sessions
        .get_session(&session_id)
        .ok_or_else(|| ApiError::not_found(format!("Session '{}' not found", session_id)))?;

    Ok(Json(DebugSessionInfo {
        id: session.id.clone(),
        ruleset_name: session.ruleset_name.clone(),
        state: session.get_state(),
        created_at: session.created_at.clone(),
        breakpoint_count: session.breakpoint_count(),
    }))
}

/// Delete a debug session
pub async fn delete_debug_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if state.debug_sessions.delete_session(&session_id) {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Session '{}' deleted", session_id)
        })))
    } else {
        Err(ApiError::not_found(format!(
            "Session '{}' not found",
            session_id
        )))
    }
}

// ==================== SSE Stream ====================

/// SSE stream for debug events
pub async fn debug_stream(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> std::result::Result<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>, ApiError>
{
    let session = state
        .debug_sessions
        .get_session(&session_id)
        .ok_or_else(|| ApiError::not_found(format!("Session '{}' not found", session_id)))?;

    let mut rx = session.subscribe();
    drop(session); // Release lock

    // Create stream from receiver
    let stream = async_stream::stream! {
        // Send initial state
        yield Ok(Event::default()
            .event("connected")
            .data(serde_json::json!({
                "session_id": session_id,
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            }).to_string()));

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            let event_type = match &event {
                                DebugEvent::StateChange { .. } => "state_change",
                                DebugEvent::VMState { .. } => "vm_state",
                                DebugEvent::BreakpointHit { .. } => "breakpoint_hit",
                                DebugEvent::ExecutionComplete { .. } => "execution_complete",
                                DebugEvent::Error { .. } => "error",
                                DebugEvent::Heartbeat { .. } => "heartbeat",
                            };
                            yield Ok(Event::default()
                                .event(event_type)
                                .data(serde_json::to_string(&event).unwrap_or_default()));
                        }
                        Err(_) => {
                            // Channel closed, end stream
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    // Send heartbeat
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    yield Ok(Event::default()
                        .event("heartbeat")
                        .data(serde_json::json!({ "timestamp": timestamp }).to_string()));
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ==================== Debug Control ====================

/// Send control command to debug session
pub async fn debug_control(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(command): Json<DebugCommand>,
) -> ApiResult<Json<DebugControlResponse>> {
    let session = state
        .debug_sessions
        .get_session(&session_id)
        .ok_or_else(|| ApiError::not_found(format!("Session '{}' not found", session_id)))?;

    let current_state = session.get_state();

    match command {
        DebugCommand::StepInto | DebugCommand::StepOver => {
            if current_state != SessionState::Paused && current_state != SessionState::Created {
                return Ok(Json(DebugControlResponse {
                    success: false,
                    state: current_state,
                    message: Some("Session must be paused or created to step".to_string()),
                }));
            }
            // Step-by-step execution requires integration with the core executor
            // to pause at each step and send intermediate state via SSE events.
            // For now, we transition to running state for the full execution.
            session.set_state(SessionState::Running);
            Ok(Json(DebugControlResponse {
                success: true,
                state: SessionState::Running,
                message: Some("Stepping...".to_string()),
            }))
        }
        DebugCommand::Continue => {
            if current_state != SessionState::Paused && current_state != SessionState::Created {
                return Ok(Json(DebugControlResponse {
                    success: false,
                    state: current_state,
                    message: Some("Session must be paused or created to continue".to_string()),
                }));
            }
            session.set_state(SessionState::Running);
            Ok(Json(DebugControlResponse {
                success: true,
                state: SessionState::Running,
                message: Some("Continuing execution...".to_string()),
            }))
        }
        DebugCommand::Pause => {
            if current_state != SessionState::Running {
                return Ok(Json(DebugControlResponse {
                    success: false,
                    state: current_state,
                    message: Some("Session must be running to pause".to_string()),
                }));
            }
            session.set_state(SessionState::Paused);
            Ok(Json(DebugControlResponse {
                success: true,
                state: SessionState::Paused,
                message: Some("Paused".to_string()),
            }))
        }
        DebugCommand::Stop => {
            session.set_state(SessionState::Terminated);
            Ok(Json(DebugControlResponse {
                success: true,
                state: SessionState::Terminated,
                message: Some("Session terminated".to_string()),
            }))
        }
        DebugCommand::SetBreakpoint { location } => {
            session.add_breakpoint(location.clone());
            Ok(Json(DebugControlResponse {
                success: true,
                state: current_state,
                message: Some(format!("Breakpoint set at {}", location)),
            }))
        }
        DebugCommand::RemoveBreakpoint { location } => {
            let removed = session.remove_breakpoint(&location);
            Ok(Json(DebugControlResponse {
                success: removed,
                state: current_state,
                message: Some(if removed {
                    format!("Breakpoint removed from {}", location)
                } else {
                    format!("No breakpoint at {}", location)
                }),
            }))
        }
    }
}
