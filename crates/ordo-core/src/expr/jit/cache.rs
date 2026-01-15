//! JIT Cache Implementation
//!
//! Provides L1 (memory) and L2 (disk) caching for JIT compiled functions.

use super::compiler::JITCompiler;
use crate::error::Result;
use crate::expr::profiler::JITPriority;
use crate::expr::Expr;
use crossbeam_channel::{bounded, Receiver, Sender};
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Configuration for the JIT cache
#[derive(Debug, Clone)]
pub struct JITCacheConfig {
    /// Maximum number of compiled functions to keep in memory (L1)
    pub max_memory_entries: usize,
    /// Whether to enable disk caching (L2)
    pub enable_disk_cache: bool,
    /// Directory for disk cache
    pub cache_dir: Option<PathBuf>,
    /// Number of background compilation threads
    pub num_compile_threads: usize,
    /// Compilation queue size
    pub queue_size: usize,
}

impl Default for JITCacheConfig {
    fn default() -> Self {
        Self {
            max_memory_entries: 1000,
            enable_disk_cache: true,
            cache_dir: None, // Will use ~/.cache/ordo/jit/
            num_compile_threads: 1,
            queue_size: 100,
        }
    }
}

/// A task for the background JIT compiler
#[derive(Debug, Clone)]
pub struct JITTask {
    /// Expression hash
    pub hash: u64,
    /// Expression to compile
    pub expr: Expr,
    /// Compilation priority
    pub priority: JITPriority,
    /// Timestamp when task was created
    pub created_at: Instant,
}

/// Internal control messages for background thread
enum ControlMessage {
    Compile(JITTask),
    Shutdown,
}

/// Statistics for the JIT cache
#[derive(Debug, Default, Clone)]
pub struct JITCacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of entries in L1 cache
    pub l1_entries: usize,
    /// Number of entries in L2 cache
    pub l2_entries: usize,
    /// Number of pending compilations
    pub pending_compilations: usize,
    /// Number of successful compilations
    pub successful_compilations: u64,
    /// Number of failed compilations
    pub failed_compilations: u64,
    /// Total compilation time in microseconds
    pub total_compile_time_us: u64,
}

/// L1 Memory Cache with LRU eviction
pub struct MemoryCache {
    /// LRU cache of compiled functions
    /// Key: expression hash
    /// Value: function pointer (as usize for storage)
    cache: Mutex<LruCache<u64, usize>>,
    /// Hit counter
    hits: AtomicU64,
    /// Miss counter
    misses: AtomicU64,
}

impl MemoryCache {
    /// Create a new memory cache
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            cache: Mutex::new(LruCache::new(cap)),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Get a compiled function by hash
    pub fn get(&self, hash: u64) -> Option<*const u8> {
        let mut cache = self.cache.lock();
        if let Some(&ptr) = cache.get(&hash) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(ptr as *const u8)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Insert a compiled function
    pub fn insert(&self, hash: u64, func_ptr: *const u8) {
        let mut cache = self.cache.lock();
        cache.put(hash, func_ptr as usize);
    }

    /// Check if an entry exists
    pub fn contains(&self, hash: u64) -> bool {
        self.cache.lock().contains(&hash)
    }

    /// Get cache statistics
    pub fn stats(&self) -> (u64, u64, usize) {
        let cache = self.cache.lock();
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            cache.len(),
        )
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.cache.lock().clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }
}

