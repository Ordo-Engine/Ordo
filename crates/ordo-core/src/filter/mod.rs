//! Data Filter API — partial evaluation for database predicate generation
//!
//! Converts rule sets into SQL WHERE clauses or JSON predicates by:
//! 1. Substituting known input fields (folding constants)
//! 2. Traversing the rule graph (DFS) to collect all paths to target results
//! 3. Each path's conditions are ANDed; multiple paths are ORed
//! 4. The combined expression is rendered as SQL or JSON
//!
//! # Typical use case
//!
//! A multi-tenant SaaS wants to query "all resources visible to user alice":
//! - known_input: `{ "user.role": "viewer", "user.id": "alice" }`
//! - target_results: `["APPROVED"]`
//! - output: `owner_id = 'alice' OR visibility = 'public'`
//!
//! This WHERE clause can be pushed directly to the database, avoiding a
//! full table scan + row-by-row rule execution.

pub mod json_predicate;
pub mod mongo;
pub mod partial_eval;
pub mod path_collector;
pub mod sql;

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use crate::context::Value;
use crate::error::Result;
use crate::expr::Expr;
use crate::rule::RuleSet;

use partial_eval::PartialEvaluator;
use path_collector::collect_paths;

/// Output format for the generated filter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterFormat {
    #[default]
    Sql,
    Json,
    /// MongoDB aggregation pipeline `$match` stage
    Mongo,
}

/// Request for filter compilation
#[derive(Debug, Clone, Deserialize)]
pub struct FilterRequest {
    /// Input fields whose values are already known (e.g. from the current user session)
    pub known_input: Value,

    /// Result codes to collect paths for (e.g. `["APPROVED"]`)
    pub target_results: Vec<String>,

    /// Output format (default: sql)
    #[serde(default)]
    pub format: FilterFormat,

    /// Mapping from rule field paths to database column names.
    /// e.g. `{ "resource.owner": "owner_id" }`
    #[serde(default)]
    pub field_mapping: HashMap<String, String>,

    /// Maximum number of paths to collect (default: 100).
    /// Prevents exponential blowup on highly branched rule graphs.
    #[serde(default = "default_max_paths")]
    pub max_paths: usize,
}

fn default_max_paths() -> usize {
    100
}

/// Result of filter compilation
#[derive(Debug, Clone, Serialize)]
pub struct FilterResult {
    /// Output format used
    pub format: FilterFormat,

    /// The generated filter:
    /// - SQL format: a string (`"owner_id = 'alice' OR visibility = 'public'"`)
    /// - JSON format: a predicate object
    pub filter: JsonValue,

    /// True when all possible inputs match (e.g. admin sees everything).
    /// The caller may skip the WHERE clause entirely.
    pub always_matches: bool,

    /// True when no inputs can ever match (e.g. rule always denies).
    /// The caller should return an empty result set immediately.
    pub never_matches: bool,

    /// True when the `max_paths` limit was reached before the full graph was
    /// explored. The generated filter is incomplete and may be more permissive
    /// than actual rule execution. `always_matches` will also be set to true
    /// in this case to avoid false negatives (hiding valid rows).
    /// Consider increasing `max_paths` if this occurs.
    pub truncated: bool,

    /// Fields that remain unknown (appear in the filter as database columns)
    pub unknown_fields: Vec<String>,
}

/// Compiles a RuleSet into a database filter for a given known context
pub struct FilterCompiler;

impl FilterCompiler {
    pub fn new() -> Self {
        FilterCompiler
    }

    /// Compile the ruleset + request into a database filter.
    pub fn compile(&self, ruleset: &RuleSet, request: FilterRequest) -> Result<FilterResult> {
        let mut evaluator = PartialEvaluator::new(request.known_input);

        let (paths, truncated) = collect_paths(
            ruleset,
            &mut evaluator,
            &request.target_results,
            request.max_paths,
        )?;

        // When the path limit was hit, the generated filter would be incomplete
        // (too strict), which could silently hide rows that the rule engine
        // accepts. Return always_matches to be safe; the caller can increase
        // max_paths and retry.
        if truncated {
            let filter = match request.format {
                FilterFormat::Mongo => json!({}),
                _ => JsonValue::String("TRUE".to_string()),
            };
            return Ok(FilterResult {
                format: request.format,
                filter,
                always_matches: true,
                never_matches: false,
                truncated: true,
                unknown_fields: vec![],
            });
        }

        if paths.is_empty() {
            return Ok(FilterResult {
                format: request.format,
                filter: JsonValue::Null,
                always_matches: false,
                never_matches: true,
                truncated: false,
                unknown_fields: vec![],
            });
        }

        let always_matches = paths.iter().any(|p| p.conditions.is_empty());

        // Collect all unknown field references across all paths
        let mut unknown: BTreeSet<String> = BTreeSet::new();
        for path in &paths {
            for cond in &path.conditions {
                collect_fields(cond, &mut unknown);
            }
        }
        let unknown_fields: Vec<String> = unknown.into_iter().collect();

        let filter = match request.format {
            FilterFormat::Sql => {
                let s = sql::to_sql(&paths, &request.field_mapping)?;
                JsonValue::String(s)
            }
            FilterFormat::Json => json_predicate::to_json(&paths, &request.field_mapping),
            FilterFormat::Mongo => mongo::to_mongo(&paths, &request.field_mapping),
        };

        Ok(FilterResult {
            format: request.format,
            filter,
            always_matches,
            never_matches: false,
            truncated: false,
            unknown_fields,
        })
    }
}

