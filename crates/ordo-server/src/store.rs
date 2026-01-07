//! Rule storage
//!
//! In-memory rule storage with future support for persistence

use ordo_core::prelude::{RuleExecutor, RuleSet, TraceConfig};
use std::collections::HashMap;
use std::sync::Arc;

/// Rule storage
pub struct RuleStore {
    /// Stored rulesets
    rulesets: HashMap<String, Arc<RuleSet>>,
    /// Executor instance
    executor: RuleExecutor,
}

impl RuleStore {
    /// Create a new store
    pub fn new() -> Self {
        Self {
            rulesets: HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
        }
    }

    /// Add or update a ruleset
    pub fn put(&mut self, ruleset: RuleSet) -> Result<(), Vec<String>> {
        // Validate before storing
        ruleset.validate()?;
        let name = ruleset.config.name.clone();
        self.rulesets.insert(name, Arc::new(ruleset));
        Ok(())
    }

    /// Get a ruleset by name
    pub fn get(&self, name: &str) -> Option<Arc<RuleSet>> {
        self.rulesets.get(name).cloned()
    }

    /// Delete a ruleset
    pub fn delete(&mut self, name: &str) -> bool {
        self.rulesets.remove(name).is_some()
    }

    /// List all ruleset names
    pub fn list(&self) -> Vec<RuleSetInfo> {
        self.rulesets
            .values()
            .map(|rs| RuleSetInfo {
                name: rs.config.name.clone(),
                version: rs.config.version.clone(),
                description: rs.config.description.clone(),
                step_count: rs.steps.len(),
            })
            .collect()
    }

    /// Check if a ruleset exists
    pub fn exists(&self, name: &str) -> bool {
        self.rulesets.contains_key(name)
    }

    /// Get executor reference
    pub fn executor(&self) -> &RuleExecutor {
        &self.executor
    }
}

impl Default for RuleStore {
    fn default() -> Self {
        Self::new()
    }
}

/// RuleSet info for listing
#[derive(serde::Serialize)]
pub struct RuleSetInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub step_count: usize,
}
