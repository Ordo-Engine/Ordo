//! Async client for the Ordo control-plane HTTP API (`/api/v1`).
//!
//! Mirrors the surface the Studio web client uses (`platform-client.ts`):
//! bearer-token auth, JSON in/out, `{error, code?}` error bodies, and the 409
//! optimistic-lock conflict on draft save. DTOs live in [`types`].

pub mod types;
pub use types::*;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// An error from the platform API.
#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("API error (HTTP {status}{}): {message}", .code.as_deref().map(|c| format!(", {c}")).unwrap_or_default())]
    Http {
        status: u16,
        code: Option<String>,
        message: String,
    },
    #[error("draft conflict: the server has newer changes (seq {}); pull and retry", .0.server_seq)]
    Conflict(DraftConflict),
    #[error("network error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ApiError>;

/// A configured client for one platform base URL + token.
pub struct Client {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
    #[serde(default)]
    code: Option<String>,
}

impl Client {
    /// `base_url` is the API root, e.g. `https://api.ordoengine.com/api/v1`.
    pub fn new(base_url: impl Into<String>, token: Option<String>) -> Self {
        Client {
            base: base_url.into().trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    async fn raw(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<reqwest::Response> {
        let mut req = self.http.request(method, format!("{}{}", self.base, path));
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        Ok(req.send().await?)
    }

    /// Send a request and decode a JSON response, mapping non-2xx to `ApiError::Http`.
    async fn send<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let resp = self.raw(method, path, body).await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if status.is_success() {
            Ok(serde_json::from_slice(&bytes)?)
        } else {
            Err(parse_error(status.as_u16(), &bytes))
        }
    }

    // ── auth ──

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthResponse> {
        self.send(
            reqwest::Method::POST,
            "/auth/login",
            Some(json!({ "email": email, "password": password })),
        )
        .await
    }

    pub async fn me(&self) -> Result<UserInfo> {
        self.send(reqwest::Method::GET, "/auth/me", None).await
    }

    // ── orgs / projects / environments ──

    pub async fn list_orgs(&self) -> Result<Vec<Org>> {
        self.send(reqwest::Method::GET, "/orgs", None).await
    }

    pub async fn list_projects(&self, org_id: &str) -> Result<Vec<Project>> {
        self.send(
            reqwest::Method::GET,
            &format!("/orgs/{}/projects", enc(org_id)),
            None,
        )
        .await
    }

    pub async fn list_environments(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<Environment>> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/orgs/{}/projects/{}/environments",
                enc(org_id),
                enc(project_id)
            ),
            None,
        )
        .await
    }

    // ── rulesets ──

