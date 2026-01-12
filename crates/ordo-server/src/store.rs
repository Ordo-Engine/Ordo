//! Rule storage
//!
//! In-memory rule storage with optional file-based persistence.
//! When a rules directory is specified, rules are automatically persisted to disk.

use ordo_core::prelude::{RuleExecutor, RuleSet, TraceConfig};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Supported file formats for rule persistence
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    Json,
    Yaml,
}

impl FileFormat {
    /// Get file extension for this format
    fn extension(&self) -> &'static str {
        match self {
            FileFormat::Json => "json",
            FileFormat::Yaml => "yaml",
        }
    }

    /// Detect format from file extension
    fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext.to_lowercase().as_str() {
                "json" => Some(FileFormat::Json),
                "yaml" | "yml" => Some(FileFormat::Yaml),
                _ => None,
            })
    }
}

/// Rule storage with optional file persistence
pub struct RuleStore {
    /// Stored rulesets (in-memory cache)
    rulesets: HashMap<String, Arc<RuleSet>>,
    /// Executor instance
    executor: RuleExecutor,
    /// Rules directory for persistence (None = pure in-memory mode)
    rules_dir: Option<PathBuf>,
    /// Default format for new rules
    default_format: FileFormat,
}

impl RuleStore {
    /// Create a new in-memory store (no persistence)
    pub fn new() -> Self {
        Self {
            rulesets: HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
            rules_dir: None,
            default_format: FileFormat::Json,
        }
    }

    /// Create a store with file persistence enabled
    pub fn new_with_persistence(rules_dir: PathBuf) -> Self {
        Self {
            rulesets: HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
            rules_dir: Some(rules_dir),
            default_format: FileFormat::Json,
        }
    }

    /// Check if persistence is enabled
    pub fn persistence_enabled(&self) -> bool {
        self.rules_dir.is_some()
    }

