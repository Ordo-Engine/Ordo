//! Rule storage
//!
//! In-memory rule storage with optional file-based persistence.
//! When a rules directory is specified, rules are automatically persisted to disk.
//! Supports version management with automatic backup of previous versions.

use ordo_core::prelude::{RuleExecutor, RuleSet, TraceConfig};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
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

/// Rule storage with optional file persistence and version management
pub struct RuleStore {
    /// Stored rulesets (in-memory cache)
    rulesets: HashMap<String, Arc<RuleSet>>,
    /// Executor instance
    executor: RuleExecutor,
    /// Rules directory for persistence (None = pure in-memory mode)
    rules_dir: Option<PathBuf>,
    /// Default format for new rules
    default_format: FileFormat,
    /// Maximum number of historical versions to keep per rule
    max_versions: usize,
}

/// Version information for a rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Version sequence number (1, 2, 3, ...)
    pub seq: u32,
    /// RuleSet version string from config
    pub version: String,
    /// Creation timestamp (ISO 8601 format)
    pub created_at: String,
}

/// Response for version list API
#[derive(Debug, Clone, Serialize)]
pub struct VersionListResponse {
    /// Rule name
    pub name: String,
    /// Current version string
    pub current_version: String,
    /// List of historical versions (newest first)
    pub versions: Vec<VersionInfo>,
}

impl RuleStore {
    /// Create a new in-memory store (no persistence)
    pub fn new() -> Self {
        Self {
            rulesets: HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
            rules_dir: None,
            default_format: FileFormat::Json,
            max_versions: 10,
        }
    }

    /// Create a store with file persistence enabled
    #[allow(dead_code)]
    pub fn new_with_persistence(rules_dir: PathBuf) -> Self {
        Self::new_with_persistence_and_versions(rules_dir, 10)
    }

    /// Create a store with file persistence and custom max versions
    pub fn new_with_persistence_and_versions(rules_dir: PathBuf, max_versions: usize) -> Self {
        Self {
            rulesets: HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
            rules_dir: Some(rules_dir),
            default_format: FileFormat::Json,
            max_versions,
        }
    }

    /// Set the maximum number of versions to keep
    #[allow(dead_code)]
    pub fn set_max_versions(&mut self, max_versions: usize) {
        self.max_versions = max_versions;
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

            // Skip version files (e.g., payment-check.v1.json)
            if Self::is_version_file(&file_stem) {
                debug!("Skipping version file {:?}", path);
                continue;
            }

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

        info!("Loaded {} rules from {:?}", loaded, rules_dir);
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

    // ==================== Version Management ====================

    /// Check if a filename is a version file (e.g., "payment-check.v1")
    fn is_version_file(file_stem: &str) -> bool {
        // Match pattern: name.v{number}
        let re = Regex::new(r"\.v\d+$").unwrap();
        re.is_match(file_stem)
    }

    /// Extract the base name from a version filename
    #[allow(dead_code)]
    fn extract_base_name(file_stem: &str) -> &str {
        // "payment-check.v1" -> "payment-check"
        if let Some(pos) = file_stem.rfind(".v") {
            let suffix = &file_stem[pos + 2..];
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                return &file_stem[..pos];
            }
        }
        file_stem
    }

    /// Get the next version sequence number for a rule
    fn get_next_version_seq(&self, name: &str) -> io::Result<u32> {
        let versions = self.list_version_files(name)?;
        Ok(versions.iter().map(|(seq, _)| *seq).max().unwrap_or(0) + 1)
    }

