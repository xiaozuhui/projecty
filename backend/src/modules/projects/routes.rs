use super::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, patch, post},
    Router,
};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects", get(handlers::list).post(handlers::create))
        .route(
            "/projects/{project_key}",
            get(handlers::detail).patch(handlers::update),
        )
        .route("/projects/{project_key}/archive", post(handlers::archive))
        .route("/projects/{project_key}/restore", post(handlers::restore))
        .route("/projects/{project_key}/delete", post(handlers::delete))
        .route(
            "/projects/{project_key}/members",
            get(handlers::list_members).post(handlers::add_member),
        )
        .route(
            "/projects/{project_key}/members/{user_id}",
            patch(handlers::update_member),
        )
        .route(
            "/projects/{project_key}/members/{user_id}/revoke",
            post(handlers::revoke_member),
        )
        .route(
            "/projects/{project_key}/member-candidates",
            get(handlers::list_member_candidates),
        )
        .route(
            "/projects/{project_key}/department-grants",
            get(handlers::list_department_grants).post(handlers::grant_department),
        )
        .route(
            "/projects/{project_key}/department-grants/{department_id}/revoke",
            post(handlers::revoke_department_grant),
        )
}
