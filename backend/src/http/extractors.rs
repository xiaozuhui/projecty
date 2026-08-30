use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use std::future::Future;
use uuid::Uuid;

use crate::domain::permissions::SystemRole;
use crate::{http::error::AppError, modules::auth::service::decode_access_claims, state::AppState};

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub account: String,
    pub system_role: SystemRole,
}

impl CurrentUser {
    pub fn dev_user() -> Self {
        Self {
            user_id: Uuid::nil(),
            account: "dev".to_owned(),
            system_role: SystemRole::SuperAdmin,
        }
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("需要提供 Bearer 登录凭证"))
            .and_then(|header| {
                let token = header
                    .strip_prefix("Bearer ")
                    .or_else(|| header.strip_prefix("bearer "))
                    .filter(|token| !token.trim().is_empty())
                    .ok_or_else(|| AppError::unauthorized("Authorization 必须使用 Bearer 方案"))?;
                let claims = decode_access_claims(&state.config, token.trim())
                    .map_err(|_| AppError::unauthorized("登录凭证无效或已过期"))?;
                let user_id = Uuid::parse_str(&claims.sub)
                    .map_err(|_| AppError::unauthorized("登录凭证中的用户标识无效"))?;
                Ok(Self {
                    user_id,
                    account: claims.account,
                    system_role: claims.system_role,
                })
            });
        async move { result }
    }
}
