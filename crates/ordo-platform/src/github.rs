//! GitHub OAuth integration and Marketplace routes.
//!
//! ## OAuth flow
//!
//! 1. Frontend calls `GET /api/v1/github/connect` (JWT-protected) → receives `{ url }`.
//! 2. Frontend opens that URL in a popup window.
//! 3. User authorises on GitHub; GitHub redirects to `/api/v1/github/callback?code=...&state=...`.
//! 4. Callback handler verifies the short-lived state JWT, exchanges the code for a
//!    GitHub access token, saves the connection, and serves a tiny HTML page that
//!    posts a message to the opener and closes itself.
//!
//! ## Marketplace
//!
//! Repositories tagged with the GitHub topic `ordo-template` are treated as
//! marketplace entries.  A repository must contain an `ordo-template.json` in its
//! root that matches the `TemplateDetail` schema (see `crate::models`).
//!
//! Search results and manifests are cached in-memory for 5 and 10 minutes
//! respectively, keyed by query + pagination parameters.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Extension, Json,
};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Role, TemplateDetail},
    org::load_org_and_check_role,
    project::ProjectResponse,
    template::extract_locale,
    AppState,
};

// ── In-memory marketplace cache ───────────────────────────────────────────────

struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

pub struct MarketplaceCache {
    searches: Mutex<HashMap<String, CacheEntry<serde_json::Value>>>,
    manifests: Mutex<HashMap<String, CacheEntry<serde_json::Value>>>,
}

impl MarketplaceCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            searches: Mutex::new(HashMap::new()),
            manifests: Mutex::new(HashMap::new()),
        })
    }

    async fn get_search(&self, key: &str) -> Option<serde_json::Value> {
        let cache = self.searches.lock().await;
        cache
            .get(key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.data.clone())
    }

    async fn set_search(&self, key: String, value: serde_json::Value) {
        let mut cache = self.searches.lock().await;
        cache.insert(
            key,
            CacheEntry {
                data: value,
                expires_at: Instant::now() + Duration::from_secs(300),
            },
        );
        // Evict expired entries to bound memory usage
        cache.retain(|_, e| e.expires_at > Instant::now());
    }

    async fn get_manifest(&self, key: &str) -> Option<serde_json::Value> {
        let cache = self.manifests.lock().await;
        cache
            .get(key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.data.clone())
    }

    async fn set_manifest(&self, key: String, value: serde_json::Value) {
        let mut cache = self.manifests.lock().await;
        cache.insert(
            key,
            CacheEntry {
                data: value,
                expires_at: Instant::now() + Duration::from_secs(600),
            },
        );
        cache.retain(|_, e| e.expires_at > Instant::now());
    }
}

// ── OAuth state JWT ────────────────────────────────────────────────────────────

/// Short-lived claims used as the GitHub OAuth `state` parameter.
/// Signed with the platform JWT secret; expires in 10 minutes.
#[derive(Debug, Serialize, Deserialize)]
struct OAuthStateClaims {
    /// user_id
    sub: String,
    /// expiry unix timestamp
    exp: usize,
    /// random nonce to prevent replay
    nonce: String,
    /// must be "gh_oauth" — prevents use of regular user JWTs as state
    pur: String,
}

