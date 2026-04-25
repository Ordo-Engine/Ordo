//! Ruleset compiler to bytecode-based compiled ruleset

use super::compiled::{
    CompiledAction, CompiledBranch, CompiledCondition, CompiledMetadata, CompiledOutput,
    CompiledRuleSet, CompiledStep, CompiledSubRuleBinding, CompiledSubRuleGraph,
    CompiledSubRuleOutput,
};
use super::model::{FieldMissingBehavior, RuleSet};
use super::step::{ActionKind, Condition, LogLevel, Step, StepKind, SubRuleGraph, TerminalResult};
use crate::context::Value;
use crate::error::{OrdoError, Result};
use crate::expr::{Expr, ExprCompiler, ExprParser};
use std::collections::HashMap;

pub struct RuleSetCompiler;

impl RuleSetCompiler {
    pub fn compile(ruleset: &RuleSet) -> Result<CompiledRuleSet> {
        let mut string_pool = StringPool::new();
        let metadata = CompiledMetadata {
            name: string_pool.intern(&ruleset.config.name),
            tenant_id: ruleset
                .config
                .tenant_id
                .as_ref()
                .map(|v| string_pool.intern(v)),
            version: string_pool.intern(&ruleset.config.version),
            description: string_pool.intern(&ruleset.config.description),
            field_missing: field_missing_tag(ruleset.config.field_missing),
            max_depth: ruleset.config.max_depth as u32,
            timeout_ms: ruleset.config.timeout_ms,
            enable_trace: ruleset.config.enable_trace,
            metadata: ruleset
                .config
                .metadata
                .iter()
                .map(|(k, v)| (string_pool.intern(k), string_pool.intern(v)))
                .collect(),
        };

        let mut expressions = Vec::new();
        let mut steps = Vec::with_capacity(ruleset.steps.len());
        let mut step_hashes = HashMap::new();
        let mut sub_rule_names = HashMap::new();

        for step_id in ruleset.steps.keys() {
            step_hashes.insert(step_id.as_str(), hash_step_id(step_id));
        }
        for name in ruleset.sub_rules.keys() {
            sub_rule_names.insert(name.clone(), string_pool.intern(name));
        }

        // Check for hash collisions
        check_hash_collisions(&step_hashes)?;

        for step in ruleset.steps.values() {
            steps.push(compile_step(
                step,
                &step_hashes,
                &sub_rule_names,
                &mut expressions,
                &mut string_pool,
            )?);
        }

        let mut sub_rules = HashMap::with_capacity(ruleset.sub_rules.len());
        for (name, graph) in &ruleset.sub_rules {
            let name_idx = *sub_rule_names.get(name).ok_or_else(|| {
                OrdoError::parse_error(format!("Sub-rule '{}' not interned", name))
            })?;
            let compiled =
                compile_sub_rule_graph(graph, &sub_rule_names, &mut expressions, &mut string_pool)?;
            sub_rules.insert(name_idx, compiled);
        }

        let entry_step = step_hashes
            .get(ruleset.config.entry_step.as_str())
            .copied()
            .ok_or_else(|| OrdoError::StepNotFound {
                step_id: ruleset.config.entry_step.clone(),
            })?;

        Ok(CompiledRuleSet::new(
            metadata,
            entry_step,
            steps,
            expressions,
            string_pool.into_vec(),
        )
        .with_sub_rules(sub_rules))
    }
}

