//! Concept materialization — inline named concept expressions into an engine
//! `RuleSet` as a computed prelude step, rewriting references to `$name`.
//!
//! This is the single source of truth for concept resolution, shared by the
//! platform (server-side publish/convert) and the CLI (offline validate/run),
//! so the two never drift. Concepts are pure derived expressions over the
//! input/facts; materializing them here means a local `ordo` run produces the
//! same engine ruleset the platform would.

use std::collections::{HashMap, HashSet};

use ordo_core::expr::{Expr, ExprParser};
use ordo_core::rule::{Action, ActionKind, Condition, RuleSet, Step, StepKind};
use serde::{Deserialize, Serialize};

use crate::types::StudioRuleSet;
use crate::ConvertError;

/// The synthesized action-step id that holds the computed concept prelude.
pub const CONCEPT_PRELUDE_STEP_ID: &str = "__ordo_concepts_prelude";

/// The data type a concept resolves to (mirrors the catalog's fact/concept type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConceptDataType {
    #[default]
    String,
    Number,
    Boolean,
    Date,
    Object,
}

/// A concept definition — a named expression derived from the input/facts.
///
/// This is the *authoring* shape (what a local `concepts.json` holds and what
/// the materializer consumes). The server's DB model carries extra bookkeeping
/// (timestamps) and maps into this at the call boundary; the materializer only
/// reads `name` / `expression` / `dependencies`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConceptDefinition {
    pub name: String,
    pub expression: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_type: Option<ConceptDataType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Errors raised while materializing concepts.
#[derive(Debug, thiserror::Error)]
pub enum ConceptError {
    #[error("Concept dependency cycle detected at '{0}'")]
    Cycle(String),
    #[error("Concept '{concept}' expression failed to parse: {message}")]
    Parse { concept: String, message: String },
    #[error(transparent)]
    Convert(#[from] ConvertError),
}

fn normalize_concept_ref(path: &str) -> String {
    path.strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path)
        .to_string()
}

