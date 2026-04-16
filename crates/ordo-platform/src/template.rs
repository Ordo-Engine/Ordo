//! Rule Template loader (M1.1 "First Decision" milestone).
//!
//! Templates live under a configurable directory. Each template is a
//! **self-contained subdirectory** — no shared manifest or global locale files:
//!
//! ```text
//! templates/
//!   ecommerce-coupon/
//!     meta.json        # TemplateMetadata for this template
//!     facts.json
//!     concepts.json
//!     ruleset.json
//!     samples.json
//!     contract.json    # optional
//!     i18n/
//!       en.json        # { "key": "text", ... }
//!       zh-CN.json
//!       zh-TW.json
//!   another-template/
//!     meta.json
//!     ...
//! ```
//!
//! Discovery: every subdirectory that contains a `meta.json` file is treated as
//! a template. Directories without `meta.json` are silently skipped.
//!
//! User-facing strings inside the JSON are written as `i18n:<key>` sentinels;
//! `apply_i18n` recursively rewrites them at API time based on the caller's
//! `Accept-Language`. Each template's `i18n/` files are merged into the store's
//! locale map at load time — keys are template-namespaced, so there are no
//! cross-template collisions.
//!
//! Design notes
//! - Missing templates dir or malformed files **must not** panic the platform —
//!   log a warning and fall back to an empty store so deployments without
//!   templates keep working.
//! - `fallback_locale` is always `en`; unknown locales degrade gracefully.
//! - Adding a new template only requires a new subdirectory — no global file
//!   needs to be edited.

use crate::models::{
    ConceptDefinition, DecisionContract, FactDefinition, TemplateDetail, TemplateMetadata,
    TemplateSample, TestCase,
};
use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;

const FALLBACK_LOCALE: &str = "en";
const I18N_PREFIX: &str = "i18n:";

/// Supported locales — kept small & explicit so we don't accidentally serve
/// a locale the UI can't render.
pub const SUPPORTED_LOCALES: &[&str] = &["en", "zh-CN", "zh-TW"];

#[derive(Debug, Clone)]
struct LoadedTemplate {
    metadata: TemplateMetadata,
    facts: Vec<FactDefinition>,
    concepts: Vec<ConceptDefinition>,
    ruleset: JsonValue,
    samples: Vec<TemplateSample>,
    contract: Option<DecisionContract>,
    tests: Vec<TestCase>,
}

/// In-memory store of all loaded templates + their i18n bundles.
#[derive(Debug, Clone, Default)]
pub struct TemplateStore {
    templates: HashMap<String, LoadedTemplate>,
    /// locale -> (key -> localized text)
    i18n: HashMap<String, HashMap<String, String>>,
}

