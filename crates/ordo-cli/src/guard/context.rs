//! Task context for `ordo guard hook` (`.ordo-guard/context.json`).
//!
//! A tool-call event says nothing about the task the agent is working on, so
//! a policy can say "never run `terraform destroy`" but not "for this task,
//! only touch `src/payments/`". An external tool (a task planner, a CI job,
//! a human) can write `context.json` next to the policy; the hook then puts
//! its `task` object into the policy input, and one hand-written policy can
//! check e.g. `glob_match(task.touches, rel_path)` instead of being
//! regenerated per task.
//!
//! ```json
//! {
//!   "root": "/abs/path/to/repo",
//!   "session_id": "optional — only this agent session",
//!   "expires_at": "2026-10-08T00:00:00Z",
//!   "task": { "id": "login-signup", "touches": ["src/auth/**"] }
//! }
//! ```
//!
//! The context is applied only when it is bound to the event: the event's
//! `cwd` must be inside `root` (default: the policy dir's parent), the
//! `session_id` (if set) must match, and `expires_at` (if set) must be in the
//! future. Otherwise it is ignored — but never silently: the policy input
//! always carries `task_context` (`active` / `absent` / `mismatch` /
//! `expired` / `invalid`) so a scope policy can choose to ASK when there is
//! no usable task. `task` itself is passed through uninterpreted.
//!
//! Paths are compared lexically (`.`/`..` resolved, symlinks are not), in
//! keeping with guard being defense-in-depth rather than a sandbox.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

pub const CONTEXT_FILE: &str = "context.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Absent,
    Active,
    Mismatch,
    Expired,
    Invalid,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Absent => "absent",
            Status::Active => "active",
            Status::Mismatch => "mismatch",
            Status::Expired => "expired",
            Status::Invalid => "invalid",
        }
    }
}

#[derive(serde::Deserialize)]
struct ContextFile {
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    task: serde_json::Value,
}

pub(crate) struct TaskContext {
    pub status: Status,
    /// The `task` object — only when `status` is `Active`.
    pub task: Option<serde_json::Value>,
    /// The bound root, normalized — only when `status` is `Active`.
    pub root: Option<PathBuf>,
    /// `sha256:<hex>` of the file bytes, whenever the file was read.
    pub hash: Option<String>,
    /// `task.id`, whenever the file parsed and it is a string.
    pub task_id: Option<String>,
    /// Why the context was not applied (for the stderr warning).
    pub note: Option<String>,
}

impl TaskContext {
    fn absent() -> Self {
        TaskContext {
            status: Status::Absent,
            task: None,
            root: None,
            hash: None,
            task_id: None,
            note: None,
        }
    }
}

