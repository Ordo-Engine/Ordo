//! Output helpers — human-readable text vs machine-readable JSON (`--json`).
//!
//! The CLI is used as much by AI coding agents as by people, so every command
//! supports a global `--json` flag that emits a stable, parseable envelope
//! instead of the pretty human output.

use anyhow::Result;
use serde::Serialize;

/// Emit a value as pretty JSON to stdout (the machine-readable form).
pub fn emit_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
