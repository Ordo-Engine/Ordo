//! Studio → engine conversion (TryFrom<StudioRuleSet> for RuleSet)

use hashbrown::HashMap as FastMap;
use ordo_core::{
    context::Value as CoreValue,
    expr::{BinaryOp, Expr, UnaryOp},
    rule::{
        Action, ActionKind, Branch, Condition, FieldMissingBehavior, LogLevel, RuleSet,
        RuleSetConfig, Step, StepKind, SubRuleGraph, TerminalResult,
    },
};

use crate::types::{
    condition::condition_to_expr_string,
    expr::{binary_op_display, expr_to_string, map_binary_op, StudioExpr},
    ruleset::{StudioRuleSet, StudioSubRuleGraph},
    step::{StudioStep, StudioStepKind, StudioTerminalMessage},
};

/// Error returned when studio → engine conversion fails.
#[derive(thiserror::Error, Debug)]
pub enum ConvertError {
    #[error("startStepId is missing or empty")]
    MissingStartStep,

    #[error("step '{0}' has no nextStepId")]
    MissingNextStep(String),

    #[error("sub-rule '{0}' not found in ruleset")]
    SubRuleNotFound(String),

    #[error("expression conversion failed in step '{0}': {1}")]
    Expr(String, String),
}

// ── Top-level conversion ──────────────────────────────────────────────────────

impl TryFrom<StudioRuleSet> for RuleSet {
    type Error = ConvertError;

    fn try_from(s: StudioRuleSet) -> Result<Self, ConvertError> {
        if s.start_step_id.is_empty() {
            return Err(ConvertError::MissingStartStep);
        }

        let config = RuleSetConfig {
            name: s.config.name,
            tenant_id: None,
            version: s.config.version.unwrap_or_else(|| "1.0.0".to_string()),
            description: s.config.description.unwrap_or_default(),
            entry_step: s.start_step_id,
            field_missing: FieldMissingBehavior::Lenient,
            max_depth: 100,
            timeout_ms: s.config.timeout.unwrap_or(5000),
            enable_trace: s.config.enable_trace.unwrap_or(false),
            metadata: s.config.metadata.into_iter().collect(),
        };

        let mut steps: FastMap<String, Step> = FastMap::new();
        for studio_step in s.steps {
            let step = convert_step(studio_step)?;
            steps.insert(step.id.clone(), step);
        }

        let mut sub_rules: FastMap<String, SubRuleGraph> = FastMap::new();
        for (name, graph) in s.sub_rules {
            let converted = convert_sub_rule_graph(graph)?;
            sub_rules.insert(name, converted);
        }

        Ok(RuleSet {
            config,
            steps,
            sub_rules,
        })
    }
}

// ── Step conversion ───────────────────────────────────────────────────────────

fn convert_step(s: StudioStep) -> Result<Step, ConvertError> {
    let step_id = s.id.clone();
    let kind = convert_step_kind(s.kind, &step_id)?;
    Ok(Step {
        id: s.id,
        name: s.name,
        kind,
    })
}

fn convert_step_kind(kind: StudioStepKind, step_id: &str) -> Result<StepKind, ConvertError> {
    match kind {
        StudioStepKind::Decision {
            branches,
            default_next_step_id,
        } => {
            let converted: Result<Vec<Branch>, _> = branches
                .into_iter()
                .map(|b| convert_branch(b, step_id))
                .collect();
            Ok(StepKind::Decision {
                branches: converted?,
                default_next: default_next_step_id,
            })
        }

        StudioStepKind::Action {
            assignments,
            external_calls,
            logging,
            next_step_id,
        } => {
            let mut actions: Vec<Action> = Vec::new();

            for assign in assignments {
                let expr = convert_expr(assign.value, step_id)?;
                actions.push(Action {
                    kind: ActionKind::SetVariable {
                        name: assign.name,
                        value: expr,
                    },
                    description: String::new(),
                });
            }

            for call in external_calls {
                let params: Result<Vec<(String, Expr)>, _> = call
                    .params
                    .into_iter()
                    .map(|(k, v)| convert_expr(v, step_id).map(|e| (k, e)))
                    .collect();
                actions.push(Action {
                    kind: ActionKind::ExternalCall {
                        service: call.target,
                        method: call.call_type,
                        params: params?,
                        result_variable: call.result_variable,
                        timeout_ms: call.timeout.unwrap_or(5000),
                    },
                    description: String::new(),
                });
            }

            if let Some(log) = logging {
                let message = expr_to_log_message(log.message);
                let level = parse_log_level(log.level.as_deref());
                actions.push(Action {
                    kind: ActionKind::Log { message, level },
                    description: String::new(),
                });
            }

            Ok(StepKind::Action {
                actions,
                next_step: next_step_id,
            })
        }

        StudioStepKind::Terminal {
            code,
            message,
            output,
        } => {
            let output_fields: Result<Vec<(String, Expr)>, _> = output
                .into_iter()
                .map(|f| convert_expr(f.value, step_id).map(|e| (f.name, e)))
                .collect();

            let result = TerminalResult {
                code,
                message: terminal_message_to_engine_string(message),
                output: output_fields?,
                data: CoreValue::Null,
            };
            Ok(StepKind::Terminal { result })
        }

        StudioStepKind::SubRule {
            ref_name,
            bindings,
            outputs,
            next_step_id,
        } => {
            let converted_bindings: Result<Vec<(String, Expr)>, _> = bindings
                .into_iter()
                .map(|b| convert_expr(b.expr, step_id).map(|e| (b.field, e)))
                .collect();
            let converted_outputs: Vec<(String, String)> = outputs
                .into_iter()
                .map(|o| (o.parent_var, o.child_var))
                .collect();

            Ok(StepKind::SubRule {
                ref_name,
                bindings: converted_bindings?,
                outputs: converted_outputs,
                next_step: next_step_id,
            })
        }
    }
}

