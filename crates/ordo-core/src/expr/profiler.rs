//! Expression and Rule Profiler for JIT Hot-Path Detection
//!
//! This module tracks execution statistics for expressions and rule paths
//! to identify hot paths that would benefit from JIT compilation.
//!
//! # Hot-Path Detection Formula
//!
//! `score = execution_count × avg_duration_ns / 1000`
//!
//! | Score Threshold | Action |
//! |-----------------|--------|
//! | < 10,000        | Stay on BytecodeVM |
//! | 10,000 - 100,000| Queue for background JIT |
//! | > 100,000       | Priority JIT compilation |

use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Priority level for JIT compilation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JITPriority {
    /// Low priority - will be compiled when queue is empty
    Low,
    /// Normal priority - standard background compilation
    Normal,
    /// High priority - should be compiled soon
    High,
    /// Critical priority - compile immediately
    Critical,
}

impl JITPriority {
    /// Determine priority from hot score
    pub fn from_score(score: u64) -> Option<Self> {
        match score {
            0..=9_999 => None,                     // Not hot enough
            10_000..=49_999 => Some(Self::Low),    // Warm
            50_000..=99_999 => Some(Self::Normal), // Hot
            100_000..=499_999 => Some(Self::High), // Very hot
            _ => Some(Self::Critical),             // Extremely hot
        }
    }
}

/// Profile data for a single expression
#[derive(Debug)]
pub struct ExprProfile {
    /// Hash of the expression
    pub hash: u64,
    /// Number of times executed
    execution_count: AtomicU64,
    /// Total execution duration in nanoseconds
    total_duration_ns: AtomicU64,
    /// Last execution timestamp
    last_execution: parking_lot::Mutex<Instant>,
    /// Whether JIT compilation has been triggered
    jit_triggered: AtomicU64, // 0 = no, 1 = yes
}

impl ExprProfile {
    /// Create a new expression profile
    pub fn new(hash: u64) -> Self {
        Self {
            hash,
            execution_count: AtomicU64::new(0),
            total_duration_ns: AtomicU64::new(0),
            last_execution: parking_lot::Mutex::new(Instant::now()),
            jit_triggered: AtomicU64::new(0),
        }
    }

    /// Record an execution
    pub fn record(&self, duration: Duration) {
        self.execution_count.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        *self.last_execution.lock() = Instant::now();
    }

    /// Get execution count
    pub fn execution_count(&self) -> u64 {
        self.execution_count.load(Ordering::Relaxed)
    }

    /// Get total duration in nanoseconds
    pub fn total_duration_ns(&self) -> u64 {
        self.total_duration_ns.load(Ordering::Relaxed)
    }

    /// Get average duration in nanoseconds
    pub fn avg_duration_ns(&self) -> u64 {
        let count = self.execution_count();
        if count == 0 {
            return 0;
        }
        self.total_duration_ns() / count
    }

    /// Calculate hot score: execution_count × avg_duration_ns / 1000
    pub fn hot_score(&self) -> u64 {
        let count = self.execution_count();
        let avg_ns = self.avg_duration_ns();
        // Avoid overflow by dividing first
        (count / 10) * (avg_ns / 100)
    }

    /// Check if JIT compilation has been triggered
    pub fn is_jit_triggered(&self) -> bool {
        self.jit_triggered.load(Ordering::Relaxed) != 0
    }

    /// Mark JIT compilation as triggered
    pub fn mark_jit_triggered(&self) {
        self.jit_triggered.store(1, Ordering::Relaxed);
    }

    /// Get time since last execution
    pub fn time_since_last_execution(&self) -> Duration {
        self.last_execution.lock().elapsed()
    }
}

/// Profile data for a rule execution path
#[derive(Debug)]
pub struct RulePathProfile {
    /// RuleSet name
    pub ruleset_name: String,
    /// Hash of the execution path (step IDs)
    pub path_hash: u64,
    /// Step IDs in the path
    pub step_ids: Vec<String>,
    /// Number of times executed
    execution_count: AtomicU64,
    /// Total execution duration in nanoseconds
    total_duration_ns: AtomicU64,
    /// Last execution timestamp
    last_execution: parking_lot::Mutex<Instant>,
    /// Whether JIT compilation has been triggered
    jit_triggered: AtomicU64,
}

impl RulePathProfile {
    /// Create a new rule path profile
    pub fn new(ruleset_name: String, path_hash: u64, step_ids: Vec<String>) -> Self {
        Self {
            ruleset_name,
            path_hash,
            step_ids,
            execution_count: AtomicU64::new(0),
            total_duration_ns: AtomicU64::new(0),
            last_execution: parking_lot::Mutex::new(Instant::now()),
            jit_triggered: AtomicU64::new(0),
        }
    }