fn make_oauth_state(user_id: &str, secret: &str) -> anyhow::Result<String> {
    let claims = OAuthStateClaims {
        sub: user_id.to_string(),
        exp: (Utc::now() + chrono::Duration::minutes(10)).timestamp() as usize,
        nonce: uuid::Uuid::new_v4().to_string(),
        pur: "gh_oauth".to_string(),
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

fn verify_oauth_state(state_token: &str, secret: &str) -> anyhow::Result<String> {
    let data = decode::<OAuthStateClaims>(
        state_token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    if data.claims.pur != "gh_oauth" {
        anyhow::bail!("invalid state purpose");
    }
    Ok(data.claims.sub)
}

// ── GitHub API helpers ─────────────────────────────────────────────────────────

const GITHUB_API: &str = "https://api.github.com";
const USER_AGENT: &str = concat!("ordo-platform/", env!("CARGO_PKG_VERSION"));

async fn gh_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
) -> anyhow::Result<T> {
    let mut req = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(token) = token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = req.send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API {} — {}", status, body);
    }
    Ok(resp.json::<T>().await?)
}

// ── Internal GitHub serde models ───────────────────────────────────────────────

#[derive(Deserialize)]
struct GhTokenResponse {
    access_token: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GhUser {
    id: i64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
struct GhRepoOwner {
    login: String,
    avatar_url: String,
}

#[derive(Deserialize, Serialize, Clone)]
struct GhRepo {
    id: i64,
    name: String,
    full_name: String,
    description: Option<String>,
    html_url: String,
    stargazers_count: u32,
    topics: Option<Vec<String>>,
    updated_at: String,
    owner: GhRepoOwner,
}

#[derive(Deserialize)]
struct GhSearchResponse {
    total_count: u32,
    items: Vec<GhRepo>,
}

#[derive(Deserialize)]
struct GhFileContent {
    content: String,
    encoding: String,
}

// ── Public store-facing model (referenced from store.rs) ──────────────────────

pub struct GitHubConnectionRow {
    pub user_id: String,
    pub github_user_id: i64,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub connected_at: DateTime<Utc>,
}

// ── Public API response models ─────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GitHubStatusResponse {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MarketplaceItem {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub stars: u32,
    pub topics: Vec<String>,
    pub updated_at: String,
    pub owner_login: String,
    pub owner_avatar: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MarketplaceSearchResponse {
    pub items: Vec<MarketplaceItem>,
    pub total_count: u32,
    pub page: u32,
    pub per_page: u32,
}

// ── Route handlers ─────────────────────────────────────────────────────────────

/// GET /api/v1/github/connect
///
/// Returns the GitHub OAuth authorization URL for the frontend to open in a popup.
/// The `state` parameter is a short-lived signed JWT containing the user's ID so
/// the callback can associate the connection without requiring a session cookie.
pub async fn get_connect_url(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<serde_json::Value>> {
    let client_id = state.config.github_client_id.as_deref().ok_or_else(|| {
        PlatformError::bad_request("GitHub OAuth is not configured on this server")
    })?;

    let state_token =
        make_oauth_state(&claims.sub, &state.config.jwt_secret).map_err(PlatformError::Internal)?;

    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user&state={}",
        client_id,
        urlencoding::encode(&state.config.github_callback_url),
        urlencoding::encode(&state_token),
    );

    info!(user_id = %claims.sub, "GitHub OAuth connect URL issued");
    Ok(Json(serde_json::json!({ "url": url })))
}

/// GET /api/v1/github/callback  (public — called by GitHub redirect)
///
/// Exchanges the OAuth code for an access token, fetches the GitHub user profile,
/// and stores the connection.  Returns a small HTML page that closes the popup
/// and notifies the opener via `postMessage`.
pub async fn github_callback(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    macro_rules! error_page {
        ($msg:expr) => {{
            let msg = $msg;
            warn!(err = %msg, "GitHub OAuth callback error");
            return Html(format!(
                r#"<!DOCTYPE html><html><head><title>GitHub Connect — Error</title>
<style>body{{font-family:sans-serif;padding:32px;color:#444}}</style></head><body>
<script>
  if(window.opener){{window.opener.postMessage({{type:"github_error",error:{msg:?}}},location.origin||"*");window.close();}}
  else{{sessionStorage.setItem("github_error",{msg:?});window.location.href="/settings?tab=integrations&github_error=1";}}
</script>
<h2>Connection failed</h2><p>{msg}</p><p>You may close this window.</p>
</body></html>"#,
                msg = msg,
            ))
            .into_response();
        }};
    }

    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => error_page!("Missing OAuth code"),
    };
    let state_token = match params.get("state") {
        Some(s) => s.clone(),
        None => error_page!("Missing OAuth state"),
    };

    // Verify state JWT
    let user_id = match verify_oauth_state(&state_token, &state.config.jwt_secret) {
        Ok(uid) => uid,
        Err(e) => error_page!(format!("Invalid OAuth state: {}", e)),
    };

    let (client_id, client_secret) = match (
        state.config.github_client_id.as_deref(),
        state.config.github_client_secret.as_deref(),
    ) {
        (Some(id), Some(sec)) => (id.to_string(), sec.to_string()),
        _ => error_page!("GitHub OAuth is not configured on this server"),
    };

    // Exchange code for token
    let token_resp = match state
        .http_client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => error_page!(format!("Token exchange request failed: {}", e)),
    };

    let gh_token: GhTokenResponse = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => error_page!(format!("Failed to parse token response: {}", e)),
    };

    if let Some(err) = &gh_token.error {
        error_page!(format!(
            "GitHub denied the request: {} — {}",
            err,
            gh_token.error_description.as_deref().unwrap_or("")
        ));
    }

    let access_token = match gh_token.access_token {
        Some(t) => t,
        None => error_page!("GitHub returned no access token"),
    };

    // Fetch GitHub user profile
    let gh_user: GhUser = match gh_get(
        &state.http_client,
        &format!("{}/user", GITHUB_API),
        Some(&access_token),
    )
    .await
    {
        Ok(u) => u,
        Err(e) => error_page!(format!("Failed to fetch GitHub profile: {}", e)),
    };

    // Persist the connection
    if let Err(e) = state
        .store
        .save_github_connection(
            &user_id,
            gh_user.id,
            &gh_user.login,
            gh_user.name.as_deref(),
            gh_user.avatar_url.as_deref(),
            &access_token,
            &gh_token.scope.unwrap_or_default(),
        )
        .await
    {
        error_page!(format!("Failed to save GitHub connection: {}", e));
    }

    info!(user_id = %user_id, github_login = %gh_user.login, "GitHub account connected");

    Html(
        r#"<!DOCTYPE html><html><head><title>GitHub Connected</title>
<style>body{font-family:sans-serif;padding:32px;color:#444;text-align:center}</style></head><body>
<script>
  if(window.opener){window.opener.postMessage({type:"github_connected"},location.origin||"*");window.close();}
  else{window.location.href="/settings?tab=integrations&github_connected=1";}
</script>
<h2>✓ GitHub connected</h2><p>You may close this window.</p>
</body></html>"#,
    )
    .into_response()
}

/// GET /api/v1/github/status
pub async fn get_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<GitHubStatusResponse>> {
    let conn = state
        .store
        .get_github_connection(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(match conn {
        None => GitHubStatusResponse {
            connected: false,
            login: None,
            name: None,
            avatar_url: None,
            connected_at: None,
        },
        Some(c) => GitHubStatusResponse {
            connected: true,
            login: Some(c.login),
            name: c.name,
            avatar_url: c.avatar_url,
            connected_at: Some(c.connected_at.to_rfc3339()),
        },
    }))
}

/// DELETE /api/v1/github/disconnect
pub async fn disconnect(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<StatusCode> {
    state
        .store
        .delete_github_connection(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    info!(user_id = %claims.sub, "GitHub account disconnected");
    Ok(StatusCode::NO_CONTENT)
}

// ── Marketplace ────────────────────────────────────────────────────────────────

async fn get_optional_github_token(state: &AppState, user_id: &str) -> ApiResult<Option<String>> {
    state
        .store
        .get_github_token(user_id)
        .await
        .map_err(PlatformError::Internal)
}

fn locale_from_headers(headers: &HeaderMap) -> &'static str {
    let raw = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());
    extract_locale(raw)
}

fn decode_file_content(file: GhFileContent) -> ApiResult<Vec<u8>> {
    if file.encoding == "base64" {
        let cleaned = file.content.replace(['\n', '\r', ' '], "");
        base64::engine::general_purpose::STANDARD
            .decode(&cleaned)
            .map_err(|e| {
                PlatformError::bad_request(&format!("Invalid base64 in GitHub content: {}", e))
            })
    } else {
        Ok(file.content.into_bytes())
    }
}

async fn fetch_manifest(
    state: &AppState,
    owner: &str,
    repo: &str,
    token: Option<&str>,
) -> ApiResult<serde_json::Value> {
    let cache_key = format!("{}/{}", owner, repo);
    if let Some(cached) = state.marketplace_cache.get_manifest(&cache_key).await {
        return Ok(cached);
    }

    let file_url = format!(
        "{}/repos/{}/{}/contents/ordo-template.json",
        GITHUB_API, owner, repo
    );
    let file: GhFileContent = gh_get(&state.http_client, &file_url, token)
        .await
        .map_err(|_| PlatformError::not_found("ordo-template.json not found in this repository. Make sure the repository contains a valid ordo-template.json at the root."))?;

    let content_bytes = decode_file_content(file)?;

    let manifest: serde_json::Value = serde_json::from_slice(&content_bytes).map_err(|e| {
        PlatformError::bad_request(&format!("Invalid JSON in ordo-template.json: {}", e))
    })?;

    state
        .marketplace_cache
        .set_manifest(cache_key, manifest.clone())
        .await;
    Ok(manifest)
}

async fn fetch_repo_json_file(
    state: &AppState,
    owner: &str,
    repo: &str,
    path: &str,
    token: Option<&str>,
) -> ApiResult<serde_json::Value> {
    let file_url = format!("{}/repos/{}/{}/contents/{}", GITHUB_API, owner, repo, path);
    let file: GhFileContent = gh_get(&state.http_client, &file_url, token)
        .await
        .map_err(|_| PlatformError::not_found(&format!("{} not found in this repository", path)))?;
    let content = decode_file_content(file)?;
    serde_json::from_slice(&content)
        .map_err(|e| PlatformError::bad_request(&format!("Invalid JSON in {}: {}", path, e)))
}

async fn fetch_repo_json_file_optional(
    state: &AppState,
    owner: &str,
    repo: &str,
    path: &str,
    token: Option<&str>,
) -> ApiResult<Option<serde_json::Value>> {
    match fetch_repo_json_file(state, owner, repo, path, token).await {
        Ok(value) => Ok(Some(value)),
        Err(PlatformError::NotFound(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

async fn fetch_template_source_manifest(
    state: &AppState,
    owner: &str,
    repo: &str,
    token: Option<&str>,
) -> ApiResult<Option<serde_json::Value>> {
    let Some(meta) =
        fetch_repo_json_file_optional(state, owner, repo, "template/meta.json", token).await?
    else {
        return Ok(None);
    };
    let facts = fetch_repo_json_file(state, owner, repo, "template/facts.json", token).await?;
    let concepts =
        fetch_repo_json_file(state, owner, repo, "template/concepts.json", token).await?;
    let ruleset = fetch_repo_json_file(state, owner, repo, "template/ruleset.json", token).await?;
    let samples = fetch_repo_json_file(state, owner, repo, "template/samples.json", token).await?;
    let contract =
        fetch_repo_json_file_optional(state, owner, repo, "template/contract.json", token).await?;
    let tests =
        fetch_repo_json_file_optional(state, owner, repo, "template/tests.json", token).await?;

    let mut manifest = match meta {
        serde_json::Value::Object(obj) => obj,
        _ => {
            return Err(PlatformError::bad_request(
                "template/meta.json must be a JSON object",
            ))
        }
    };
    manifest.insert("facts".into(), facts);
    manifest.insert("concepts".into(), concepts);
    manifest.insert("ruleset".into(), ruleset);
    manifest.insert("samples".into(), samples);
    if let Some(contract) = contract {
        manifest.insert("contract".into(), contract);
    }
    manifest.insert(
        "tests".into(),
        tests.unwrap_or_else(|| serde_json::json!([])),
    );

    Ok(Some(serde_json::Value::Object(manifest)))
}

async fn fetch_i18n_bundle(
    state: &AppState,
    owner: &str,
    repo: &str,
    locale: &str,
    token: Option<&str>,
) -> ApiResult<HashMap<String, String>> {
    let file_url = format!(
        "{}/repos/{}/{}/contents/template/i18n/{}.json",
        GITHUB_API, owner, repo, locale
    );
    let file: GhFileContent = match gh_get(&state.http_client, &file_url, token).await {
        Ok(file) => file,
        Err(_) => return Ok(HashMap::new()),
    };
    let content = decode_file_content(file)?;
    let bundle: HashMap<String, String> = serde_json::from_slice(&content).map_err(|e| {
        PlatformError::bad_request(&format!(
            "Invalid JSON in template/i18n/{}.json: {}",
            locale, e
        ))
    })?;
    Ok(bundle)
}

fn localize_string(
    value: &str,
    locale_bundle: &HashMap<String, String>,
    fallback_bundle: &HashMap<String, String>,
) -> String {
    let Some(key) = value.strip_prefix("i18n:") else {
        return value.to_string();
    };
    locale_bundle
        .get(key)
        .or_else(|| fallback_bundle.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn apply_i18n(
    value: &mut serde_json::Value,
    locale_bundle: &HashMap<String, String>,
    fallback_bundle: &HashMap<String, String>,
) {
    match value {
        serde_json::Value::String(s) => {
            *s = localize_string(s, locale_bundle, fallback_bundle);
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                apply_i18n(item, locale_bundle, fallback_bundle);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, item) in obj {
                apply_i18n(item, locale_bundle, fallback_bundle);
            }
        }
        _ => {}
    }
}

async fn fetch_localized_manifest(
    state: &AppState,
    owner: &str,
    repo: &str,
    locale: &str,
    token: Option<&str>,
) -> ApiResult<serde_json::Value> {
    let mut manifest = if let Some(template_manifest) =
        fetch_template_source_manifest(state, owner, repo, token).await?
    {
        template_manifest
    } else {
        fetch_manifest(state, owner, repo, token).await?
    };
    let fallback_bundle = fetch_i18n_bundle(state, owner, repo, "en", token).await?;
    let locale_bundle = if locale == "en" {
        fallback_bundle.clone()
    } else {
        fetch_i18n_bundle(state, owner, repo, locale, token).await?
    };
    apply_i18n(&mut manifest, &locale_bundle, &fallback_bundle);
    Ok(manifest)
}

// ── Search ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchQuery {
    /// Optional free-text filter, combined with `topic:ordo-template`
    pub q: Option<String>,
    /// Sort field: `stars` (default) or `updated`
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// GET /api/v1/marketplace/search
pub async fn search_marketplace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Json<MarketplaceSearchResponse>> {
    let token = get_optional_github_token(&state, &claims.sub).await?;
    let locale = locale_from_headers(&headers);

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(24).clamp(1, 100);
    let sort = match params.sort.as_deref() {
        Some("updated") => "updated",
        _ => "stars",
    };
    let q_extra = params.q.as_deref().unwrap_or("").trim();
    let search_query = if q_extra.is_empty() {
        "topic:ordo-template".to_string()
    } else {
        format!("topic:ordo-template {}", q_extra)
    };

    let cache_key = format!(
        "search:{}:{}:{}:{}:{}",
        locale, search_query, sort, page, per_page
    );
    if let Some(cached) = state.marketplace_cache.get_search(&cache_key).await {
        let result: MarketplaceSearchResponse =
            serde_json::from_value(cached).map_err(|e| PlatformError::Internal(e.into()))?;
        return Ok(Json(result));
    }

    let url = format!(
        "{}/search/repositories?q={}&sort={}&order=desc&per_page={}&page={}",
        GITHUB_API,
        urlencoding::encode(&search_query),
        sort,
        per_page,
        page,
    );

    let gh_resp: GhSearchResponse = gh_get(&state.http_client, &url, token.as_deref())
        .await
        .map_err(|e| PlatformError::bad_request(&format!("GitHub search failed: {}", e)))?;

    let mut items = Vec::new();
    for r in gh_resp.items {
        let localized =
            fetch_localized_manifest(&state, &r.owner.login, &r.name, locale, token.as_deref())
                .await
                .ok();
        let metadata = localized.as_ref().and_then(|v| {
            serde_json::from_value::<crate::models::TemplateMetadata>(v.clone()).ok()
        });
        items.push(MarketplaceItem {
            id: r.full_name.clone(),
            name: metadata
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or(r.name.clone()),
            full_name: r.full_name,
            description: metadata
                .as_ref()
                .map(|m| m.description.clone())
                .or(r.description),
            html_url: r.html_url,
            stars: r.stargazers_count,
            topics: r.topics.unwrap_or_default(),
            updated_at: r.updated_at,
            owner_login: r.owner.login,
            owner_avatar: r.owner.avatar_url,
            icon: metadata.as_ref().and_then(|m| m.icon.clone()),
            difficulty: metadata.as_ref().map(|m| m.difficulty.clone()),
            tags: metadata
                .as_ref()
                .map(|m| m.tags.clone())
                .unwrap_or_default(),
            features: metadata
                .as_ref()
                .map(|m| m.features.clone())
                .unwrap_or_default(),
        });
    }

    let result = MarketplaceSearchResponse {
        items,
        total_count: gh_resp.total_count,
        page,
        per_page,
    };

    let cached_val =
        serde_json::to_value(&result).map_err(|e| PlatformError::Internal(e.into()))?;
    state
        .marketplace_cache
        .set_search(cache_key, cached_val)
        .await;

    Ok(Json(result))
}

/// GET /api/v1/marketplace/repos/:owner/:repo
///
/// Fetches repository metadata from GitHub and its `ordo-template.json` manifest,
/// enriches the manifest with GitHub metadata, and returns the combined payload.
pub async fn get_marketplace_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Path((owner, repo)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let token = get_optional_github_token(&state, &claims.sub).await?;
    let locale = locale_from_headers(&headers);

    // Fetch repo metadata for star count / updated_at enrichment
    let repo_url = format!("{}/repos/{}/{}", GITHUB_API, owner, repo);
    let gh_repo: GhRepo = gh_get(&state.http_client, &repo_url, token.as_deref())
        .await
        .map_err(|_| PlatformError::not_found("GitHub repository not found"))?;

    let mut manifest =
        fetch_localized_manifest(&state, &owner, &repo, locale, token.as_deref()).await?;

    // Enrich manifest with live GitHub metadata
    if let Some(obj) = manifest.as_object_mut() {
        obj.insert("github_url".into(), serde_json::json!(gh_repo.html_url));
        obj.insert("stars".into(), serde_json::json!(gh_repo.stargazers_count));
        obj.insert("owner_login".into(), serde_json::json!(gh_repo.owner.login));
        obj.insert(
            "owner_avatar".into(),
            serde_json::json!(gh_repo.owner.avatar_url),
        );
        obj.insert("full_name".into(), serde_json::json!(gh_repo.full_name));
        obj.insert("updated_at".into(), serde_json::json!(gh_repo.updated_at));
        obj.insert(
            "topics".into(),
            serde_json::json!(gh_repo.topics.unwrap_or_default()),
        );
    }

    Ok(Json(manifest))
}

/// POST /api/v1/marketplace/install/:owner/:repo
///
/// Downloads `ordo-template.json`, validates it against the `TemplateDetail` schema,
/// then runs the same installation pipeline as the built-in template system.
#[derive(Deserialize)]
pub struct InstallPayload {
    pub org_id: String,
    pub project_name: String,
    #[serde(default)]
    pub project_description: Option<String>,
}

pub async fn install_marketplace_item(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((owner, repo)): Path<(String, String)>,
    Json(payload): Json<InstallPayload>,
) -> ApiResult<Json<ProjectResponse>> {
    // Validate org access (Admin+)
    load_org_and_check_role(&state, &payload.org_id, &claims.sub, Role::Admin).await?;

    let name = payload.project_name.trim();
    if name.is_empty() {
        return Err(PlatformError::bad_request("Project name is required"));
    }

    let token = get_optional_github_token(&state, &claims.sub).await?;
    let manifest = fetch_manifest(&state, &owner, &repo, token.as_deref()).await?;

    // Strip GitHub-enriched fields before deserializing into TemplateDetail
    let mut clean = manifest.clone();
    if let Some(obj) = clean.as_object_mut() {
        for key in &[
            "github_url",
            "stars",
            "owner_login",
            "owner_avatar",
            "full_name",
            "updated_at",
            "topics",
        ] {
            obj.remove(*key);
        }
    }

    let tpl: TemplateDetail = serde_json::from_value(clean).map_err(|e| {
        PlatformError::bad_request(&format!(
            "ordo-template.json does not match the required schema: {}",
            e
        ))
    })?;

    let source_label = format!("{}/{}", owner, repo);
    info!(
        user_id = %claims.sub,
        org_id  = %payload.org_id,
        source  = %source_label,
        project = %name,
        "Installing marketplace template"
    );

    crate::templates_api::install_template_detail(
        &state,
        &claims,
        &payload.org_id,
        name,
        payload.project_description.as_deref(),
        tpl,
        &source_label,
    )
    .await
}