/// Background JIT Compilation System
///
/// Manages asynchronous compilation of expressions in a background thread.
pub struct BackgroundJIT {
    /// Task sender for submitting compilation requests
    task_sender: Sender<ControlMessage>,
    /// L1 memory cache
    memory_cache: Arc<MemoryCache>,
    /// Background thread handle
    worker_handle: Option<JoinHandle<()>>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Statistics
    stats: Arc<RwLock<JITCacheStats>>,
    /// Pending hashes (to avoid duplicate compilations)
    pending: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl BackgroundJIT {
    /// Create a new background JIT system
    pub fn new(config: JITCacheConfig) -> Result<Self> {
        let compiler = JITCompiler::new()?;
        let memory_cache = Arc::new(MemoryCache::new(config.max_memory_entries));
        let (task_sender, task_receiver) = bounded::<ControlMessage>(config.queue_size);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(RwLock::new(JITCacheStats::default()));
        let pending = Arc::new(Mutex::new(HashMap::new()));

        let compiler = Arc::new(Mutex::new(compiler));

        // Clone references for the worker thread
        let worker_compiler = Arc::clone(&compiler);
        let worker_cache = Arc::clone(&memory_cache);
        let worker_shutdown = Arc::clone(&shutdown);
        let worker_stats = Arc::clone(&stats);
        let worker_pending = Arc::clone(&pending);

        // Spawn background worker thread
        let worker_handle = thread::spawn(move || {
            Self::worker_loop(
                task_receiver,
                worker_compiler,
                worker_cache,
                worker_shutdown,
                worker_stats,
                worker_pending,
            );
        });

        Ok(Self {
            task_sender,
            memory_cache,
            worker_handle: Some(worker_handle),
            shutdown,
            stats,
            pending,
        })
    }

    /// Worker thread main loop
    fn worker_loop(
        receiver: Receiver<ControlMessage>,
        compiler: Arc<Mutex<JITCompiler>>,
        cache: Arc<MemoryCache>,
        shutdown: Arc<AtomicBool>,
        stats: Arc<RwLock<JITCacheStats>>,
        pending: Arc<Mutex<HashMap<u64, Instant>>>,
    ) {
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(ControlMessage::Compile(task)) => {
                    // Skip if already compiled
                    if cache.contains(task.hash) {
                        pending.lock().remove(&task.hash);
                        continue;
                    }

                    let start = Instant::now();

                    // Compile the expression and extract the function pointer
                    let compile_result = {
                        let mut compiler_guard = compiler.lock();
                        compiler_guard
                            .compile(&task.expr, task.hash)
                            .map(|compiled| compiled.func_ptr)
                    };

                    let compile_time = start.elapsed();

                    match compile_result {
                        Ok(func_ptr) => {
                            // Insert into memory cache
                            cache.insert(task.hash, func_ptr);

                            // Update stats
                            let mut stats_guard = stats.write();
                            stats_guard.successful_compilations += 1;
                            stats_guard.total_compile_time_us += compile_time.as_micros() as u64;
                            stats_guard.l1_entries = cache.stats().2;

                            tracing::debug!(
                                hash = task.hash,
                                time_us = compile_time.as_micros(),
                                "JIT compilation successful"
                            );
                        }
                        Err(e) => {
                            let mut stats_guard = stats.write();
                            stats_guard.failed_compilations += 1;

                            tracing::warn!(
                                hash = task.hash,
                                error = %e,
                                "JIT compilation failed"
                            );
                        }
                    }

                    // Remove from pending
                    pending.lock().remove(&task.hash);
                }
                Ok(ControlMessage::Shutdown) => {
                    break;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Continue checking shutdown flag
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        tracing::info!("Background JIT worker thread shutting down");
    }

    /// Submit an expression for background compilation
    pub fn submit(&self, expr: Expr, hash: u64, priority: JITPriority) -> bool {
        // Check if already compiled or pending
        if self.memory_cache.contains(hash) {
            return false;
        }

        {
            let mut pending = self.pending.lock();
            if pending.contains_key(&hash) {
                return false;
            }
            pending.insert(hash, Instant::now());
        }

        // Update pending count in stats
        {
            let mut stats = self.stats.write();
            stats.pending_compilations = self.pending.lock().len();
        }

        let task = JITTask {
            hash,
            expr,
            priority,
            created_at: Instant::now(),
        };

        match self.task_sender.try_send(ControlMessage::Compile(task)) {
            Ok(_) => true,
            Err(_) => {
                // Queue is full, remove from pending
                self.pending.lock().remove(&hash);
                false
            }
        }
    }

    /// Get a compiled function from cache
    pub fn get(&self, hash: u64) -> Option<*const u8> {
        let result = self.memory_cache.get(hash);

        // Update stats
        {
            let mut stats = self.stats.write();
            if result.is_some() {
                stats.hits += 1;
            } else {
                stats.misses += 1;
            }
        }

        result
    }

    /// Check if an expression is compiled
    pub fn is_compiled(&self, hash: u64) -> bool {
        self.memory_cache.contains(hash)
    }

    /// Check if an expression is pending compilation
    pub fn is_pending(&self, hash: u64) -> bool {
        self.pending.lock().contains_key(&hash)
    }

    /// Get cache statistics
    pub fn stats(&self) -> JITCacheStats {
        let mut stats = self.stats.read().clone();
        let (hits, misses, entries) = self.memory_cache.stats();
        stats.hits = hits;
        stats.misses = misses;
        stats.l1_entries = entries;
        stats.pending_compilations = self.pending.lock().len();
        stats
    }

    /// Shutdown the background JIT system
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        // Send shutdown message
        let _ = self.task_sender.try_send(ControlMessage::Shutdown);

        // Wait for worker thread to finish
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }

    /// Clear all caches
    pub fn clear(&self) {
        self.memory_cache.clear();
        self.pending.lock().clear();

        let mut stats = self.stats.write();
        *stats = JITCacheStats::default();
    }
}

impl Drop for BackgroundJIT {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ==================== L2 Disk Cache ====================

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, BufWriter};

/// Metadata for a cached expression (stored on disk)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryMetadata {
    /// Expression hash
    pub hash: u64,
    /// Source expression string (for display/debugging)
    pub source: String,
    /// Number of times accessed
    pub access_count: u64,
    /// Last compilation time in microseconds
    pub compile_time_us: u64,
    /// Timestamp of last access (seconds since epoch)
    pub last_accessed: u64,
    /// Ordo version that compiled this
    pub ordo_version: String,
    /// Target triple (e.g., "aarch64-apple-darwin")
    pub target_triple: String,
}