/// Load `context.json` from `policy_dir` and bind it to the event. Never
/// fails: every problem becomes a non-`Active` status.
pub(crate) fn load(
    policy_dir: &Path,
    cwd: Option<&str>,
    session_id: Option<&str>,
    now: DateTime<Utc>,
) -> TaskContext {
    let path = policy_dir.join(CONTEXT_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return TaskContext::absent(),
        Err(e) => {
            return TaskContext {
                status: Status::Invalid,
                note: Some(format!("cannot read {}: {e}", path.display())),
                ..TaskContext::absent()
            }
        }
    };
    let mut ctx = TaskContext {
        hash: Some(sha256_hex(&bytes)),
        ..TaskContext::absent()
    };
    let invalid = |mut ctx: TaskContext, note: String| {
        ctx.status = Status::Invalid;
        ctx.note = Some(format!("{}: {note}", path.display()));
        ctx
    };

    let file: ContextFile = match serde_json::from_slice(&bytes) {
        Ok(f) => f,
        Err(e) => return invalid(ctx, format!("invalid JSON: {e}")),
    };
    ctx.task_id = file
        .task
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    if !file.task.is_object() {
        return invalid(ctx, "`task` must be an object".into());
    }

    let root = match &file.root {
        Some(r) if Path::new(r).is_absolute() => normalize(Path::new(r)),
        Some(r) => return invalid(ctx, format!("`root` must be an absolute path, got {r:?}")),
        None => match default_root(policy_dir) {
            Some(r) => r,
            None => return invalid(ctx, "cannot determine the default root".into()),
        },
    };

    if let Some(expires_at) = &file.expires_at {
        match DateTime::parse_from_rfc3339(expires_at) {
            Ok(t) if t.with_timezone(&Utc) <= now => {
                ctx.status = Status::Expired;
                ctx.note = Some(format!("task context expired at {expires_at}"));
                return ctx;
            }
            Ok(_) => {}
            Err(e) => return invalid(ctx, format!("`expires_at` is not RFC 3339: {e}")),
        }
    }

    if let Some(want) = &file.session_id {
        if session_id != Some(want.as_str()) {
            ctx.status = Status::Mismatch;
            ctx.note = Some("task context is bound to another session".into());
            return ctx;
        }
    }

    match cwd.map(Path::new) {
        Some(cwd) if cwd.is_absolute() && normalize(cwd).starts_with(&root) => {}
        _ => {
            ctx.status = Status::Mismatch;
            ctx.note = Some(format!(
                "event cwd is not inside the task root {}",
                root.display()
            ));
            return ctx;
        }
    }

    ctx.status = Status::Active;
    ctx.task = Some(file.task);
    ctx.root = Some(root);
    ctx
}

