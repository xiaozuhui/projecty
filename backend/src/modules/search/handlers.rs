use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::search::service::{self, SearchQuery, SearchResult},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    Json,
};
fn map_error(e: service::SearchError) -> AppError {
    match e {
        service::SearchError::Database(e) => {
            tracing::error!(?e, "search operation failed");
            AppError::internal("搜索服务暂时不可用")
        }
    }
}
pub async fn search(
    State(s): State<AppState>,
    u: CurrentUser,
    Query(q): Query<SearchQuery>,
) -> Result<Json<ApiEnvelope<SearchResult>>, AppError> {
    Ok(success(
        service::search(&s.db, &u, &q).await.map_err(map_error)?,
    ))
}