fn convert_branch(
    b: crate::types::step::StudioBranch,
    _step_id: &str,
) -> Result<Branch, ConvertError> {
    let expr_str = condition_to_expr_string(&b.condition);
    Ok(Branch {
        condition: Condition::ExpressionString(expr_str),
        next_step: b.next_step_id,
        actions: vec![],
    })
}

fn convert_sub_rule_graph(g: StudioSubRuleGraph) -> Result<SubRuleGraph, ConvertError> {
    let mut steps: FastMap<String, Step> = FastMap::new();
    for studio_step in g.steps {
        let step = convert_step(studio_step)?;
        steps.insert(step.id.clone(), step);
    }
    Ok(SubRuleGraph {
        entry_step: g.entry_step,
        steps,
    })
}

// ── Expr conversion ───────────────────────────────────────────────────────────

fn convert_expr(expr: StudioExpr, step_id: &str) -> Result<Expr, ConvertError> {
    match expr {
        StudioExpr::Literal { value, .. } => {
            let v = json_to_core_value(value)
                .map_err(|e| ConvertError::Expr(step_id.to_string(), e))?;
            Ok(Expr::Literal(v))
        }

        StudioExpr::Variable { path } => {
            let field = if let Some(stripped) = path.strip_prefix("$.") {
                stripped.to_string()
            } else {
                path
            };
            Ok(Expr::Field(field))
        }

        StudioExpr::Binary { op, left, right } => {
            let bin_op = parse_binary_op_enum(&op)
                .map_err(|e| ConvertError::Expr(step_id.to_string(), e))?;
            Ok(Expr::Binary {
                op: bin_op,
                left: Box::new(convert_expr(*left, step_id)?),
                right: Box::new(convert_expr(*right, step_id)?),
            })
        }

        StudioExpr::Unary { op, operand } => {
            let unary_op = match op.as_str() {
                "not" | "!" => UnaryOp::Not,
                "neg" | "-" => UnaryOp::Neg,
                other => {
                    return Err(ConvertError::Expr(
                        step_id.to_string(),
                        format!("unknown unary op: {}", other),
                    ))
                }
            };
            Ok(Expr::Unary {
                op: unary_op,
                operand: Box::new(convert_expr(*operand, step_id)?),
            })
        }

        StudioExpr::Function { name, args } => {
            let converted: Result<Vec<Expr>, _> =
                args.into_iter().map(|a| convert_expr(a, step_id)).collect();
            Ok(Expr::Call {
                name,
                args: converted?,
            })
        }

        StudioExpr::Member { object, property } => {
            // Flatten member access into a dotted field path when possible
            match convert_expr(*object, step_id)? {
                Expr::Field(path) => Ok(Expr::Field(format!("{}.{}", path, property))),
                other => Ok(Expr::Binary {
                    op: BinaryOp::Eq, // fallback — shouldn't happen in practice
                    left: Box::new(other),
                    right: Box::new(Expr::Field(property)),
                }),
            }
        }

        StudioExpr::Array { elements } => {
            let converted: Result<Vec<Expr>, _> = elements
                .into_iter()
                .map(|e| convert_expr(e, step_id))
                .collect();
            Ok(Expr::Array(converted?))
        }

        StudioExpr::Object { entries } => {
            let converted: Result<Vec<(String, Expr)>, _> = entries
                .into_iter()
                .map(|(k, v)| convert_expr(v, step_id).map(|e| (k, e)))
                .collect();
            Ok(Expr::Object(converted?))
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn json_to_core_value(v: serde_json::Value) -> Result<CoreValue, String> {
    serde_json::from_value(v).map_err(|e| e.to_string())
}

fn parse_binary_op_enum(op: &str) -> Result<BinaryOp, String> {
    match map_binary_op(op) {
        Some("+") => Ok(BinaryOp::Add),
        Some("-") => Ok(BinaryOp::Sub),
        Some("*") => Ok(BinaryOp::Mul),
        Some("/") => Ok(BinaryOp::Div),
        Some("%") => Ok(BinaryOp::Mod),
        Some("==") => Ok(BinaryOp::Eq),
        Some("!=") => Ok(BinaryOp::Ne),
        Some(">") => Ok(BinaryOp::Gt),
        Some(">=") => Ok(BinaryOp::Ge),
        Some("<") => Ok(BinaryOp::Lt),
        Some("<=") => Ok(BinaryOp::Le),
        Some("&&") => Ok(BinaryOp::And),
        Some("||") => Ok(BinaryOp::Or),
        Some("in") => Ok(BinaryOp::In),
        Some("not in") => Ok(BinaryOp::NotIn),
        Some("contains") => Ok(BinaryOp::Contains),
        _ => Err(format!("unknown binary op: {}", binary_op_display(op))),
    }
}

fn expr_to_log_message(expr: StudioExpr) -> String {
    match &expr {
        StudioExpr::Literal {
            value: serde_json::Value::String(s),
            ..
        } => s.clone(),
        other => expr_to_string(other),
    }
}

fn terminal_message_to_engine_string(message: Option<StudioTerminalMessage>) -> String {
    match message {
        None => String::new(),
        Some(StudioTerminalMessage::String(message)) => message,
        Some(StudioTerminalMessage::Expr(StudioExpr::Literal {
            value: serde_json::Value::String(message),
            ..
        })) => message,
        Some(StudioTerminalMessage::Expr(expr)) => expr_to_string(&expr),
    }
}

fn parse_log_level(level: Option<&str>) -> LogLevel {
    match level {
        Some("debug") => LogLevel::Debug,
        Some("warn") => LogLevel::Warn,
        Some("error") => LogLevel::Error,
        _ => LogLevel::Info,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        condition::StudioCondition,
        expr::StudioExpr,
        ruleset::{StudioConfig, StudioRuleSet},
        step::{StudioAssignment, StudioBranch, StudioStep, StudioStepKind, StudioTerminalMessage},
    };

    fn base_config(name: &str) -> StudioConfig {
        StudioConfig {
            name: name.to_string(),
            version: Some("1.0.0".to_string()),
            description: None,
            tags: None,
            enable_trace: None,
            timeout: None,
            input_schema: None,
            output_schema: None,
            metadata: Default::default(),
        }
    }

    fn terminal_step(id: &str, code: &str) -> StudioStep {
        StudioStep {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            position: None,
            kind: StudioStepKind::Terminal {
                code: code.to_string(),
                message: None,
                output: vec![],
            },
        }
    }

    #[test]
    fn test_terminal_message_accepts_expression_object() {
        let rs = StudioRuleSet {
            config: base_config("terminal_message_expr"),
            start_step_id: "done".to_string(),
            steps: vec![StudioStep {
                id: "done".to_string(),
                name: "Done".to_string(),
                description: None,
                position: None,
                kind: StudioStepKind::Terminal {
                    code: "OK".to_string(),
                    message: Some(StudioTerminalMessage::Expr(StudioExpr::Literal {
                        value: serde_json::Value::String("expr message".to_string()),
                        value_type: Some("string".to_string()),
                    })),
                    output: vec![],
                },
            }],
            groups: None,
            metadata: None,
            sub_rules: Default::default(),
        };

        let converted = RuleSet::try_from(rs).expect("studio ruleset should convert");
        let step = converted
            .steps
            .get("done")
            .expect("terminal step should exist");
        match &step.kind {
            StepKind::Terminal { result } => {
                assert_eq!(result.code, "OK");
                assert_eq!(result.message, "expr message");
            }
            other => panic!("expected terminal step, got {other:?}"),
        }
    }

    #[test]
    fn test_terminal_message_accepts_legacy_string() {
        let rs = StudioRuleSet {
            config: base_config("terminal_message_string"),
            start_step_id: "done".to_string(),
            steps: vec![StudioStep {
                id: "done".to_string(),
                name: "Done".to_string(),
                description: None,
                position: None,
                kind: StudioStepKind::Terminal {
                    code: "OK".to_string(),
                    message: Some(StudioTerminalMessage::String("plain message".to_string())),
                    output: vec![],
                },
            }],
            groups: None,
            metadata: None,
            sub_rules: Default::default(),
        };

        let converted = RuleSet::try_from(rs).expect("studio ruleset should convert");
        let step = converted
            .steps
            .get("done")
            .expect("terminal step should exist");
        match &step.kind {
            StepKind::Terminal { result } => assert_eq!(result.message, "plain message"),
            other => panic!("expected terminal step, got {other:?}"),
        }
    }

    #[test]
    fn test_simple_decision_ruleset() {
        let rs = StudioRuleSet {
            config: base_config("test"),
            start_step_id: "decide".to_string(),
            steps: vec![
                StudioStep {
                    id: "decide".to_string(),
                    name: "Decide".to_string(),
                    description: None,
                    position: None,
                    kind: StudioStepKind::Decision {
                        branches: vec![StudioBranch {
                            id: "b1".to_string(),
                            label: None,
                            condition: StudioCondition::Simple {
                                left: StudioExpr::Variable {
                                    path: "$.age".to_string(),
                                },
                                operator: "gte".to_string(),
                                right: StudioExpr::Literal {
                                    value: serde_json::Value::Number(18.into()),
                                    value_type: None,
                                },
                            },
                            next_step_id: "adult".to_string(),
                        }],
                        default_next_step_id: Some("child".to_string()),
                    },
                },
                terminal_step("adult", "ADULT"),
                terminal_step("child", "CHILD"),
            ],
            sub_rules: Default::default(),
            groups: None,
            metadata: None,
        };

        let engine: RuleSet = rs.try_into().unwrap();
        assert_eq!(engine.config.entry_step, "decide");
        assert_eq!(engine.steps.len(), 3);

        let decide = engine.steps.get("decide").unwrap();
        match &decide.kind {
            StepKind::Decision {
                branches,
                default_next,
            } => {
                assert_eq!(branches.len(), 1);
                assert_eq!(default_next.as_deref(), Some("child"));
                match &branches[0].condition {
                    Condition::ExpressionString(s) => assert_eq!(s, "age >= 18"),
                    _ => panic!("expected ExpressionString"),
                }
            }
            _ => panic!("expected Decision"),
        }
    }

    #[test]
    fn test_action_step_assignment() {
        let rs = StudioRuleSet {
            config: base_config("test_action"),
            start_step_id: "act".to_string(),
            steps: vec![
                StudioStep {
                    id: "act".to_string(),
                    name: "Act".to_string(),
                    description: None,
                    position: None,
                    kind: StudioStepKind::Action {
                        assignments: vec![StudioAssignment {
                            name: "result".to_string(),
                            value: StudioExpr::Literal {
                                value: serde_json::Value::Number(42.into()),
                                value_type: None,
                            },
                        }],
                        external_calls: vec![],
                        logging: None,
                        next_step_id: "done".to_string(),
                    },
                },
                terminal_step("done", "DONE"),
            ],
            sub_rules: Default::default(),
            groups: None,
            metadata: None,
        };

        let engine: RuleSet = rs.try_into().unwrap();
        let act = engine.steps.get("act").unwrap();
        match &act.kind {
            StepKind::Action { actions, next_step } => {
                assert_eq!(next_step, "done");
                assert_eq!(actions.len(), 1);
                match &actions[0].kind {
                    ActionKind::SetVariable {
                        name,
                        value: Expr::Literal(v),
                    } => {
                        assert_eq!(name, "result");
                        // JSON integer 42 deserializes as CoreValue::Int
                        assert!(
                            matches!(v, CoreValue::Int(42))
                                || matches!(v, CoreValue::Float(f) if (*f - 42.0).abs() < f64::EPSILON)
                        );
                    }
                    _ => panic!("expected SetVariable"),
                }
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn test_missing_start_step_error() {
        let rs = StudioRuleSet {
            config: base_config("bad"),
            start_step_id: String::new(),
            steps: vec![],
            sub_rules: Default::default(),
            groups: None,
            metadata: None,
        };
        assert!(matches!(
            RuleSet::try_from(rs),
            Err(ConvertError::MissingStartStep)
        ));
    }

    #[test]
    fn test_logical_condition_to_string() {
        use crate::types::condition::condition_to_expr_string;

        let cond = StudioCondition::Logical {
            operator: "and".to_string(),
            conditions: vec![
                StudioCondition::Simple {
                    left: StudioExpr::Variable {
                        path: "$.age".to_string(),
                    },
                    operator: "gt".to_string(),
                    right: StudioExpr::Literal {
                        value: 18.into(),
                        value_type: None,
                    },
                },
                StudioCondition::Simple {
                    left: StudioExpr::Variable {
                        path: "$.active".to_string(),
                    },
                    operator: "eq".to_string(),
                    right: StudioExpr::Literal {
                        value: true.into(),
                        value_type: None,
                    },
                },
            ],
        };

        let s = condition_to_expr_string(&cond);
        assert_eq!(s, "(age > 18 && active == true)");
    }
}