impl TemplateStore {
    /// Load all templates under `dir` by scanning for subdirectories that
    /// contain a `meta.json` file.
    ///
    /// Returns an empty store (not an error) if `dir` does not exist —
    /// templates are an optional feature for the platform.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        if !dir.exists() {
            tracing::info!(
                "Templates directory {:?} does not exist — template system disabled",
                dir
            );
            return Ok(Self::default());
        }

        let mut store = Self::default();

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Reading templates directory {:?}", dir))?;

        for entry in entries {
            let entry = entry.with_context(|| format!("Iterating {:?}", dir))?;
            let tpl_dir = entry.path();

            if !tpl_dir.is_dir() {
                continue;
            }

            let meta_path = tpl_dir.join("meta.json");
            if !meta_path.exists() {
                // Not a template directory — skip silently.
                continue;
            }

            // Load this template's i18n bundles and merge them into the store.
            let i18n_dir = tpl_dir.join("i18n");
            if i18n_dir.exists() {
                for locale in SUPPORTED_LOCALES {
                    let path = i18n_dir.join(format!("{}.json", locale));
                    match std::fs::read_to_string(&path) {
                        Ok(text) => match serde_json::from_str::<HashMap<String, String>>(&text) {
                            Ok(map) => {
                                store
                                    .i18n
                                    .entry((*locale).to_string())
                                    .or_default()
                                    .extend(map);
                            }
                            Err(e) => tracing::warn!(
                                "Failed to parse i18n bundle {:?}: {} — falling back to key",
                                path,
                                e
                            ),
                        },
                        Err(_) => tracing::warn!("i18n bundle {:?} not found", path),
                    }
                }
            }

            // Load meta.json → TemplateMetadata
            let meta_text = match std::fs::read_to_string(&meta_path) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("Failed to read {:?}: {} — skipping", meta_path, e);
                    continue;
                }
            };
            let meta: TemplateMetadata = match serde_json::from_str(&meta_text)
                .with_context(|| format!("Parsing {:?}", meta_path))
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("{:#} — skipping", e);
                    continue;
                }
            };

            // Load the remaining data files for this template.
            match load_one_template(&tpl_dir, meta.clone()) {
                Ok(tpl) => {
                    tracing::info!("Loaded template '{}'", tpl.metadata.id);
                    store.templates.insert(tpl.metadata.id.clone(), tpl);
                }
                Err(e) => {
                    tracing::warn!("Failed to load template '{}': {:#}", meta.id, e);
                }
            }
        }

        tracing::info!(
            "Template store ready: {} templates, locales=[{}]",
            store.templates.len(),
            store.i18n.keys().cloned().collect::<Vec<_>>().join(", ")
        );

        Ok(store)
    }

    /// List all templates with i18n applied for the given locale.
    pub fn list(&self, locale: &str) -> Vec<TemplateMetadata> {
        self.templates
            .values()
            .map(|t| {
                let mut m = t.metadata.clone();
                self.translate_metadata(&mut m, locale);
                m
            })
            .collect()
    }

    /// Get a single template with i18n applied. Returns `None` if the id
    /// is unknown.
    pub fn get(&self, id: &str, locale: &str) -> Option<TemplateDetail> {
        let t = self.templates.get(id)?;

        let mut metadata = t.metadata.clone();
        self.translate_metadata(&mut metadata, locale);

        let mut facts = t.facts.clone();
        for f in &mut facts {
            if let Some(desc) = f.description.as_mut() {
                *desc = self.resolve_string(desc, locale);
            }
            f.source = self.resolve_string(&f.source, locale);
        }

        let mut concepts = t.concepts.clone();
        for c in &mut concepts {
            if let Some(desc) = c.description.as_mut() {
                *desc = self.resolve_string(desc, locale);
            }
        }

        let mut ruleset = t.ruleset.clone();
        self.apply_i18n(&mut ruleset, locale);

        let mut samples = t.samples.clone();
        for s in &mut samples {
            s.label = self.resolve_string(&s.label, locale);
            if let Some(r) = s.expected_result.as_mut() {
                *r = self.resolve_string(r, locale);
            }
        }

        let contract = t.contract.as_ref().map(|c| {
            let mut c = c.clone();
            for f in &mut c.input_fields {
                if let Some(desc) = f.description.as_mut() {
                    *desc = self.resolve_string(desc, locale);
                }
            }
            for f in &mut c.output_fields {
                if let Some(desc) = f.description.as_mut() {
                    *desc = self.resolve_string(desc, locale);
                }
            }
            if let Some(notes) = c.notes.as_mut() {
                *notes = self.resolve_string(notes, locale);
            }
            c
        });

        // Apply i18n to test case names and descriptions
        let mut tests = t.tests.clone();
        for tc in &mut tests {
            tc.name = self.resolve_string(&tc.name, locale);
            if let Some(desc) = tc.description.as_mut() {
                *desc = self.resolve_string(desc, locale);
            }
        }

        Some(TemplateDetail {
            metadata,
            facts,
            concepts,
            ruleset,
            samples,
            contract,
            tests,
        })
    }

    /// Raw (un-localised) template — used by the from-template endpoint
    /// when we want to persist canonical fact/concept definitions and push
    /// the ruleset to the engine without baked-in localised strings.
    pub fn get_raw(&self, id: &str) -> Option<TemplateDetail> {
        let t = self.templates.get(id)?;
        Some(TemplateDetail {
            metadata: t.metadata.clone(),
            facts: t.facts.clone(),
            concepts: t.concepts.clone(),
            ruleset: t.ruleset.clone(),
            samples: t.samples.clone(),
            contract: t.contract.clone(),
            tests: t.tests.clone(),
        })
    }

    // ── internals ───────────────────────────────────────────────────────────

    fn translate_metadata(&self, m: &mut TemplateMetadata, locale: &str) {
        m.name = self.resolve_string(&m.name, locale);
        m.description = self.resolve_string(&m.description, locale);
        m.tags = m
            .tags
            .iter()
            .map(|t| self.resolve_string(t, locale))
            .collect();
        m.features = m
            .features
            .iter()
            .map(|f| self.resolve_string(f, locale))
            .collect();
    }

    /// Recursively walk a serde_json::Value, rewriting any string of the
    /// form `i18n:<key>` with its localised value (or the raw key if the
    /// key is unknown in both the requested and fallback locales).
    fn apply_i18n(&self, value: &mut JsonValue, locale: &str) {
        match value {
            JsonValue::String(s) => {
                if let Some(new) = self.maybe_translate(s, locale) {
                    *s = new;
                }
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.apply_i18n(v, locale);
                }
            }
            JsonValue::Object(map) => {
                for (_, v) in map.iter_mut() {
                    self.apply_i18n(v, locale);
                }
            }
            _ => {}
        }
    }

    /// Resolve a single `i18n:<key>` token. If `s` is not a token, returns `s`
    /// unchanged.
    fn resolve_string(&self, s: &str, locale: &str) -> String {
        self.maybe_translate(s, locale)
            .unwrap_or_else(|| s.to_string())
    }

    fn maybe_translate(&self, s: &str, locale: &str) -> Option<String> {
        let key = s.strip_prefix(I18N_PREFIX)?;
        if let Some(bundle) = self.i18n.get(locale) {
            if let Some(v) = bundle.get(key) {
                return Some(v.clone());
            }
        }
        if locale != FALLBACK_LOCALE {
            if let Some(bundle) = self.i18n.get(FALLBACK_LOCALE) {
                if let Some(v) = bundle.get(key) {
                    return Some(v.clone());
                }
            }
        }
        tracing::warn!("i18n key not found: {} (locale={})", key, locale);
        Some(key.to_string()) // show the key, not the `i18n:` prefix
    }
}

