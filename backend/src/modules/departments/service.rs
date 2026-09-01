//! 部门树、闭包表、用户多部门关系和父部门可见性的应用服务。

use chrono::{DateTime, Utc};
use projecty_entity::{departments, operation_logs, projects, user_departments, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::domain::permissions::SystemRole;
use crate::http::extractors::CurrentUser;
use crate::modules::projects::service::ProjectView;

#[derive(Debug, thiserror::Error)]
pub enum DepartmentError {
    #[error("部门不存在")]
    NotFound,
    #[error("只有超级管理员可以管理部门")]
    Forbidden,
    #[error("请求参数无效：{0}")]
    InvalidInput(String),
    #[error("当前操作不允许：{0}")]
    Conflict(String),
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("数据序列化错误：{0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
pub struct ListDepartmentsQuery {
    pub include_deleted: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDepartmentRequest {
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDepartmentRequest {
    pub name: Option<String>,
    pub code: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteDepartmentRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentView {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentListResponse {
    pub items: Vec<DepartmentView>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentProjectsResponse {
    pub department_id: Uuid,
    pub items: Vec<ProjectView>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentMemberView {
    pub user_id: Uuid,
    pub account: String,
    pub display_name: String,
    pub system_role: String,
    pub is_active: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentMembersResponse {
    pub department_id: Uuid,
    pub items: Vec<DepartmentMemberView>,
}

#[derive(Debug, FromQueryResult)]
struct DepartmentIdRow {
    id: Uuid,
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &ListDepartmentsQuery,
) -> Result<DepartmentListResponse, DepartmentError> {
    let include_deleted = query.include_deleted.unwrap_or(false);
    let mut statement = departments::Entity::find()
        .order_by_asc(departments::Column::SortOrder)
        .order_by_asc(departments::Column::Name)
        .order_by_asc(departments::Column::Id);
    if !include_deleted {
        statement = statement.filter(departments::Column::DeletedAt.is_null());
    }
    if !current_user.system_role.is_super_admin() {
        let ids = visible_department_ids(db, current_user.user_id, include_deleted).await?;
        if ids.is_empty() {
            return Ok(DepartmentListResponse { items: vec![] });
        }
        statement = statement.filter(departments::Column::Id.is_in(ids));
    }
    Ok(DepartmentListResponse {
        items: statement
            .all(db)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    })
}

pub async fn create(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    request: CreateDepartmentRequest,
) -> Result<DepartmentView, DepartmentError> {
    require_admin(current_user)?;
    let name = required_name(request.name)?;
    let code = normalize_code(request.code)?;
    if let Some(parent_id) = request.parent_id {
        find_active_department(db, parent_id).await?;
    }
    let now = Utc::now();
    let txn = db.begin().await?;
    let department = departments::ActiveModel {
        id: Set(Uuid::now_v7()),
        parent_id: Set(request.parent_id),
        name: Set(name),
        code: Set(code),
        sort_order: Set(request.sort_order.unwrap_or(0)),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
    }
    .insert(&txn)
    .await
    .map_err(map_unique_code)?;
    let id = department.id;
    if let Some(parent_id) = department.parent_id {
        let result = DepartmentClosureRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT ancestor_id, descendant_id, depth FROM department_closure WHERE descendant_id = $1",
            [parent_id.into()],
        ))
        .all(&txn)
        .await?;
        for row in result {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO department_closure (ancestor_id, descendant_id, depth) VALUES ($1, $2, $3)",
                [row.ancestor_id.into(), id.into(), (row.depth + 1).into()],
            ))
            .await?;
        }
    } else {
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO department_closure (ancestor_id, descendant_id, depth) VALUES ($1, $2, 0)",
            [id.into(), id.into()],
        ))
        .await?;
    }
    write_department_log(
        &txn,
        current_user.user_id,
        &department,
        "create",
        format!("创建部门 {}", department.name),
        json!({ "code": department.code, "parent_id": department.parent_id }),
        Some(serde_json::to_value(&department)?),
    )
    .await?;
    txn.commit().await?;
    Ok(department.into())
}

pub async fn update(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    department_id: Uuid,
    request: UpdateDepartmentRequest,
) -> Result<DepartmentView, DepartmentError> {
    require_admin(current_user)?;
    let department = find_active_department(db, department_id).await?;
    let old = serde_json::to_value(&department)?;
    let mut active: departments::ActiveModel = department.clone().into();
    let mut diff = serde_json::Map::new();
    if let Some(name) = request.name {
        let name = required_name(name)?;
        active.name = Set(name.clone());
        diff.insert("name".to_owned(), json!(name));
    }
    if let Some(code) = request.code {
        let code = normalize_code(code)?;
        active.code = Set(code.clone());
        diff.insert("code".to_owned(), json!(code));
    }
    if let Some(sort_order) = request.sort_order {
        active.sort_order = Set(sort_order);
        diff.insert("sort_order".to_owned(), json!(sort_order));
    }
    if diff.is_empty() {
        return Ok(department.into());
    }
    let now = Utc::now();
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let updated = active.update(&txn).await.map_err(map_unique_code)?;
    write_department_log(
        &txn,
        current_user.user_id,
        &updated,
        "update",
        format!("更新部门 {}", updated.name),
        serde_json::Value::Object(diff),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(updated.into())
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    department_id: Uuid,
    request: DeleteDepartmentRequest,
) -> Result<(), DepartmentError> {
    require_admin(current_user)?;
    let department = find_active_department(db, department_id).await?;
    let child_count = departments::Entity::find()
        .filter(departments::Column::ParentId.eq(department_id))
        .filter(departments::Column::DeletedAt.is_null())
        .count(db)
        .await?;
    if child_count > 0 {
        return Err(DepartmentError::Conflict(
            "部门仍有未删除的子部门，不能删除".to_owned(),
        ));
    }
    let project_count = projects::Entity::find()
        .filter(projects::Column::PrimaryDepartmentId.eq(department_id))
        .filter(projects::Column::DeletedAt.is_null())
        .count(db)
        .await?;
    if project_count > 0 {
        return Err(DepartmentError::Conflict(
            "部门仍关联有效项目，不能删除".to_owned(),
        ));
    }
    let old = serde_json::to_value(&department)?;
    let now = Utc::now();
    let mut active: departments::ActiveModel = department.into();
    active.deleted_at = Set(Some(now));
    active.deleted_by = Set(Some(current_user.user_id));
    active.delete_reason = Set(request.reason.clone());
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let deleted = active.update(&txn).await?;
    write_department_log(
        &txn,
        current_user.user_id,
        &deleted,
        "logical_delete",
        format!("逻辑删除部门 {}", deleted.name),
        json!({ "deleted_at": now, "deleted_by": current_user.user_id, "reason": request.reason }),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

pub async fn projects(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    department_id: Uuid,
) -> Result<DepartmentProjectsResponse, DepartmentError> {
    find_active_department(db, department_id).await?;
    if !current_user.system_role.is_super_admin()
        && !visible_department_ids(db, current_user.user_id, false)
            .await?
            .contains(&department_id)
    {
        return Err(DepartmentError::Forbidden);
    }
    let department_ids = descendant_department_ids(db, department_id).await?;
    let items = projects::Entity::find()
        .filter(projects::Column::PrimaryDepartmentId.is_in(department_ids))
        .filter(projects::Column::DeletedAt.is_null())
        .order_by_desc(projects::Column::UpdatedAt)
        .order_by_desc(projects::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(DepartmentProjectsResponse {
        department_id,
        items,
    })
}

/// 直属成员列表:与 projects 端点同一套可见性规则,成员按加入时间倒序,
/// 用户被逻辑删除后自动从列表隐藏(关系仍在,便于恢复)。
pub async fn members(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    department_id: Uuid,
) -> Result<DepartmentMembersResponse, DepartmentError> {
    find_active_department(db, department_id).await?;
    if !current_user.system_role.is_super_admin()
        && !visible_department_ids(db, current_user.user_id, false)
            .await?
            .contains(&department_id)
    {
        return Err(DepartmentError::Forbidden);
    }
    let memberships = user_departments::Entity::find()
        .filter(user_departments::Column::DepartmentId.eq(department_id))
        .filter(user_departments::Column::RevokedAt.is_null())
        .order_by_desc(user_departments::Column::JoinedAt)
        .order_by_desc(user_departments::Column::UserId)
        .all(db)
        .await?;
    if memberships.is_empty() {
        return Ok(DepartmentMembersResponse {
            department_id,
            items: vec![],
        });
    }
    let user_ids: Vec<Uuid> = memberships
        .iter()
        .map(|membership| membership.user_id)
        .collect();
    let active_users = users::Entity::find()
        .filter(users::Column::Id.is_in(user_ids))
        .filter(users::Column::DeletedAt.is_null())
        .all(db)
        .await?;
    let items = memberships
        .iter()
        .filter_map(|membership| {
            active_users
                .iter()
                .find(|user| user.id == membership.user_id)
                .map(|user| DepartmentMemberView {
                    user_id: user.id,
                    account: user.account.clone(),
                    display_name: user.display_name.clone(),
                    system_role: user.system_role.clone(),
                    is_active: user.is_active,
                    joined_at: membership.joined_at,
                })
        })
        .collect();
    Ok(DepartmentMembersResponse {
        department_id,
        items,
    })
}

fn require_admin(current_user: &CurrentUser) -> Result<(), DepartmentError> {
    if current_user.system_role == SystemRole::SuperAdmin {
        Ok(())
    } else {
        Err(DepartmentError::Forbidden)
    }
}

async fn find_active_department(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<departments::Model, DepartmentError> {
    departments::Entity::find_by_id(id)
        .filter(departments::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(DepartmentError::NotFound)
}

async fn visible_department_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
    include_deleted: bool,
) -> Result<Vec<Uuid>, DepartmentError> {
    let deleted_filter = if include_deleted {
        ""
    } else {
        "AND d.deleted_at IS NULL"
    };
    let sql = format!(
        "SELECT DISTINCT d.id FROM departments d JOIN department_closure dc ON dc.descendant_id = d.id JOIN user_departments ud ON ud.department_id = dc.ancestor_id WHERE ud.user_id = $1 AND ud.revoked_at IS NULL {deleted_filter}"
    );
    Ok(
        DepartmentIdRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            &sql,
            [user_id.into()],
        ))
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect(),
    )
}

async fn descendant_department_ids(
    db: &DatabaseConnection,
    department_id: Uuid,
) -> Result<Vec<Uuid>, DepartmentError> {
    Ok(
        DepartmentIdRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT descendant_id AS id FROM department_closure WHERE ancestor_id = $1",
            [department_id.into()],
        ))
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect(),
    )
}

#[derive(Debug, FromQueryResult)]
struct DepartmentClosureRow {
    ancestor_id: Uuid,
    descendant_id: Uuid,
    depth: i32,
}

async fn write_department_log<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor_user_id: Uuid,
    department: &departments::Model,
    action: &str,
    summary: String,
    diff: serde_json::Value,
    snapshot: Option<serde_json::Value>,
) -> Result<(), DepartmentError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor_user_id),
        module: Set("department".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(None),
        task_id: Set(None),
        target_type: Set("department".to_owned()),
        target_id: Set(Some(department.id)),
        summary: Set(summary),
        diff: Set(Some(diff)),
        snapshot: Set(snapshot),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await?;
    Ok(())
}

fn required_name(value: String) -> Result<String, DepartmentError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 120 {
        return Err(DepartmentError::InvalidInput(
            "部门名称不能为空且不能超过 120 个字符".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn normalize_code(value: String) -> Result<String, DepartmentError> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(DepartmentError::InvalidInput(
            "部门编码不能为空，且只允许字母、数字、短横线和下划线".to_owned(),
        ));
    }
    Ok(value)
}

fn map_unique_code(error: sea_orm::DbErr) -> DepartmentError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("code") || text.contains("departments_code_key") {
        DepartmentError::Conflict("部门编码已存在".to_owned())
    } else {
        DepartmentError::Database(error)
    }
}

impl From<departments::Model> for DepartmentView {
    fn from(value: departments::Model) -> Self {
        Self {
            id: value.id,
            parent_id: value.parent_id,
            name: value.name,
            code: value.code,
            sort_order: value.sort_order,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        }
    }
}
