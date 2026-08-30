use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T: Serialize> {
    pub data: T,
    pub meta: ResponseMeta,
}
#[derive(Debug, Serialize)]
pub struct ResponseMeta {
    pub request_id: String,
}
pub type ApiResponse = Json<ApiEnvelope<Value>>;

pub fn placeholder(module: &'static str, operation: &'static str) -> ApiResponse {
    Json(ApiEnvelope {
        data: json!({ "message": "not implemented", "module": module, "operation": operation }),
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
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ApiEnvelope {
                data: json!(ErrorBody {
                    code: self.code,
                    message: self.message
                }),
                meta: ResponseMeta {
                    request_id: "dev-request".to_owned(),
                },
            }),
        )
            .into_response()
    }
}
