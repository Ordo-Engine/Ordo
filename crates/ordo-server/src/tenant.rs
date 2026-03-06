//! Tenant management and configuration
//!
//! Provides tenant configs with optional file persistence.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub qps_limit: Option<u32>,
    pub burst_limit: Option<u32>,
    pub execution_timeout_ms: u64,
    pub max_rules: Option<usize>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TenantConfig {
    pub fn default_for_id(id: &str, defaults: &TenantDefaults) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            enabled: true,
            qps_limit: defaults.default_qps_limit,
            burst_limit: defaults.default_burst_limit,
            execution_timeout_ms: defaults.default_timeout_ms,
            max_rules: None,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TenantDefaults {
    pub default_qps_limit: Option<u32>,
    pub default_burst_limit: Option<u32>,
    pub default_timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TenantStore {
    path: PathBuf,
}

impl TenantStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn load(&self) -> io::Result<HashMap<String, TenantConfig>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }
        let data = tokio::fs::read(&self.path).await?;
        let tenants: HashMap<String, TenantConfig> = serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(tenants)
    }

    pub async fn save(&self, tenants: &HashMap<String, TenantConfig>) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_vec_pretty(tenants)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let temp_path = self.path.with_extension("tmp");
        tokio::fs::write(&temp_path, data).await?;
        tokio::fs::rename(&temp_path, &self.path).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TenantManager {
    tenants: RwLock<HashMap<String, TenantConfig>>,
    store: Option<TenantStore>,
    defaults: TenantDefaults,
}

impl TenantManager {
    pub async fn new(store: Option<TenantStore>, defaults: TenantDefaults) -> io::Result<Self> {
        let tenants = if let Some(ref store) = store {
            store.load().await?
        } else {
            HashMap::new()
        };
        Ok(Self {
            tenants: RwLock::new(tenants),
            store,
            defaults,
        })
    }

    pub fn defaults(&self) -> &TenantDefaults {
        &self.defaults
    }

    pub async fn ensure_default(&self, tenant_id: &str) -> io::Result<()> {
        let mut guard = self.tenants.write().await;
        if !guard.contains_key(tenant_id) {
            let config = TenantConfig::default_for_id(tenant_id, &self.defaults);
            guard.insert(tenant_id.to_string(), config);
            if let Some(store) = &self.store {
                store.save(&guard).await?;
            }
            info!("Created default tenant '{}'", tenant_id);
        }
        Ok(())
    }

    pub async fn list(&self) -> Vec<TenantConfig> {
        let guard = self.tenants.read().await;
        guard.values().cloned().collect()
    }

    pub async fn get(&self, tenant_id: &str) -> Option<TenantConfig> {
        let guard = self.tenants.read().await;
        guard.get(tenant_id).cloned()
    }

    pub async fn upsert(&self, config: TenantConfig) -> io::Result<()> {
        let mut guard = self.tenants.write().await;
        guard.insert(config.id.clone(), config);
        if let Some(store) = &self.store {
            store.save(&guard).await?;
        }
        Ok(())
    }

    pub async fn delete(&self, tenant_id: &str) -> io::Result<bool> {
        let mut guard = self.tenants.write().await;
        let existed = guard.remove(tenant_id).is_some();
        if existed {
            if let Some(store) = &self.store {
                store.save(&guard).await?;
            }
        }
        Ok(existed)
    }

    pub async fn validate_enabled(&self, tenant_id: &str) -> Result<TenantConfig, String> {
        match self.get(tenant_id).await {
            Some(config) if config.enabled => Ok(config),
            Some(_) => Err(format!("Tenant '{}' is disabled", tenant_id)),
            None => Err(format!("Tenant '{}' not found", tenant_id)),
        }
    }

    /// Reload tenant configuration from the backing store.
    ///
    /// This is called by the file watcher when `tenants.json` changes on disk.
    /// The in-memory map is replaced atomically; existing references obtained
    /// before the reload are unaffected (they are clones).
    pub async fn reload(&self) -> io::Result<()> {
        let store = match &self.store {
            Some(s) => s,
            None => return Ok(()),
        };
        let new_tenants = store.load().await?;
        let count = new_tenants.len();
        let mut guard = self.tenants.write().await;
        *guard = new_tenants;
        info!("Reloaded tenant config ({} tenants)", count);
        Ok(())
    }

    /// Returns the store path (if configured) so the file watcher can monitor it.
    pub fn store_path(&self) -> Option<&Path> {
        self.store.as_ref().map(|s| s.path.as_path())
    }
}

pub fn default_tenant_store_path(rules_dir: &Path) -> PathBuf {
    rules_dir.join("tenants").join("tenants.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_tenant_store_persistence() {
        let temp = TempDir::new().unwrap();
        let store = TenantStore::new(temp.path().join("tenants.json"));
        let defaults = TenantDefaults {
            default_qps_limit: Some(100),
            default_burst_limit: Some(10),
            default_timeout_ms: 100,
        };

        let manager = TenantManager::new(Some(store.clone()), defaults)
            .await
            .unwrap();
        manager.ensure_default("default").await.unwrap();

        let list = manager.list().await;
        assert_eq!(list.len(), 1);

        let store2 = TenantStore::new(temp.path().join("tenants.json"));
        let manager2 = TenantManager::new(Some(store2), manager.defaults().clone())
            .await
            .unwrap();
        let list2 = manager2.list().await;
        assert_eq!(list2.len(), 1);
    }

    #[tokio::test]
    async fn test_tenant_reload() {
        let temp = TempDir::new().unwrap();
        let store_path = temp.path().join("tenants.json");
        let store = TenantStore::new(store_path.clone());
        let defaults = TenantDefaults {
            default_qps_limit: None,
            default_burst_limit: None,
            default_timeout_ms: 100,
        };

        let manager = TenantManager::new(Some(store), defaults).await.unwrap();
        manager.ensure_default("default").await.unwrap();
        assert_eq!(manager.list().await.len(), 1);

        // Externally add a second tenant to the file
        let mut tenants = HashMap::new();
        tenants.insert(
            "default".to_string(),
            TenantConfig::default_for_id("default", manager.defaults()),
        );
        tenants.insert(
            "new-tenant".to_string(),
            TenantConfig::default_for_id("new-tenant", manager.defaults()),
        );
        let data = serde_json::to_vec_pretty(&tenants).unwrap();
        tokio::fs::write(&store_path, data).await.unwrap();

        // Reload and verify
        manager.reload().await.unwrap();
        assert_eq!(manager.list().await.len(), 2);
        assert!(manager.get("new-tenant").await.is_some());
    }
}
