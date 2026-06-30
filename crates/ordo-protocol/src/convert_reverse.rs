//! Engine → studio conversion (the inverse of `convert.rs`).
//!
//! The studio (visual editor) format is richer than the engine format — it carries
//! structured conditions, positions, and groups that the engine discards. So this
//! direction is best-effort and *semantically* faithful rather than byte-identical:
//! `engine_to_studio` followed by `StudioRuleSet: TryInto<RuleSet>` reproduces an
//! equivalent engine ruleset, not the original studio document.
//!
//! This is the Rust counterpart of the TypeScript `reverse-adapter.ts`. It is the
//! inverse of `convert.rs` (not a copy of the TS reverse): in particular external
//! calls are mapped as the plain inverse of the studio→engine mapping, so the Rust
//! forward and reverse round-trip cleanly.

use ordo_core::{
    context::Value as CoreValue,
    expr::{BinaryOp, Expr, UnaryOp},
    rule::{ActionKind, Branch, Condition, RuleSet, Step, StepKind, SubRuleGraph},
};

use crate::types::{
    condition::StudioCondition,
    expr::StudioExpr,
    ruleset::{StudioConfig, StudioRuleSet, StudioSubRuleGraph},
    step::{
        StudioAssignment, StudioBranch, StudioExternalCall, StudioLogging, StudioOutputField,
        StudioStep, StudioStepKind, StudioSubRuleBinding, StudioSubRuleOutput,
        StudioTerminalMessage,
    },
};

/// Convert an engine `RuleSet` into the studio format the visual editor consumes.
pub fn engine_to_studio(rs: &RuleSet) -> StudioRuleSet {
    let config = StudioConfig {
        name: rs.config.name.clone(),
        version: Some(rs.config.version.clone()),
        description: if rs.config.description.is_empty() {
            None
        } else {
            Some(rs.config.description.clone())
        },
        tags: None,
        enable_trace: Some(rs.config.enable_trace),
        timeout: Some(rs.config.timeout_ms),
        input_schema: None,
        output_schema: None,
        metadata: rs
            .config
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    };

    StudioRuleSet {
        config,
        start_step_id: rs.config.entry_step.clone(),
        steps: ordered_steps(rs.steps.iter(), &rs.config.entry_step),
        sub_rules: rs
            .sub_rules
            .iter()
            .map(|(name, graph)| (name.clone(), sub_rule_to_studio(graph)))
            .collect(),
        groups: None,
        metadata: None,
    }
}

/// Deterministic step ordering: the entry step first, then the rest sorted by id.
/// (The engine stores steps in an unordered map; studio uses `start_step_id` for
/// the entry, so order is purely cosmetic — we make it stable for tests/diffs.)
fn ordered_steps<'a, I>(steps: I, entry: &str) -> Vec<StudioStep>
where
    I: Iterator<Item = (&'a String, &'a Step)>,
{
    let mut collected: Vec<(&String, &Step)> = steps.collect();
    collected.sort_by(|(a, _), (b, _)| {
        let a_entry = a.as_str() == entry;
        let b_entry = b.as_str() == entry;
        b_entry.cmp(&a_entry).then_with(|| a.cmp(b))
    });
    collected
        .into_iter()
        .map(|(_, step)| step_to_studio(step))
        .collect()
}

fn sub_rule_to_studio(graph: &SubRuleGraph) -> StudioSubRuleGraph {
    StudioSubRuleGraph {
        entry_step: graph.entry_step.clone(),
        steps: ordered_steps(graph.steps.iter(), &graph.entry_step),
        input_schema: None,
        output_schema: None,
    }
}

fn step_to_studio(step: &Step) -> StudioStep {
    StudioStep {
        id: step.id.clone(),
        name: step.name.clone(),
        description: None,
        position: None,
        system_generated: None,
        kind: step_kind_to_studio(&step.id, &step.kind),
    }
}