    /// List all version files for a rule, returns (seq, path) sorted by seq descending
    fn list_version_files(&self, name: &str) -> io::Result<Vec<(u32, PathBuf)>> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir,
            None => return Ok(vec![]),
        };

        if !rules_dir.exists() {
            return Ok(vec![]);
        }

        let mut versions = Vec::new();
        let pattern = format!("{}.v", name);

        for entry in fs::read_dir(rules_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };

            // Check if this is a version file for this rule
            if file_stem.starts_with(&pattern) {
                let seq_str = &file_stem[pattern.len()..];
                if let Ok(seq) = seq_str.parse::<u32>() {
                    versions.push((seq, path));
                }
            }
        }

        // Sort by seq descending (newest first)
        versions.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(versions)
    }

    /// Backup the current version before updating
    fn backup_current_version(&self, name: &str) -> io::Result<()> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };

        // Find current file
        let current_path = rules_dir.join(format!("{}.{}", name, self.default_format.extension()));
        if !current_path.exists() {
            // Try other formats
            for format in [FileFormat::Json, FileFormat::Yaml] {
                let path = rules_dir.join(format!("{}.{}", name, format.extension()));
                if path.exists() {
                    return self.backup_file(&path, name);
                }
            }
            // Also try .yml
            let yml_path = rules_dir.join(format!("{}.yml", name));
            if yml_path.exists() {
                return self.backup_file(&yml_path, name);
            }
            return Ok(()); // No current file to backup
        }

        self.backup_file(&current_path, name)
    }

    /// Backup a specific file as a version
    fn backup_file(&self, current_path: &Path, name: &str) -> io::Result<()> {
        let rules_dir = match &self.rules_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };

        let next_seq = self.get_next_version_seq(name)?;
        let ext = current_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json");
        let version_filename = format!("{}.v{}.{}", name, next_seq, ext);
        let version_path = rules_dir.join(&version_filename);

        fs::copy(current_path, &version_path)?;
        debug!(
            "Backed up '{}' to {:?} (version {})",
            name, version_path, next_seq
        );

        Ok(())
    }

    /// Clean up old versions beyond the limit
    fn cleanup_old_versions(&self, name: &str) -> io::Result<()> {
        if self.max_versions == 0 {
            return Ok(()); // Keep all versions
        }

        let versions = self.list_version_files(name)?;

        // Delete versions beyond the limit
        for (seq, path) in versions.iter().skip(self.max_versions) {
            fs::remove_file(path)?;
            debug!("Deleted old version {} of '{}': {:?}", seq, name, path);
        }

        Ok(())
    }

    /// Delete all version files for a rule
    fn delete_all_versions(&self, name: &str) -> io::Result<()> {
        let versions = self.list_version_files(name)?;

        for (seq, path) in versions {
            fs::remove_file(&path)?;
            debug!("Deleted version {} of '{}': {:?}", seq, name, path);
        }

        Ok(())
    }

    /// List all versions of a rule
    pub fn list_versions(&self, name: &str) -> io::Result<VersionListResponse> {
        let current = self.get(name);
        let current_version = current
            .as_ref()
            .map(|r| r.config.version.clone())
            .unwrap_or_default();

        let version_files = self.list_version_files(name)?;
        let mut versions = Vec::new();

        for (seq, path) in version_files {
            // Get file modification time
            let created_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .map(format_system_time)
                .unwrap_or_else(|_| "unknown".to_string());

            // Load version to get the version string
            let version = match self.load_ruleset_file(&path) {
                Ok(ruleset) => ruleset.config.version,
                Err(_) => "unknown".to_string(),
            };

            versions.push(VersionInfo {
                seq,
                version,
                created_at,
            });
        }

        Ok(VersionListResponse {
            name: name.to_string(),
            current_version,
            versions,
        })
    }

    /// Get a specific version of a rule
    pub fn get_version(&self, name: &str, seq: u32) -> io::Result<Option<RuleSet>> {
        let versions = self.list_version_files(name)?;

        for (v_seq, path) in versions {
            if v_seq == seq {
                let ruleset = self.load_ruleset_file(&path)?;
                return Ok(Some(ruleset));
            }
        }

        Ok(None)
    }

    /// Rollback to a specific version
    pub fn rollback_to_version(
        &mut self,
        name: &str,
        seq: u32,
    ) -> io::Result<Option<(String, String)>> {
        // Get the version to rollback to
        let version_ruleset = match self.get_version(name, seq)? {
            Some(r) => r,
            None => return Ok(None),
        };

        // Get current version for response
        let from_version = self
            .get(name)
            .map(|r| r.config.version.clone())
            .unwrap_or_default();
        let to_version = version_ruleset.config.version.clone();

        // Backup current version first
        self.backup_current_version(name)?;

        // Persist the rolled-back version as current
        self.persist_ruleset(name, &version_ruleset)?;

        // Update memory cache
        self.rulesets
            .insert(name.to_string(), Arc::new(version_ruleset));

        // Cleanup old versions
        self.cleanup_old_versions(name)?;

        info!(
            "Rolled back '{}' from {} to {} (seq {})",
            name, from_version, to_version, seq
        );

        Ok(Some((from_version, to_version)))
    }

    /// Add or update a ruleset
    ///
    /// If persistence is enabled, the ruleset is also written to disk.
    /// If the rule already exists, the current version is backed up first.
    pub fn put(&mut self, ruleset: RuleSet) -> Result<(), Vec<String>> {
        // Validate before storing
        ruleset.validate()?;
        let name = ruleset.config.name.clone();

        // Backup current version if it exists (for version history)
        if self.rules_dir.is_some() && self.exists(&name) {
            if let Err(e) = self.backup_current_version(&name) {
                warn!("Failed to backup current version of '{}': {}", name, e);
                // Continue anyway - backup failure shouldn't block update
            }
        }

        // Persist to disk if enabled
        if let Err(e) = self.persist_ruleset(&name, &ruleset) {
            error!("Failed to persist rule '{}': {}", name, e);
            return Err(vec![format!("Persistence error: {}", e)]);
        }

        // Cleanup old versions beyond the limit
        if self.rules_dir.is_some() {
            if let Err(e) = self.cleanup_old_versions(&name) {
                warn!("Failed to cleanup old versions of '{}': {}", name, e);
            }
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
    /// If persistence is enabled, the ruleset file and all version files are deleted from disk.
    pub fn delete(&mut self, name: &str) -> bool {
        let existed = self.rulesets.remove(name).is_some();

        if existed {
            // Delete current file
            if let Err(e) = self.delete_file(name) {
                error!("Failed to delete rule file for '{}': {}", name, e);
            }

            // Delete all version files
            if let Err(e) = self.delete_all_versions(name) {
                error!("Failed to delete version files for '{}': {}", name, e);
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
    #[allow(dead_code)]
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

/// Format a SystemTime as ISO 8601 string
fn format_system_time(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
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

    // ==================== Version Management Tests ====================

    fn create_test_ruleset_with_version(name: &str, version: &str) -> RuleSet {
        let mut ruleset = RuleSet::new(name, "start");
        ruleset.config.version = version.to_string();
        ruleset.add_step(Step::terminal(
            "start",
            "Start",
            TerminalResult::new("OK").with_message(format!("Version {}", version)),
        ));
        ruleset
    }

    #[test]
    fn test_version_backup_on_update() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);

        // Create initial version
        let v1 = create_test_ruleset_with_version("versioned-rule", "1.0.0");
        store.put(v1).unwrap();

        // Update to version 2
        let v2 = create_test_ruleset_with_version("versioned-rule", "2.0.0");
        store.put(v2).unwrap();

        // Check that version file was created
        let version_file = rules_dir.join("versioned-rule.v1.json");
        assert!(version_file.exists(), "Version backup file should exist");

        // Check current version is 2.0.0
        let current = store.get("versioned-rule").unwrap();
        assert_eq!(current.config.version, "2.0.0");
    }

    #[test]
    fn test_list_versions() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);

        // Create and update multiple times
        for i in 1..=4 {
            let ruleset = create_test_ruleset_with_version("multi-version", &format!("{}.0.0", i));
            store.put(ruleset).unwrap();
        }

        // List versions
        let versions = store.list_versions("multi-version").unwrap();

        assert_eq!(versions.current_version, "4.0.0");
        assert_eq!(versions.versions.len(), 3); // 3 historical versions (1, 2, 3)

        // Check versions are in descending order
        assert_eq!(versions.versions[0].seq, 3);
        assert_eq!(versions.versions[1].seq, 2);
        assert_eq!(versions.versions[2].seq, 1);
    }

    #[test]
    fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);

        // Create versions
        for i in 1..=3 {
            let ruleset = create_test_ruleset_with_version("rollback-test", &format!("{}.0.0", i));
            store.put(ruleset).unwrap();
        }

        // Current is 3.0.0
        assert_eq!(store.get("rollback-test").unwrap().config.version, "3.0.0");

        // Rollback to version 1 (seq 1)
        let result = store.rollback_to_version("rollback-test", 1).unwrap();
        assert!(result.is_some());

        let (from, to) = result.unwrap();
        assert_eq!(from, "3.0.0");
        assert_eq!(to, "1.0.0");

        // Verify current is now 1.0.0
        assert_eq!(store.get("rollback-test").unwrap().config.version, "1.0.0");
    }

    #[test]
    fn test_version_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        // Only keep 3 versions
        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 3);

        // Create 6 versions
        for i in 1..=6 {
            let ruleset = create_test_ruleset_with_version("cleanup-test", &format!("{}.0.0", i));
            store.put(ruleset).unwrap();
        }

        // Should only have 3 historical versions (4, 5, 6 are kept, current is 6)
        let versions = store.list_versions("cleanup-test").unwrap();
        assert_eq!(versions.versions.len(), 3);

        // Oldest kept should be v3
        let seqs: Vec<u32> = versions.versions.iter().map(|v| v.seq).collect();
        assert!(seqs.contains(&5));
        assert!(seqs.contains(&4));
        assert!(seqs.contains(&3));
        assert!(!seqs.contains(&1)); // v1 should be deleted
        assert!(!seqs.contains(&2)); // v2 should be deleted
    }

    #[test]
    fn test_delete_removes_all_versions() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);

        // Create versions
        for i in 1..=3 {
            let ruleset =
                create_test_ruleset_with_version("delete-versions", &format!("{}.0.0", i));
            store.put(ruleset).unwrap();
        }

        // Verify version files exist
        assert!(rules_dir.join("delete-versions.v1.json").exists());
        assert!(rules_dir.join("delete-versions.v2.json").exists());

        // Delete the rule
        store.delete("delete-versions");

        // All files should be gone
        assert!(!rules_dir.join("delete-versions.json").exists());
        assert!(!rules_dir.join("delete-versions.v1.json").exists());
        assert!(!rules_dir.join("delete-versions.v2.json").exists());
    }

    #[test]
    fn test_version_files_not_loaded_as_rules() {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().to_path_buf();

        let mut store = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);

        // Create versions
        for i in 1..=3 {
            let ruleset = create_test_ruleset_with_version("load-test", &format!("{}.0.0", i));
            store.put(ruleset).unwrap();
        }

        // Create new store and load
        let mut store2 = RuleStore::new_with_persistence_and_versions(rules_dir.clone(), 10);
        let loaded = store2.load_from_dir().unwrap();

        // Should only load 1 rule (not the version files)
        assert_eq!(loaded, 1);
        assert!(store2.exists("load-test"));
        assert!(!store2.exists("load-test.v1")); // Version files should not be loaded as rules
    }
}