/// Index of all cached expressions
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiskCacheIndex {
    /// Version of the cache format
    pub format_version: u32,
    /// All cached entries
    pub entries: HashMap<u64, CacheEntryMetadata>,
}

impl DiskCacheIndex {
    /// Current format version
    pub const FORMAT_VERSION: u32 = 1;

    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            entries: HashMap::new(),
        }
    }
}

/// L2 Disk Cache for JIT metadata
///
/// Stores expression metadata for warm-up recompilation on startup.
/// Note: Native code cannot be directly persisted, so we store metadata
/// that allows quick recompilation of hot expressions.
pub struct DiskCache {
    /// Cache directory
    cache_dir: PathBuf,
    /// In-memory index
    index: RwLock<DiskCacheIndex>,
    /// Whether disk cache is enabled
    enabled: bool,
}

impl DiskCache {
    /// Create a new disk cache
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            dirs_cache_path().unwrap_or_else(|| PathBuf::from(".cache/ordo/jit"))
        });

        let mut cache = Self {
            cache_dir,
            index: RwLock::new(DiskCacheIndex::new()),
            enabled: true,
        };

        // Try to load existing index
        if let Err(e) = cache.load_index() {
            tracing::warn!(error = %e, "Failed to load JIT disk cache index");
        }

        cache
    }

    /// Create a disabled disk cache
    pub fn disabled() -> Self {
        Self {
            cache_dir: PathBuf::new(),
            index: RwLock::new(DiskCacheIndex::new()),
            enabled: false,
        }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Check if disk cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the index file path
    fn index_path(&self) -> PathBuf {
        self.cache_dir.join("index.json")
    }

    /// Load the index from disk
    fn load_index(&mut self) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let index_path = self.index_path();
        if !index_path.exists() {
            return Ok(()); // No index yet, use empty
        }

        let file = fs::File::open(&index_path)?;
        let reader = BufReader::new(file);
        let index: DiskCacheIndex = serde_json::from_reader(reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Check version compatibility
        if index.format_version != DiskCacheIndex::FORMAT_VERSION {
            tracing::warn!(
                found = index.format_version,
                expected = DiskCacheIndex::FORMAT_VERSION,
                "JIT cache format version mismatch, clearing cache"
            );
            return Ok(());
        }

        *self.index.write() = index;
        Ok(())
    }

    /// Save the index to disk
    pub fn save_index(&self) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Ensure cache directory exists
        fs::create_dir_all(&self.cache_dir)?;

        let index_path = self.index_path();
        let file = fs::File::create(&index_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &*self.index.read())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(())
    }

    /// Record a compilation
    pub fn record_compilation(&self, hash: u64, source: String, compile_time_us: u64) {
        if !self.enabled {
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntryMetadata {
            hash,
            source,
            access_count: 1,
            compile_time_us,
            last_accessed: now,
            ordo_version: env!("CARGO_PKG_VERSION").to_string(),
            target_triple: target_triple(),
        };

        self.index.write().entries.insert(hash, entry);
    }

    /// Record an access (cache hit)
    pub fn record_access(&self, hash: u64) {
        if !self.enabled {
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut index = self.index.write();
        if let Some(entry) = index.entries.get_mut(&hash) {
            entry.access_count += 1;
            entry.last_accessed = now;
        }
    }

    /// Get hot expressions sorted by access count (for warm-up)
    pub fn get_hot_expressions(&self, limit: usize) -> Vec<CacheEntryMetadata> {
        let index = self.index.read();
        let mut entries: Vec<_> = index.entries.values().cloned().collect();
        entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        entries.truncate(limit);
        entries
    }

    /// Get entry by hash
    pub fn get(&self, hash: u64) -> Option<CacheEntryMetadata> {
        self.index.read().entries.get(&hash).cloned()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.index.read().entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.index.write().entries.clear();
        if self.enabled {
            let _ = fs::remove_file(self.index_path());
        }
    }

    /// Remove entries not accessed in the last N seconds
    pub fn evict_stale(&self, max_age_secs: u64) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cutoff = now.saturating_sub(max_age_secs);

        let mut index = self.index.write();
        let before = index.entries.len();
        index
            .entries
            .retain(|_, entry| entry.last_accessed >= cutoff);
        before - index.entries.len()
    }
}

