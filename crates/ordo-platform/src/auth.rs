//! Authentication handlers: register, login, refresh, me

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, User, UserInfo},
    AppState,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Extension, State},
    Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/system/config — public, returns feature flags the UI needs before login
pub async fn system_config(State(state): State<AppState>) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "allow_registration": state.config.allow_registration,
        "allow_org_creation": state.config.allow_org_creation,
    }))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<AuthResponse>> {
    if !state.config.allow_registration {
        return Err(PlatformError::forbidden(
            "Self-registration is disabled. Contact your organization admin for an invitation.",
        ));
    }

    // Validate input
    if req.email.trim().is_empty() || !req.email.contains('@') {
        return Err(PlatformError::bad_request("Invalid email address"));
    }
    if req.password.len() < 8 {
        return Err(PlatformError::bad_request(
            "Password must be at least 8 characters",
        ));
    }
    if req.display_name.trim().is_empty() {
        return Err(PlatformError::bad_request("Display name is required"));
    }

    // Check email not taken
    if state
        .store
        .find_user_by_email(&req.email)
        .await
        .map_err(PlatformError::Internal)?
        .is_some()
    {
        return Err(PlatformError::conflict("Email already registered"));
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| PlatformError::internal(format!("Failed to hash password: {}", e)))?
        .to_string();

    let user = User {
        id: Uuid::new_v4().to_string(),
        email: req.email.to_lowercase(),
        password_hash: hash,
        display_name: req.display_name.trim().to_string(),
        created_at: Utc::now(),
        last_login: None,
    };

    state
        .store
        .save_user(&user)
        .await
        .map_err(PlatformError::Internal)?;

    let token = issue_token(
        &user,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo::from(&user),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let mut user = state
        .store
        .find_user_by_email(&req.email)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::unauthorized("Invalid email or password"))?;

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|_| PlatformError::internal("Invalid stored password hash"))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| PlatformError::unauthorized("Invalid email or password"))?;

    user.last_login = Some(Utc::now());
    state
        .store
        .update_user(&user)
        .await
        .map_err(PlatformError::Internal)?;

    let token = issue_token(
        &user,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo::from(&user),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<UserInfo>> {
    let user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;
    Ok(Json(UserInfo::from(&user)))
}

pub async fn update_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<UpdateProfileRequest>,
) -> ApiResult<Json<UserInfo>> {
    let mut user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;
    if let Some(name) = req.display_name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(PlatformError::bad_request("Display name cannot be empty"));
        }
        user.display_name = name;
    }
    state
        .store
        .update_user(&user)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(UserInfo::from(&user)))
}

pub async fn change_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> ApiResult<axum::http::StatusCode> {
    let mut user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;

    // Verify current password
    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|_| PlatformError::internal("Invalid stored password hash"))?;
    Argon2::default()
        .verify_password(req.current_password.as_bytes(), &parsed)
        .map_err(|_| PlatformError::unauthorized("Current password is incorrect"))?;

    if req.new_password.len() < 8 {
        return Err(PlatformError::bad_request(
            "New password must be at least 8 characters",
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    user.password_hash = Argon2::default()
        .hash_password(req.new_password.as_bytes(), &salt)
        .map_err(|e| PlatformError::internal(format!("Failed to hash password: {}", e)))?
        .to_string();

    state
        .store
        .update_user(&user)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn refresh(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<AuthResponse>> {
    let user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;
    let token = issue_token(
        &user,
        &state.config.jwt_secret,
        state.config.jwt_expiry_hours,
    )?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo::from(&user),
    }))
}

// ── JWT helpers ───────────────────────────────────────────────────────────────

pub fn issue_token(user: &User, secret: &str, expiry_hours: u64) -> ApiResult<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = now + (expiry_hours as usize * 3600);
    let claims = Claims {
        sub: user.id.clone(),
        email: user.email.clone(),
        exp,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| PlatformError::internal(format!("Failed to sign JWT: {}", e)))
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, PlatformError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| PlatformError::invalid_token(e.to_string()))
}
