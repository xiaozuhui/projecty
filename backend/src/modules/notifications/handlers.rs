use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::notifications::service::{self, ListNotificationsQuery},
    state::AppState,
};

fn map_error(error: service::NotificationError) -> AppError {
    match error {
        service::NotificationError::NotFound => AppError {
            status: axum::http::StatusCode::NOT_FOUND,
            code: "not_found",
            message: "通知不存在".to_owned(),
        },
        service::NotificationError::InvalidInput(message) => AppError::bad_request(message),
        service::NotificationError::Database(error) => {
            tracing::error!(?error, "notification operation failed");
            AppError::internal("通知服务暂时不可用")
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<Json<ApiEnvelope<service::NotificationListResponse>>, AppError> {
    let response = service::list(&state.db, &current_user, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn unread_count(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiEnvelope<service::UnreadCountResponse>>, AppError> {
    let count = service::unread_count(&state.db, &current_user)
        .await
        .map_err(map_error)?;
    Ok(success(service::UnreadCountResponse { count }))
}

pub async fn mark_read(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::mark_read(&state.db, &current_user, id)
        .await
        .map_err(map_error)?;
    Ok(success(serde_json::json!({ "message": "通知已标记为已读" })))
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    let updated = service::mark_all_read(&state.db, &current_user)
        .await
        .map_err(map_error)?;
    Ok(success(
        serde_json::json!({ "message": "全部通知已标记为已读", "updated": updated }),
    ))
}
