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
    modules::projects::service::{
        self, AddMemberRequest, CreateProjectRequest, DeleteProjectRequest, ListProjectsQuery,
        MemberCandidatesQuery, MemberCandidatesResponse, ProjectDepartmentGrantsResponse,
        ProjectListResponse, ProjectMembersResponse, ProjectView, UpdateMemberRequest,
        UpdateProjectRequest,
    },
    state::AppState,
};

fn map_error(error: service::ProjectError) -> AppError {
    match error {
        service::ProjectError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "项目、成员或部门授权不存在".to_owned(),
        },
        service::ProjectError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有当前项目的管理权限".to_owned(),
        },
        service::ProjectError::InvalidInput(message) => AppError::bad_request(message),
        service::ProjectError::Conflict(message) => AppError {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        },
        service::ProjectError::Database(error) => {
            tracing::error!(?error, "project operation failed");
            AppError::internal("项目服务暂时不可用")
        }
        service::ProjectError::Serialization(error) => {
            tracing::error!(?error, "project audit serialization failed");
            AppError::internal("项目操作记录暂时不可用")
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<ApiEnvelope<ProjectListResponse>>, AppError> {
    let response = service::list(&state.db, &current_user, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<ApiEnvelope<ProjectView>>, AppError> {
    let response = service::create(&state.db, &current_user, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn detail(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<ProjectView>>, AppError> {
    let response = service::detail(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<Json<ApiEnvelope<ProjectView>>, AppError> {
    let response = service::update(&state.db, &current_user, &project_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn archive(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<ProjectView>>, AppError> {
    let response = service::archive(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn restore(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<ProjectView>>, AppError> {
    let response = service::restore(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    request: Option<Json<DeleteProjectRequest>>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(
        &state.db,
        &current_user,
        &project_key,
        request
            .map(|Json(value)| value)
            .unwrap_or(DeleteProjectRequest { reason: None }),
    )
    .await
    .map_err(map_error)?;
    Ok(success(json!({ "message": "项目已逻辑删除" })))
}

pub async fn list_members(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<ProjectMembersResponse>>, AppError> {
    let response = service::list_members(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn add_member(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Json(request): Json<AddMemberRequest>,
) -> Result<Json<ApiEnvelope<ProjectMembersResponse>>, AppError> {
    let response = service::add_member(&state.db, &current_user, &project_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn update_member(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((project_key, user_id)): Path<(String, Uuid)>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<Json<ApiEnvelope<ProjectMembersResponse>>, AppError> {
    let response = service::update_member(&state.db, &current_user, &project_key, user_id, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn revoke_member(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((project_key, user_id)): Path<(String, Uuid)>,
) -> Result<Json<ApiEnvelope<ProjectMembersResponse>>, AppError> {
    let response = service::revoke_member(&state.db, &current_user, &project_key, user_id)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn list_member_candidates(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Query(query): Query<MemberCandidatesQuery>,
) -> Result<Json<ApiEnvelope<MemberCandidatesResponse>>, AppError> {
    let response = service::list_member_candidates(&state.db, &current_user, &project_key, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn list_department_grants(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
) -> Result<Json<ApiEnvelope<ProjectDepartmentGrantsResponse>>, AppError> {
    let response = service::list_department_grants(&state.db, &current_user, &project_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn grant_department(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Json(request): Json<GrantDepartmentRequest>,
) -> Result<Json<ApiEnvelope<ProjectDepartmentGrantsResponse>>, AppError> {
    let response = service::grant_department(
        &state.db,
        &current_user,
        &project_key,
        request.department_id,
        request.role,
    )
    .await
    .map_err(map_error)?;
    Ok(success(response))
}

pub async fn revoke_department_grant(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path((project_key, department_id)): Path<(String, Uuid)>,
) -> Result<Json<ApiEnvelope<ProjectDepartmentGrantsResponse>>, AppError> {
    let response =
        service::revoke_department_grant(&state.db, &current_user, &project_key, department_id)
            .await
            .map_err(map_error)?;
    Ok(success(response))
}

#[derive(Debug, serde::Deserialize)]
pub struct GrantDepartmentRequest {
    pub department_id: Uuid,
    pub role: String,
}
