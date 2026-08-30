use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::milestones::service,
    state::AppState,
};

fn map_status_error(error: service::StatusError) -> AppError {
    match error {
        service::StatusError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "项目不存在".to_owned(),
        },
        service::StatusError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有当前项目的读取权限".to_owned(),
        },
        service::StatusError::Database(error) => {
            tracing::error!(?error, "project status operation failed");
            AppError::internal("项目状态服务暂时不可用")
        }
    }
}

pub async fn statuses(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<Vec<service::ProjectStatusView>>>, AppError> {
    let response = service::list_statuses(&state.db, &current_user, &project_key)
        .await
        .map_err(map_status_error)?;
    Ok(success(response))
}

pub async fn reorder_statuses() -> crate::http::error::ApiResponse {
    crate::http::error::placeholder("statuses", "reorder")
}
pub async fn list() -> crate::http::error::ApiResponse {
    crate::http::error::placeholder("milestones", "list")
}
pub async fn create() -> crate::http::error::ApiResponse {
    crate::http::error::placeholder("milestones", "create")
}
pub async fn update() -> crate::http::error::ApiResponse {
    crate::http::error::placeholder("milestones", "update")
}
pub async fn delete() -> crate::http::error::ApiResponse {
    crate::http::error::placeholder("milestones", "logical_delete")
}
