//! Rule model definitions
//!
//! Defines the structure of rule sets

use super::step::Step;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RuleSet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSetConfig {
    /// RuleSet name
    pub name: String,

    /// RuleSet version
    #[serde(default = "default_version")]
    pub version: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Entry step ID
    pub entry_step: String,

    /// Field missing behavior (default: lenient)
    #[serde(default)]
    pub field_missing: FieldMissingBehavior,

    /// Max execution depth
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// Timeout in milliseconds (0 = no timeout)
    #[serde(default)]
    pub timeout_ms: u64,

    /// Whether to enable tracing
    #[serde(default)]
    pub enable_trace: bool,

    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_max_depth() -> usize {
    100
}

/// Field missing behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldMissingBehavior {
    /// Treat missing field as null (lenient)
    #[default]
    Lenient,
    /// Return error on missing field (strict)
    Strict,
    /// Use default value if provided
    Default,
}

/// Complete RuleSet definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    /// Configuration
    pub config: RuleSetConfig,

    /// Steps by ID
    pub steps: HashMap<String, Step>,
}

impl RuleSet {
    /// Create a new RuleSet
    pub fn new(name: impl Into<String>, entry_step: impl Into<String>) -> Self {
        Self {
            config: RuleSetConfig {
                name: name.into(),
                version: default_version(),
                description: String::new(),
                entry_step: entry_step.into(),
                field_missing: FieldMissingBehavior::default(),
                max_depth: default_max_depth(),
                timeout_ms: 0,
                enable_trace: false,
                metadata: HashMap::new(),
            },
            steps: HashMap::new(),
        }
    }

    /// Add a step
    pub fn add_step(&mut self, step: Step) -> &mut Self {
        self.steps.insert(step.id.clone(), step);
        self
    }

    /// Get a step by ID
    pub fn get_step(&self, id: &str) -> Option<&Step> {
        self.steps.get(id)
    }

    /// Get entry step
    pub fn entry_step(&self) -> Option<&Step> {
        self.steps.get(&self.config.entry_step)
    }

    /// Validate the RuleSet
    pub fn validate(&self) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Check entry step exists
        if !self.steps.contains_key(&self.config.entry_step) {
            errors.push(format!("Entry step '{}' not found", self.config.entry_step));
        }

        // Check all referenced steps exist
        for step in self.steps.values() {
            for next_step in step.referenced_steps() {
                if !self.steps.contains_key(&next_step) {
                    errors.push(format!(
                        "Step '{}' references non-existent step '{}'",
                        step.id, next_step
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Load from JSON string
    pub fn from_json(json: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Load from YAML string
    pub fn from_yaml(yaml: &str) -> std::result::Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Serialize to YAML
    pub fn to_yaml(&self) -> std::result::Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Compile all expression strings in this ruleset to expression ASTs.
    ///
    /// This pre-parses all condition expressions for faster evaluation at runtime.
    /// Call this after loading a ruleset from JSON/YAML to avoid repeated parsing
    /// during rule execution.
    ///
    /// # Example
    /// ```
    /// use ordo_core::prelude::*;
    ///
    /// let json = r#"{
    ///     "config": { "name": "test", "entry_step": "start" },
    ///     "steps": {
    ///         "start": {
    ///             "id": "start",
    ///             "name": "Start",
    ///             "type": "terminal",
    ///             "result": { "code": "OK" }
    ///         }
    ///     }
    /// }"#;
    ///
    /// let mut ruleset = RuleSet::from_json(json).unwrap();
    /// ruleset.compile().unwrap(); // Pre-compile all expressions
    /// ```
    pub fn compile(&mut self) -> Result<()> {
        for step in self.steps.values_mut() {
            step.compile()?;
        }
        Ok(())
    }

    /// Load from JSON string and compile expressions
    pub fn from_json_compiled(json: &str) -> Result<Self> {
        let mut ruleset: Self = serde_json::from_str(json)
            .map_err(|e| crate::error::OrdoError::parse_error(e.to_string()))?;
        ruleset.compile()?;
        Ok(ruleset)
    }

    /// Load from YAML string and compile expressions
    pub fn from_yaml_compiled(yaml: &str) -> Result<Self> {
        let mut ruleset: Self = serde_yaml::from_str(yaml)
            .map_err(|e| crate::error::OrdoError::parse_error(e.to_string()))?;
        ruleset.compile()?;
        Ok(ruleset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::step::{Branch, Condition, StepKind, TerminalResult};

    #[test]
    fn test_ruleset_validation() {
        let mut ruleset = RuleSet::new("test", "start");

        // Should fail: entry step doesn't exist
        assert!(ruleset.validate().is_err());

        // Add entry step
        ruleset.add_step(Step {
            id: "start".to_string(),
            name: "Start".to_string(),
            kind: StepKind::Decision {
                branches: vec![Branch {
                    condition: Condition::Always,
                    next_step: "end".to_string(),
                    actions: vec![],
                }],
                default_next: None,
            },
        });

        // Should fail: references non-existent step
        assert!(ruleset.validate().is_err());

        // Add end step
        ruleset.add_step(Step {
            id: "end".to_string(),
            name: "End".to_string(),
            kind: StepKind::Terminal {
                result: TerminalResult::default(),
            },
        });

        // Should pass now
        assert!(ruleset.validate().is_ok());
    }
}
