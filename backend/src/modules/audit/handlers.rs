use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::audit::service::{self, AuditListResponse, AuditQuery},
    state::AppState,
};

fn map_error(error: service::AuditError) -> AppError {
    match error {
        service::AuditError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "日志目标不存在".to_owned(),
        },
        service::AuditError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有查看当前项目日志的权限".to_owned(),
        },
        service::AuditError::SuperAdminRequired => AppError {
            status: StatusCode::FORBIDDEN,
            code: "super_admin_required",
            message: "只有超级管理员可以导出全局操作日志".to_owned(),
        },
        service::AuditError::Database(error) => {
            tracing::error!(?error, "audit operation failed");
            AppError::internal("操作日志服务暂时不可用")
        }
    }
}

pub async fn project_logs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<ApiEnvelope<AuditListResponse>>, AppError> {
    let response = service::project_logs(&state.db, &current_user, &project_key, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn export_project_logs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let csv = service::export_project_logs(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(csv_response(
        format!("project-{}-operation-logs.csv", project_key),
        csv,
    ))
}

pub async fn task_logs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<ApiEnvelope<AuditListResponse>>, AppError> {
    let response = service::task_logs(&state.db, &current_user, &task_key, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn export_task_logs(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let csv = service::export_task_logs(&state.db, &current_user, &task_key)
        .await
        .map_err(map_error)?;
    Ok(csv_response(
        format!("task-{}-operation-logs.csv", task_key),
        csv,
    ))
}

pub async fn export_admin_logs(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, AppError> {
    let csv = service::export_admin_logs(&state.db, &current_user)
        .await
        .map_err(map_error)?;
    Ok(csv_response("admin-operation-logs.csv".to_owned(), csv))
}

fn csv_response(filename: String, csv: String) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        csv,
    )
}