/// `path` relative to `root`, `/`-separated. A relative `path` is resolved
/// against `cwd` first. Paths outside `root` come back as `../…`, so a scope
/// pattern like `src/**` can never match them.
pub(crate) fn rel_path(root: &Path, cwd: &Path, path: &str) -> String {
    let path = Path::new(path);
    let abs = normalize(&if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    });
    let root: Vec<Component> = root.components().collect();
    let abs: Vec<Component> = abs.components().collect();
    let common = root.iter().zip(&abs).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); root.len() - common];
    parts.extend(
        abs[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Lexically resolve `.` and `..` (and drop repeated separators). Does not
/// touch the filesystem, so symlinks are not followed.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    None | Some(Component::RootDir | Component::Prefix(_))
                ) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn default_root(policy_dir: &Path) -> Option<PathBuf> {
    let abs = if policy_dir.is_absolute() {
        policy_dir.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(policy_dir)
    };
    normalize(&abs).parent().map(Path::to_path_buf)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ordo-guard-ctx-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-07T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn write(dir: &Path, json: &str) {
        std::fs::write(dir.join(CONTEXT_FILE), json).unwrap();
    }

    #[test]
    fn absent_file_is_absent() {
        let dir = temp_dir("absent");
        let ctx = load(&dir, Some("/w"), None, now());
        assert_eq!(ctx.status, Status::Absent);
        assert!(ctx.hash.is_none());
    }

    // Uses POSIX absolute paths like `/w`.
    #[cfg(unix)]
    #[test]
    fn active_when_cwd_is_inside_root() {
        let dir = temp_dir("active");
        write(
            &dir,
            r#"{"root":"/w","task":{"id":"t1","touches":["src/**"]}}"#,
        );
        let ctx = load(&dir, Some("/w/sub/.."), None, now());
        assert_eq!(ctx.status, Status::Active);
        assert_eq!(ctx.task.unwrap()["id"], "t1");
        assert_eq!(ctx.task_id.as_deref(), Some("t1"));
        assert!(ctx.hash.unwrap().starts_with("sha256:"));
        assert_eq!(ctx.root.unwrap(), PathBuf::from("/w"));
    }

    // Uses POSIX absolute paths like `/w`.
    #[cfg(unix)]
    #[test]
    fn cwd_outside_root_or_missing_is_mismatch() {
        let dir = temp_dir("cwd");
        write(&dir, r#"{"root":"/w","task":{}}"#);
        assert_eq!(
            load(&dir, Some("/other"), None, now()).status,
            Status::Mismatch
        );
        assert_eq!(
            load(&dir, Some("/w/../x"), None, now()).status,
            Status::Mismatch
        );
        assert_eq!(
            load(&dir, Some("/wx"), None, now()).status,
            Status::Mismatch
        );
        assert_eq!(load(&dir, None, None, now()).status, Status::Mismatch);
        assert_eq!(load(&dir, Some("w"), None, now()).status, Status::Mismatch);
    }

    // Uses POSIX absolute paths like `/w`.
    #[cfg(unix)]
    #[test]
    fn session_binding() {
        let dir = temp_dir("session");
        write(&dir, r#"{"root":"/w","session_id":"s1","task":{}}"#);
        assert_eq!(
            load(&dir, Some("/w"), Some("s1"), now()).status,
            Status::Active
        );
        assert_eq!(
            load(&dir, Some("/w"), Some("s2"), now()).status,
            Status::Mismatch
        );
        assert_eq!(load(&dir, Some("/w"), None, now()).status, Status::Mismatch);
    }

    // Uses POSIX absolute paths like `/w`.
    #[cfg(unix)]
    #[test]
    fn expiry() {
        let dir = temp_dir("expiry");
        write(
            &dir,
            r#"{"root":"/w","expires_at":"2026-10-07T11:59:59Z","task":{"id":"old"}}"#,
        );
        let ctx = load(&dir, Some("/w"), None, now());
        assert_eq!(ctx.status, Status::Expired);
        assert!(ctx.task.is_none());
        assert_eq!(ctx.task_id.as_deref(), Some("old"));

        write(
            &dir,
            r#"{"root":"/w","expires_at":"2026-10-08T00:00:00+08:00","task":{}}"#,
        );
        assert_eq!(load(&dir, Some("/w"), None, now()).status, Status::Active);
    }

    #[test]
    fn malformed_files_are_invalid() {
        let dir = temp_dir("invalid");
        for body in [
            "not json",
            r#"{"root":"/w"}"#,
            r#"{"root":"/w","task":["a"]}"#,
            r#"{"root":"relative","task":{}}"#,
            r#"{"root":"/w","expires_at":"tomorrow","task":{}}"#,
        ] {
            write(&dir, body);
            let ctx = load(&dir, Some("/w"), None, now());
            assert_eq!(ctx.status, Status::Invalid, "{body}");
            assert!(ctx.note.is_some());
            assert!(ctx.task.is_none());
        }
    }

    #[test]
    fn root_defaults_to_policy_dir_parent() {
        let repo = temp_dir("default-root");
        let policy = repo.join(".ordo-guard");
        std::fs::create_dir_all(&policy).unwrap();
        write(&policy, r#"{"task":{}}"#);
        let cwd = repo.join("src");
        let ctx = load(&policy, cwd.to_str(), None, now());
        assert_eq!(ctx.status, Status::Active);
        assert_eq!(ctx.root.unwrap(), normalize(&repo));
    }

    // Uses POSIX absolute paths like `/w`.
    #[cfg(unix)]
    #[test]
    fn rel_path_normalizes_and_marks_outside_root() {
        let root = Path::new("/w");
        let cwd = Path::new("/w/src");
        assert_eq!(rel_path(root, cwd, "auth/login.ts"), "src/auth/login.ts");
        assert_eq!(rel_path(root, cwd, "/w/src//./auth/x.ts"), "src/auth/x.ts");
        assert_eq!(rel_path(root, cwd, "../db/schema.sql"), "db/schema.sql");
        assert_eq!(rel_path(root, cwd, "../../etc/passwd"), "../etc/passwd");
        assert_eq!(rel_path(root, cwd, "/w/src/auth/../../.."), "..");
        assert_eq!(rel_path(root, Path::new("/w"), "."), ".");
    }
}