    /// Record an execution
    pub fn record(&self, duration: Duration) {
        self.execution_count.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        *self.last_execution.lock() = Instant::now();
    }

    /// Get execution count
    pub fn execution_count(&self) -> u64 {
        self.execution_count.load(Ordering::Relaxed)
    }

    /// Get total duration in nanoseconds
    pub fn total_duration_ns(&self) -> u64 {
        self.total_duration_ns.load(Ordering::Relaxed)
    }

    /// Get average duration in nanoseconds
    pub fn avg_duration_ns(&self) -> u64 {
        let count = self.execution_count();
        if count == 0 {
            return 0;
        }
        self.total_duration_ns() / count
    }

    /// Calculate hot score
    pub fn hot_score(&self) -> u64 {
        let count = self.execution_count();
        let avg_ns = self.avg_duration_ns();
        (count / 10) * (avg_ns / 100)
    }

    /// Check if JIT compilation has been triggered
    pub fn is_jit_triggered(&self) -> bool {
        self.jit_triggered.load(Ordering::Relaxed) != 0
    }

    /// Mark JIT compilation as triggered
    pub fn mark_jit_triggered(&self) {
        self.jit_triggered.store(1, Ordering::Relaxed);
    }
}

/// Configuration for the profiler
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// Minimum hot score to trigger JIT (default: 10,000)
    pub hot_threshold: u64,
    /// Maximum number of expression profiles to keep
    pub max_expr_profiles: usize,
    /// Maximum number of rule path profiles to keep
    pub max_rule_profiles: usize,
    /// Whether profiling is enabled
    pub enabled: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            hot_threshold: 10_000,
            max_expr_profiles: 10_000,
            max_rule_profiles: 1_000,
            enabled: true,
        }
    }
}

/// Result of checking if an expression should be JIT compiled
#[derive(Debug, Clone)]
pub struct JITDecision {
    /// Whether JIT compilation should be triggered
    pub should_jit: bool,
    /// Priority if should_jit is true
    pub priority: Option<JITPriority>,
    /// Current hot score
    pub hot_score: u64,
    /// Execution count
    pub execution_count: u64,
    /// Average duration in nanoseconds
    pub avg_duration_ns: u64,
}

/// Global profiler for expression and rule execution tracking
pub struct Profiler {
    /// Expression profiles indexed by hash
    expressions: DashMap<u64, ExprProfile>,
    /// Rule path profiles indexed by path hash
    rule_paths: DashMap<u64, RulePathProfile>,
    /// Configuration
    config: ProfilerConfig,
    /// Total expressions profiled (for statistics)
    total_expr_executions: AtomicU64,
    /// Total rule executions profiled
    total_rule_executions: AtomicU64,
}

impl Profiler {
    /// Create a new profiler with default configuration
    pub fn new() -> Self {
        Self::with_config(ProfilerConfig::default())
    }

    /// Create a new profiler with custom configuration
    pub fn with_config(config: ProfilerConfig) -> Self {
        Self {
            expressions: DashMap::new(),
            rule_paths: DashMap::new(),
            config,
            total_expr_executions: AtomicU64::new(0),
            total_rule_executions: AtomicU64::new(0),
        }
    }

    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Record an expression execution
    pub fn record_expr(&self, hash: u64, duration: Duration) {
        if !self.config.enabled {
            return;
        }

        self.total_expr_executions.fetch_add(1, Ordering::Relaxed);

        self.expressions
            .entry(hash)
            .or_insert_with(|| ExprProfile::new(hash))
            .record(duration);
    }

    /// Record a rule path execution
    pub fn record_rule_path(&self, ruleset_name: &str, step_ids: &[String], duration: Duration) {
        if !self.config.enabled {
            return;
        }

        self.total_rule_executions.fetch_add(1, Ordering::Relaxed);

        let path_hash = hash_step_ids(step_ids);

        self.rule_paths
            .entry(path_hash)
            .or_insert_with(|| {
                RulePathProfile::new(ruleset_name.to_string(), path_hash, step_ids.to_vec())
            })
            .record(duration);
    }

