use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;

use crate::modules::auth::service::AuthError;

#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T: Serialize> {
    pub data: T,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize)]
pub struct ResponseMeta {
    pub request_id: String,
}

pub fn success<T: Serialize>(data: T) -> Json<ApiEnvelope<T>> {
    Json(ApiEnvelope {
        data,
        meta: ResponseMeta {
            request_id: "dev-request".to_owned(),
        },
    })
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.into(),
        }
    }
}

impl From<AuthError> for AppError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidCredentials => Self::unauthorized("账号或密码错误"),
            AuthError::InvalidToken => Self::unauthorized("登录凭证无效或已过期"),
            AuthError::InactiveUser => Self::unauthorized("账号已停用"),
            AuthError::InvalidSystemRole => Self::internal("用户系统角色配置无效"),
            AuthError::InvalidPassword => Self::bad_request("密码不能为空"),
            AuthError::Database(error) => {
                tracing::error!(?error, "authentication database operation failed");
                Self::internal("认证服务暂时不可用")
            }
            AuthError::Token(error) => {
                tracing::warn!(?error, "JWT operation failed");
                Self::unauthorized("登录凭证无效或已过期")
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ApiEnvelope {
                data: json!(ErrorBody {
                    code: self.code,
                    message: self.message,
                }),
                meta: ResponseMeta {
                    request_id: "dev-request".to_owned(),
                },
            }),
        )
            .into_response()
    }
}
