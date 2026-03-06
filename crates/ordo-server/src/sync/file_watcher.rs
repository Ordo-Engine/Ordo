//! File-system watcher for live rule reload.
//!
//! Uses the `notify` crate to monitor `--rules-dir` for changes.
//! Debounces events (200 ms) and filters to `.json` / `.yaml` / `.yml` files.
//!
//! Writer instances maintain a `RecentWrites` set to suppress reloads for
//! files they just persisted themselves (self-write suppression).
//! If the watcher fails to start, falls back to a 30-second full-scan polling loop.

use crate::store::RuleStore;
use crate::tenant::TenantManager;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// How long to remember self-writes to suppress watcher echoes.
const SELF_WRITE_TTL: Duration = Duration::from_secs(2);

/// Polling interval when the native watcher is unavailable.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Debounce window for coalescing rapid file-system events.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(200);

/// Tracks files recently written by **this** process so the watcher
/// doesn't trigger a redundant reload.
#[derive(Debug)]
pub struct RecentWrites {
    entries: Mutex<Vec<(PathBuf, Instant)>>,
}

impl RecentWrites {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Record that we just wrote `path`. The entry expires after [`SELF_WRITE_TTL`].
    /// Called by `RuleStore::persist_ruleset` on writer instances to suppress watcher echoes.
    #[allow(dead_code)]
    pub async fn record(&self, path: PathBuf) {
        let mut entries = self.entries.lock().await;
        entries.retain(|(_, ts)| ts.elapsed() < SELF_WRITE_TTL);
        entries.push((path, Instant::now()));
    }

    /// Returns `true` if `path` was written by us recently.
    pub async fn contains(&self, path: &Path) -> bool {
        let mut entries = self.entries.lock().await;
        entries.retain(|(_, ts)| ts.elapsed() < SELF_WRITE_TTL);
        entries.iter().any(|(p, _)| p == path)
    }
}

/// Determines whether a path is a rule file we should react to.
fn is_rule_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "json" | "yaml" | "yml")
}

/// Returns true if the path looks like the tenant config file.
fn is_tenant_config(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "tenants.json")
        .unwrap_or(false)
}

/// Start the file watcher as a background task.
///
/// Returns a `JoinHandle` that runs until `shutdown_rx` fires.
/// The caller should `abort()` the handle during shutdown.
pub async fn start_file_watcher(
    rules_dir: PathBuf,
    store: Arc<RwLock<RuleStore>>,
    tenant_manager: Arc<TenantManager>,
    recent_writes: Arc<RecentWrites>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        match try_native_watcher(
            &rules_dir,
            store.clone(),
            tenant_manager.clone(),
            recent_writes.clone(),
            shutdown_rx.clone(),
        )
        .await
        {
            Ok(()) => {
                info!("File watcher stopped normally");
            }
            Err(e) => {
                warn!(
                    "Native file watcher failed ({}), falling back to {}s polling",
                    e,
                    POLL_INTERVAL.as_secs()
                );
                poll_fallback(&rules_dir, store, tenant_manager, &mut shutdown_rx).await;
            }
        }
    })
}