    /// Check if an expression should be JIT compiled
    pub fn should_jit_expr(&self, hash: u64) -> JITDecision {
        if !self.config.enabled {
            return JITDecision {
                should_jit: false,
                priority: None,
                hot_score: 0,
                execution_count: 0,
                avg_duration_ns: 0,
            };
        }

        if let Some(profile) = self.expressions.get(&hash) {
            // Already triggered? Don't trigger again
            if profile.is_jit_triggered() {
                return JITDecision {
                    should_jit: false,
                    priority: None,
                    hot_score: profile.hot_score(),
                    execution_count: profile.execution_count(),
                    avg_duration_ns: profile.avg_duration_ns(),
                };
            }

            let score = profile.hot_score();
            let priority = JITPriority::from_score(score);
            let should_jit = priority.is_some();

            JITDecision {
                should_jit,
                priority,
                hot_score: score,
                execution_count: profile.execution_count(),
                avg_duration_ns: profile.avg_duration_ns(),
            }
        } else {
            JITDecision {
                should_jit: false,
                priority: None,
                hot_score: 0,
                execution_count: 0,
                avg_duration_ns: 0,
            }
        }
    }

    /// Check if a rule path should be JIT compiled
    pub fn should_jit_rule_path(&self, step_ids: &[String]) -> JITDecision {
        if !self.config.enabled {
            return JITDecision {
                should_jit: false,
                priority: None,
                hot_score: 0,
                execution_count: 0,
                avg_duration_ns: 0,
            };
        }

        let path_hash = hash_step_ids(step_ids);

        if let Some(profile) = self.rule_paths.get(&path_hash) {
            if profile.is_jit_triggered() {
                return JITDecision {
                    should_jit: false,
                    priority: None,
                    hot_score: profile.hot_score(),
                    execution_count: profile.execution_count(),
                    avg_duration_ns: profile.avg_duration_ns(),
                };
            }

            let score = profile.hot_score();
            let priority = JITPriority::from_score(score);
            let should_jit = priority.is_some();

            JITDecision {
                should_jit,
                priority,
                hot_score: score,
                execution_count: profile.execution_count(),
                avg_duration_ns: profile.avg_duration_ns(),
            }
        } else {
            JITDecision {
                should_jit: false,
                priority: None,
                hot_score: 0,
                execution_count: 0,
                avg_duration_ns: 0,
            }
        }
    }

    /// Mark an expression as JIT triggered
    pub fn mark_expr_jit_triggered(&self, hash: u64) {
        if let Some(profile) = self.expressions.get(&hash) {
            profile.mark_jit_triggered();
        }
    }

    /// Mark a rule path as JIT triggered
    pub fn mark_rule_path_jit_triggered(&self, step_ids: &[String]) {
        let path_hash = hash_step_ids(step_ids);
        if let Some(profile) = self.rule_paths.get(&path_hash) {
            profile.mark_jit_triggered();
        }
    }

    /// Get expression profile by hash
    pub fn get_expr_profile(&self, hash: u64) -> Option<(u64, u64, u64)> {
        self.expressions
            .get(&hash)
            .map(|p| (p.execution_count(), p.avg_duration_ns(), p.hot_score()))
    }

    /// Get all hot expressions (above threshold)
    pub fn get_hot_expressions(&self) -> Vec<(u64, u64, JITPriority)> {
        self.expressions
            .iter()
            .filter_map(|entry| {
                let score = entry.hot_score();
                JITPriority::from_score(score).map(|priority| (entry.hash, score, priority))
            })
            .collect()
    }

    /// Get all hot rule paths (above threshold)
    pub fn get_hot_rule_paths(&self) -> Vec<(u64, String, Vec<String>, u64, JITPriority)> {
        self.rule_paths
            .iter()
            .filter_map(|entry| {
                let score = entry.hot_score();
                JITPriority::from_score(score).map(|priority| {
                    (
                        entry.path_hash,
                        entry.ruleset_name.clone(),
                        entry.step_ids.clone(),
                        score,
                        priority,
                    )
                })
            })
            .collect()
    }

    /// Get profiler statistics
    pub fn stats(&self) -> ProfilerStats {
        ProfilerStats {
            total_expr_executions: self.total_expr_executions.load(Ordering::Relaxed),
            total_rule_executions: self.total_rule_executions.load(Ordering::Relaxed),
            unique_expressions: self.expressions.len(),
            unique_rule_paths: self.rule_paths.len(),
            hot_expressions: self.get_hot_expressions().len(),
            hot_rule_paths: self.get_hot_rule_paths().len(),
        }
    }

