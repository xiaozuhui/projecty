use super::handlers;
use crate::{config, state::AppState};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post, put},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks/{task_key}/attachments",
            get(handlers::list).post(handlers::upload),
        )
        .route("/attachments/{object_key}/content", get(handlers::content))
        .route("/attachments/{id}/delete", post(handlers::delete))
        .route(
            "/tasks/{task_key}/attachments/uploads",
            post(handlers::init_upload),
        )
        .route(
            "/attachments/uploads/{upload_id}",
            get(handlers::upload_state),
        )
        .route(
            "/attachments/uploads/{upload_id}/complete",
            post(handlers::complete_upload),
        )
        .route(
            "/attachments/uploads/{upload_id}/abort",
            post(handlers::abort_upload),
        )
        .route(
            "/attachments/uploads/{upload_id}/chunks/{index}",
            put(handlers::upload_chunk)
                .layer(DefaultBodyLimit::max(config::UPLOAD_CHUNK_BYTES + 65_536)),
        )
}
