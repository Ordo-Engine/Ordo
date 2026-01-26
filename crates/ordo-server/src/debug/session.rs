//! Debug session management

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ordo_core::prelude::Value;
use parking_lot::RwLock;
use tokio::sync::broadcast;

use super::types::{DebugEvent, SessionState, TraceLevel};

/// Unique session ID counter
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique session ID
fn generate_session_id() -> String {
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    format!("dbg_{:x}_{:04x}", timestamp, counter)
}

/// Debug session
pub struct DebugSession {
    /// Session ID
    pub id: String,
    /// Ruleset name being debugged
    pub ruleset_name: String,
    /// Input data
    pub input: Value,
    /// Current state
    pub state: RwLock<SessionState>,
    /// Breakpoints
    pub breakpoints: RwLock<Vec<String>>,
    /// Trace level
    pub trace_level: TraceLevel,
    /// Created timestamp
    pub created_at: String,
    /// Event broadcaster
    pub event_tx: broadcast::Sender<DebugEvent>,
}

impl DebugSession {
    /// Create a new debug session
    pub fn new(ruleset_name: String, input: Value, trace_level: TraceLevel) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        let created_at = chrono::Utc::now().to_rfc3339();

        Self {
            id: generate_session_id(),
            ruleset_name,
            input,
            state: RwLock::new(SessionState::Created),
            breakpoints: RwLock::new(Vec::new()),
            trace_level,
            created_at,
            event_tx,
        }
    }

    /// Get current state
    pub fn get_state(&self) -> SessionState {
        *self.state.read()
    }

    /// Set state and broadcast event
    pub fn set_state(&self, new_state: SessionState) {
        *self.state.write() = new_state;
        let _ = self
            .event_tx
            .send(DebugEvent::StateChange { state: new_state });
    }

    /// Add breakpoint
    pub fn add_breakpoint(&self, location: String) {
        self.breakpoints.write().push(location);
    }

    /// Remove breakpoint
    pub fn remove_breakpoint(&self, location: &str) -> bool {
        let mut breakpoints = self.breakpoints.write();
        if let Some(pos) = breakpoints.iter().position(|b| b == location) {
            breakpoints.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get breakpoint count
    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.read().len()
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<DebugEvent> {
        self.event_tx.subscribe()
    }

    /// Send event
    pub fn send_event(&self, event: DebugEvent) {
        let _ = self.event_tx.send(event);
    }
}

/// Debug session manager
pub struct DebugSessionManager {
    sessions: RwLock<HashMap<String, DebugSession>>,
}

impl DebugSessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new debug session
    pub fn create_session(
        &self,
        ruleset_name: String,
        input: Value,
        trace_level: TraceLevel,
        breakpoints: Vec<String>,
    ) -> String {
        let session = DebugSession::new(ruleset_name, input, trace_level);
        let session_id = session.id.clone();

        // Add initial breakpoints
        for bp in breakpoints {
            session.add_breakpoint(bp);
        }

        self.sessions.write().insert(session_id.clone(), session);
        session_id
    }

    /// Get session by ID
    pub fn get_session(
        &self,
        session_id: &str,
    ) -> Option<impl std::ops::Deref<Target = DebugSession> + '_> {
        let sessions = self.sessions.read();
        if sessions.contains_key(session_id) {
            // Safe: we just checked contains_key above
            Some(parking_lot::RwLockReadGuard::map(sessions, |s| {
                s.get(session_id)
                    .expect("session should exist after contains_key check")
            }))
        } else {
            None
        }
    }

    /// Delete session
    pub fn delete_session(&self, session_id: &str) -> bool {
        self.sessions.write().remove(session_id).is_some()
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<super::types::DebugSessionInfo> {
        self.sessions
            .read()
            .values()
            .map(|s| super::types::DebugSessionInfo {
                id: s.id.clone(),
                ruleset_name: s.ruleset_name.clone(),
                state: s.get_state(),
                created_at: s.created_at.clone(),
                breakpoint_count: s.breakpoint_count(),
            })
            .collect()
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Clean up terminated sessions
    pub fn cleanup_terminated(&self) {
        self.sessions
            .write()
            .retain(|_, s| s.get_state() != SessionState::Terminated);
    }
}

impl Default for DebugSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