fn step_kind_to_studio(step_id: &str, kind: &StepKind) -> StudioStepKind {
    match kind {
        StepKind::Decision {
            branches,
            default_next,
        } => StudioStepKind::Decision {
            branches: branches
                .iter()
                .enumerate()
                .map(|(i, b)| branch_to_studio(step_id, i, b))
                .collect(),
            default_next_step_id: default_next.clone(),
        },

        StepKind::Action { actions, next_step } => {
            let mut assignments = Vec::new();
            let mut external_calls = Vec::new();
            let mut logging = None;

            for action in actions {
                match &action.kind {
                    ActionKind::SetVariable { name, value } => assignments.push(StudioAssignment {
                        name: name.clone(),
                        value: expr_to_studio(value),
                    }),
                    ActionKind::ExternalCall {
                        service,
                        method,
                        params,
                        result_variable,
                        timeout_ms,
                    } => external_calls.push(StudioExternalCall {
                        call_type: method.clone(),
                        target: service.clone(),
                        params: params
                            .iter()
                            .map(|(k, v)| (k.clone(), expr_to_studio(v)))
                            .collect(),
                        result_variable: result_variable.clone(),
                        timeout: Some(*timeout_ms),
                    }),
                    ActionKind::Log { message, level } => {
                        logging = Some(StudioLogging {
                            message: StudioExpr::Literal {
                                value: serde_json::Value::String(message.clone()),
                                value_type: Some("string".to_string()),
                            },
                            level: Some(log_level_to_string(*level)),
                        });
                    }
                    // Metric / CallRuleSet have no studio representation — they are
                    // never produced by studio→engine, so dropping them on the
                    // reverse path is lossless for studio-authored rulesets.
                    _ => {}
                }
            }

            StudioStepKind::Action {
                assignments,
                external_calls,
                logging,
                next_step_id: next_step.clone(),
            }
        }

        StepKind::Terminal { result } => StudioStepKind::Terminal {
            code: result.code.clone(),
            message: if result.message.is_empty() {
                None
            } else {
                Some(StudioTerminalMessage::String(result.message.clone()))
            },
            output: result
                .output
                .iter()
                .map(|(name, expr)| StudioOutputField {
                    name: name.clone(),
                    value: expr_to_studio(expr),
                })
                .collect(),
        },

        StepKind::SubRule {
            ref_name,
            bindings,
            outputs,
            next_step,
        } => StudioStepKind::SubRule {
            ref_name: ref_name.clone(),
            bindings: bindings
                .iter()
                .map(|(field, expr)| StudioSubRuleBinding {
                    field: field.clone(),
                    expr: expr_to_studio(expr),
                })
                .collect(),
            outputs: outputs
                .iter()
                .map(|(parent_var, child_var)| StudioSubRuleOutput {
                    parent_var: parent_var.clone(),
                    child_var: child_var.clone(),
                })
                .collect(),
            return_policy: Some(if next_step.is_empty() {
                "propagate_terminal".to_string()
            } else {
                "continue".to_string()
            }),
            next_step_id: next_step.clone(),
        },
    }
}

fn branch_to_studio(step_id: &str, index: usize, branch: &Branch) -> StudioBranch {
    let cond_string = condition_to_string(&branch.condition);
    StudioBranch {
        // Deterministic, unique-within-step id (avoids nondeterministic clocks).
        id: format!("{step_id}-b{index}"),
        // Keep the raw condition string as a hedge against lossy re-parsing
        // (mirrors reverse-adapter.ts, which stashes it in `label`).
        label: Some(cond_string.clone()),
        condition: parse_condition_string(&cond_string),
        next_step_id: branch.next_step.clone(),
    }
}

/// Stringify any engine `Condition` into a parseable expression string.
fn condition_to_string(cond: &Condition) -> String {
    match cond {
        Condition::Always => "true".to_string(),
        Condition::ExpressionString(s) => s.clone(),
        Condition::Expression(expr) => engine_expr_to_string(expr),
    }
}

// ── Condition string → StudioCondition (port of reverse-adapter.ts) ─────────────

