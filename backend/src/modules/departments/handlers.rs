use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::departments::service::{
        self, CreateDepartmentRequest, DeleteDepartmentRequest, DepartmentListResponse,
        DepartmentMembersResponse, DepartmentProjectsResponse, DepartmentView,
        ListDepartmentsQuery, UpdateDepartmentRequest,
    },
    state::AppState,
};

fn map_error(error: service::DepartmentError) -> AppError {
    match error {
        service::DepartmentError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "部门不存在".to_owned(),
        },
        service::DepartmentError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "只有超级管理员可以管理或查看全部部门".to_owned(),
        },
        service::DepartmentError::InvalidInput(message) => AppError::bad_request(message),
        service::DepartmentError::Conflict(message) => AppError {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        },
        service::DepartmentError::Database(error) => {
            tracing::error!(?error, "department operation failed");
            AppError::internal("部门服务暂时不可用")
        }
        service::DepartmentError::Serialization(error) => {
            tracing::error!(?error, "department audit serialization failed");
            AppError::internal("部门操作记录暂时不可用")
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ListDepartmentsQuery>,
) -> Result<Json<ApiEnvelope<DepartmentListResponse>>, AppError> {
    let response = service::list(&state.db, &current_user, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<CreateDepartmentRequest>,
) -> Result<Json<ApiEnvelope<DepartmentView>>, AppError> {
    let response = service::create(&state.db, &current_user, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(department_id): Path<Uuid>,
    Json(request): Json<UpdateDepartmentRequest>,
) -> Result<Json<ApiEnvelope<DepartmentView>>, AppError> {
    let response = service::update(&state.db, &current_user, department_id, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(department_id): Path<Uuid>,
    Json(request): Json<DeleteDepartmentRequest>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(&state.db, &current_user, department_id, request)
        .await
        .map_err(map_error)?;
    Ok(success(json!({ "message": "部门已逻辑删除" })))
}

pub async fn members(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(department_id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<DepartmentMembersResponse>>, AppError> {
    let response = service::members(&state.db, &current_user, department_id)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn projects(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(department_id): Path<Uuid>,
) -> Result<Json<ApiEnvelope<DepartmentProjectsResponse>>, AppError> {
    let response = service::projects(&state.db, &current_user, department_id)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}
