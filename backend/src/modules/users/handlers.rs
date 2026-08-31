use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use uuid::Uuid;

use crate::{
    http::{
        error::{success, ApiEnvelope, AppError},
        extractors::CurrentUser,
    },
    modules::users::service::{
        self, CreateUserRequest, ImportReport, ListUsersQuery, UpdateUserRequest, UserListResponse,
        UserView,
    },
    state::AppState,
};

fn map_error(error: service::UserError) -> AppError {
    match error {
        service::UserError::Forbidden => AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "只有超级管理员可以管理用户".to_owned(),
        },
        service::UserError::NotFound => AppError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "用户不存在".to_owned(),
        },
        service::UserError::InvalidInput(message) => AppError::bad_request(message),
        service::UserError::Conflict(message) => AppError {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        },
        service::UserError::Database(error) => {
            tracing::error!(?error, "user operation failed");
            AppError::internal("用户服务暂时不可用")
        }
        service::UserError::Serialization(error) => {
            tracing::error!(?error, "user audit serialization failed");
            AppError::internal("用户操作记录暂时不可用")
        }
        service::UserError::Excel(message) => AppError::bad_request(message),
    }
}

pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<ApiEnvelope<UserListResponse>>, AppError> {
    let response = service::list_users(&state.db, &current_user, &query)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<ApiEnvelope<UserView>>, AppError> {
    let response = service::create_user(
        &state.db,
        Some(&current_user),
        service::NewUser {
            account: request.account,
            password: request.password,
            display_name: request.display_name,
            system_role: request
                .system_role
                .unwrap_or(crate::domain::permissions::SystemRole::User),
            department_ids: request.department_ids.unwrap_or_default(),
        },
    )
    .await
    .map_err(map_error)?;
    Ok(success(response))
}

pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiEnvelope<UserView>>, AppError> {
    let response = service::update_user(&state.db, &current_user, user_id, request)
        .await
        .map_err(map_error)?;
    Ok(success(response))
}

pub async fn import_template(current_user: CurrentUser) -> Result<Response, AppError> {
    if !current_user.system_role.is_super_admin() {
        return Err(AppError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "只有超级管理员可以管理用户".to_owned(),
        });
    }
    let bytes = service::build_import_template().map_err(map_error)?;
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"projecty-users-template.xlsx\"",
        )
        .body(Body::from(bytes))
        .map_err(|_| AppError::internal("模板下载暂时不可用"))
}

pub async fn import(
    State(state): State<AppState>,
    current_user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<ApiEnvelope<ImportReport>>, AppError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::bad_request(format!("文件上传失败：{error}")))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|error| AppError::bad_request(format!("文件读取失败：{error}")))?;
            file_bytes = Some(bytes.to_vec());
            break;
        }
    }
    let bytes =
        file_bytes.ok_or_else(|| AppError::bad_request("需要上传名为 file 的 Excel 文件"))?;
    let rows = service::parse_import_workbook(&bytes).map_err(map_error)?;
    if rows.is_empty() {
        return Err(AppError::bad_request("Excel 中没有可导入的数据行"));
    }
    let report = service::import_users(&state.db, &current_user, rows)
        .await
        .map_err(map_error)?;
    Ok(success(report))
}
