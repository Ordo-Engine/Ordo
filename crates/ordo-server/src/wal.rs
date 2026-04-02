//! Write-Ahead Log (WAL) for crash-safe rule persistence.
//!
//! Before any disk mutation the WAL records a `Prepared` entry (fsynced).
//! After all disk operations succeed a `Committed` entry is appended.
//! On startup, any `Prepared` entry without a matching `Committed` entry is
//! replayed — re-applying the mutation idempotently.
//!
//! ## On-disk format
//!
//! WAL segments live in `{rules_dir}/wal/` and are named `{N:010}.log`
//! (e.g. `0000000001.log`).  Each segment is a sequence of newline-delimited
//! JSON objects (`\n`-terminated).  A line that does not end with `\n` is a
//! torn write and is silently discarded on read.
//!
//! Example segment content:
//!
//! ```json
//! {"seq":1,"timestamp":"2026-04-01T10:00:00.123Z","op":"put","state":"prepared","tenant_id":"default","name":"loan-check","ruleset_json":"{...}","crc32":3141592653}
//! {"seq":1,"timestamp":"2026-04-01T10:00:00.145Z","op":"put","state":"committed","tenant_id":"default","name":"loan-check"}
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tracing::{debug, error, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The kind of operation being logged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalOpKind {
    Put,
    Delete,
}

/// Whether the operation has completed on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalEntryState {
    /// Intent recorded before any disk write.
    Prepared,
    /// All disk writes succeeded.
    Committed,
}

/// One entry in the WAL (serialized as a single JSON line).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Monotonically increasing sequence number (unique across all segments).
    pub seq: u64,
    /// RFC 3339 timestamp with millisecond precision.
    pub timestamp: String,
    /// Operation kind.
    pub op: WalOpKind,
    /// Whether this record is a prepare or a commit.
    pub state: WalEntryState,
    /// Tenant identifier.
    pub tenant_id: String,
    /// Rule name.
    pub name: String,
    /// Full JSON of the RuleSet — only present on `Put + Prepared`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruleset_json: Option<String>,
    /// CRC32 of `ruleset_json` bytes — only present on `Put + Prepared`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crc32: Option<u32>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct SegmentWriter {
    path: PathBuf,
    file: BufWriter<File>,
    bytes_written: u64,
    segment_number: u64,
}

// ---------------------------------------------------------------------------
// WalManager
// ---------------------------------------------------------------------------

/// Manages WAL segment files and provides `prepare` / `commit` operations.
pub struct WalManager {
    wal_dir: PathBuf,
    current: Mutex<SegmentWriter>,
    next_seq: AtomicU64,
    max_segment_bytes: u64,
    max_closed_segments: usize,
}

impl WalManager {
    /// Open (or create) the WAL directory.
    ///
    /// Scans existing segments to recover the highest observed sequence number
    /// so that subsequent calls to [`prepare`] assign strictly increasing IDs.
    pub fn open(
        wal_dir: PathBuf,
        max_segment_bytes: u64,
        max_closed_segments: usize,
    ) -> io::Result<Self> {
        fs::create_dir_all(&wal_dir)?;

        let segments = list_segments(&wal_dir)?;
        let (current_seg_num, max_seq) = recover_state(&wal_dir, &segments)?;

        let seg_path = segment_path(&wal_dir, current_seg_num);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&seg_path)?;
        let bytes_written = file.metadata()?.len();
        let writer = SegmentWriter {
            path: seg_path,
            file: BufWriter::new(file),
            bytes_written,
            segment_number: current_seg_num,
        };

