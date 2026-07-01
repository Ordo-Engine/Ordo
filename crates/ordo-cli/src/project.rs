//! An Ordo decision project on disk — a folder of files that the CLI (and any
//! coding agent) reads and edits like source code.
//!
//! Layout (mirrors the Studio project model):
//! ```text
//! ordo.yaml              project + link config
//! rulesets/<name>.json   a ruleset in studio format
//! facts.json             fact catalog
//! concepts.json          concept catalog
//! tests/<name>.json      test cases for a ruleset
//! contracts/<name>.json  decision contract
//! AGENTS.md              coding-agent guidance
//! ```
//!
//! Local ops (validate/run/trace/test) resolve a project ruleset to an engine
//! `RuleSet` entirely offline — studio→engine conversion plus concept
//! materialization from `concepts.json`, the same pipeline the platform runs.

use anyhow::{Context, Result};
use ordo_core::prelude::RuleSet;
use ordo_studio_format::{
    engine_to_studio, studio_draft_to_engine_with_concepts, ConceptDefinition, StudioRuleSet,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "ordo.yaml";

/// Normalize a ruleset identifier the user might type — a bare name, a
/// `rulesets/<name>.json` path, or `<name>.json` — down to the bare name.
pub fn ruleset_name(id: &str) -> String {
    let s = id.strip_prefix("rulesets/").unwrap_or(id);
    s.strip_suffix(".json").unwrap_or(s).to_string()
}

/// Project + link configuration, persisted as `ordo.yaml`. The link fields are
/// populated by `ordo link` (Phase 2) and are safe to commit (ids, not secrets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    /// Environment name → id aliases, cached by `link` to resolve `--env`.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub environments: std::collections::BTreeMap<String, String>,
}

/// A resolved project rooted at the directory containing `ordo.yaml`.
pub struct Project {
    pub root: PathBuf,
    /// Parsed `ordo.yaml`. Consumed by the platform-sync commands (Phase 2).
    #[allow(dead_code)]
    pub config: ProjectConfig,
}

impl Project {
    /// Walk up from `start` (default: cwd) until a directory holds `ordo.yaml`.
    pub fn discover(start: Option<&Path>) -> Result<Project> {
        let start = match start {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir().context("cannot determine current directory")?,
        };
        let mut dir = start.as_path();
        loop {
            let cfg = dir.join(CONFIG_FILE);
            if cfg.is_file() {
                let text = std::fs::read_to_string(&cfg)
                    .with_context(|| format!("failed to read {}", cfg.display()))?;
                let config: ProjectConfig = serde_yaml::from_str(&text)
                    .with_context(|| format!("invalid {}", CONFIG_FILE))?;
                return Ok(Project {
                    root: dir.to_path_buf(),
                    config,
                });
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => anyhow::bail!(
                    "not inside an Ordo project (no {} found in this or any parent directory). Run `ordo init` first.",
                    CONFIG_FILE
                ),
            }
        }
    }

    pub fn rulesets_dir(&self) -> PathBuf {
        self.root.join("rulesets")
    }
    pub fn tests_dir(&self) -> PathBuf {
        self.root.join("tests")
    }
    pub fn ruleset_path(&self, name: &str) -> PathBuf {
        self.rulesets_dir().join(format!("{name}.json"))
    }
    pub fn tests_path(&self, name: &str) -> PathBuf {
        self.tests_dir().join(format!("{name}.json"))
    }
    pub fn facts_path(&self) -> PathBuf {
        self.root.join("facts.json")
    }
    pub fn concepts_path(&self) -> PathBuf {
        self.root.join("concepts.json")
    }

    /// Every ruleset name (sorted) — the stems of `rulesets/*.json`.
    pub fn ruleset_names(&self) -> Result<Vec<String>> {
        let dir = self.rulesets_dir();
        let mut names = Vec::new();
        if dir.is_dir() {
            for entry in std::fs::read_dir(&dir)
                .with_context(|| format!("failed to read {}", dir.display()))?
            {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load `concepts.json` (empty if absent). Extra fields (server timestamps)
    /// are ignored.
    pub fn load_concepts(&self) -> Result<Vec<ConceptDefinition>> {
        let path = self.concepts_path();
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))
    }

    /// Read a ruleset file as a `StudioRuleSet`, normalizing an engine-format
    /// file (steps as an object — e.g. a template) into studio format on read.
    pub fn load_studio(&self, name: &str) -> Result<StudioRuleSet> {
        let path = self.ruleset_path(name);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("ruleset '{name}' not found ({})", path.display()))?;
        let raw: serde_json::Value =
            serde_json::from_str(&text).with_context(|| format!("invalid JSON in {name}"))?;
        if raw.get("steps").map(|s| s.is_object()).unwrap_or(false) {
            // Engine format (steps as a map) → normalize to studio.
            let engine: RuleSet = serde_json::from_value(raw)
                .with_context(|| format!("invalid engine-format ruleset '{name}'"))?;
            Ok(engine_to_studio(&engine))
        } else {
            serde_json::from_value(raw)
                .with_context(|| format!("invalid studio-format ruleset '{name}'"))
        }
    }

    /// The project's virtual file tree (the paths an agent sees), mirroring the
    /// Studio file list: config + catalogs + each ruleset's files.
    pub fn list_files(&self) -> Vec<String> {
        let mut files = vec![
            "ordo.yaml".to_string(),
            "facts.json".to_string(),
            "concepts.json".to_string(),
        ];
        if let Ok(names) = self.ruleset_names() {
            for n in &names {
                files.push(format!("rulesets/{n}.json"));
                if self.tests_path(n).is_file() {
                    files.push(format!("tests/{n}.json"));
                }
                if self
                    .root
                    .join("contracts")
                    .join(format!("{n}.json"))
                    .is_file()
                {
                    files.push(format!("contracts/{n}.json"));
                }
            }
        }
        files
    }

    /// Resolve a project-relative path, rejecting traversal outside the root.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        if rel.split(['/', '\\']).any(|seg| seg == "..") {
            anyhow::bail!("path must stay within the project: {rel}");
        }
        Ok(self.root.join(rel))
    }

    /// Build the executable engine `RuleSet` for a ruleset: studio→engine plus
    /// concept materialization from `concepts.json`. This mirrors the platform's
    /// `convert` endpoint, offline.
    pub fn load_engine(&self, name: &str) -> Result<RuleSet> {
        let studio = self.load_studio(name)?;
        let concepts = self.load_concepts()?;
        studio_draft_to_engine_with_concepts(&studio, &concepts).map_err(|e| anyhow::anyhow!("{e}"))
    }
}
