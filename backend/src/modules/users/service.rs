//! 用户管理：超级管理员创建/维护员工账号、部门归属与 Excel 批量导入。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use projecty_entity::{departments, jwt_refresh_tokens, operation_logs, user_departments, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::domain::permissions::SystemRole;
use crate::http::extractors::CurrentUser;
use crate::modules::auth::service::hash_password;

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("只有超级管理员可以管理用户")]
    Forbidden,
    #[error("用户不存在")]
    NotFound,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("当前操作不允许：{0}")]
    Conflict(String),
    #[error("数据库错误：{0}")]
    Database(#[from] DbErr),
    #[error("数据序列化错误：{0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Excel 处理失败：{0}")]
    Excel(String),
}

#[derive(Debug, Serialize)]
pub struct DepartmentBrief {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub account: String,
    pub display_name: String,
    pub system_role: SystemRole,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub departments: Vec<DepartmentBrief>,
}

#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub items: Vec<UserView>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub search: Option<String>,
    pub department_id: Option<Uuid>,
    pub include_inactive: Option<bool>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

impl ListUsersQuery {
    fn normalized(&self) -> (u64, u64) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self.page_size.unwrap_or(30).clamp(1, 100);
        (page, page_size)
    }
}

#[derive(Debug)]
pub struct NewUser {
    pub account: String,
    pub password: String,
    pub display_name: String,
    pub system_role: SystemRole,
    pub department_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub account: String,
    pub password: String,
    pub display_name: String,
    pub system_role: Option<SystemRole>,
    pub department_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
    pub department_ids: Option<Vec<Uuid>>,
}

#[derive(Debug)]
pub struct ImportRow {
    pub account: String,
    pub display_name: String,
    pub department_names: Vec<String>,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct ImportRowResult {
    pub row_number: u64,
    pub account: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub rows: Vec<ImportRowResult>,
}

fn require_admin(current_user: &CurrentUser) -> Result<(), UserError> {
    if current_user.system_role.is_super_admin() {
        Ok(())
    } else {
        Err(UserError::Forbidden)
    }
}

fn system_role_str(role: SystemRole) -> &'static str {
    match role {
        SystemRole::SuperAdmin => "super_admin",
        SystemRole::User => "user",
    }
}

fn normalize_account(raw: &str) -> Result<String, UserError> {
    let account = raw.trim().to_owned();
    let length = account.chars().count();
    if !(2..=64).contains(&length) {
        return Err(UserError::InvalidInput("账号长度需在 2-64 个字符之间".to_owned()));
    }
    Ok(account)
}

fn normalize_display_name(raw: &str) -> Result<String, UserError> {
    let display_name = raw.trim().to_owned();
    if display_name.is_empty() || display_name.chars().count() > 80 {
        return Err(UserError::InvalidInput("姓名不能为空且不超过 80 个字符".to_owned()));
    }
    Ok(display_name)
}

fn validate_password(raw: &str) -> Result<(), UserError> {
    let length = raw.chars().count();
    if !(8..=128).contains(&length) {
        return Err(UserError::InvalidInput("密码长度需在 8-128 个字符之间".to_owned()));
    }
    Ok(())
}

fn map_unique_account(error: DbErr) -> UserError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("account") || text.contains("users_account_key") {
        UserError::Conflict("账号已存在".to_owned())
    } else {
        UserError::Database(error)
    }
}

async fn write_user_log(
    txn: &sea_orm::DatabaseTransaction,
    actor_user_id: Uuid,
    user_id: Uuid,
    action: &str,
    summary: String,
    diff: serde_json::Value,
) -> Result<(), UserError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor_user_id),
        module: Set("users".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(None),
        task_id: Set(None),
        target_type: Set("user".to_owned()),
        target_id: Set(Some(user_id)),
        summary: Set(summary),
        diff: Set(Some(diff)),
        snapshot: Set(None),
        created_at: Set(Utc::now()),
    }
    .insert(txn)
    .await?;
    Ok(())
}

