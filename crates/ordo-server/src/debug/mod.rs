//! Debug module for VM visualization and step debugging
//!
//! This module provides debug API endpoints that are only available when
//! the server is started with `--debug-mode` or `ORDO_DEBUG_MODE=true`.
//!
//! **WARNING**: Do NOT enable in production environments!

pub mod api;
mod session;
mod types;

pub use session::DebugSessionManager;
pub use types::*;