fn load_one_template(tpl_dir: &Path, metadata: TemplateMetadata) -> Result<LoadedTemplate> {
    let facts_text = std::fs::read_to_string(tpl_dir.join("facts.json"))
        .with_context(|| format!("Reading facts.json for template '{}'", metadata.id))?;
    let facts: Vec<FactDefinition> = serde_json::from_str(&facts_text)
        .with_context(|| format!("Parsing facts.json for template '{}'", metadata.id))?;

    let concepts_text = std::fs::read_to_string(tpl_dir.join("concepts.json"))
        .with_context(|| format!("Reading concepts.json for template '{}'", metadata.id))?;
    let concepts: Vec<ConceptDefinition> = serde_json::from_str(&concepts_text)
        .with_context(|| format!("Parsing concepts.json for template '{}'", metadata.id))?;

    let ruleset_text = std::fs::read_to_string(tpl_dir.join("ruleset.json"))
        .with_context(|| format!("Reading ruleset.json for template '{}'", metadata.id))?;
    let ruleset: JsonValue = serde_json::from_str(&ruleset_text)
        .with_context(|| format!("Parsing ruleset.json for template '{}'", metadata.id))?;

    let samples_text = std::fs::read_to_string(tpl_dir.join("samples.json"))
        .with_context(|| format!("Reading samples.json for template '{}'", metadata.id))?;
    let samples: Vec<TemplateSample> = serde_json::from_str(&samples_text)
        .with_context(|| format!("Parsing samples.json for template '{}'", metadata.id))?;

    // contract.json is optional — templates that don't define a contract are valid.
    let contract_path = tpl_dir.join("contract.json");
    let contract: Option<DecisionContract> = if contract_path.exists() {
        let text = std::fs::read_to_string(&contract_path)
            .with_context(|| format!("Reading contract.json for template '{}'", metadata.id))?;
        Some(
            serde_json::from_str(&text)
                .with_context(|| format!("Parsing contract.json for template '{}'", metadata.id))?,
        )
    } else {
        None
    };

    // tests.json is optional — templates that don't include test cases are valid.
    let tests_path = tpl_dir.join("tests.json");
    let tests: Vec<TestCase> = if tests_path.exists() {
        let text = std::fs::read_to_string(&tests_path)
            .with_context(|| format!("Reading tests.json for template '{}'", metadata.id))?;
        serde_json::from_str(&text)
            .with_context(|| format!("Parsing tests.json for template '{}'", metadata.id))?
    } else {
        Vec::new()
    };

    Ok(LoadedTemplate {
        metadata,
        facts,
        concepts,
        ruleset,
        samples,
        contract,
        tests,
    })
}

