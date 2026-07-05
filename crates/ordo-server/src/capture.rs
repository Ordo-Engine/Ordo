//! IO capture — opt-in recording of full execution inputs + outputs.
//!
//! Unlike the audit log (which records that an execution happened, sampled, but
//! not the input), the capture log writes the whole `{input, code, output}` of a
//! decision to a JSONL file — the exact shape `ordo replay` consumes. That turns
//! production traffic into a replayable regression corpus: change a rule, replay
//! last week's real decisions, see which ones flip.
//!
//! Disabled by default (no path configured → `should_capture()` is false and no
//! input is ever cloned on the hot path). Capturing full inputs may record PII —
//! it is deliberately opt-in and sample-rate bounded. Mirrors `AuditLogger`.

use chrono::Utc;
use ordo_core::prelude::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// File writer state for daily rotation.
struct FileWriter {
    writer: BufWriter<File>,
    current_date: String,
}

/// Records full execution IO to a JSONL file, with daily rotation and sampling.
pub struct CaptureLogger {
    sample_rate: AtomicU8,
    /// Capture directory (None = capture disabled).
    dir: Option<PathBuf>,
    file_writer: Mutex<Option<FileWriter>>,
}

impl CaptureLogger {
    pub fn new(dir: Option<PathBuf>, sample_rate: u8) -> Self {
        Self {
            sample_rate: AtomicU8::new(sample_rate.min(100)),
            dir,
            file_writer: Mutex::new(None),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.dir.is_some()
    }

    /// Whether this execution should be captured. Computed once, before the
    /// input is cloned, so a disabled or unsampled request never pays the clone.
    pub fn should_capture(&self) -> bool {
        if self.dir.is_none() {
            return false;
        }
        let rate = self.sample_rate.load(Ordering::Relaxed);
        if rate >= 100 {
            return true;
        }
        if rate == 0 {
            return false;
        }
        rand::random::<u8>() % 100 < rate
    }

    /// Append one captured decision. Only called when `should_capture()` was true.
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        &self,
        rule_name: &str,
        tenant: &str,
        input: &Value,
        code: &str,
        output: &Value,
        duration_us: u64,
        source_ip: Option<&str>,
    ) {
        let Some(ref dir) = self.dir else {
            return;
        };
        let record = serde_json::json!({
            "ts": Utc::now().to_rfc3339(),
            "rule_name": rule_name,
            "tenant": tenant,
            "input": serde_json::to_value(input).unwrap_or(serde_json::Value::Null),
            "code": code,
            "output": serde_json::to_value(output).unwrap_or(serde_json::Value::Null),
            "duration_us": duration_us,
            "source_ip": source_ip,
        });
        let line = match serde_json::to_string(&record) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("failed to serialize capture record: {}", e);
                return;
            }
        };
        if let Err(e) = self.write_line(dir, &line) {
            tracing::error!("failed to write capture record: {}", e);
        }
    }

    fn write_line(&self, dir: &PathBuf, line: &str) -> std::io::Result<()> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let mut guard = self
            .file_writer
            .lock()
            .map_err(|_| std::io::Error::other("capture mutex poisoned"))?;

        let needs_new_file = match &*guard {
            None => true,
            Some(fw) => fw.current_date != today,
        };
        if needs_new_file {
            fs::create_dir_all(dir)?;
            let path = dir.join(format!("capture-{}.jsonl", today));
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            *guard = Some(FileWriter {
                writer: BufWriter::new(file),
                current_date: today,
            });
        }
        if let Some(ref mut fw) = *guard {
            writeln!(fw.writer, "{}", line)?;
            fw.writer.flush()?;
        }
        Ok(())
    }
}