    /// Clear all profiles
    pub fn clear(&self) {
        self.expressions.clear();
        self.rule_paths.clear();
        self.total_expr_executions.store(0, Ordering::Relaxed);
        self.total_rule_executions.store(0, Ordering::Relaxed);
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Profiler statistics
#[derive(Debug, Clone)]
pub struct ProfilerStats {
    /// Total expression executions
    pub total_expr_executions: u64,
    /// Total rule executions
    pub total_rule_executions: u64,
    /// Number of unique expressions tracked
    pub unique_expressions: usize,
    /// Number of unique rule paths tracked
    pub unique_rule_paths: usize,
    /// Number of hot expressions
    pub hot_expressions: usize,
    /// Number of hot rule paths
    pub hot_rule_paths: usize,
}

/// Hash a list of step IDs to create a path hash
fn hash_step_ids(step_ids: &[String]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    for id in step_ids {
        id.hash(&mut hasher);
    }
    hasher.finish()
}

/// Compute hash for an expression string
pub fn hash_expr(expr: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    expr.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_expr_profile_basic() {
        let profile = ExprProfile::new(12345);
        assert_eq!(profile.execution_count(), 0);
        assert_eq!(profile.avg_duration_ns(), 0);
        assert_eq!(profile.hot_score(), 0);

        // Record some executions
        profile.record(Duration::from_nanos(1000));
        profile.record(Duration::from_nanos(2000));
        profile.record(Duration::from_nanos(3000));

        assert_eq!(profile.execution_count(), 3);
        assert_eq!(profile.total_duration_ns(), 6000);
        assert_eq!(profile.avg_duration_ns(), 2000);
    }

    #[test]
    fn test_hot_score_calculation() {
        let profile = ExprProfile::new(12345);

        // Simulate 1000 executions at 10μs each
        for _ in 0..1000 {
            profile.record(Duration::from_micros(10));
        }

        let score = profile.hot_score();
        // score = (1000/10) * (10000/100) = 100 * 100 = 10,000
        assert!((9000..=11000).contains(&score), "Score was {}", score);

        let priority = JITPriority::from_score(score);
        assert!(priority.is_some());
    }

    #[test]
    fn test_jit_priority_thresholds() {
        assert_eq!(JITPriority::from_score(5000), None);
        assert_eq!(JITPriority::from_score(10000), Some(JITPriority::Low));
        assert_eq!(JITPriority::from_score(50000), Some(JITPriority::Normal));
        assert_eq!(JITPriority::from_score(100000), Some(JITPriority::High));
        assert_eq!(JITPriority::from_score(500000), Some(JITPriority::Critical));
    }

    #[test]
    fn test_profiler_expr_tracking() {
        let profiler = Profiler::new();
        let hash = hash_expr("age > 18");

        // Record executions
        for _ in 0..100 {
            profiler.record_expr(hash, Duration::from_micros(50));
        }

        let decision = profiler.should_jit_expr(hash);
        assert!(!decision.should_jit); // Not hot enough yet
        assert_eq!(decision.execution_count, 100);

        // Record more to make it hot
        for _ in 0..2000 {
            profiler.record_expr(hash, Duration::from_micros(50));
        }

        let decision = profiler.should_jit_expr(hash);
        // 2100 executions * 50μs = ~10,000 score
        assert!(decision.hot_score > 0);
    }

    #[test]
    fn test_profiler_rule_path_tracking() {
        let profiler = Profiler::new();
        let step_ids = vec![
            "step1".to_string(),
            "step2".to_string(),
            "step3".to_string(),
        ];

        for _ in 0..100 {
            profiler.record_rule_path("test_ruleset", &step_ids, Duration::from_micros(100));
        }

        let decision = profiler.should_jit_rule_path(&step_ids);
        assert_eq!(decision.execution_count, 100);
    }

    #[test]
    fn test_profiler_stats() {
        let profiler = Profiler::new();

        profiler.record_expr(1, Duration::from_micros(10));
        profiler.record_expr(2, Duration::from_micros(20));
        profiler.record_rule_path("rs1", &["s1".to_string()], Duration::from_micros(30));

        let stats = profiler.stats();
        assert_eq!(stats.total_expr_executions, 2);
        assert_eq!(stats.total_rule_executions, 1);
        assert_eq!(stats.unique_expressions, 2);
        assert_eq!(stats.unique_rule_paths, 1);
    }

    #[test]
    fn test_jit_triggered_flag() {
        let profiler = Profiler::new();
        let hash = hash_expr("test");

        profiler.record_expr(hash, Duration::from_micros(100));

        // Mark as triggered
        profiler.mark_expr_jit_triggered(hash);

        // Should not trigger again
        let decision = profiler.should_jit_expr(hash);
        assert!(!decision.should_jit);
    }
}