impl Default for FilterCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively collect all Expr::Field / Expr::Exists paths from an expression
fn collect_fields(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Field(path) | Expr::Exists(path) => {
            out.insert(path.clone());
        }
        Expr::Binary { left, right, .. } => {
            collect_fields(left, out);
            collect_fields(right, out);
        }
        Expr::Unary { operand, .. } => collect_fields(operand, out),
        Expr::Call { args, .. } => {
            for a in args {
                collect_fields(a, out);
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_fields(condition, out);
            collect_fields(then_branch, out);
            collect_fields(else_branch, out);
        }
        Expr::Array(elems) => {
            for e in elems {
                collect_fields(e, out);
            }
        }
        Expr::Coalesce(exprs) => {
            for e in exprs {
                collect_fields(e, out);
            }
        }
        Expr::Object(pairs) => {
            for (_, v) in pairs {
                collect_fields(v, out);
            }
        }
        Expr::Literal(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use crate::rule::{Branch, Condition, StepKind, TerminalResult};

    fn build_access_ruleset() -> RuleSet {
        let mut rs = RuleSet::new("access", "check_access");

        rs.add_step(Step {
            id: "check_access".to_string(),
            name: "Check Access".to_string(),
            kind: StepKind::Decision {
                branches: vec![
                    Branch {
                        condition: Condition::from_string("user.role == \"admin\""),
                        next_step: "approved".to_string(),
                        actions: vec![],
                    },
                    Branch {
                        condition: Condition::from_string("resource.owner == user.id"),
                        next_step: "approved".to_string(),
                        actions: vec![],
                    },
                    Branch {
                        condition: Condition::from_string("resource.visibility == \"public\""),
                        next_step: "approved".to_string(),
                        actions: vec![],
                    },
                ],
                default_next: Some("denied".to_string()),
            },
        });

        rs.add_step(Step {
            id: "approved".to_string(),
            name: "Approved".to_string(),
            kind: StepKind::Terminal {
                result: TerminalResult::new("APPROVED"),
            },
        });

        rs.add_step(Step {
            id: "denied".to_string(),
            name: "Denied".to_string(),
            kind: StepKind::Terminal {
                result: TerminalResult::new("DENIED"),
            },
        });

        rs
    }

    #[test]
    fn test_filter_viewer_sql() {
        let rs = build_access_ruleset();
        let compiler = FilterCompiler::new();

        let known: Value =
            serde_json::from_str(r#"{"user": {"role": "viewer", "id": "alice"}}"#).unwrap();

        let mut mapping = HashMap::new();
        mapping.insert("resource.owner".to_string(), "owner_id".to_string());
        mapping.insert("resource.visibility".to_string(), "visibility".to_string());

        let request = FilterRequest {
            known_input: known,
            target_results: vec!["APPROVED".to_string()],
            format: FilterFormat::Sql,
            field_mapping: mapping,
            max_paths: 100,
        };

        let result = compiler.compile(&rs, request).unwrap();

        assert!(!result.always_matches);
        assert!(!result.never_matches);

        let sql = result.filter.as_str().unwrap();
        // Should contain the two unknown conditions joined with OR
        assert!(sql.contains("owner_id = 'alice'"), "sql = {}", sql);
        assert!(sql.contains("visibility = 'public'"), "sql = {}", sql);
        assert!(sql.contains(" OR "), "sql = {}", sql);
    }

    #[test]
    fn test_filter_admin_always_matches() {
        let rs = build_access_ruleset();
        let compiler = FilterCompiler::new();

        let known: Value =
            serde_json::from_str(r#"{"user": {"role": "admin", "id": "bob"}}"#).unwrap();

        let request = FilterRequest {
            known_input: known,
            target_results: vec!["APPROVED".to_string()],
            format: FilterFormat::Sql,
            field_mapping: HashMap::new(),
            max_paths: 100,
        };

        let result = compiler.compile(&rs, request).unwrap();
        assert!(result.always_matches);
        assert_eq!(result.filter.as_str().unwrap(), "TRUE");
    }

    #[test]
    fn test_filter_never_matches() {
        let rs = build_access_ruleset();
        let compiler = FilterCompiler::new();

        let known: Value =
            serde_json::from_str(r#"{"user": {"role": "viewer", "id": "alice"}}"#).unwrap();

        let request = FilterRequest {
            known_input: known,
            target_results: vec!["NONEXISTENT".to_string()],
            format: FilterFormat::Sql,
            field_mapping: HashMap::new(),
            max_paths: 100,
        };

        let result = compiler.compile(&rs, request).unwrap();
        assert!(result.never_matches);
    }

    #[test]
    fn test_filter_json_format() {
        let rs = build_access_ruleset();
        let compiler = FilterCompiler::new();

        let known: Value =
            serde_json::from_str(r#"{"user": {"role": "viewer", "id": "alice"}}"#).unwrap();

        let mut mapping = HashMap::new();
        mapping.insert("resource.owner".to_string(), "owner_id".to_string());
        mapping.insert("resource.visibility".to_string(), "visibility".to_string());

        let request = FilterRequest {
            known_input: known,
            target_results: vec!["APPROVED".to_string()],
            format: FilterFormat::Json,
            field_mapping: mapping,
            max_paths: 100,
        };

        let result = compiler.compile(&rs, request).unwrap();
        assert!(result.filter.is_object() || result.filter.is_array());
        assert_eq!(result.filter["type"], "or");
    }
}
