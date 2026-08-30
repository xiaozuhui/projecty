use axum::{extract::State, Json};

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::auth::service::{
        self, AuthSession, ChangePasswordRequest, LoginRequest, MeResponse, RefreshTokenRequest,
    },
    state::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiEnvelope<AuthSession>>, AppError> {
    let session = service::login(
        &state.db,
        &state.config,
        &request.account,
        &request.password,
    )
    .await?;
    Ok(success(session))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<ApiEnvelope<AuthSession>>, AppError> {
    let session = service::refresh(&state.db, &state.config, &request.refresh_token).await?;
    Ok(success(session))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::logout(&state.db, &state.config, &request.refresh_token).await?;
    Ok(success(serde_json::json!({ "message": "已退出登录" })))
}

pub async fn me(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ApiEnvelope<MeResponse>>, AppError> {
    let response = service::me(&state.db, current_user.user_id).await?;
    Ok(success(response))
}

pub async fn change_password(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<ApiEnvelope<serde_json::Value>>, AppError> {
    service::change_password(
        &state.db,
        current_user.user_id,
        &request.current_password,
        &request.new_password,
    )
    .await?;
    Ok(success(
        serde_json::json!({ "message": "密码已修改，请重新登录" }),
    ))
}