/// Try to set up a native OS file watcher via `notify`.
async fn try_native_watcher(
    rules_dir: &Path,
    store: Arc<RwLock<RuleStore>>,
    tenant_manager: Arc<TenantManager>,
    recent_writes: Arc<RecentWrites>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), notify::Error> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<Event, notify::Error>>(256);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let _ = tx.blocking_send(res);
        },
        Config::default(),
    )?;

    watcher.watch(rules_dir, RecursiveMode::Recursive)?;
    info!("File watcher started on {:?}", rules_dir);

    // Debounce: collect changed paths over a window, then process batch.
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut debounce_deadline: Option<Instant> = None;

    loop {
        let timeout = debounce_deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3600));

        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("File watcher: shutdown signal received");
                break;
            }
            event = tokio::time::timeout(timeout, rx.recv()) => {
                match event {
                    Ok(Some(Ok(ev))) => {
                        for path in ev.paths {
                            if !is_rule_file(&path) && !is_tenant_config(&path) {
                                continue;
                            }
                            match ev.kind {
                                EventKind::Create(_)
                                | EventKind::Modify(_)
                                | EventKind::Remove(_) => {
                                    pending.insert(path);
                                    debounce_deadline = Some(Instant::now() + DEBOUNCE_WINDOW);
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        warn!("File watcher error: {}", e);
                    }
                    Ok(None) => {
                        warn!("File watcher channel closed");
                        break;
                    }
                    Err(_) => {
                        // Timeout — debounce window elapsed, process pending
                        if !pending.is_empty() {
                            process_changes(
                                std::mem::take(&mut pending),
                                &store,
                                &tenant_manager,
                                &recent_writes,
                            )
                            .await;
                            debounce_deadline = None;
                        }
                    }
                }
            }
        }
    }

    // Process any remaining events
    if !pending.is_empty() {
        process_changes(pending, &store, &tenant_manager, &recent_writes).await;
    }

    Ok(())
}

/// Process a batch of file-change paths.
async fn process_changes(
    paths: HashSet<PathBuf>,
    store: &Arc<RwLock<RuleStore>>,
    tenant_manager: &Arc<TenantManager>,
    recent_writes: &Arc<RecentWrites>,
) {
    for path in paths {
        if recent_writes.contains(&path).await {
            debug!("Skipping self-written file {:?}", path);
            continue;
        }

        if is_tenant_config(&path) {
            if let Err(e) = tenant_manager.reload().await {
                error!("Failed to reload tenant config from {:?}: {}", path, e);
            }
            continue;
        }

        if path.exists() {
            let mut store_guard = store.write().await;
            if let Err(e) = store_guard.reload_file(&path) {
                error!("Failed to reload rule from {:?}: {}", path, e);
            }
        } else {
            let mut store_guard = store.write().await;
            if let Err(e) = store_guard.remove_by_path(&path) {
                error!("Failed to remove rule for deleted file {:?}: {}", path, e);
            }
        }
    }
}

/// Fallback: periodically reload all rules from disk.
async fn poll_fallback(
    rules_dir: &Path,
    store: Arc<RwLock<RuleStore>>,
    tenant_manager: Arc<TenantManager>,
    shutdown_rx: &mut watch::Receiver<bool>,
) {
    info!(
        "Polling {:?} every {}s for rule changes",
        rules_dir,
        POLL_INTERVAL.as_secs()
    );

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Poll fallback: shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {
                let mut store_guard = store.write().await;
                match store_guard.load_from_dir() {
                    Ok(count) => debug!("Poll reload: {} rules loaded", count),
                    Err(e) => error!("Poll reload failed: {}", e),
                }
                drop(store_guard);

                if let Err(e) = tenant_manager.reload().await {
                    error!("Poll tenant reload failed: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_rule_file() {
        assert!(is_rule_file(Path::new("rules/payment.json")));
        assert!(is_rule_file(Path::new("rules/payment.yaml")));
        assert!(is_rule_file(Path::new("rules/payment.yml")));
        assert!(!is_rule_file(Path::new("rules/payment.txt")));
        assert!(!is_rule_file(Path::new("rules/payment.toml")));
        assert!(!is_rule_file(Path::new("rules/.payment.tmp")));
    }

    #[test]
    fn test_is_tenant_config() {
        assert!(is_tenant_config(Path::new(
            "/data/rules/tenants/tenants.json"
        )));
        assert!(!is_tenant_config(Path::new("/data/rules/payment.json")));
    }

    #[tokio::test]
    async fn test_recent_writes() {
        let rw = RecentWrites::new();
        let path = PathBuf::from("/tmp/test.json");

        assert!(!rw.contains(&path).await);
        rw.record(path.clone()).await;
        assert!(rw.contains(&path).await);
    }
}