fn compile_step(
    step: &Step,
    step_hashes: &HashMap<&str, u32>,
    sub_rule_names: &HashMap<String, u32>,
    expressions: &mut Vec<crate::expr::CompiledExpr>,
    string_pool: &mut StringPool,
) -> Result<CompiledStep> {
    let id_hash = step_hashes[step.id.as_str()];
    match &step.kind {
        StepKind::Decision {
            branches,
            default_next,
        } => {
            let mut compiled_branches = Vec::with_capacity(branches.len());
            for branch in branches {
                let condition = compile_condition(&branch.condition, expressions)?;
                let next_step = *step_hashes.get(branch.next_step.as_str()).ok_or_else(|| {
                    OrdoError::StepNotFound {
                        step_id: branch.next_step.clone(),
                    }
                })?;
                let actions = compile_actions(&branch.actions, expressions, string_pool)?;
                compiled_branches.push(CompiledBranch {
                    condition,
                    next_step,
                    actions,
                });
            }
            let default_next =
                match default_next {
                    Some(id) => Some(*step_hashes.get(id.as_str()).ok_or_else(|| {
                        OrdoError::StepNotFound {
                            step_id: id.clone(),
                        }
                    })?),
                    None => None,
                };
            Ok(CompiledStep::Decision {
                id_hash,
                branches: compiled_branches,
                default_next,
            })
        }
        StepKind::Action { actions, next_step } => {
            let compiled_actions = compile_actions(actions, expressions, string_pool)?;
            let next_step_hash =
                *step_hashes
                    .get(next_step.as_str())
                    .ok_or_else(|| OrdoError::StepNotFound {
                        step_id: next_step.clone(),
                    })?;
            Ok(CompiledStep::Action {
                id_hash,
                actions: compiled_actions,
                next_step: next_step_hash,
            })
        }
        StepKind::Terminal { result } => {
            let compiled = compile_terminal(result, expressions, string_pool)?;
            Ok(CompiledStep::Terminal {
                id_hash,
                code: compiled.code,
                message: compiled.message,
                outputs: compiled.outputs,
                data: compiled.data,
            })
        }
        StepKind::SubRule {
            ref_name,
            bindings,
            outputs,
            next_step,
        } => {
            let ref_name = *sub_rule_names.get(ref_name).ok_or_else(|| {
                OrdoError::parse_error(format!("Sub-rule '{}' not found", ref_name))
            })?;
            let bindings = bindings
                .iter()
                .map(|(name, expr)| CompiledSubRuleBinding {
                    name: string_pool.intern(name),
                    expr: compile_expr(expr, expressions),
                })
                .collect();
            let outputs = outputs
                .iter()
                .map(|(parent_variable, child_variable)| CompiledSubRuleOutput {
                    parent_variable: string_pool.intern(parent_variable),
                    child_variable: string_pool.intern(child_variable),
                })
                .collect();
            let next_step =
                *step_hashes
                    .get(next_step.as_str())
                    .ok_or_else(|| OrdoError::StepNotFound {
                        step_id: next_step.clone(),
                    })?;
            Ok(CompiledStep::SubRule {
                id_hash,
                ref_name,
                bindings,
                outputs,
                next_step,
            })
        }
    }
}

fn compile_sub_rule_graph(
    graph: &SubRuleGraph,
    sub_rule_names: &HashMap<String, u32>,
    expressions: &mut Vec<crate::expr::CompiledExpr>,
    string_pool: &mut StringPool,
) -> Result<CompiledSubRuleGraph> {
    let mut step_hashes = HashMap::new();
    for step_id in graph.steps.keys() {
        step_hashes.insert(step_id.as_str(), hash_step_id(step_id));
    }
    check_hash_collisions(&step_hashes)?;

    let mut steps = Vec::with_capacity(graph.steps.len());
    for step in graph.steps.values() {
        steps.push(compile_step(
            step,
            &step_hashes,
            sub_rule_names,
            expressions,
            string_pool,
        )?);
    }

    let entry_step = step_hashes
        .get(graph.entry_step.as_str())
        .copied()
        .ok_or_else(|| OrdoError::StepNotFound {
            step_id: graph.entry_step.clone(),
        })?;

    Ok(CompiledSubRuleGraph::new(entry_step, steps))
}

fn compile_condition(
    condition: &Condition,
    expressions: &mut Vec<crate::expr::CompiledExpr>,
) -> Result<CompiledCondition> {
    match condition {
        Condition::Always => Ok(CompiledCondition::Always),
        Condition::Expression(expr) => {
            let idx = compile_expr(expr, expressions);
            Ok(CompiledCondition::Expr(idx))
        }
        Condition::ExpressionString(s) => {
            let expr = ExprParser::parse(s)?;
            let idx = compile_expr(&expr, expressions);
            Ok(CompiledCondition::Expr(idx))
        }
    }
}

fn compile_expr(expr: &Expr, expressions: &mut Vec<crate::expr::CompiledExpr>) -> u32 {
    let compiled = ExprCompiler::new().compile(expr);
    expressions.push(compiled);
    (expressions.len() - 1) as u32
}