fn scan_expression_refs(expression: &str, refs: &mut HashSet<String>) {
    let mut chars = expression.char_indices().peekable();
    let mut quote: Option<char> = None;

    while let Some((idx, ch)) = chars.next() {
        if let Some(q) = quote {
            if ch == '\\' {
                chars.next();
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }

        if !(ch.is_ascii_alphabetic() || ch == '_' || ch == '$') {
            continue;
        }

        let mut end = idx + ch.len_utf8();
        while let Some((next_idx, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' || next == '.' || next == '$' {
                end = next_idx + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let token = &expression[idx..end];
        let normalized = normalize_concept_ref(token);
        let next_non_ws = expression[end..].chars().find(|c| !c.is_whitespace());
        if matches!(
            normalized.as_str(),
            "true" | "false" | "null" | "undefined" | "and" | "or" | "not" | "in"
        ) || next_non_ws == Some('(')
        {
            continue;
        }
        refs.insert(normalized);
    }
}

fn collect_expr_refs(expr: &Expr, refs: &mut HashSet<String>) {
    match expr {
        Expr::Field(path) => {
            refs.insert(normalize_concept_ref(path));
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_refs(left, refs);
            collect_expr_refs(right, refs);
        }
        Expr::Unary { operand, .. } => collect_expr_refs(operand, refs),
        Expr::Call { args, .. } | Expr::Array(args) | Expr::Coalesce(args) => {
            for arg in args {
                collect_expr_refs(arg, refs);
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_refs(condition, refs);
            collect_expr_refs(then_branch, refs);
            collect_expr_refs(else_branch, refs);
        }
        Expr::Object(entries) => {
            for (_, value) in entries {
                collect_expr_refs(value, refs);
            }
        }
        Expr::Literal(_) | Expr::Exists(_) => {}
    }
}

fn collect_condition_refs(condition: &Condition, refs: &mut HashSet<String>) {
    match condition {
        Condition::Always => {}
        Condition::Expression(expr) => collect_expr_refs(expr, refs),
        Condition::ExpressionString(expression) => match ExprParser::parse(expression) {
            Ok(expr) => collect_expr_refs(&expr, refs),
            Err(_) => scan_expression_refs(expression, refs),
        },
    }
}

fn collect_action_refs(action: &Action, refs: &mut HashSet<String>) {
    match &action.kind {
        ActionKind::SetVariable { value, .. } | ActionKind::Metric { value, .. } => {
            collect_expr_refs(value, refs);
        }
        ActionKind::CallRuleSet { input_mapping, .. } => {
            if let Some(expr) = input_mapping {
                collect_expr_refs(expr, refs);
            }
        }
        ActionKind::ExternalCall { params, .. } => {
            for (_, expr) in params {
                collect_expr_refs(expr, refs);
            }
        }
        ActionKind::Log { .. } => {}
    }
}

fn collect_step_refs(steps: &hashbrown::HashMap<String, Step>) -> HashSet<String> {
    let mut refs = HashSet::new();

    for step in steps.values() {
        match &step.kind {
            StepKind::Decision { branches, .. } => {
                for branch in branches {
                    collect_condition_refs(&branch.condition, &mut refs);
                    for action in &branch.actions {
                        collect_action_refs(action, &mut refs);
                    }
                }
            }
            StepKind::Action { actions, .. } => {
                for action in actions {
                    collect_action_refs(action, &mut refs);
                }
            }
            StepKind::Terminal { result } => {
                for (_, expr) in &result.output {
                    collect_expr_refs(expr, &mut refs);
                }
            }
            StepKind::SubRule { bindings, .. } => {
                for (_, expr) in bindings {
                    collect_expr_refs(expr, &mut refs);
                }
            }
        }
    }

    refs
}

fn rewrite_expression_string_concept_refs(
    expression: &str,
    concept_names: &HashSet<String>,
) -> String {
    let mut output = String::with_capacity(expression.len());
    let mut chars = expression.char_indices().peekable();
    let mut quote: Option<char> = None;

    while let Some((idx, ch)) = chars.next() {
        if let Some(q) = quote {
            output.push(ch);
            if ch == '\\' {
                if let Some((_, escaped)) = chars.next() {
                    output.push(escaped);
                }
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            output.push(ch);
            continue;
        }

        if !(ch.is_ascii_alphabetic() || ch == '_' || ch == '$') {
            output.push(ch);
            continue;
        }

        let mut end = idx + ch.len_utf8();
        while let Some((next_idx, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' || next == '.' || next == '$' {
                end = next_idx + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let token = &expression[idx..end];
        let normalized = normalize_concept_ref(token);
        let next_non_ws = expression[end..].chars().find(|c| !c.is_whitespace());
        if concept_names.contains(&normalized)
            && token != format!("${}", normalized)
            && next_non_ws != Some('(')
        {
            output.push('$');
            output.push_str(&normalized);
        } else {
            output.push_str(token);
        }
    }

    output
}

fn rewrite_expr_concept_refs(expr: &mut Expr, concept_names: &HashSet<String>) {
    match expr {
        Expr::Field(path) => {
            let normalized = normalize_concept_ref(path);
            if concept_names.contains(&normalized) && *path != format!("${}", normalized) {
                *path = format!("${}", normalized);
            }
        }
        Expr::Binary { left, right, .. } => {
            rewrite_expr_concept_refs(left, concept_names);
            rewrite_expr_concept_refs(right, concept_names);
        }
        Expr::Unary { operand, .. } => rewrite_expr_concept_refs(operand, concept_names),
        Expr::Call { args, .. } | Expr::Array(args) | Expr::Coalesce(args) => {
            for arg in args {
                rewrite_expr_concept_refs(arg, concept_names);
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            rewrite_expr_concept_refs(condition, concept_names);
            rewrite_expr_concept_refs(then_branch, concept_names);
            rewrite_expr_concept_refs(else_branch, concept_names);
        }
        Expr::Object(entries) => {
            for (_, value) in entries {
                rewrite_expr_concept_refs(value, concept_names);
            }
        }
        Expr::Literal(_) | Expr::Exists(_) => {}
    }
}

fn rewrite_condition_concept_refs(condition: &mut Condition, concept_names: &HashSet<String>) {
    match condition {
        Condition::Always => {}
        Condition::Expression(expr) => rewrite_expr_concept_refs(expr, concept_names),
        Condition::ExpressionString(expression) => {
            *expression = rewrite_expression_string_concept_refs(expression, concept_names);
        }
    }
}

fn rewrite_action_concept_refs(action: &mut Action, concept_names: &HashSet<String>) {
    match &mut action.kind {
        ActionKind::SetVariable { value, .. } | ActionKind::Metric { value, .. } => {
            rewrite_expr_concept_refs(value, concept_names);
        }
        ActionKind::CallRuleSet { input_mapping, .. } => {
            if let Some(expr) = input_mapping {
                rewrite_expr_concept_refs(expr, concept_names);
            }
        }
        ActionKind::ExternalCall { params, .. } => {
            for (_, expr) in params {
                rewrite_expr_concept_refs(expr, concept_names);
            }
        }
        ActionKind::Log { .. } => {}
    }
}

fn rewrite_steps_concept_refs(
    steps: &mut hashbrown::HashMap<String, Step>,
    concept_names: &HashSet<String>,
) {
    for step in steps.values_mut() {
        match &mut step.kind {
            StepKind::Decision { branches, .. } => {
                for branch in branches {
                    rewrite_condition_concept_refs(&mut branch.condition, concept_names);
                    for action in &mut branch.actions {
                        rewrite_action_concept_refs(action, concept_names);
                    }
                }
            }
            StepKind::Action { actions, .. } => {
                for action in actions {
                    rewrite_action_concept_refs(action, concept_names);
                }
            }
            StepKind::Terminal { result } => {
                for (_, expr) in &mut result.output {
                    rewrite_expr_concept_refs(expr, concept_names);
                }
            }
            StepKind::SubRule { bindings, .. } => {
                for (_, expr) in bindings {
                    rewrite_expr_concept_refs(expr, concept_names);
                }
            }
        }
    }
}

fn concept_expression_refs(concept: &ConceptDefinition) -> HashSet<String> {
    let mut refs = concept
        .dependencies
        .iter()
        .map(|dep| normalize_concept_ref(dep))
        .collect::<HashSet<_>>();
    match ExprParser::parse(&concept.expression) {
        Ok(expr) => collect_expr_refs(&expr, &mut refs),
        Err(_) => scan_expression_refs(&concept.expression, &mut refs),
    }
    refs
}

fn resolve_concept_order(
    roots: &HashSet<String>,
    concepts: &[ConceptDefinition],
) -> Result<Vec<ConceptDefinition>, ConceptError> {
    let by_name = concepts
        .iter()
        .cloned()
        .map(|concept| (concept.name.clone(), concept))
        .collect::<HashMap<_, _>>();
    let mut order = Vec::new();
    let mut visiting = HashSet::<String>::new();
    let mut visited = HashSet::<String>::new();

    fn visit(
        name: &str,
        by_name: &HashMap<String, ConceptDefinition>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        order: &mut Vec<ConceptDefinition>,
    ) -> Result<(), ConceptError> {
        let Some(concept) = by_name.get(name) else {
            return Ok(());
        };
        if visited.contains(name) {
            return Ok(());
        }
        if !visiting.insert(name.to_string()) {
            return Err(ConceptError::Cycle(name.to_string()));
        }
        for dep in concept_expression_refs(concept) {
            if by_name.contains_key(&dep) {
                visit(&dep, by_name, visiting, visited, order)?;
            }
        }
        visiting.remove(name);
        visited.insert(name.to_string());
        order.push(concept.clone());
        Ok(())
    }

    for root in roots {
        visit(root, &by_name, &mut visiting, &mut visited, &mut order)?;
    }

    Ok(order)
}

fn materialize_concepts_for_step_graph(
    entry_step: &mut String,
    steps: &mut hashbrown::HashMap<String, Step>,
    concepts: &[ConceptDefinition],
) -> Result<(), ConceptError> {
    if concepts.is_empty() {
        return Ok(());
    }

    steps.remove(CONCEPT_PRELUDE_STEP_ID);
    if *entry_step == CONCEPT_PRELUDE_STEP_ID {
        *entry_step = steps.keys().next().cloned().unwrap_or_default();
    }

    let concept_names = concepts
        .iter()
        .map(|concept| concept.name.clone())
        .collect::<HashSet<_>>();
    let roots = collect_step_refs(steps)
        .into_iter()
        .filter(|name| concept_names.contains(name))
        .collect::<HashSet<_>>();
    let order = resolve_concept_order(&roots, concepts)?;

    rewrite_steps_concept_refs(steps, &concept_names);

    if order.is_empty() {
        return Ok(());
    }

    let mut actions = Vec::with_capacity(order.len());
    for concept in order {
        let mut expr = ExprParser::parse(&concept.expression).map_err(|e| ConceptError::Parse {
            concept: concept.name.clone(),
            message: e.to_string(),
        })?;
        rewrite_expr_concept_refs(&mut expr, &concept_names);
        actions.push(Action::set_var(concept.name, expr));
    }

    let original_entry = entry_step.clone();
    steps.insert(
        CONCEPT_PRELUDE_STEP_ID.to_string(),
        Step::action(
            CONCEPT_PRELUDE_STEP_ID,
            "Compute Concepts",
            actions,
            original_entry,
        ),
    );
    *entry_step = CONCEPT_PRELUDE_STEP_ID.to_string();
    Ok(())
}

/// Inline every concept referenced by `ruleset` (and its sub-rules) as a
/// computed prelude step, rewriting references to `$name`. Idempotent: a prior
/// prelude step is removed and rebuilt.
pub fn materialize_concepts(
    ruleset: &mut RuleSet,
    concepts: &[ConceptDefinition],
) -> Result<(), ConceptError> {
    materialize_concepts_for_step_graph(
        &mut ruleset.config.entry_step,
        &mut ruleset.steps,
        concepts,
    )?;

    for graph in ruleset.sub_rules.values_mut() {
        materialize_concepts_for_step_graph(&mut graph.entry_step, &mut graph.steps, concepts)?;
    }

    Ok(())
}

/// Convert a studio-format ruleset to an engine `RuleSet` and materialize the
/// given concepts into it — the offline equivalent of the platform's `convert`
/// endpoint. (The platform additionally normalizes engine-format template
/// drafts and sub-rule runtime propagation server-side.)
pub fn studio_draft_to_engine_with_concepts(
    studio: &StudioRuleSet,
    concepts: &[ConceptDefinition],
) -> Result<RuleSet, ConceptError> {
    let mut engine: RuleSet = studio.clone().try_into()?;
    materialize_concepts(&mut engine, concepts)?;
    Ok(engine)
}
