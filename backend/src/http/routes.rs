use crate::{modules, state::AppState};
use axum::{routing::get, Json, Router};
use serde_json::json;

pub fn api_router() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz)).nest(
        "/api/v1",
        Router::new()
            .merge(modules::auth::routes::routes())
            .merge(modules::attachments::routes::routes())
            .merge(modules::departments::routes::routes())
            .merge(modules::projects::routes::routes())
            .merge(modules::tasks::routes::routes())
            .merge(modules::milestones::routes::routes())
            .merge(modules::notifications::routes::routes())
            .merge(modules::comments::routes::routes())
            .merge(modules::audit::routes::routes())
            .merge(modules::search::routes::routes())
            .merge(modules::users::routes::routes()),
    )
}
async fn healthz() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "service": "projecty-api"}))
}