/// Re-parse an engine expression string into a structured `StudioCondition`.
///
/// Handles top-level `&&`/`||`, outer-paren stripping, and the six comparison
/// operators (the patterns the studio condition builder can represent); anything
/// else falls back to a raw `Expression` (lossless — `condition_to_expr_string`
/// returns the string verbatim). Faithful port of `parseConditionString`.
pub fn parse_condition_string(expr: &str) -> StudioCondition {
    let trimmed = expr.trim();
    if trimmed.is_empty() || trimmed == "true" {
        return StudioCondition::Expression {
            expression: if trimmed.is_empty() {
                "true".to_string()
            } else {
                trimmed.to_string()
            },
        };
    }

    // Strip outer parens only when they truly wrap the whole expression.
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = trimmed[1..trimmed.len() - 1].trim();
        if paren_depth_zero_after_open(inner) {
            return parse_condition_string(inner);
        }
    }

    let and_parts = split_top_level(trimmed, "&&");
    if and_parts.len() > 1 {
        return StudioCondition::Logical {
            operator: "and".to_string(),
            conditions: and_parts
                .iter()
                .map(|p| parse_condition_string(p))
                .collect(),
        };
    }

    let or_parts = split_top_level(trimmed, "||");
    if or_parts.len() > 1 {
        return StudioCondition::Logical {
            operator: "or".to_string(),
            conditions: or_parts.iter().map(|p| parse_condition_string(p)).collect(),
        };
    }

    // Simple binary "left op right" — longest operators first.
    for (op, editor_op) in [
        (">=", "gte"),
        ("<=", "lte"),
        ("!=", "ne"),
        ("==", "eq"),
        (">", "gt"),
        ("<", "lt"),
    ] {
        if let Some(idx) = trimmed.find(op) {
            let left = trimmed[..idx].trim();
            let right = trimmed[idx + op.len()..].trim();
            if !left.is_empty()
                && !right.is_empty()
                && !left.contains(['(', ')', '&', '|'])
                && !right.contains(['(', ')', '&', '|'])
            {
                return StudioCondition::Simple {
                    left: parse_value_token(left),
                    operator: editor_op.to_string(),
                    right: parse_value_token(right),
                };
            }
        }
    }

    StudioCondition::Expression {
        expression: trimmed.to_string(),
    }
}

/// Split on a top-level `&&`/`||` (depth 0, whitespace-bounded). Returns a single
/// element when the operator is not found at the top level.
fn split_top_level(expr: &str, op: &str) -> Vec<String> {
    let bytes = expr.as_bytes();
    let op_len = op.len();
    let mut parts: Vec<String> = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {
                if depth == 0 && expr[i..].starts_with(op) {
                    let before = if i == 0 { b' ' } else { bytes[i - 1] };
                    let after = bytes.get(i + op_len).copied().unwrap_or(b' ');
                    if before == b' ' && after == b' ' {
                        parts.push(expr[start..i].trim().to_string());
                        start = i + op_len;
                        i += op_len;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    parts.push(expr[start..].trim().to_string());
    let non_empty: Vec<String> = parts.iter().filter(|p| !p.is_empty()).cloned().collect();
    if non_empty.len() > 1 {
        non_empty
    } else {
        vec![expr.to_string()]
    }
}

fn paren_depth_zero_after_open(s: &str) -> bool {
    let mut depth: i32 = 0;
    for ch in s.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            if depth == 0 {
                return false;
            }
            depth -= 1;
        }
    }
    depth == 0
}

fn parse_value_token(token: &str) -> StudioExpr {
    let t = token.trim();
    // Quoted string literal
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        return StudioExpr::Literal {
            value: serde_json::Value::String(t[1..t.len() - 1].to_string()),
            value_type: Some("string".to_string()),
        };
    }
    // Numeric literal — preserve integers as integers (so "18" round-trips to
    // "18", not "18.0"). Only treat purely numeric tokens as numbers.
    if !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        if let Ok(i) = t.parse::<i64>() {
            return StudioExpr::Literal {
                value: serde_json::Value::Number(i.into()),
                value_type: Some("number".to_string()),
            };
        }
        if let Ok(f) = t.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(f) {
                return StudioExpr::Literal {
                    value: serde_json::Value::Number(num),
                    value_type: Some("number".to_string()),
                };
            }
        }
    }
    match t {
        "true" => StudioExpr::Literal {
            value: serde_json::Value::Bool(true),
            value_type: Some("boolean".to_string()),
        },
        "false" => StudioExpr::Literal {
            value: serde_json::Value::Bool(false),
            value_type: Some("boolean".to_string()),
        },
        "null" => StudioExpr::Literal {
            value: serde_json::Value::Null,
            value_type: Some("null".to_string()),
        },
        // Field reference (dotted path like user.age) — kept raw, matching
        // parseValueToken and the studio→engine forward (no "$." re-prefixing).
        _ => StudioExpr::Variable {
            path: t.to_string(),
        },
    }
}

// ── Engine Expr → StudioExpr (inverse of convert_expr) ──────────────────────────