fn compile_actions(
    actions: &[super::step::Action],
    expressions: &mut Vec<crate::expr::CompiledExpr>,
    string_pool: &mut StringPool,
) -> Result<Vec<CompiledAction>> {
    let mut compiled = Vec::with_capacity(actions.len());
    for action in actions {
        match &action.kind {
            ActionKind::SetVariable { name, value } => {
                let name_idx = string_pool.intern(name);
                let expr_idx = compile_expr(value, expressions);
                compiled.push(CompiledAction::SetVariable {
                    name: name_idx,
                    value: expr_idx,
                });
            }
            ActionKind::Log { message, level } => {
                let message_idx = string_pool.intern(message);
                compiled.push(CompiledAction::Log {
                    message: message_idx,
                    level: log_level_tag(*level),
                });
            }
            ActionKind::Metric { name, value, tags } => {
                let name_idx = string_pool.intern(name);
                let expr_idx = compile_expr(value, expressions);
                let mut compiled_tags = Vec::with_capacity(tags.len());
                for (k, v) in tags {
                    compiled_tags.push((string_pool.intern(k), string_pool.intern(v)));
                }
                compiled.push(CompiledAction::Metric {
                    name: name_idx,
                    value: expr_idx,
                    tags: compiled_tags,
                });
            }
            ActionKind::CallRuleSet { .. } => {
                return Err(OrdoError::parse_error(
                    "CallRuleSet is not supported in compiled rules",
                ));
            }
            ActionKind::ExternalCall {
                service,
                method,
                params,
                result_variable,
                timeout_ms,
            } => {
                let service_idx = string_pool.intern(service);
                let method_idx = string_pool.intern(method);
                let mut compiled_params = Vec::with_capacity(params.len());
                for (name, expr) in params {
                    compiled_params
                        .push((string_pool.intern(name), compile_expr(expr, expressions)));
                }
                compiled.push(CompiledAction::ExternalCall {
                    service: service_idx,
                    method: method_idx,
                    params: compiled_params,
                    result_variable: result_variable.as_ref().map(|v| string_pool.intern(v)),
                    timeout_ms: *timeout_ms,
                });
            }
        }
    }
    Ok(compiled)
}

struct CompiledTerminal {
    code: u32,
    message: u32,
    outputs: Vec<CompiledOutput>,
    data: Value,
}

fn compile_terminal(
    result: &TerminalResult,
    expressions: &mut Vec<crate::expr::CompiledExpr>,
    string_pool: &mut StringPool,
) -> Result<CompiledTerminal> {
    let code = string_pool.intern(&result.code);
    let message = string_pool.intern(&result.message);
    let mut outputs = Vec::with_capacity(result.output.len());
    for (key, expr) in &result.output {
        let key_idx = string_pool.intern(key);
        let expr_idx = compile_expr(expr, expressions);
        outputs.push(CompiledOutput {
            key: key_idx,
            expr: expr_idx,
        });
    }
    Ok(CompiledTerminal {
        code,
        message,
        outputs,
        data: result.data.clone(),
    })
}

fn field_missing_tag(value: FieldMissingBehavior) -> u8 {
    match value {
        FieldMissingBehavior::Lenient => 0,
        FieldMissingBehavior::Strict => 1,
        FieldMissingBehavior::Default => 2,
    }
}

fn log_level_tag(value: LogLevel) -> u8 {
    match value {
        LogLevel::Debug => 0,
        LogLevel::Info => 1,
        LogLevel::Warn => 2,
        LogLevel::Error => 3,
    }
}

/// Hash step_id using FNV-1a algorithm.
/// Note: This is a 32-bit hash, collision is possible but unlikely for typical step counts.
/// For production use with many steps, consider using a collision detection mechanism.
fn hash_step_id(value: &str) -> u32 {
    let mut hash: u32 = 0x811C9DC5;
    for byte in value.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Check for hash collisions in step_hashes
fn check_hash_collisions(step_hashes: &HashMap<&str, u32>) -> Result<()> {
    let mut seen: HashMap<u32, &str> = HashMap::new();
    for (step_id, hash) in step_hashes {
        if let Some(existing) = seen.get(hash) {
            return Err(OrdoError::parse_error(format!(
                "Hash collision detected between step '{}' and '{}' (hash: {:08x})",
                existing, step_id, hash
            )));
        }
        seen.insert(*hash, step_id);
    }
    Ok(())
}

struct StringPool {
    map: HashMap<String, u32>,
    values: Vec<String>,
}

impl StringPool {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            values: Vec::new(),
        }
    }

    fn intern(&mut self, value: &str) -> u32 {
        if let Some(idx) = self.map.get(value) {
            return *idx;
        }
        let idx = self.values.len() as u32;
        self.values.push(value.to_string());
        self.map.insert(value.to_string(), idx);
        idx
    }

    fn into_vec(self) -> Vec<String> {
        self.values
    }
}