async fn load_active_departments(
    db: &DatabaseConnection,
    department_ids: &[Uuid],
) -> Result<Vec<departments::Model>, UserError> {
    let mut ids = department_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let found = departments::Entity::find()
        .filter(departments::Column::Id.is_in(ids.clone()))
        .filter(departments::Column::DeletedAt.is_null())
        .order_by_asc(departments::Column::SortOrder)
        .all(db)
        .await?;
    if found.len() != ids.len() {
        return Err(UserError::InvalidInput("部分部门不存在或已删除".to_owned()));
    }
    Ok(found)
}

async fn load_department_index(
    db: &DatabaseConnection,
) -> Result<HashMap<String, departments::Model>, UserError> {
    let all = departments::Entity::find()
        .filter(departments::Column::DeletedAt.is_null())
        .all(db)
        .await?;
    Ok(all
        .into_iter()
        .map(|department| (department.name.trim().to_owned(), department))
        .collect())
}

async fn attach_departments(
    db: &DatabaseConnection,
    models: Vec<users::Model>,
) -> Result<Vec<UserView>, UserError> {
    if models.is_empty() {
        return Ok(vec![]);
    }
    let user_ids: Vec<Uuid> = models.iter().map(|user| user.id).collect();
    let memberships = user_departments::Entity::find()
        .filter(user_departments::Column::UserId.is_in(user_ids))
        .filter(user_departments::Column::RevokedAt.is_null())
        .all(db)
        .await?;
    let department_ids: Vec<Uuid> = memberships
        .iter()
        .map(|membership| membership.department_id)
        .collect();
    let department_map: HashMap<Uuid, String> = if department_ids.is_empty() {
        HashMap::new()
    } else {
        departments::Entity::find()
            .filter(departments::Column::Id.is_in(department_ids))
            .filter(departments::Column::DeletedAt.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(|department| (department.id, department.name))
            .collect()
    };
    let mut membership_map: HashMap<Uuid, Vec<DepartmentBrief>> = HashMap::new();
    for membership in memberships {
        if let Some(name) = department_map.get(&membership.department_id) {
            membership_map
                .entry(membership.user_id)
                .or_default()
                .push(DepartmentBrief {
                    id: membership.department_id,
                    name: name.clone(),
                });
        }
    }
    let mut views = Vec::with_capacity(models.len());
    for user in models {
        let system_role = parse_role(&user.system_role)?;
        views.push(UserView {
            departments: membership_map.remove(&user.id).unwrap_or_default(),
            id: user.id,
            account: user.account,
            display_name: user.display_name,
            system_role,
            is_active: user.is_active,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
        });
    }
    Ok(views)
}

fn parse_role(raw: &str) -> Result<SystemRole, UserError> {
    match raw {
        "super_admin" => Ok(SystemRole::SuperAdmin),
        "user" => Ok(SystemRole::User),
        _ => Err(UserError::Conflict("账号角色数据异常".to_owned())),
    }
}

/// 创建用户。`actor` 为 `None` 时表示 CLI 引导创建(仅用于首个超级管理员)。
pub async fn create_user(
    db: &DatabaseConnection,
    actor: Option<&CurrentUser>,
    payload: NewUser,
) -> Result<UserView, UserError> {
    if let Some(current_user) = actor {
        require_admin(current_user)?;
    }
    let account = normalize_account(&payload.account)?;
    let display_name = normalize_display_name(&payload.display_name)?;
    validate_password(&payload.password)?;
    let password_hash = hash_password(&payload.password)
        .map_err(|_| UserError::InvalidInput("密码处理失败".to_owned()))?;
    let departments = load_active_departments(db, &payload.department_ids).await?;
    let department_names: Vec<&str> = departments.iter().map(|d| d.name.as_str()).collect();

    let now = Utc::now();
    let user_id = Uuid::now_v7();
    let txn = db.begin().await?;
    let user = users::ActiveModel {
        id: Set(user_id),
        account: Set(account.clone()),
        password_hash: Set(password_hash),
        display_name: Set(display_name.clone()),
        system_role: Set(system_role_str(payload.system_role).to_owned()),
        is_active: Set(true),
        last_login_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
    }
    .insert(&txn)
    .await
    .map_err(map_unique_account)?;
    for department in &departments {
        user_departments::ActiveModel {
            user_id: Set(user_id),
            department_id: Set(department.id),
            joined_at: Set(now),
            revoked_at: Set(None),
        }
        .insert(&txn)
        .await?;
    }
    write_user_log(
        &txn,
        actor.map(|user| user.user_id).unwrap_or_default(),
        user_id,
        "create",
        format!("创建用户 {account}"),
        json!({
            "account": account,
            "display_name": display_name,
            "system_role": system_role_str(payload.system_role),
            "departments": department_names,
        }),
    )
    .await?;
    txn.commit().await?;
    Ok(UserView {
        id: user.id,
        account: user.account,
        display_name: user.display_name,
        system_role: payload.system_role,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        departments: departments
            .into_iter()
            .map(|department| DepartmentBrief {
                id: department.id,
                name: department.name,
            })
            .collect(),
    })
}

pub async fn list_users(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &ListUsersQuery,
) -> Result<UserListResponse, UserError> {
    require_admin(current_user)?;
    let (page, page_size) = query.normalized();
    let mut statement = users::Entity::find()
        .filter(users::Column::DeletedAt.is_null())
        .order_by_desc(users::Column::CreatedAt)
        .order_by_desc(users::Column::Id);
    if !query.include_inactive.unwrap_or(false) {
        statement = statement.filter(users::Column::IsActive.eq(true));
    }
    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search| !search.is_empty())
    {
        statement = statement.filter(
            Condition::any()
                .add(users::Column::Account.contains(search))
                .add(users::Column::DisplayName.contains(search)),
        );
    }
    if let Some(department_id) = query.department_id {
        let user_ids: Vec<Uuid> = user_departments::Entity::find()
            .filter(user_departments::Column::DepartmentId.eq(department_id))
            .filter(user_departments::Column::RevokedAt.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(|membership| membership.user_id)
            .collect();
        if user_ids.is_empty() {
            return Ok(UserListResponse {
                items: vec![],
                page,
                page_size,
                has_more: false,
            });
        }
        statement = statement.filter(users::Column::Id.is_in(user_ids));
    }
    let mut models = statement
        .offset((page - 1) * page_size)
        .limit(page_size + 1)
        .all(db)
        .await?;
    let has_more = models.len() > page_size as usize;
    models.truncate(page_size as usize);
    Ok(UserListResponse {
        items: attach_departments(db, models).await?,
        page,
        page_size,
        has_more,
    })
}

pub async fn update_user(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    user_id: Uuid,
    request: UpdateUserRequest,
) -> Result<UserView, UserError> {
    require_admin(current_user)?;
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(UserError::NotFound)?;
    if request.is_active == Some(false) && user_id == current_user.user_id {
        return Err(UserError::Conflict("不能停用当前登录账号".to_owned()));
    }
    let mut diff = serde_json::Map::new();
    let password_changed = request.password.is_some();
    if let Some(password) = request.password.as_deref() {
        validate_password(password)?;
        diff.insert("password".to_owned(), json!("***"));
    }
    if let Some(display_name) = request.display_name.as_deref() {
        let display_name = normalize_display_name(display_name)?;
        diff.insert("display_name".to_owned(), json!(display_name));
    }
    if let Some(is_active) = request.is_active {
        diff.insert("is_active".to_owned(), json!(is_active));
    }
    let departments = match request.department_ids.as_deref() {
        Some(department_ids) => {
            let departments = load_active_departments(db, department_ids).await?;
            diff.insert(
                "departments".to_owned(),
                json!(departments.iter().map(|d| d.name.clone()).collect::<Vec<_>>()),
            );
            Some(departments)
        }
        None => None,
    };

    let now = Utc::now();
    let txn = db.begin().await?;
    let mut active: users::ActiveModel = user.clone().into();
    active.updated_at = Set(now);
    if let Some(display_name) = request.display_name {
        active.display_name = Set(normalize_display_name(&display_name)?);
    }
    if let Some(is_active) = request.is_active {
        active.is_active = Set(is_active);
    }
    if let Some(password) = request.password {
        let password_hash = hash_password(&password)
            .map_err(|_| UserError::InvalidInput("密码处理失败".to_owned()))?;
        active.password_hash = Set(password_hash);
    }
    active.update(&txn).await.map_err(map_unique_account)?;

    if password_changed {
        let tokens = jwt_refresh_tokens::Entity::find()
            .filter(jwt_refresh_tokens::Column::UserId.eq(user_id))
            .filter(jwt_refresh_tokens::Column::RevokedAt.is_null())
            .all(&txn)
            .await?;
        for token in tokens {
            let mut token_active: jwt_refresh_tokens::ActiveModel = token.into();
            token_active.revoked_at = Set(Some(now));
            token_active.update(&txn).await?;
        }
    }

    if let Some(departments) = departments {
        replace_memberships(&txn, user_id, departments, now).await?;
    }

    write_user_log(
        &txn,
        current_user.user_id,
        user_id,
        "update",
        format!("更新用户 {}", user.account),
        serde_json::Value::Object(diff),
    )
    .await?;
    txn.commit().await?;

    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(UserError::NotFound)?;
    Ok(attach_departments(db, vec![user])
        .await?
        .pop()
        .expect("single user view"))
}

async fn replace_memberships(
    txn: &sea_orm::DatabaseTransaction,
    user_id: Uuid,
    departments: Vec<departments::Model>,
    now: DateTime<Utc>,
) -> Result<(), UserError> {
    let current = user_departments::Entity::find()
        .filter(user_departments::Column::UserId.eq(user_id))
        .filter(user_departments::Column::RevokedAt.is_null())
        .all(txn)
        .await?;
    for membership in current {
        let mut active: user_departments::ActiveModel = membership.into();
        active.revoked_at = Set(Some(now));
        active.update(txn).await?;
    }
    for department in departments {
        user_departments::ActiveModel {
            user_id: Set(user_id),
            department_id: Set(department.id),
            joined_at: Set(now),
            revoked_at: Set(None),
        }
        .insert(txn)
        .await?;
    }
    Ok(())
}

pub async fn import_users(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    rows: Vec<(u64, ImportRow)>,
) -> Result<ImportReport, UserError> {
    require_admin(current_user)?;
    let department_index = load_department_index(db).await?;
    let mut report = ImportReport {
        total: rows.len() as u64,
        succeeded: 0,
        failed: 0,
        rows: vec![],
    };
    for (row_number, row) in rows {
        let account = row.account.trim().to_owned();
        let result = import_one(db, current_user, &department_index, row).await;
        let (success, message) = match result {
            Ok(()) => (true, "创建成功".to_owned()),
            Err(message) => (false, message),
        };
        if success {
            report.succeeded += 1;
        } else {
            report.failed += 1;
        }
        report.rows.push(ImportRowResult {
            row_number,
            account,
            success,
            message,
        });
    }
    Ok(report)
}

async fn import_one(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    department_index: &HashMap<String, departments::Model>,
    row: ImportRow,
) -> Result<(), String> {
    let mut department_ids = Vec::new();
    for name in row.department_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        match department_index.get(name) {
            Some(department) => department_ids.push(department.id),
            None => return Err(format!("部门不存在：{name}")),
        }
    }
    let result = create_user(
        db,
        Some(current_user),
        NewUser {
            account: row.account,
            password: row.password,
            display_name: row.display_name,
            system_role: SystemRole::User,
            department_ids,
        },
    )
    .await;
    match result {
        Ok(_) => Ok(()),
        Err(UserError::Conflict(message)) | Err(UserError::InvalidInput(message)) => Err(message),
        Err(_) => Err("服务暂时不可用，请稍后重试".to_owned()),
    }
}