fn expr_to_studio(expr: &Expr) -> StudioExpr {
    match expr {
        Expr::Literal(v) => StudioExpr::Literal {
            value: core_value_to_json(v),
            value_type: None,
        },
        Expr::Field(path) => StudioExpr::Variable { path: path.clone() },
        Expr::Binary { op, left, right } => StudioExpr::Binary {
            op: binary_op_word(*op).to_string(),
            left: Box::new(expr_to_studio(left)),
            right: Box::new(expr_to_studio(right)),
        },
        Expr::Unary { op, operand } => StudioExpr::Unary {
            op: match op {
                UnaryOp::Not => "not".to_string(),
                UnaryOp::Neg => "neg".to_string(),
            },
            operand: Box::new(expr_to_studio(operand)),
        },
        Expr::Call { name, args } => StudioExpr::Function {
            name: name.clone(),
            args: args.iter().map(expr_to_studio).collect(),
        },
        Expr::Array(elements) => StudioExpr::Array {
            elements: elements.iter().map(expr_to_studio).collect(),
        },
        Expr::Object(entries) => StudioExpr::Object {
            entries: entries
                .iter()
                .map(|(k, v)| (k.clone(), expr_to_studio(v)))
                .collect(),
        },
        // No direct studio representation — map to function-call shapes so they at
        // least survive as editable function expressions (rare in studio-authored
        // rulesets, which never produce these).
        Expr::Exists(field) => StudioExpr::Function {
            name: "exists".to_string(),
            args: vec![StudioExpr::Variable {
                path: field.clone(),
            }],
        },
        Expr::Coalesce(args) => StudioExpr::Function {
            name: "coalesce".to_string(),
            args: args.iter().map(expr_to_studio).collect(),
        },
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => StudioExpr::Function {
            name: "if".to_string(),
            args: vec![
                expr_to_studio(condition),
                expr_to_studio(then_branch),
                expr_to_studio(else_branch),
            ],
        },
    }
}

// ── Engine Expr → string (port of engineExprAstToString) ────────────────────────

fn engine_expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Literal(v) => core_value_to_literal_string(v),
        Expr::Field(path) => path.clone(),
        Expr::Binary { op, left, right } => format!(
            "{} {} {}",
            engine_expr_to_string(left),
            binary_op_symbol(*op),
            engine_expr_to_string(right)
        ),
        Expr::Unary { op, operand } => match op {
            UnaryOp::Not => format!("!{}", engine_expr_to_string(operand)),
            UnaryOp::Neg => format!("-{}", engine_expr_to_string(operand)),
        },
        Expr::Call { name, args } => {
            let a: Vec<String> = args.iter().map(engine_expr_to_string).collect();
            format!("{}({})", name, a.join(", "))
        }
        Expr::Exists(field) => format!("exists({field})"),
        Expr::Coalesce(args) => {
            let a: Vec<String> = args.iter().map(engine_expr_to_string).collect();
            format!("coalesce({})", a.join(", "))
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} then {} else {}",
            engine_expr_to_string(condition),
            engine_expr_to_string(then_branch),
            engine_expr_to_string(else_branch)
        ),
        Expr::Array(elements) => {
            let e: Vec<String> = elements.iter().map(engine_expr_to_string).collect();
            format!("[{}]", e.join(", "))
        }
        Expr::Object(_) => "null".to_string(),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

fn core_value_to_json(v: &CoreValue) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

fn core_value_to_literal_string(v: &CoreValue) -> String {
    json_to_literal_string(&core_value_to_json(v))
}

fn json_to_literal_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_literal_string).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(_) => "null".to_string(),
    }
}

/// Engine BinaryOp → studio word form (eq/gte/and/...), matching the studio editor.
fn binary_op_word(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "mod",
        BinaryOp::Eq => "eq",
        BinaryOp::Ne => "ne",
        BinaryOp::Lt => "lt",
        BinaryOp::Le => "lte",
        BinaryOp::Gt => "gt",
        BinaryOp::Ge => "gte",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::In => "in",
        BinaryOp::NotIn => "not_in",
        BinaryOp::Contains => "contains",
    }
}

/// Engine BinaryOp → parseable symbol form (==/>=/&&/...).
fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::In => "in",
        BinaryOp::NotIn => "not in",
        BinaryOp::Contains => "contains",
    }
}

