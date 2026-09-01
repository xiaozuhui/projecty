use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::tasks::service::{
        self, CreateTaskRequest, CrossProjectTaskListResponse, CrossProjectTasksQuery,
        DeleteTaskRequest, ListTasksQuery, MoveTaskRequest, TaskListResponse, TaskView,
        TransitionTaskRequest, UpdateTaskRequest,
    },
    state::AppState,
};

fn map_error(error: service::TaskError) -> AppError {
    match error {
        service::TaskError::NotFound => AppError {
            status: axum::http::StatusCode::NOT_FOUND,
            code: "not_found",
            message: "任务或项目不存在".to_owned(),
        },
        service::TaskError::Forbidden => AppError {
            status: axum::http::StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有当前项目的任务操作权限".to_owned(),
        },
        service::TaskError::ForbiddenAction(message) => AppError {
            status: axum::http::StatusCode::FORBIDDEN,
            code: "forbidden_action",
            message,
        },
        service::TaskError::InvalidInput(message) => AppError::bad_request(message),
        service::TaskError::Conflict(message) => AppError {
            status: axum::http::StatusCode::CONFLICT,
            code: "conflict",
            message,
        },
        service::TaskError::Database(error) => {
            tracing::error!(?error, "task operation failed");
            AppError::internal("任务服务暂时不可用")
        }
        service::TaskError::Serialization(error) => {
            tracing::error!(?error, "task audit serialization failed");
            AppError::internal("任务操作记录暂时不可用")
        }
    }
}

pub async fn list_project_tasks(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<ApiEnvelope<TaskListResponse>>, AppError> {
    let response = service::list_project_tasks(&state.db, &current_user, &project_key, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn create_project_task(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(project_key): Path<String>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::create_project_task(&state.db, &current_user, &project_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn list_cross_project_tasks(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<CrossProjectTasksQuery>,
) -> Result<Json<ApiEnvelope<CrossProjectTaskListResponse>>, AppError> {
    let response = service::list_cross_project_tasks(&state.db, &current_user, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn detail(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::detail(&state.db, &current_user, &task_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Json(request): Json<UpdateTaskRequest>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::update(&state.db, &current_user, &task_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn transition(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Json(request): Json<TransitionTaskRequest>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::transition(&state.db, &current_user, &task_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn move_task(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Json(request): Json<MoveTaskRequest>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::move_task(&state.db, &current_user, &task_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    request: Option<Json<DeleteTaskRequest>>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(
        &state.db,
        &current_user,
        &task_key,
        request
            .map(|Json(value)| value)
            .unwrap_or(DeleteTaskRequest { reason: None }),
    )
    .await
    .map_err(map_error)?;
    Ok(success(serde_json::json!({ "message": "任务已逻辑删除" })))
}

pub async fn restore(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::restore(&state.db, &current_user, &task_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn subtasks(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
) -> Result<Json<ApiEnvelope<Vec<TaskView>>>, AppError> {
    let response = service::subtasks(&state.db, &current_user, &task_key)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn create_subtask(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(task_key): Path<String>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<ApiEnvelope<TaskView>>, AppError> {
    let response = service::create_subtask(&state.db, &current_user, &task_key, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}
