//! Tenant extraction and rate limiting middleware

use crate::metrics;
use crate::tenant::TenantConfig;
use crate::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub id: String,
    pub config: TenantConfig,
}

pub async fn tenant_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // Skip tenant validation for tenant management endpoints
    let path = req.uri().path();
    if is_tenant_management_path(path) {
        // This is a tenant management endpoint, use default tenant context
        let tenant_id = state.config.default_tenant.clone();
        let config = TenantConfig::default_for_id(&tenant_id, state.tenant_manager.defaults());
        req.extensions_mut().insert(TenantContext {
            id: tenant_id,
            config,
        });
        return next.run(req).await;
    }

    let tenant_id = extract_tenant_id(&req).unwrap_or_else(|| state.config.default_tenant.clone());

    if !state.config.multi_tenancy_enabled {
        let config = TenantConfig::default_for_id(&tenant_id, state.tenant_manager.defaults());
        req.extensions_mut().insert(TenantContext {
            id: tenant_id,
            config,
        });
        return next.run(req).await;
    }

    let config = match state.tenant_manager.validate_enabled(&tenant_id).await {
        Ok(config) => config,
        Err(message) => return (StatusCode::NOT_FOUND, message).into_response(),
    };

    let allowed = state
        .rate_limiter
        .allow(&tenant_id, config.qps_limit, config.burst_limit);
    if !allowed {
        metrics::record_tenant_rate_limited(&tenant_id);
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
    }

    req.extensions_mut().insert(TenantContext {
        id: tenant_id,
        config,
    });
    next.run(req).await
}

fn extract_tenant_id(req: &Request<Body>) -> Option<String> {
    if let Some(value) = req.headers().get("X-Tenant-ID") {
        if let Ok(s) = value.to_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    if let Some(path_tenant) = extract_from_path(req.uri().path()) {
        return Some(path_tenant);
    }

    if let Some(query) = req.uri().query() {
        if let Some(value) = extract_from_query(query, "tenant_id") {
            return Some(value);
        }
    }

    None
}

fn extract_from_path(path: &str) -> Option<String> {
    let mut segments = path.split('/').filter(|s| !s.is_empty());
    if segments.next()? == "api" && segments.next()? == "v1" && segments.next()? == "tenants" {
        return segments.next().map(|s| s.to_string());
    }
    None
}

fn is_tenant_management_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 3 {
        return false;
    }
    if segments[0] != "api" || segments[1] != "v1" || segments[2] != "tenants" {
        return false;
    }
    segments.len() == 3 || segments.len() == 4
}

fn extract_from_query(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut iter = pair.splitn(2, '=');
        let k = iter.next().unwrap_or("");
        let v = iter.next().unwrap_or("");
        if k == key && !v.is_empty() {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.as_bytes().iter().copied();
    while let Some(c) = chars.next() {
        if c == b'%' {
            let hi = chars.next();
            let lo = chars.next();
            if let (Some(hi), Some(lo)) = (hi, lo) {
                if let Ok(hex) = std::str::from_utf8(&[hi, lo]) {
                    if let Ok(byte) = u8::from_str_radix(hex, 16) {
                        output.push(byte as char);
                        continue;
                    }
                }
            }
        }
        output.push(c as char);
    }
    output
}
