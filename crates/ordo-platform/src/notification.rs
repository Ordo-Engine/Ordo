use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, NotificationCount, PlatformNotification, ReleaseRequest},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    #[serde(default)]
    pub unread_only: bool,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

pub async fn list_notifications(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Query(query): Query<ListNotificationsQuery>,
) -> ApiResult<Json<Vec<PlatformNotification>>> {
    let items = state
        .store
        .list_notifications(
            &claims.sub,
            &org_id,
            query.limit.clamp(1, 200),
            query.offset.max(0),
            query.unread_only,
        )
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(items))
}

pub async fn get_notification_count(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<NotificationCount>> {
    let unread = state
        .store
        .get_unread_notification_count(&claims.sub, &org_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(NotificationCount { unread }))
}

pub async fn mark_notification_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, notif_id)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    let _ = org_id;
    state
        .store
        .mark_notification_read(&notif_id, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn mark_all_notifications_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    state
        .store
        .mark_all_notifications_read(&claims.sub, &org_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn list_pending_approvals_for_me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<ReleaseRequest>>> {
    let items = state
        .store
        .list_pending_approvals_for_reviewer(&org_id, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(items))
}