/// 生成 Excel 导入模板(表头 + 示例行 + 填写说明)。
pub fn build_import_template() -> Result<Vec<u8>, UserError> {
    use rust_xlsxwriter::{Format, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("员工导入").map_err(excel_error)?;
    let header_format = Format::new().set_bold();
    let headers = [
        "账号（必填）",
        "姓名（必填）",
        "部门（多选用 / 分隔，可留空）",
        "初始密码（必填，8-128 位）",
    ];
    for (column, header) in headers.iter().enumerate() {
        worksheet
            .write_with_format(0, column as u16, *header, &header_format)
            .map_err(excel_error)?;
    }
    let example = ["zhang.san", "张三", "研发部/平台组", "Projecty@2026"];
    for (column, value) in example.iter().enumerate() {
        worksheet
            .write(1, column as u16, *value)
            .map_err(excel_error)?;
    }
    let widths = [18.0, 16.0, 30.0, 24.0];
    for (column, width) in widths.iter().enumerate() {
        worksheet
            .set_column_width(column as u16, *width)
            .map_err(excel_error)?;
    }

    let notes = workbook.add_worksheet();
    notes.set_name("填写说明").map_err(excel_error)?;
    let note_lines = [
        "1. 第一行为表头，请勿修改列名与顺序；从第二行开始每行一个员工。",
        "2. 账号：登录用唯一标识，2-64 个字符；已存在的账号该行会失败，其余行不受影响。",
        "3. 部门：填写系统中已存在的部门名称，多个部门用 / 分隔；留空表示暂不归属部门。",
        "4. 初始密码：8-128 个字符，员工登录后可在个人设置中修改。",
        "5. 部门不匹配的行会标记失败并说明原因，不会自动创建新部门。",
    ];
    for (row, line) in note_lines.iter().enumerate() {
        notes
            .write(row as u32, 0, *line)
            .map_err(excel_error)?;
    }
    notes
        .set_column_width(0, 80.0)
        .map_err(excel_error)?;

    workbook
        .save_to_buffer()
        .map_err(|error| UserError::Excel(format!("模板生成失败：{error}")))
}

fn excel_error(error: rust_xlsxwriter::XlsxError) -> UserError {
    UserError::Excel(format!("模板生成失败：{error}"))
}

/// 解析导入的 Excel，返回 (行号, 行数据)；行号从 2 开始(第一行为表头)。
pub fn parse_import_workbook(bytes: &[u8]) -> Result<Vec<(u64, ImportRow)>, UserError> {
    use calamine::{open_workbook_auto_from_rs, Reader};
    use std::io::Cursor;

    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|error| UserError::Excel(format!("文件无法解析：{error}")))?;
    let range = match workbook.worksheet_range_at(0) {
        Some(result) => result.map_err(|error| UserError::Excel(format!("工作表读取失败：{error}")))?,
        None => return Err(UserError::Excel("Excel 中没有工作表".to_owned())),
    };
    let mut rows = range.rows();
    let header = rows.next().ok_or(UserError::Excel("Excel 内容为空".to_owned()))?;
    let column_of = |keyword: &str| -> Option<usize> {
        header
            .iter()
            .position(|cell| cell_string(cell).contains(keyword))
    };
    let account_column = column_of("账号")
        .ok_or_else(|| UserError::InvalidInput("模板缺少「账号」列".to_owned()))?;
    let name_column = column_of("姓名")
        .ok_or_else(|| UserError::InvalidInput("模板缺少「姓名」列".to_owned()))?;
    let department_column = column_of("部门");
    let password_column = column_of("密码")
        .ok_or_else(|| UserError::InvalidInput("模板缺少「初始密码」列".to_owned()))?;

    let mut parsed = Vec::new();
    for (index, row) in rows.enumerate() {
        let row_number = (index + 2) as u64;
        let cell = |column: usize| row.get(column).map(cell_string).unwrap_or_default();
        let account = cell(account_column);
        let display_name = cell(name_column);
        let password = cell(password_column);
        let department_names = department_column
            .map(|column| {
                cell(column)
                    .split('/')
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if account.trim().is_empty()
            && display_name.trim().is_empty()
            && password.trim().is_empty()
        {
            continue;
        }
        parsed.push((
            row_number,
            ImportRow {
                account,
                display_name,
                department_names,
                password,
            },
        ));
    }
    Ok(parsed)
}

fn cell_string(cell: &calamine::Data) -> String {
    use calamine::Data;
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_owned(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        other => other.to_string().trim().to_owned(),
    }
}
