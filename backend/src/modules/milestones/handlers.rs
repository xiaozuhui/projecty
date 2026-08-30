use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::milestones::service::{
        self, CreateMilestoneRequest, DeleteMilestoneRequest, MilestoneListResponse, MilestoneView,
        StatusOrderRequest, UpdateMilestoneRequest,
    },
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
fn map_error(error: service::MilestoneError) -> AppError {
    match error {
        service::MilestoneError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "项目或里程碑不存在".to_owned(),
        },
        service::MilestoneError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有项目管理权限".to_owned(),
        },
        service::MilestoneError::InvalidInput(m) => AppError::bad_request(m),
        service::MilestoneError::Database(e) => {
            tracing::error!(?e, "milestone operation failed");
            AppError::internal("里程碑服务暂时不可用")
        }
        service::MilestoneError::Serialization(e) => {
            tracing::error!(?e, "milestone audit serialization failed");
            AppError::internal("里程碑操作记录暂时不可用")
        }
    }
}
pub async fn statuses(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
) -> Result<Json<ApiEnvelope<Vec<service::ProjectStatusView>>>, AppError> {
    Ok(success(
        service::list_statuses(&s.db, &u, &k)
            .await
            .map_err(map_error)?,
    ))
}
pub async fn reorder_statuses(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
    Json(r): Json<StatusOrderRequest>,
) -> Result<Json<ApiEnvelope<Vec<service::ProjectStatusView>>>, AppError> {
    Ok(success(
        service::reorder_statuses(&s.db, &u, &k, r)
            .await
            .map_err(map_error)?,
    ))
}
pub async fn list(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
) -> Result<Json<ApiEnvelope<MilestoneListResponse>>, AppError> {
    Ok(success(
        service::list(&s.db, &u, &k).await.map_err(map_error)?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
    Json(r): Json<CreateMilestoneRequest>,
) -> Result<Json<ApiEnvelope<MilestoneView>>, AppError> {
    Ok(success(
        service::create(&s.db, &u, &k, r).await.map_err(map_error)?,
    ))
}
pub async fn update(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<Uuid>,
    Json(r): Json<UpdateMilestoneRequest>,
) -> Result<Json<ApiEnvelope<MilestoneView>>, AppError> {
    Ok(success(
        service::update(&s.db, &u, id, r).await.map_err(map_error)?,
    ))
}
pub async fn delete(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<Uuid>,
    request: Option<Json<DeleteMilestoneRequest>>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(
        &s.db,
        &u,
        id,
        request
            .map(|Json(v)| v)
            .unwrap_or(DeleteMilestoneRequest { reason: None }),
    )
    .await
    .map_err(map_error)?;
    Ok(success(serde_json::json!({"message":"里程碑已逻辑删除"})))
}