    pub async fn list_rulesets(&self, org_id: &str, project_id: &str) -> Result<Vec<RulesetMeta>> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/orgs/{}/projects/{}/rulesets",
                enc(org_id),
                enc(project_id)
            ),
            None,
        )
        .await
    }

    pub async fn get_ruleset(
        &self,
        org_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<RulesetDraft> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/orgs/{}/projects/{}/rulesets/{}",
                enc(org_id),
                enc(project_id),
                enc(name)
            ),
            None,
        )
        .await
    }

    /// Save (push) a studio-format ruleset draft with optimistic locking.
    /// A 409 becomes `ApiError::Conflict`.
    pub async fn save_ruleset(
        &self,
        org_id: &str,
        project_id: &str,
        name: &str,
        ruleset: Value,
        expected_seq: i64,
    ) -> Result<RulesetDraft> {
        let path = format!(
            "/orgs/{}/projects/{}/rulesets/{}",
            enc(org_id),
            enc(project_id),
            enc(name)
        );
        let resp = self
            .raw(
                reqwest::Method::PUT,
                &path,
                Some(json!({ "ruleset": ruleset, "expected_seq": expected_seq })),
            )
            .await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if status.is_success() {
            return Ok(serde_json::from_slice(&bytes)?);
        }
        if status.as_u16() == 409 {
            if let Ok(conflict) = serde_json::from_slice::<DraftConflict>(&bytes) {
                return Err(ApiError::Conflict(conflict));
            }
        }
        Err(parse_error(status.as_u16(), &bytes))
    }

    pub async fn publish(
        &self,
        org_id: &str,
        project_id: &str,
        name: &str,
        environment_id: &str,
        release_note: Option<&str>,
    ) -> Result<Deployment> {
        let mut body = json!({ "environment_id": environment_id });
        if let Some(note) = release_note {
            body["release_note"] = json!(note);
        }
        self.send(
            reqwest::Method::POST,
            &format!(
                "/orgs/{}/projects/{}/rulesets/{}/publish",
                enc(org_id),
                enc(project_id),
                enc(name)
            ),
            Some(body),
        )
        .await
    }

    pub async fn list_deployments(
        &self,
        org_id: &str,
        project_id: &str,
        name: &str,
        limit: u32,
    ) -> Result<Vec<Deployment>> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/orgs/{}/projects/{}/rulesets/{}/deployments?limit={}",
                enc(org_id),
                enc(project_id),
                enc(name),
                limit
            ),
            None,
        )
        .await
    }

    pub async fn list_project_deployments(
        &self,
        org_id: &str,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<Deployment>> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/orgs/{}/projects/{}/deployments?limit={}",
                enc(org_id),
                enc(project_id),
                limit
            ),
            None,
        )
        .await
    }

    // ── catalog (project-scoped, returned verbatim for facts.json / concepts.json) ──

    pub async fn list_facts(&self, project_id: &str) -> Result<Vec<Value>> {
        self.send(
            reqwest::Method::GET,
            &format!("/projects/{}/facts", enc(project_id)),
            None,
        )
        .await
    }

    pub async fn list_concepts(&self, project_id: &str) -> Result<Vec<Value>> {
        self.send(
            reqwest::Method::GET,
            &format!("/projects/{}/concepts", enc(project_id)),
            None,
        )
        .await
    }

    /// Send a request that returns no meaningful body (e.g. DELETE/204, or an
    /// upsert whose response we ignore). Non-2xx maps to `ApiError::Http`.
    async fn send_empty(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<()> {
        let resp = self.raw(method, path, body).await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(parse_error(status.as_u16(), &bytes))
        }
    }

    pub async fn upsert_fact(&self, project_id: &str, fact: Value) -> Result<()> {
        self.send_empty(
            reqwest::Method::POST,
            &format!("/projects/{}/facts", enc(project_id)),
            Some(fact),
        )
        .await
    }

    pub async fn delete_fact(&self, project_id: &str, name: &str) -> Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!("/projects/{}/facts/{}", enc(project_id), enc(name)),
            None,
        )
        .await
    }

    pub async fn upsert_concept(&self, project_id: &str, concept: Value) -> Result<()> {
        self.send_empty(
            reqwest::Method::POST,
            &format!("/projects/{}/concepts", enc(project_id)),
            Some(concept),
        )
        .await
    }

    pub async fn delete_concept(&self, project_id: &str, name: &str) -> Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!("/projects/{}/concepts/{}", enc(project_id), enc(name)),
            None,
        )
        .await
    }

    pub async fn list_contracts(&self, project_id: &str) -> Result<Vec<Value>> {
        self.send(
            reqwest::Method::GET,
            &format!("/projects/{}/contracts", enc(project_id)),
            None,
        )
        .await
    }

    pub async fn upsert_contract(
        &self,
        project_id: &str,
        name: &str,
        contract: Value,
    ) -> Result<()> {
        self.send_empty(
            reqwest::Method::PUT,
            &format!("/projects/{}/contracts/{}", enc(project_id), enc(name)),
            Some(contract),
        )
        .await
    }

    // ── test cases (project-scoped, per ruleset) ──

    pub async fn list_tests(&self, project_id: &str, ruleset: &str) -> Result<Vec<Value>> {
        self.send(
            reqwest::Method::GET,
            &format!(
                "/projects/{}/rulesets/{}/tests",
                enc(project_id),
                enc(ruleset)
            ),
            None,
        )
        .await
    }

    pub async fn create_test(&self, project_id: &str, ruleset: &str, test: Value) -> Result<Value> {
        self.send(
            reqwest::Method::POST,
            &format!(
                "/projects/{}/rulesets/{}/tests",
                enc(project_id),
                enc(ruleset)
            ),
            Some(test),
        )
        .await
    }

    pub async fn update_test(
        &self,
        project_id: &str,
        ruleset: &str,
        test_id: &str,
        test: Value,
    ) -> Result<()> {
        self.send_empty(
            reqwest::Method::PUT,
            &format!(
                "/projects/{}/rulesets/{}/tests/{}",
                enc(project_id),
                enc(ruleset),
                enc(test_id)
            ),
            Some(test),
        )
        .await
    }

    pub async fn delete_test(&self, project_id: &str, ruleset: &str, test_id: &str) -> Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!(
                "/projects/{}/rulesets/{}/tests/{}",
                enc(project_id),
                enc(ruleset),
                enc(test_id)
            ),
            None,
        )
        .await
    }
}

fn parse_error(status: u16, bytes: &[u8]) -> ApiError {
    let (message, code) = serde_json::from_slice::<ErrorBody>(bytes)
        .map(|e| (e.error, e.code))
        .unwrap_or_else(|_| (String::from_utf8_lossy(bytes).trim().to_string(), None));
    let message = if message.is_empty() {
        format!("request failed with status {status}")
    } else {
        message
    };
    ApiError::Http {
        status,
        code,
        message,
    }
}

/// Percent-encode a single URL path segment (unreserved chars pass through).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enc_passes_unreserved_and_escapes_the_rest() {
        assert_eq!(enc("loan-approval"), "loan-approval");
        assert_eq!(enc("a b/c"), "a%20b%2Fc");
        assert_eq!(enc("x.y_z~1"), "x.y_z~1");
    }

    #[test]
    fn parse_error_reads_error_body() {
        let e = parse_error(400, br#"{"error":"bad","code":"x"}"#);
        match e {
            ApiError::Http {
                status,
                code,
                message,
            } => {
                assert_eq!(status, 400);
                assert_eq!(code.as_deref(), Some("x"));
                assert_eq!(message, "bad");
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn parse_error_falls_back_to_raw_body() {
        let e = parse_error(500, b"boom");
        assert!(matches!(e, ApiError::Http { message, .. } if message == "boom"));
    }

    #[test]
    fn draft_conflict_deserializes() {
        let c: DraftConflict =
            serde_json::from_str(r#"{"conflict":true,"server_seq":7,"server_draft":{}}"#).unwrap();
        assert_eq!(c.server_seq, 7);
    }
}
