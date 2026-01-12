//! Audit logging module
//!
//! Provides structured audit logging for rule changes, executions, and system events.
//! Logs are written to both stdout and optional JSON Lines files with daily rotation.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use tracing::info;

/// Audit event types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AuditEvent {
    /// Server started
    ServerStarted { version: String, rules_count: usize },
    /// Server stopped
    ServerStopped { uptime_seconds: u64 },
    /// Rule created
    RuleCreated {
        rule_name: String,
        version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
    /// Rule updated
    RuleUpdated {
        rule_name: String,
        from_version: String,
        to_version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
    /// Rule deleted
    RuleDeleted {
        rule_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
    /// Rule rolled back
    RuleRollback {
        rule_name: String,
        from_version: String,
        to_version: String,
        seq: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
    /// Rule executed (sampled)
    RuleExecuted {
        rule_name: String,
        duration_us: u64,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
    /// Sample rate changed
    SampleRateChanged {
        from_rate: u8,
        to_rate: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_ip: Option<String>,
    },
}

/// Audit log entry with metadata
#[derive(Debug, Serialize)]
struct AuditLogEntry {
    timestamp: DateTime<Utc>,
    level: &'static str,
    #[serde(flatten)]
    event: AuditEvent,
}

/// File writer state for daily rotation
struct FileWriter {
    writer: BufWriter<File>,
    current_date: String,
}

/// Audit logger with atomic sample rate
pub struct AuditLogger {
    /// Sampling rate (0-100), stored atomically for lock-free updates
    sample_rate: AtomicU8,
    /// Audit log directory (None = stdout only)
    audit_dir: Option<PathBuf>,
    /// Current file writer (protected by mutex for file rotation)
    file_writer: Mutex<Option<FileWriter>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(audit_dir: Option<PathBuf>, initial_sample_rate: u8) -> Self {
        let rate = initial_sample_rate.min(100);
        Self {
            sample_rate: AtomicU8::new(rate),
            audit_dir,
            file_writer: Mutex::new(None),
        }
    }

    /// Update sampling rate at runtime (0-100)
    pub fn set_sample_rate(&self, rate: u8) -> u8 {
        let rate = rate.min(100);
        let previous = self.sample_rate.swap(rate, Ordering::Relaxed);
        previous
    }

    /// Get current sampling rate
    pub fn get_sample_rate(&self) -> u8 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Check if this execution should be sampled
    pub fn should_sample(&self) -> bool {
        let rate = self.sample_rate.load(Ordering::Relaxed);
        if rate >= 100 {
            return true;
        }
        if rate == 0 {
            return false;
        }
        rand::random::<u8>() % 100 < rate
    }

    /// Log an audit event
    pub fn log(&self, event: AuditEvent) {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            level: "INFO",
            event,
        };

        // Serialize to JSON
        let json = match serde_json::to_string(&entry) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("Failed to serialize audit event: {}", e);
                return;
            }
        };

        // Log to stdout via tracing
        info!(target: "audit", "{}", json);

        // Write to file if audit_dir is configured
        if let Some(ref audit_dir) = self.audit_dir {
            if let Err(e) = self.write_to_file(audit_dir, &json) {
                tracing::error!("Failed to write audit log to file: {}", e);
            }
        }
    }

    /// Write audit log to file with daily rotation
    fn write_to_file(&self, audit_dir: &PathBuf, json: &str) -> std::io::Result<()> {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        let mut guard = self.file_writer.lock().unwrap();

        // Check if we need to rotate or create a new file
        let needs_new_file = match &*guard {
            None => true,
            Some(fw) => fw.current_date != today,
        };

        if needs_new_file {
            // Ensure directory exists
            fs::create_dir_all(audit_dir)?;

            // Create new file
            let file_path = audit_dir.join(format!("audit-{}.jsonl", today));
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)?;

            *guard = Some(FileWriter {
                writer: BufWriter::new(file),
                current_date: today,
            });
        }

        // Write to file
        if let Some(ref mut fw) = *guard {
            writeln!(fw.writer, "{}", json)?;
            fw.writer.flush()?;
        }

        Ok(())
    }

    /// Log a rule execution event (with sampling)
    pub fn log_execution(
        &self,
        rule_name: &str,
        duration_us: u64,
        result: &str,
        source_ip: Option<String>,
    ) {
        if self.should_sample() {
            self.log(AuditEvent::RuleExecuted {
                rule_name: rule_name.to_string(),
                duration_us,
                result: result.to_string(),
                source_ip,
            });
        }
    }

    /// Log a rule created event
    pub fn log_rule_created(&self, rule_name: &str, version: &str, source_ip: Option<String>) {
        self.log(AuditEvent::RuleCreated {
            rule_name: rule_name.to_string(),
            version: version.to_string(),
            source_ip,
        });
    }

    /// Log a rule updated event
    pub fn log_rule_updated(
        &self,
        rule_name: &str,
        from_version: &str,
        to_version: &str,
        source_ip: Option<String>,
    ) {
        self.log(AuditEvent::RuleUpdated {
            rule_name: rule_name.to_string(),
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            source_ip,
        });
    }

    /// Log a rule deleted event
    pub fn log_rule_deleted(&self, rule_name: &str, source_ip: Option<String>) {
        self.log(AuditEvent::RuleDeleted {
            rule_name: rule_name.to_string(),
            source_ip,
        });
    }

    /// Log a rule rollback event
    pub fn log_rule_rollback(
        &self,
        rule_name: &str,
        from_version: &str,
        to_version: &str,
        seq: u32,
        source_ip: Option<String>,
    ) {
        self.log(AuditEvent::RuleRollback {
            rule_name: rule_name.to_string(),
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            seq,
            source_ip,
        });
    }

    /// Log server started event
    pub fn log_server_started(&self, version: &str, rules_count: usize) {
        self.log(AuditEvent::ServerStarted {
            version: version.to_string(),
            rules_count,
        });
    }

    /// Log server stopped event
    pub fn log_server_stopped(&self, uptime_seconds: u64) {
        self.log(AuditEvent::ServerStopped { uptime_seconds });
    }

    /// Log sample rate changed event
    pub fn log_sample_rate_changed(&self, from_rate: u8, to_rate: u8, source_ip: Option<String>) {
        self.log(AuditEvent::SampleRateChanged {
            from_rate,
            to_rate,
            source_ip,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sample_rate_atomic_update() {
        let logger = AuditLogger::new(None, 10);
        assert_eq!(logger.get_sample_rate(), 10);

        let previous = logger.set_sample_rate(50);
        assert_eq!(previous, 10);
        assert_eq!(logger.get_sample_rate(), 50);

        // Test clamping to 100
        logger.set_sample_rate(150);
        assert_eq!(logger.get_sample_rate(), 100);
    }

    #[test]
    fn test_should_sample_boundaries() {
        // 0% should never sample
        let logger = AuditLogger::new(None, 0);
        for _ in 0..100 {
            assert!(!logger.should_sample());
        }

        // 100% should always sample
        let logger = AuditLogger::new(None, 100);
        for _ in 0..100 {
            assert!(logger.should_sample());
        }
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::RuleCreated {
            rule_name: "test-rule".to_string(),
            version: "1.0.0".to_string(),
            source_ip: Some("127.0.0.1".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"rule_created\""));
        assert!(json.contains("\"rule_name\":\"test-rule\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }

    #[test]
    fn test_file_logging() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(Some(temp_dir.path().to_path_buf()), 100);

        logger.log_rule_created("test-rule", "1.0.0", None);

        // Check that file was created
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let file_path = temp_dir.path().join(format!("audit-{}.jsonl", today));
        assert!(file_path.exists());

        // Check file contents
        let contents = fs::read_to_string(&file_path).unwrap();
        assert!(contents.contains("rule_created"));
        assert!(contents.contains("test-rule"));
    }
}
