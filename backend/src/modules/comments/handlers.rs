use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::comments::service::{self, CommentView, CreateCommentRequest, DeleteCommentRequest},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
fn map_error(e: service::CommentError) -> AppError {
    match e {
        service::CommentError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "任务或评论不存在".to_owned(),
        },
        service::CommentError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "没有当前任务的操作权限".to_owned(),
        },
        service::CommentError::InvalidInput(m) => AppError::bad_request(m),
        service::CommentError::Database(e) => {
            tracing::error!(?e, "comment operation failed");
            AppError::internal("评论服务暂时不可用")
        }
    }
}
pub async fn list(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
) -> Result<Json<ApiEnvelope<Vec<CommentView>>>, AppError> {
    Ok(success(
        service::list(&s.db, &u, &k).await.map_err(map_error)?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(k): Path<String>,
    Json(r): Json<CreateCommentRequest>,
) -> Result<Json<ApiEnvelope<CommentView>>, AppError> {
    Ok(success(
        service::create(&s.db, &u, &k, r).await.map_err(map_error)?,
    ))
}
pub async fn delete(
    State(s): State<AppState>,
    u: CurrentUser,
    Path(id): Path<Uuid>,
    request: Option<Json<DeleteCommentRequest>>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::delete(
        &s.db,
        &u,
        id,
        request
            .map(|Json(v)| v)
            .unwrap_or(DeleteCommentRequest { reason: None }),
    )
    .await
    .map_err(map_error)?;
    Ok(success(serde_json::json!({"message":"评论已逻辑删除"})))
}