/// Extract a supported locale from an HTTP `Accept-Language` header.
/// Returns the first supported match, or `en` as the fallback.
pub fn extract_locale(raw: Option<&str>) -> &'static str {
    let Some(raw) = raw else {
        return FALLBACK_LOCALE;
    };
    for part in raw.split(',') {
        let tag = part.split(';').next().unwrap_or("").trim();
        for loc in SUPPORTED_LOCALES {
            if tag.eq_ignore_ascii_case(loc) {
                return loc;
            }
        }
        // Accept `zh` as zh-CN
        if tag.eq_ignore_ascii_case("zh") {
            return "zh-CN";
        }
    }
    FALLBACK_LOCALE
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store_with_bundles() -> TemplateStore {
        let mut store = TemplateStore::default();
        let mut en = HashMap::new();
        en.insert("tpl.name".to_string(), "Coupon".to_string());
        en.insert("tpl.desc".to_string(), "A coupon template".to_string());
        let mut zh = HashMap::new();
        zh.insert("tpl.name".to_string(), "优惠券".to_string());
        store.i18n.insert("en".to_string(), en);
        store.i18n.insert("zh-CN".to_string(), zh);
        store
    }

    #[test]
    fn apply_i18n_replaces_nested_strings() {
        let store = store_with_bundles();
        let mut v = json!({
            "config": {"name": "i18n:tpl.name"},
            "list": ["i18n:tpl.desc", "raw"]
        });
        store.apply_i18n(&mut v, "en");
        assert_eq!(v["config"]["name"], "Coupon");
        assert_eq!(v["list"][0], "A coupon template");
        assert_eq!(v["list"][1], "raw");
    }

    #[test]
    fn i18n_falls_back_to_en_when_locale_missing_key() {
        let store = store_with_bundles();
        // zh-CN bundle doesn't have tpl.desc — should fall back to en
        let out = store.resolve_string("i18n:tpl.desc", "zh-CN");
        assert_eq!(out, "A coupon template");
    }

    #[test]
    fn i18n_uses_locale_when_present() {
        let store = store_with_bundles();
        let out = store.resolve_string("i18n:tpl.name", "zh-CN");
        assert_eq!(out, "优惠券");
    }

    #[test]
    fn i18n_keeps_raw_string_when_no_prefix() {
        let store = store_with_bundles();
        let out = store.resolve_string("plain text", "en");
        assert_eq!(out, "plain text");
    }

    #[test]
    fn extract_locale_picks_first_supported_match() {
        assert_eq!(extract_locale(Some("zh-CN,en;q=0.8")), "zh-CN");
        assert_eq!(extract_locale(Some("en-US,en;q=0.9")), "en");
        assert_eq!(extract_locale(Some("fr;q=1.0")), "en");
        assert_eq!(extract_locale(Some("zh")), "zh-CN");
        assert_eq!(extract_locale(None), "en");
    }

    #[test]
    fn empty_dir_returns_empty_store() {
        let tmp = std::env::temp_dir().join("ordo-template-empty-test");
        let _ = std::fs::remove_dir_all(&tmp);
        let store = TemplateStore::load_from_dir(&tmp).unwrap();
        assert_eq!(store.list("en").len(), 0);
    }

    #[test]
    fn load_from_dir_loads_template_with_per_template_i18n() {
        use std::fs;

        let tmp = std::env::temp_dir().join("ordo-template-load-test");
        let _ = fs::remove_dir_all(&tmp);

        let tpl_dir = tmp.join("test-tpl");
        let i18n_dir = tpl_dir.join("i18n");
        fs::create_dir_all(&i18n_dir).unwrap();

        fs::write(
            tpl_dir.join("meta.json"),
            r#"{"id":"test-tpl","name":"i18n:tpl.name","description":"desc","tags":[],"icon":"","difficulty":"beginner","features":[]}"#,
        )
        .unwrap();
        fs::write(i18n_dir.join("en.json"), r#"{"tpl.name":"Test Template"}"#).unwrap();
        fs::write(tpl_dir.join("facts.json"), "[]").unwrap();
        fs::write(tpl_dir.join("concepts.json"), "[]").unwrap();
        fs::write(
            tpl_dir.join("ruleset.json"),
            r#"{"config":{"name":"t","entry_step":"s","tenant_id":""},"steps":{"s":{"kind":{"Terminal":{"code":"OK","message":"","outputs":{}}}}}}"#,
        )
        .unwrap();
        fs::write(tpl_dir.join("samples.json"), "[]").unwrap();

        let store = TemplateStore::load_from_dir(&tmp).unwrap();
        assert_eq!(store.list("en").len(), 1);
        let meta = &store.list("en")[0];
        assert_eq!(meta.name, "Test Template");

        let _ = fs::remove_dir_all(&tmp);
    }
}