        Ok(WalManager {
            wal_dir,
            current: Mutex::new(writer),
            next_seq: AtomicU64::new(max_seq + 1),
            max_segment_bytes,
            max_closed_segments,
        })
    }

    /// Record a `Prepared` entry and return the assigned sequence number.
    ///
    /// The entry is fsynced before returning.  The caller **must** call
    /// [`commit`] after the disk mutations succeed (or log a warning if they
    /// cannot).
    pub fn prepare(
        &self,
        op: WalOpKind,
        tenant_id: &str,
        name: &str,
        ruleset_json: Option<&str>,
    ) -> io::Result<u64> {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);

        let crc32 = ruleset_json.map(|s| crc32(s.as_bytes()));

        let entry = WalEntry {
            seq,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            op,
            state: WalEntryState::Prepared,
            tenant_id: tenant_id.to_owned(),
            name: name.to_owned(),
            ruleset_json: ruleset_json.map(str::to_owned),
            crc32,
        };

        self.append_entry(&entry)?;

        crate::metrics::record_wal_write(entry.op.as_str(), "prepared");
        crate::metrics::set_wal_pending_entries(self.pending_entry_count() as i64);

        debug!(seq, tenant_id, name, "WAL prepared");
        Ok(seq)
    }

    /// Record a `Committed` entry for the given sequence number.
    ///
    /// Commit failures are non-fatal: the disk mutation has already succeeded.
    /// On the next restart the uncommitted entry will be replayed, which is
    /// idempotent (the file already exists with the correct content).
    pub fn commit(&self, seq: u64, op: &WalOpKind, tenant_id: &str, name: &str) -> io::Result<()> {
        let entry = WalEntry {
            seq,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            op: op.clone(),
            state: WalEntryState::Committed,
            tenant_id: tenant_id.to_owned(),
            name: name.to_owned(),
            ruleset_json: None,
            crc32: None,
        };

        self.append_entry(&entry)?;

        crate::metrics::record_wal_write(op.as_str(), "committed");
        crate::metrics::set_wal_pending_entries(self.pending_entry_count() as i64);

        debug!(seq, tenant_id, name, "WAL committed");
        Ok(())
    }

    /// Read every valid entry from all segments, in chronological order.
    ///
    /// Lines that are not valid UTF-8, do not parse as JSON, or do not end
    /// with `\n` (torn writes) are silently skipped.
    pub fn read_all_entries(&self) -> io::Result<Vec<WalEntry>> {
        let segments = list_segments(&self.wal_dir)?;
        let mut entries = Vec::new();

        for seg_num in &segments {
            let path = segment_path(&self.wal_dir, *seg_num);
            read_segment(&path, &mut entries);
        }

        Ok(entries)
    }

    /// Approximate total size of all WAL segment files.
    pub fn total_size_bytes(&self) -> u64 {
        list_segments(&self.wal_dir)
            .unwrap_or_default()
            .iter()
            .map(|n| {
                segment_path(&self.wal_dir, *n)
                    .metadata()
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Number of segments currently on disk.
    pub fn segment_count(&self) -> usize {
        list_segments(&self.wal_dir).unwrap_or_default().len()
    }

    /// Count `Prepared` entries in all segments that have no matching
    /// `Committed` entry.  Used for metrics / health checks.
    pub fn pending_entry_count(&self) -> usize {
        let entries = match self.read_all_entries() {
            Ok(e) => e,
            Err(_) => return 0,
        };
        let committed: HashSet<u64> = entries
            .iter()
            .filter(|e| e.state == WalEntryState::Committed)
            .map(|e| e.seq)
            .collect();
        entries
            .iter()
            .filter(|e| e.state == WalEntryState::Prepared && !committed.contains(&e.seq))
            .count()
    }

    /// Delete closed segments beyond `max_closed_segments`.
    ///
    /// The currently-open segment is never deleted.
    pub fn cleanup_old_segments(&self) -> io::Result<()> {
        let current_num = self.current.lock().unwrap().segment_number;
        let mut segments = list_segments(&self.wal_dir)?;
        segments.retain(|n| *n != current_num); // exclude open segment

        if segments.len() > self.max_closed_segments {
            let to_delete = segments.len() - self.max_closed_segments;
            for seg_num in segments.iter().take(to_delete) {
                let path = segment_path(&self.wal_dir, *seg_num);
                if let Err(e) = fs::remove_file(&path) {
                    warn!(path = %path.display(), error = %e, "Failed to remove old WAL segment");
                } else {
                    debug!(segment = seg_num, "Removed old WAL segment");
                }
            }
        }

        self.update_size_metrics();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn append_entry(&self, entry: &WalEntry) -> io::Result<()> {
        let mut guard = self.current.lock().unwrap();

        // Rotate if needed
        if guard.bytes_written >= self.max_segment_bytes {
            self.rotate_locked(&mut guard)?;
        }

        let mut line = serde_json::to_string(entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push('\n');

        let len = line.len() as u64;
        guard.file.write_all(line.as_bytes())?;
        guard.file.flush()?;
        guard.file.get_ref().sync_data()?;
        guard.bytes_written += len;

        Ok(())
    }

    fn rotate_locked(&self, w: &mut SegmentWriter) -> io::Result<()> {
        w.file.flush()?;
        let next_num = w.segment_number + 1;
        let next_path = segment_path(&self.wal_dir, next_num);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&next_path)?;
        *w = SegmentWriter {
            path: next_path,
            file: BufWriter::new(file),
            bytes_written: 0,
            segment_number: next_num,
        };
        crate::metrics::inc_wal_rotations();
        self.update_size_metrics();
        debug!(segment = next_num, "WAL segment rotated");
        Ok(())
    }

    fn update_size_metrics(&self) {
        crate::metrics::set_wal_size_bytes(self.total_size_bytes());
        crate::metrics::set_wal_segments_total(self.segment_count() as i64);
    }
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// Information needed to re-apply a `Put` operation that was never committed.
pub struct PendingPut {
    pub seq: u64,
    pub tenant_id: String,
    pub name: String,
    pub ruleset_json: String,
    pub crc32: u32,
}

/// Information needed to re-apply a `Delete` operation that was never committed.
pub struct PendingDelete {
    pub seq: u64,
    pub tenant_id: String,
    pub name: String,
}

/// Pending operations found in the WAL that need to be replayed.
pub struct PendingOps {
    pub puts: Vec<PendingPut>,
    pub deletes: Vec<PendingDelete>,
}

/// Scan all segments and collect operations that have a `Prepared` entry but
/// no corresponding `Committed` entry.
pub fn scan_pending(wal: &WalManager) -> io::Result<PendingOps> {
    let entries = wal.read_all_entries()?;

    let committed: HashSet<u64> = entries
        .iter()
        .filter(|e| e.state == WalEntryState::Committed)
        .map(|e| e.seq)
        .collect();

    let mut puts: HashMap<(String, String), WalEntry> = HashMap::new();
    let mut deletes: HashMap<(String, String), WalEntry> = HashMap::new();

    for entry in &entries {
        if entry.state != WalEntryState::Prepared || committed.contains(&entry.seq) {
            continue;
        }
        let key = (entry.tenant_id.clone(), entry.name.clone());
        match entry.op {
            WalOpKind::Put => {
                puts.insert(key, entry.clone());
            }
            WalOpKind::Delete => {
                deletes.remove(&key); // a pending put may have been overwritten by a delete
                deletes.insert(key, entry.clone());
            }
        }
    }

    // If a key appears in both puts and deletes, the most recent operation wins.
    // Remove any put whose (tenant, name) also has a pending delete with higher seq.
    puts.retain(|k, put_entry| {
        if let Some(del_entry) = deletes.get(k) {
            del_entry.seq < put_entry.seq
        } else {
            true
        }
    });

    let pending_puts = puts
        .into_values()
        .filter_map(|e| {
            let json = e.ruleset_json?;
            let crc = e.crc32?;
            Some(PendingPut {
                seq: e.seq,
                tenant_id: e.tenant_id,
                name: e.name,
                ruleset_json: json,
                crc32: crc,
            })
        })
        .collect();

    let pending_deletes = deletes
        .into_values()
        .map(|e| PendingDelete {
            seq: e.seq,
            tenant_id: e.tenant_id,
            name: e.name,
        })
        .collect();

    Ok(PendingOps {
        puts: pending_puts,
        deletes: pending_deletes,
    })
}

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

fn segment_path(wal_dir: &Path, number: u64) -> PathBuf {
    wal_dir.join(format!("{:010}.log", number))
}

/// Return segment numbers sorted ascending.
fn list_segments(wal_dir: &Path) -> io::Result<Vec<u64>> {
    let mut numbers = Vec::new();
    for entry in fs::read_dir(wal_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".log") {
            if let Ok(n) = name.trim_end_matches(".log").parse::<u64>() {
                numbers.push(n);
            }
        }
    }
    numbers.sort_unstable();
    Ok(numbers)
}

/// Read the highest seq from existing segments and the current segment number.
/// Returns `(current_segment_number, max_seq)`.
fn recover_state(wal_dir: &Path, segments: &[u64]) -> io::Result<(u64, u64)> {
    if segments.is_empty() {
        return Ok((1, 0));
    }
    let current_seg = *segments.last().unwrap();
    let mut max_seq: u64 = 0;
    for seg_num in segments {
        let path = segment_path(wal_dir, *seg_num);
        let mut entries = Vec::new();
        read_segment(&path, &mut entries);
        for e in entries {
            if e.seq > max_seq {
                max_seq = e.seq;
            }
        }
    }
    Ok((current_seg, max_seq))
}

/// Parse a single segment file, appending valid entries to `out`.
/// Torn writes (lines without `\n`) and unparseable lines are skipped.
fn read_segment(path: &Path, out: &mut Vec<WalEntry>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Could not open WAL segment");
            return;
        }
    };

    // Read raw bytes so we can detect torn writes (lines without \n).
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                if !line.ends_with('\n') {
                    // Torn write — partial line at end of file.
                    warn!(
                        path = %path.display(),
                        "Discarding torn WAL entry (no trailing newline)"
                    );
                    break;
                }
                match serde_json::from_str::<WalEntry>(line.trim_end()) {
                    Ok(entry) => out.push(entry),
                    Err(e) => {
                        warn!(
                            path = %path.display(),
                            error = %e,
                            line = %line.trim_end(),
                            "Skipping unparseable WAL entry"
                        );
                    }
                }
            }
            Err(e) => {
                error!(path = %path.display(), error = %e, "Error reading WAL segment");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CRC32 (IEEE polynomial, no external dep)
// ---------------------------------------------------------------------------

/// CRC32 checksum (IEEE 802.3 polynomial).
/// Identical algorithm to `ordo-core/src/rule/compiled.rs`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// WalOpKind helper
// ---------------------------------------------------------------------------

impl WalOpKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            WalOpKind::Put => "put",
            WalOpKind::Delete => "delete",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_wal(dir: &Path) -> WalManager {
        WalManager::open(dir.to_path_buf(), 64 * 1024 * 1024, 3).unwrap()
    }

    #[test]
    fn test_prepare_commit_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let wal = open_wal(tmp.path());

        let seq = wal
            .prepare(
                WalOpKind::Put,
                "default",
                "payment-check",
                Some(r#"{"name":"payment-check"}"#),
            )
            .unwrap();
        wal.commit(seq, &WalOpKind::Put, "default", "payment-check")
            .unwrap();

        let entries = wal.read_all_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].state, WalEntryState::Prepared);
        assert_eq!(entries[0].seq, seq);
        assert_eq!(entries[1].state, WalEntryState::Committed);
        assert_eq!(entries[1].seq, seq);
        assert_eq!(entries[0].name, "payment-check");
    }

    #[test]
    fn test_pending_count_after_prepare_without_commit() {
        let tmp = TempDir::new().unwrap();
        let wal = open_wal(tmp.path());

        wal.prepare(WalOpKind::Put, "t1", "rule-a", Some("{}"))
            .unwrap();
        assert_eq!(wal.pending_entry_count(), 1);

        let seq2 = wal
            .prepare(WalOpKind::Put, "t1", "rule-b", Some("{}"))
            .unwrap();
        wal.commit(seq2, &WalOpKind::Put, "t1", "rule-b").unwrap();
        assert_eq!(wal.pending_entry_count(), 1); // only rule-a is pending
    }

    #[test]
    fn test_torn_write_skipped() {
        let tmp = TempDir::new().unwrap();
        let seg_path = segment_path(tmp.path(), 1);

        // Write a complete line followed by a torn line (no \n).
        let complete = r#"{"seq":1,"timestamp":"2026-01-01T00:00:00.000Z","op":"put","state":"prepared","tenant_id":"t","name":"n"}"#;
        let torn = r#"{"seq":2,"timestamp":"20"#; // no newline
        fs::write(&seg_path, format!("{}\n{}", complete, torn)).unwrap();

        let mut entries = Vec::new();
        read_segment(&seg_path, &mut entries);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].seq, 1);
    }

    #[test]
    fn test_segment_rotation() {
        let tmp = TempDir::new().unwrap();
        // Very small max size to force rotation after first entry.
        let wal = WalManager::open(tmp.path().to_path_buf(), 1, 3).unwrap();

        wal.prepare(WalOpKind::Put, "t", "r1", Some("{}")).unwrap();
        wal.prepare(WalOpKind::Put, "t", "r2", Some("{}")).unwrap();

        let segs = list_segments(tmp.path()).unwrap();
        assert!(
            segs.len() >= 2,
            "expected at least 2 segments after rotation"
        );
    }

    #[test]
    fn test_reopen_recovers_seq() {
        let tmp = TempDir::new().unwrap();
        {
            let wal = open_wal(tmp.path());
            let seq = wal.prepare(WalOpKind::Put, "t", "r", Some("{}")).unwrap();
            wal.commit(seq, &WalOpKind::Put, "t", "r").unwrap();
            assert_eq!(seq, 1);
        }
        // Reopen — next seq must be > 1
        let wal2 = open_wal(tmp.path());
        let seq2 = wal2.prepare(WalOpKind::Put, "t", "r2", Some("{}")).unwrap();
        assert!(seq2 > 1, "next_seq should continue from previous session");
    }

    #[test]
    fn test_cleanup_old_segments() {
        let tmp = TempDir::new().unwrap();
        let wal = WalManager::open(tmp.path().to_path_buf(), 1, 1).unwrap();

        // Force three rotations
        wal.prepare(WalOpKind::Put, "t", "r1", Some("{}")).unwrap();
        wal.prepare(WalOpKind::Put, "t", "r2", Some("{}")).unwrap();
        wal.prepare(WalOpKind::Put, "t", "r3", Some("{}")).unwrap();

        wal.cleanup_old_segments().unwrap();

        let segs = list_segments(tmp.path()).unwrap();
        // max_closed_segments=1 + 1 open = 2 max total
        assert!(
            segs.len() <= 2,
            "expected ≤ 2 segments after cleanup, got {}",
            segs.len()
        );
    }

    #[test]
    fn test_crc32_basic() {
        // Known value: CRC32("hello") = 0x3610A686
        assert_eq!(crc32(b"hello"), 0x3610_A686);
        assert_ne!(crc32(b"hello"), crc32(b"world"));
    }

    #[test]
    fn test_scan_pending_finds_uncommitted_put() {
        let tmp = TempDir::new().unwrap();
        let wal = open_wal(tmp.path());

        let seq = wal
            .prepare(WalOpKind::Put, "default", "fraud-check", Some(r#"{"x":1}"#))
            .unwrap();
        // Intentionally skip commit to simulate crash.
        drop(wal);

        let wal2 = open_wal(tmp.path());
        let pending = scan_pending(&wal2).unwrap();
        assert_eq!(pending.puts.len(), 1);
        assert_eq!(pending.puts[0].seq, seq);
        assert_eq!(pending.puts[0].name, "fraud-check");
        assert!(pending.deletes.is_empty());
    }

    #[test]
    fn test_scan_pending_empty_after_commit() {
        let tmp = TempDir::new().unwrap();
        let wal = open_wal(tmp.path());

        let seq = wal.prepare(WalOpKind::Put, "t", "r", Some("{}")).unwrap();
        wal.commit(seq, &WalOpKind::Put, "t", "r").unwrap();

        let pending = scan_pending(&wal).unwrap();
        assert!(pending.puts.is_empty());
        assert!(pending.deletes.is_empty());
    }

    #[test]
    fn test_scan_pending_delete_wins_over_put() {
        let tmp = TempDir::new().unwrap();
        let wal = open_wal(tmp.path());

        // Uncommitted put then uncommitted delete for the same rule.
        wal.prepare(WalOpKind::Put, "t", "r", Some("{}")).unwrap();
        wal.prepare(WalOpKind::Delete, "t", "r", None).unwrap();
        drop(wal);

        let wal2 = open_wal(tmp.path());
        let pending = scan_pending(&wal2).unwrap();
        assert!(
            pending.puts.is_empty(),
            "put should be superseded by delete"
        );
        assert_eq!(pending.deletes.len(), 1);
    }
}
