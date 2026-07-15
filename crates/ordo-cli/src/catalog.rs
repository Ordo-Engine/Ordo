//! Local validation for `facts.json` / `concepts.json` against the enum
//! values the platform's catalog endpoints actually accept — mirroring
//! `NullPolicy` / `FactDataType` in `ordo-platform`'s `models/catalog.rs`.
//!
//! Duplicated deliberately, not shared: `ordo-cli` talks to the platform over
//! HTTP via `ordo-api-client`, not as a Rust dependency on `ordo-platform`, so
//! there's no existing path to import the real enum types, and pulling one in
//! just for this would be the wrong direction. This is a small,
//! independently-maintained mirror of the two enums the platform enforces
//! strictly (an unrecognized variant fails the server's JSON
//! deserialization outright, surfacing as an unhelpful round-trip to a 4xx)
//! — catching it here means a bad value fails locally with a clear message
//! before any network call.
//!
//! Scoped to exactly these two enums, not full schema validation: a missing
//! *required* field the platform doesn't constrain to an enum (e.g. an empty
//! `source` on a fact) still only surfaces at push time.

use serde_json::Value;

const VALID_DATA_TYPES: &[&str] = &["string", "number", "boolean", "date", "object"];
const VALID_NULL_POLICIES: &[&str] = &["error", "default", "skip"];

/// Validate every fact's `data_type` and `null_policy` against the platform's
/// accepted values. Returns one message per bad field, so multiple failures
/// in one file are all reported together rather than one-at-a-time.
pub fn validate_facts(facts: &[Value]) -> Vec<String> {
    facts
        .iter()
        .enumerate()
        .flat_map(|(i, f)| {
            let label = entry_label(f, i);
            let mut errors = check_field(f, "data_type", VALID_DATA_TYPES, &label);
            errors.extend(check_field(f, "null_policy", VALID_NULL_POLICIES, &label));
            errors
        })
        .collect()
}

/// Concepts share the same `data_type` enum as facts but have no
/// `null_policy` field.
pub fn validate_concepts(concepts: &[Value]) -> Vec<String> {
    concepts
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            let label = entry_label(c, i);
            check_field(c, "data_type", VALID_DATA_TYPES, &label)
        })
        .collect()
}

fn entry_label(entry: &Value, index: usize) -> String {
    entry
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", index + 1))
}

fn check_field(entry: &Value, field: &str, allowed: &[&str], label: &str) -> Vec<String> {
    let Some(value) = entry.get(field) else {
        return vec![format!("{label}: missing \"{field}\"")];
    };
    let Some(s) = value.as_str() else {
        return vec![format!(
            "{label}: \"{field}\" must be a string, got {value}"
        )];
    };
    if allowed.contains(&s) {
        Vec::new()
    } else {
        vec![format!(
            "{label}: \"{field}\" is \"{s}\" — must be one of: {}",
            allowed.join(", ")
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_all_valid_values() {
        let facts = vec![
            json!({ "name": "a", "data_type": "string", "null_policy": "error" }),
            json!({ "name": "b", "data_type": "number", "null_policy": "default" }),
            json!({ "name": "c", "data_type": "boolean", "null_policy": "skip" }),
        ];
        assert!(validate_facts(&facts).is_empty());
    }

    #[test]
    fn rejects_legacy_null_policy_aliases() {
        // The exact incident this guards against: a name that reads like a
        // plausible synonym of the real enum but was never valid.
        let facts = vec![json!({
            "name": "user_score", "data_type": "number", "null_policy": "reject"
        })];
        let errors = validate_facts(&facts);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("user_score"));
        assert!(errors[0].contains("null_policy"));
        assert!(errors[0].contains("\"reject\""));
        assert!(errors[0].contains("error, default, skip"));
    }

    #[test]
    fn rejects_bad_data_type_and_reports_index_when_unnamed() {
        let facts = vec![json!({ "data_type": "int", "null_policy": "error" })];
        let errors = validate_facts(&facts);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("#1:"), "got: {}", errors[0]);
        assert!(errors[0].contains("data_type"));
    }

    #[test]
    fn reports_missing_field() {
        let facts = vec![json!({ "name": "x", "data_type": "string" })];
        assert_eq!(validate_facts(&facts), vec!["x: missing \"null_policy\""]);
    }

    #[test]
    fn concepts_only_check_data_type() {
        let concepts = vec![json!({ "name": "risk", "data_type": "notatype" })];
        let errors = validate_concepts(&concepts);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("data_type"));
    }
}