fn log_level_to_string(level: ordo_core::rule::LogLevel) -> String {
    use ordo_core::rule::LogLevel;
    match level {
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ordo_core::rule::RuleSet;

    const ENGINE_JSON: &str = r#"{
      "config": { "name": "demo", "version": "1.2.3", "entry_step": "decide" },
      "steps": {
        "decide": {
          "id": "decide", "name": "Decide", "type": "decision",
          "branches": [{ "condition": "age >= 18 && status == \"active\"", "next_step": "adult" }],
          "default_next": "child"
        },
        "adult": {
          "id": "adult", "name": "Adult", "type": "terminal",
          "result": { "code": "ADULT", "message": "ok", "output": [["tier", { "Literal": "gold" }]] }
        },
        "child": { "id": "child", "name": "Child", "type": "terminal", "result": { "code": "CHILD" } }
      }
    }"#;

    #[test]
    fn engine_to_studio_shapes() {
        let rs = RuleSet::from_json(ENGINE_JSON).unwrap();
        let studio = engine_to_studio(&rs);

        assert_eq!(studio.start_step_id, "decide");
        assert_eq!(studio.steps.len(), 3);
        // Entry step is first.
        assert_eq!(studio.steps[0].id, "decide");

        match &studio.steps[0].kind {
            StudioStepKind::Decision {
                branches,
                default_next_step_id,
            } => {
                assert_eq!(default_next_step_id.as_deref(), Some("child"));
                assert_eq!(branches.len(), 1);
                // Top-level "&&" parses into a logical condition; raw string kept in label.
                assert_eq!(branches[0].next_step_id, "adult");
                assert_eq!(
                    branches[0].label.as_deref(),
                    Some("age >= 18 && status == \"active\"")
                );
                match &branches[0].condition {
                    StudioCondition::Logical {
                        operator,
                        conditions,
                    } => {
                        assert_eq!(operator, "and");
                        assert_eq!(conditions.len(), 2);
                    }
                    other => panic!("expected logical condition, got {other:?}"),
                }
            }
            other => panic!("expected decision, got {other:?}"),
        }
    }

    #[test]
    fn round_trip_engine_studio_engine_is_semantic() {
        let rs = RuleSet::from_json(ENGINE_JSON).unwrap();
        let studio = engine_to_studio(&rs);
        let rs2: RuleSet = studio.try_into().expect("studio converts back to engine");

        assert_eq!(rs2.config.entry_step, "decide");
        assert_eq!(rs2.config.name, "demo");
        assert_eq!(rs2.config.version, "1.2.3");
        assert_eq!(rs2.steps.len(), 3);

        // Branch condition survives the round trip as a parseable string.
        match &rs2.steps.get("decide").unwrap().kind {
            StepKind::Decision { branches, .. } => match &branches[0].condition {
                Condition::ExpressionString(s) => {
                    assert_eq!(s, "(age >= 18 && status == \"active\")")
                }
                other => panic!("expected ExpressionString, got {other:?}"),
            },
            _ => panic!("expected decision"),
        }

        // Terminal output expression survives.
        match &rs2.steps.get("adult").unwrap().kind {
            StepKind::Terminal { result } => {
                assert_eq!(result.code, "ADULT");
                assert_eq!(result.message, "ok");
                assert_eq!(result.output.len(), 1);
                assert_eq!(result.output[0].0, "tier");
                match &result.output[0].1 {
                    Expr::Literal(v) => {
                        assert_eq!(serde_json::to_value(v).unwrap(), serde_json::json!("gold"))
                    }
                    other => panic!("expected literal, got {other:?}"),
                }
            }
            _ => panic!("expected terminal"),
        }
    }

    #[test]
    fn parse_condition_simple_binary() {
        match parse_condition_string("age >= 18") {
            StudioCondition::Simple {
                left,
                operator,
                right,
            } => {
                assert_eq!(operator, "gte");
                assert!(matches!(left, StudioExpr::Variable { path } if path == "age"));
                assert!(matches!(
                    right,
                    StudioExpr::Literal { ref value, .. } if *value == serde_json::json!(18)
                ));
            }
            other => panic!("expected simple, got {other:?}"),
        }
    }

    #[test]
    fn parse_condition_logical_and_paren_strip() {
        match parse_condition_string("(a > 1 && b < 2)") {
            StudioCondition::Logical {
                operator,
                conditions,
            } => {
                assert_eq!(operator, "and");
                assert_eq!(conditions.len(), 2);
            }
            other => panic!("expected logical, got {other:?}"),
        }
    }

    #[test]
    fn parse_condition_unhandled_falls_back_to_expression() {
        // `in` / function-style conditions aren't structurally parsed → raw expression.
        match parse_condition_string("tags contains \"vip\"") {
            StudioCondition::Expression { expression } => {
                assert_eq!(expression, "tags contains \"vip\"")
            }
            other => panic!("expected expression fallback, got {other:?}"),
        }
    }
}