    /// Load all rules from the configured directory
    ///
    /// This scans the rules directory and loads all .json, .yaml, .yml files.
    /// If multiple files exist for the same rule name (different formats),
    /// JSON takes priority.
    pub fn load_from_dir(&mut self) -> io::Result<usize> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir.clone(),
            None => {
                warn!("load_from_dir called but no rules directory configured");
                return Ok(0);
            }
        };

        // Create directory if it doesn't exist
        if !rules_dir.exists() {
            info!("Creating rules directory: {:?}", rules_dir);
            fs::create_dir_all(&rules_dir)?;
            return Ok(0);
        }

        let mut loaded = 0;
        let mut seen_names: HashMap<String, PathBuf> = HashMap::new();

        // Collect all rule files
        let entries: Vec<_> = fs::read_dir(&rules_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter(|e| FileFormat::from_path(&e.path()).is_some())
            .collect();

        // Sort to ensure JSON files are processed first (priority)
        let mut entries: Vec<_> = entries.into_iter().map(|e| e.path()).collect();
        entries.sort_by(|a, b| {
            let a_is_json = a.extension().map(|e| e == "json").unwrap_or(false);
            let b_is_json = b.extension().map(|e| e == "json").unwrap_or(false);
            b_is_json.cmp(&a_is_json) // JSON first
        });

        for path in entries {
            let file_stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip if we already loaded this rule (from a higher priority format)
            if seen_names.contains_key(&file_stem) {
                debug!(
                    "Skipping {:?} (already loaded from {:?})",
                    path,
                    seen_names.get(&file_stem)
                );
                continue;
            }

            match self.load_ruleset_file(&path) {
                Ok(ruleset) => {
                    let name = ruleset.config.name.clone();
                    if name != file_stem {
                        warn!(
                            "Rule name '{}' doesn't match filename '{}', using filename",
                            name, file_stem
                        );
                    }
                    self.rulesets.insert(file_stem.clone(), Arc::new(ruleset));
                    seen_names.insert(file_stem.clone(), path.clone());
                    loaded += 1;
                    info!("Loaded rule '{}' from {:?}", file_stem, path);
                }
                Err(e) => {
                    error!("Failed to load {:?}: {}", path, e);
                }
            }
        }

        info!(
            "Loaded {} rules from {:?}",
            loaded, rules_dir
        );
        Ok(loaded)
    }

    /// Load a single ruleset from a file
    fn load_ruleset_file(&self, path: &Path) -> io::Result<RuleSet> {
        let format = FileFormat::from_path(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Unknown file format"))?;

        let content = fs::read_to_string(path)?;

        let ruleset: RuleSet = match format {
            FileFormat::Json => serde_json::from_str(&content)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            FileFormat::Yaml => serde_yaml::from_str(&content)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        };

        // Validate the loaded ruleset
        ruleset.validate().map_err(|errors| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Validation failed: {}", errors.join(", ")),
            )
        })?;

        Ok(ruleset)
    }

    /// Persist a ruleset to disk
    fn persist_ruleset(&self, name: &str, ruleset: &RuleSet) -> io::Result<()> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir,
            None => return Ok(()), // No persistence configured
        };

        // Ensure directory exists
        if !rules_dir.exists() {
            fs::create_dir_all(rules_dir)?;
        }

        let filename = format!("{}.{}", name, self.default_format.extension());
        let path = rules_dir.join(&filename);

        let content = match self.default_format {
            FileFormat::Json => serde_json::to_string_pretty(ruleset)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            FileFormat::Yaml => serde_yaml::to_string(ruleset)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        };

        // Write atomically: write to temp file, then rename
        let temp_path = rules_dir.join(format!(".{}.tmp", name));
        fs::write(&temp_path, &content)?;
        fs::rename(&temp_path, &path)?;

        debug!("Persisted rule '{}' to {:?}", name, path);
        Ok(())
    }

    /// Delete a ruleset file from disk
    fn delete_file(&self, name: &str) -> io::Result<()> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir,
            None => return Ok(()), // No persistence configured
        };

        // Try to delete any format
        for format in [FileFormat::Json, FileFormat::Yaml] {
            let filename = format!("{}.{}", name, format.extension());
            let path = rules_dir.join(&filename);
            if path.exists() {
                fs::remove_file(&path)?;
                debug!("Deleted rule file {:?}", path);
            }
        }

        // Also try .yml extension
        let yml_path = rules_dir.join(format!("{}.yml", name));
        if yml_path.exists() {
            fs::remove_file(&yml_path)?;
            debug!("Deleted rule file {:?}", yml_path);
        }

        Ok(())
    }

    /// Add or update a ruleset
    ///
    /// If persistence is enabled, the ruleset is also written to disk.
    pub fn put(&mut self, ruleset: RuleSet) -> Result<(), Vec<String>> {
        // Validate before storing
        ruleset.validate()?;
        let name = ruleset.config.name.clone();

        // Persist to disk if enabled
        if let Err(e) = self.persist_ruleset(&name, &ruleset) {
            error!("Failed to persist rule '{}': {}", name, e);
            return Err(vec![format!("Persistence error: {}", e)]);
        }

        self.rulesets.insert(name, Arc::new(ruleset));
        Ok(())
    }

    /// Get a ruleset by name
    pub fn get(&self, name: &str) -> Option<Arc<RuleSet>> {
        self.rulesets.get(name).cloned()
    }

    /// Delete a ruleset
    ///
    /// If persistence is enabled, the ruleset file is also deleted from disk.
    pub fn delete(&mut self, name: &str) -> bool {
        let existed = self.rulesets.remove(name).is_some();

        if existed {
            if let Err(e) = self.delete_file(name) {
                error!("Failed to delete rule file for '{}': {}", name, e);
            }
        }

        existed
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

    /// Get the rules directory path (if configured)
    pub fn rules_dir(&self) -> Option<&Path> {
        self.rules_dir.as_deref()
    }

    /// Get the number of loaded rules
    pub fn len(&self) -> usize {
        self.rulesets.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.rulesets.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use ordo_core::prelude::{RuleSet, Step, TerminalResult};
    use tempfile::TempDir;

    fn create_test_ruleset(name: &str) -> RuleSet {
        let mut ruleset = RuleSet::new(name, "start");
        ruleset.add_step(Step::terminal(
            "start",
            "Start",
            TerminalResult::new("OK").with_message("Test completed"),
        ));
        ruleset
    }

    #[test]
    fn test_in_memory_store() {
        let mut store = RuleStore::new();
        assert!(!store.persistence_enabled());

        let ruleset = create_test_ruleset("test-rule");
        store.put(ruleset).unwrap();

        assert!(store.exists("test-rule"));
        assert_eq!(store.len(), 1);

        store.delete("test-rule");
        assert!(!store.exists("test-rule"));
    }

    #[test]
    fn test_persistence_store() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence(rules_dir.clone());
        assert!(store.persistence_enabled());

        // Add a rule
        let ruleset = create_test_ruleset("payment-check");
        store.put(ruleset).unwrap();

        // Verify file was created
        let file_path = rules_dir.join("payment-check.json");
        assert!(file_path.exists());

        // Create a new store and load from directory
        let mut store2 = RuleStore::new_with_persistence(rules_dir.clone());
        let loaded = store2.load_from_dir().unwrap();
        assert_eq!(loaded, 1);
        assert!(store2.exists("payment-check"));

        // Delete and verify file is removed
        store2.delete("payment-check");
        assert!(!file_path.exists());
    }

    #[test]
    fn test_load_yaml_files() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        // Create a YAML rule file with correct structure
        let yaml_content = r#"
config:
  name: yaml-rule
  version: "1.0.0"
  description: YAML test rule
  entry_step: start
steps:
  start:
    id: start
    name: Start
    type: terminal
    result:
      code: OK
      message: YAML rule executed
"#;
        fs::write(rules_dir.join("yaml-rule.yaml"), yaml_content).unwrap();

        let mut store = RuleStore::new_with_persistence(rules_dir);
        let loaded = store.load_from_dir().unwrap();
        assert_eq!(loaded, 1);
        assert!(store.exists("yaml-rule"));
    }

    #[test]
    fn test_json_priority_over_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        // Create both JSON and YAML for the same rule
        let json_ruleset = create_test_ruleset("priority-test");
        let json_content = serde_json::to_string_pretty(&json_ruleset).unwrap();
        fs::write(rules_dir.join("priority-test.json"), &json_content).unwrap();

        let yaml_content = r#"
config:
  name: priority-test
  version: "2.0.0"
  description: YAML version (should be ignored)
  entry_step: start
steps:
  start:
    id: start
    name: Start
    type: terminal
    result:
      code: YAML
      message: From YAML
"#;
        fs::write(rules_dir.join("priority-test.yaml"), yaml_content).unwrap();

        let mut store = RuleStore::new_with_persistence(rules_dir);
        let loaded = store.load_from_dir().unwrap();
        assert_eq!(loaded, 1); // Only one should be loaded

        // JSON should take priority (version 1.0.0)
        let loaded_rule = store.get("priority-test").unwrap();
        assert_eq!(loaded_rule.config.version, "1.0.0");
    }

    #[test]
    fn test_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = RuleStore::new_with_persistence(temp_dir.path().to_path_buf());
        let loaded = store.load_from_dir().unwrap();
        assert_eq!(loaded, 0);
    }

    #[test]
    fn test_nonexistent_directory_created() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("new_rules_dir");

        let mut store = RuleStore::new_with_persistence(rules_dir.clone());
        store.load_from_dir().unwrap();

        assert!(rules_dir.exists());
    }
}