/// Get the default cache directory
fn dirs_cache_path() -> Option<PathBuf> {
    // Try to use standard cache directory
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join("Library/Caches/ordo/jit"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".cache"))
            })
            .map(|p| p.join("ordo/jit"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("ordo/jit"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Get the target triple for the current platform
fn target_triple() -> String {
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        return "aarch64-apple-darwin".to_string();
        #[cfg(target_arch = "x86_64")]
        return "x86_64-apple-darwin".to_string();
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "aarch64")]
        return "aarch64-unknown-linux-gnu".to_string();
        #[cfg(target_arch = "x86_64")]
        return "x86_64-unknown-linux-gnu".to_string();
    }
    #[cfg(target_os = "windows")]
    {
        return "x86_64-pc-windows-msvc".to_string();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return "unknown".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Value;
    use crate::expr::BinaryOp;

    #[test]
    fn test_memory_cache() {
        let cache = MemoryCache::new(100);

        // Initially empty
        assert!(cache.get(123).is_none());

        // Insert a fake pointer
        let fake_ptr = 0x12345678 as *const u8;
        cache.insert(123, fake_ptr);

        // Now should find it
        assert!(cache.get(123).is_some());
        assert_eq!(cache.get(123).unwrap(), fake_ptr);

        // Check stats
        let (hits, misses, entries) = cache.stats();
        assert_eq!(hits, 2); // Two successful gets
        assert_eq!(misses, 1); // One miss at the start
        assert_eq!(entries, 1);
    }

    #[test]
    fn test_memory_cache_lru_eviction() {
        let cache = MemoryCache::new(3);

        // Insert 3 entries
        cache.insert(1, 0x1 as *const u8);
        cache.insert(2, 0x2 as *const u8);
        cache.insert(3, 0x3 as *const u8);

        // All should be present
        assert!(cache.contains(1));
        assert!(cache.contains(2));
        assert!(cache.contains(3));

        // Insert 4th entry, should evict oldest (1)
        cache.insert(4, 0x4 as *const u8);

        assert!(!cache.contains(1)); // Evicted
        assert!(cache.contains(2));
        assert!(cache.contains(3));
        assert!(cache.contains(4));
    }

    #[test]
    fn test_background_jit_creation() {
        let config = JITCacheConfig::default();
        let result = BackgroundJIT::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_background_jit_submit() {
        let config = JITCacheConfig::default();
        let mut jit = BackgroundJIT::new(config).unwrap();

        // Create a simple expression
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(1.0))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(2.0))),
        };

        // Submit for compilation
        let submitted = jit.submit(expr.clone(), 12345, JITPriority::Normal);
        assert!(submitted);

        // Submitting same hash should return false (already pending)
        let submitted2 = jit.submit(expr, 12345, JITPriority::Normal);
        assert!(!submitted2);

        // Wait a bit for compilation
        std::thread::sleep(Duration::from_millis(200));

        // Should be compiled now
        assert!(jit.is_compiled(12345));

        // Clean up
        jit.shutdown();
    }

    #[test]
    fn test_background_jit_stats() {
        let config = JITCacheConfig::default();
        let mut jit = BackgroundJIT::new(config).unwrap();

        let stats = jit.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.successful_compilations, 0);

        // Submit and wait
        let expr = Expr::Literal(Value::Float(42.0));
        jit.submit(expr, 999, JITPriority::Normal);
        std::thread::sleep(Duration::from_millis(200));

        let stats = jit.stats();
        assert_eq!(stats.successful_compilations, 1);

        jit.shutdown();
    }

    #[test]
    fn test_disk_cache_disabled() {
        let cache = DiskCache::disabled();
        assert!(!cache.is_enabled());
        assert!(cache.is_empty());

        // Operations should be no-ops
        cache.record_compilation(123, "test".to_string(), 100);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_disk_cache_record() {
        let temp_dir = std::env::temp_dir().join("ordo_test_jit_cache");
        let _ = fs::remove_dir_all(&temp_dir);

        let cache = DiskCache::new(Some(temp_dir.clone()));
        assert!(cache.is_enabled());

        // Record a compilation
        cache.record_compilation(111, "1 + 1".to_string(), 50);
        cache.record_compilation(222, "2 * 2".to_string(), 100);

        assert_eq!(cache.len(), 2);

        // Record access
        cache.record_access(111);
        cache.record_access(111);

        // Get hot expressions
        let hot = cache.get_hot_expressions(10);
        assert_eq!(hot.len(), 2);
        assert_eq!(hot[0].hash, 111); // Most accessed first
        assert_eq!(hot[0].access_count, 3); // 1 initial + 2 accesses

        // Clean up
        cache.clear();
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_disk_cache_persistence() {
        let temp_dir = std::env::temp_dir().join("ordo_test_jit_persist");
        let _ = fs::remove_dir_all(&temp_dir);

        // Create and populate cache
        {
            let cache = DiskCache::new(Some(temp_dir.clone()));
            cache.record_compilation(333, "3 + 3".to_string(), 75);
            cache.save_index().unwrap();
        }

        // Load from disk
        {
            let cache = DiskCache::new(Some(temp_dir.clone()));
            assert_eq!(cache.len(), 1);
            let entry = cache.get(333).unwrap();
            assert_eq!(entry.source, "3 + 3");
        }

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
