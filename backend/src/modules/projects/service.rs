//! 项目、多负责人、显式成员、部门授权和归档/恢复/逻辑删除的应用服务。

use chrono::{DateTime, Utc};
use projecty_entity::{
    operation_logs, project_department_grants, project_members, project_statuses, projects, users,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::http::extractors::CurrentUser;

const VALID_PROJECT_ROLES: [&str; 3] = ["manager", "member", "viewer"];

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("项目不存在")]
    NotFound,
    #[error("没有项目管理权限")]
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
pub struct ListProjectsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

impl ListProjectsQuery {
    fn normalized(&self) -> (u64, u64) {
        (
            self.page.unwrap_or(1).max(1),
            self.page_size.unwrap_or(30).clamp(1, 100),
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub project_key: String,
    pub name: String,
    pub description: Option<String>,
    pub primary_department_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub primary_department_id: Option<Option<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteProjectRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectView {
    pub id: Uuid,
    pub project_key: String,
    pub name: String,
    pub description: Option<String>,
    pub primary_department_id: Option<Uuid>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task_number_seed: i64,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub items: Vec<ProjectView>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectMemberView {
    pub user_id: Uuid,
    pub account: String,
    pub display_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMembersResponse {
    pub items: Vec<ProjectMemberView>,
}

#[derive(Debug, Serialize)]
pub struct ProjectDepartmentGrantView {
    pub department_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ProjectDepartmentGrantsResponse {
    pub items: Vec<ProjectDepartmentGrantView>,
}

#[derive(Debug, FromQueryResult)]
struct ProjectIdRow {
    id: Uuid,
}

#[derive(Debug, FromQueryResult)]
struct MemberRow {
    user_id: Uuid,
    account: String,
    display_name: String,
    role: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromQueryResult)]
struct DepartmentGrantRow {
    department_id: Uuid,
    role: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromQueryResult)]
struct DepartmentIdRow {
    id: Uuid,
}

pub async fn list(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &ListProjectsQuery,
) -> Result<ProjectListResponse, ProjectError> {
    let (page, page_size) = query.normalized();
    let mut statement = projects::Entity::find()
        .filter(projects::Column::DeletedAt.is_null())
        .order_by_desc(projects::Column::UpdatedAt)
        .order_by_desc(projects::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1);

    if !current_user.system_role.is_super_admin() {
        let ids = visible_project_ids(db, current_user.user_id).await?;
        if ids.is_empty() {
            return Ok(ProjectListResponse {
                items: vec![],
                page,
                page_size,
                has_more: false,
            });
        }
        statement = statement.filter(projects::Column::Id.is_in(ids));
    }

    let mut models = statement.all(db).await?;
    let has_more = models.len() > page_size as usize;
    models.truncate(page_size as usize);
    Ok(ProjectListResponse {
        items: models.into_iter().map(Into::into).collect(),
        page,
        page_size,
        has_more,
    })
}

pub async fn create(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    request: CreateProjectRequest,
) -> Result<ProjectView, ProjectError> {
    let project_key = normalize_project_key(request.project_key)?;
    let name = required_name(request.name)?;
    if let Some(department_id) = request.primary_department_id {
        ensure_department(db, department_id).await?;
    }
    let now = Utc::now();
    let txn = db.begin().await?;
    let project = projects::ActiveModel {
        id: Set(Uuid::now_v7()),
        project_key: Set(project_key),
        name: Set(name),
        description: Set(request.description),
        primary_department_id: Set(request.primary_department_id),
        archived_at: Set(None),
        archived_by: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
        task_number_seed: Set(0),
    }
    .insert(&txn)
    .await
    .map_err(map_unique_project_key)?;

    for (sort_order, (name, category, is_default)) in [
        ("待处理", "todo", true),
        ("进行中", "active", false),
        ("评审中", "review", false),
        ("已完成", "done", false),
    ]
    .into_iter()
    .enumerate()
    {
        project_statuses::ActiveModel {
            id: Set(Uuid::now_v7()),
            project_id: Set(project.id),
            name: Set(name.to_owned()),
            category: Set(category.to_owned()),
            sort_order: Set(sort_order as i32),
            is_default: Set(is_default),
            created_at: Set(now),
        }
        .insert(&txn)
        .await?;
    }

    project_members::ActiveModel {
        project_id: Set(project.id),
        user_id: Set(current_user.user_id),
        role: Set("manager".to_owned()),
        created_at: Set(now),
        revoked_at: Set(None),
    }
    .insert(&txn)
    .await?;

    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "create",
        format!("创建项目 {}", project.project_key),
        json!({ "project_key": project.project_key, "manager_user_id": current_user.user_id }),
        Some(serde_json::to_value(&project)?),
    )
    .await?;
    txn.commit().await?;
    Ok(project.into())
}

pub async fn detail(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectView, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_visible(db, current_user, project.id).await?;
    Ok(project.into())
}

pub async fn update(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    request: UpdateProjectRequest,
) -> Result<ProjectView, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    if let Some(primary_department_id) = request.primary_department_id.flatten() {
        ensure_department(db, primary_department_id).await?;
    }
    let old = serde_json::to_value(&project)?;
    let mut active: projects::ActiveModel = project.clone().into();
    let mut diff = serde_json::Map::new();
    if let Some(name) = request.name {
        let name = required_name(name)?;
        active.name = Set(name.clone());
        diff.insert("name".to_owned(), json!(name));
    }
    if let Some(description) = request.description {
        let description = description.trim().to_owned();
        active.description = Set((!description.is_empty()).then_some(description.clone()));
        diff.insert("description".to_owned(), json!(description));
    }
    if let Some(primary_department_id) = request.primary_department_id {
        active.primary_department_id = Set(primary_department_id);
        diff.insert(
            "primary_department_id".to_owned(),
            json!(primary_department_id),
        );
    }
    if diff.is_empty() {
        return Ok(project.into());
    }
    let now = Utc::now();
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &updated,
        "update",
        format!("更新项目 {}", updated.project_key),
        serde_json::Value::Object(diff),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(updated.into())
}

pub async fn archive(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectView, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    if project.archived_at.is_some() {
        return Ok(project.into());
    }
    let old = serde_json::to_value(&project)?;
    let now = Utc::now();
    let mut active: projects::ActiveModel = project.into();
    active.archived_at = Set(Some(now));
    active.archived_by = Set(Some(current_user.user_id));
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let archived = active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &archived,
        "archive",
        format!("归档项目 {}", archived.project_key),
        json!({ "archived_at": now, "archived_by": current_user.user_id }),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(archived.into())
}

pub async fn restore(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectView, ProjectError> {
    let project = find_project_any(db, project_key).await?;
    require_manager_for_project(db, current_user, project.id).await?;
    if project.archived_at.is_none() && project.deleted_at.is_none() {
        return Ok(project.into());
    }
    let old = serde_json::to_value(&project)?;
    let now = Utc::now();
    let mut active: projects::ActiveModel = project.into();
    active.archived_at = Set(None);
    active.archived_by = Set(None);
    active.deleted_at = Set(None);
    active.deleted_by = Set(None);
    active.delete_reason = Set(None);
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let restored = active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &restored,
        "restore",
        format!("恢复项目 {}", restored.project_key),
        json!({ "restored": true }),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(restored.into())
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    request: DeleteProjectRequest,
) -> Result<(), ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let now = Utc::now();
    let old = serde_json::to_value(&project)?;
    let mut active: projects::ActiveModel = project.into();
    active.deleted_at = Set(Some(now));
    active.deleted_by = Set(Some(current_user.user_id));
    active.delete_reason = Set(request.reason.clone());
    active.updated_at = Set(now);
    let txn = db.begin().await?;
    let deleted = active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &deleted,
        "logical_delete",
        format!("逻辑删除项目 {}", deleted.project_key),
        json!({ "deleted_at": now, "deleted_by": current_user.user_id, "reason": request.reason }),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

pub async fn list_members(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectMembersResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_visible(db, current_user, project.id).await?;
    let rows = MemberRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT pm.user_id, u.account, u.display_name, pm.role, pm.created_at, pm.revoked_at FROM project_members pm JOIN users u ON u.id = pm.user_id WHERE pm.project_id = $1 ORDER BY pm.revoked_at NULLS FIRST, pm.created_at ASC, pm.user_id ASC",
        [project.id.into()],
    ))
    .all(db)
    .await?;
    Ok(ProjectMembersResponse {
        items: rows
            .into_iter()
            .map(|row| ProjectMemberView {
                user_id: row.user_id,
                account: row.account,
                display_name: row.display_name,
                role: row.role,
                created_at: row.created_at,
                revoked_at: row.revoked_at,
            })
            .collect(),
    })
}

pub async fn add_member(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    request: AddMemberRequest,
) -> Result<ProjectMembersResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let role = normalize_role(request.role)?;
    let user = users::Entity::find_by_id(request.user_id)
        .filter(users::Column::DeletedAt.is_null())
        .filter(users::Column::IsActive.eq(true))
        .one(db)
        .await?
        .ok_or_else(|| ProjectError::InvalidInput("目标用户不存在或已停用".to_owned()))?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let existing = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project.id))
        .filter(project_members::Column::UserId.eq(request.user_id))
        .one(&txn)
        .await?;
    if let Some(existing) = existing {
        let mut active: project_members::ActiveModel = existing.into();
        active.role = Set(role.clone());
        active.revoked_at = Set(None);
        active.update(&txn).await?;
    } else {
        project_members::ActiveModel {
            project_id: Set(project.id),
            user_id: Set(request.user_id),
            role: Set(role.clone()),
            created_at: Set(now),
            revoked_at: Set(None),
        }
        .insert(&txn)
        .await?;
    }
    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "member_add",
        format!("添加项目成员 {}", user.account),
        json!({ "user_id": request.user_id, "role": role }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_members(db, current_user, project_key).await
}

pub async fn update_member(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    user_id: Uuid,
    request: UpdateMemberRequest,
) -> Result<ProjectMembersResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let role = normalize_role(request.role)?;
    let member = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project.id))
        .filter(project_members::Column::UserId.eq(user_id))
        .filter(project_members::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(ProjectError::NotFound)?;
    if member.role == "manager" && role != "manager" {
        ensure_another_manager(db, project.id, user_id).await?;
    }
    let old_role = member.role.clone();
    let txn = db.begin().await?;
    let mut active: project_members::ActiveModel = member.into();
    active.role = Set(role.clone());
    active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "member_update",
        format!("变更项目成员角色 {} -> {}", old_role, role),
        json!({ "user_id": user_id, "from_role": old_role, "to_role": role }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_members(db, current_user, project_key).await
}

pub async fn revoke_member(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    user_id: Uuid,
) -> Result<ProjectMembersResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let member = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project.id))
        .filter(project_members::Column::UserId.eq(user_id))
        .filter(project_members::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(ProjectError::NotFound)?;
    if member.role == "manager" {
        ensure_another_manager(db, project.id, user_id).await?;
    }
    let now = Utc::now();
    let txn = db.begin().await?;
    let mut active: project_members::ActiveModel = member.into();
    active.revoked_at = Set(Some(now));
    active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "member_revoke",
        format!("撤销项目成员 {}", user_id),
        json!({ "user_id": user_id, "revoked_at": now }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_members(db, current_user, project_key).await
}

pub async fn list_department_grants(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectDepartmentGrantsResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_visible(db, current_user, project.id).await?;
    let rows = DepartmentGrantRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT department_id, role, created_at, revoked_at FROM project_department_grants WHERE project_id = $1 ORDER BY revoked_at NULLS FIRST, created_at ASC, department_id ASC",
        [project.id.into()],
    ))
    .all(db)
    .await?;
    Ok(ProjectDepartmentGrantsResponse {
        items: rows
            .into_iter()
            .map(|row| ProjectDepartmentGrantView {
                department_id: row.department_id,
                role: row.role,
                created_at: row.created_at,
                revoked_at: row.revoked_at,
            })
            .collect(),
    })
}

pub async fn grant_department(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    department_id: Uuid,
    role: String,
) -> Result<ProjectDepartmentGrantsResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let role = normalize_department_role(role)?;
    ensure_department(db, department_id).await?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let existing = project_department_grants::Entity::find()
        .filter(project_department_grants::Column::ProjectId.eq(project.id))
        .filter(project_department_grants::Column::DepartmentId.eq(department_id))
        .one(&txn)
        .await?;
    if let Some(existing) = existing {
        let mut active: project_department_grants::ActiveModel = existing.into();
        active.role = Set(role.clone());
        active.revoked_at = Set(None);
        active.update(&txn).await?;
    } else {
        project_department_grants::ActiveModel {
            project_id: Set(project.id),
            department_id: Set(department_id),
            role: Set(role.clone()),
            created_at: Set(now),
            revoked_at: Set(None),
        }
        .insert(&txn)
        .await?;
    }
    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "department_grant",
        format!("授权部门 {} 访问项目", department_id),
        json!({ "department_id": department_id, "role": role }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_department_grants(db, current_user, project_key).await
}

pub async fn revoke_department_grant(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    department_id: Uuid,
) -> Result<ProjectDepartmentGrantsResponse, ProjectError> {
    let project = find_project(db, project_key).await?;
    require_manager(db, current_user, project.id).await?;
    let grant = project_department_grants::Entity::find()
        .filter(project_department_grants::Column::ProjectId.eq(project.id))
        .filter(project_department_grants::Column::DepartmentId.eq(department_id))
        .filter(project_department_grants::Column::RevokedAt.is_null())
        .one(db)
        .await?
        .ok_or(ProjectError::NotFound)?;
    let now = Utc::now();
    let txn = db.begin().await?;
    let mut active: project_department_grants::ActiveModel = grant.into();
    active.revoked_at = Set(Some(now));
    active.update(&txn).await?;
    write_project_log(
        &txn,
        current_user.user_id,
        &project,
        "department_revoke",
        format!("撤销部门 {} 的项目授权", department_id),
        json!({ "department_id": department_id, "revoked_at": now }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_department_grants(db, current_user, project_key).await
}

async fn find_project(
    db: &DatabaseConnection,
    project_key: &str,
) -> Result<projects::Model, ProjectError> {
    projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(normalize_lookup_key(project_key)))
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(ProjectError::NotFound)
}

async fn find_project_any(
    db: &DatabaseConnection,
    project_key: &str,
) -> Result<projects::Model, ProjectError> {
    projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(normalize_lookup_key(project_key)))
        .one(db)
        .await?
        .ok_or(ProjectError::NotFound)
}

async fn require_visible(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), ProjectError> {
    if current_user.system_role.is_super_admin()
        || visible_project_ids(db, current_user.user_id)
            .await?
            .contains(&project_id)
    {
        Ok(())
    } else {
        Err(ProjectError::Forbidden)
    }
}

async fn require_manager(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), ProjectError> {
    if current_user.system_role.is_super_admin() {
        return Ok(());
    }
    let is_manager = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project_id))
        .filter(project_members::Column::UserId.eq(current_user.user_id))
        .filter(project_members::Column::Role.eq("manager"))
        .filter(project_members::Column::RevokedAt.is_null())
        .count(db)
        .await?
        > 0;
    if is_manager {
        Ok(())
    } else {
        Err(ProjectError::Forbidden)
    }
}

async fn require_manager_for_project(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), ProjectError> {
    require_manager(db, current_user, project_id).await
}

async fn ensure_another_manager(
    db: &DatabaseConnection,
    project_id: Uuid,
    excluded_user_id: Uuid,
) -> Result<(), ProjectError> {
    let count = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project_id))
        .filter(project_members::Column::Role.eq("manager"))
        .filter(project_members::Column::RevokedAt.is_null())
        .filter(project_members::Column::UserId.ne(excluded_user_id))
        .count(db)
        .await?;
    if count == 0 {
        return Err(ProjectError::Conflict(
            "项目至少需要保留一名负责人".to_owned(),
        ));
    }
    Ok(())
}

async fn ensure_department(
    db: &DatabaseConnection,
    department_id: Uuid,
) -> Result<(), ProjectError> {
    let exists = DepartmentIdRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id FROM departments WHERE id = $1 AND deleted_at IS NULL",
        [department_id.into()],
    ))
    .one(db)
    .await?
    .is_some();
    if exists {
        Ok(())
    } else {
        Err(ProjectError::InvalidInput("部门不存在或已删除".to_owned()))
    }
}

async fn visible_project_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, ProjectError> {
    let rows = ProjectIdRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT DISTINCT project_id AS id FROM project_members WHERE user_id = $1 AND revoked_at IS NULL UNION SELECT DISTINCT pdg.project_id AS id FROM project_department_grants pdg JOIN department_closure dc ON dc.ancestor_id = pdg.department_id JOIN user_departments ud ON ud.department_id = dc.descendant_id WHERE ud.user_id = $1 AND ud.revoked_at IS NULL AND pdg.revoked_at IS NULL",
        [user_id.into()],
    ))
    .all(db)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn write_project_log<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor_user_id: Uuid,
    project: &projects::Model,
    action: &str,
    summary: String,
    diff: serde_json::Value,
    snapshot: Option<serde_json::Value>,
) -> Result<(), ProjectError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor_user_id),
        module: Set("project".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(Some(project.id)),
        task_id: Set(None),
        target_type: Set("project".to_owned()),
        target_id: Set(Some(project.id)),
        summary: Set(summary),
        diff: Set(Some(diff)),
        snapshot: Set(snapshot),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await?;
    Ok(())
}

fn normalize_project_key(value: String) -> Result<String, ProjectError> {
    let key = value.trim().to_ascii_uppercase();
    if key.is_empty() || key.len() > 32 {
        return Err(ProjectError::InvalidInput(
            "项目 Key 不能为空且不能超过 32 个字符".to_owned(),
        ));
    }
    if !key
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(ProjectError::InvalidInput(
            "项目 Key 只允许大写字母、数字和短横线".to_owned(),
        ));
    }
    Ok(key)
}

fn normalize_lookup_key(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn required_name(value: String) -> Result<String, ProjectError> {
    let name = value.trim();
    if name.is_empty() {
        return Err(ProjectError::InvalidInput("项目名称不能为空".to_owned()));
    }
    if name.chars().count() > 160 {
        return Err(ProjectError::InvalidInput(
            "项目名称不能超过 160 个字符".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

fn normalize_department_role(value: String) -> Result<String, ProjectError> {
    let role = value.trim().to_ascii_lowercase();
    if matches!(role.as_str(), "member" | "viewer") {
        Ok(role)
    } else {
        Err(ProjectError::InvalidInput(
            "部门授权 role 只能是 member/viewer；项目负责人请使用显式成员".to_owned(),
        ))
    }
}

fn normalize_role(value: String) -> Result<String, ProjectError> {
    let role = value.trim().to_ascii_lowercase();
    if VALID_PROJECT_ROLES.contains(&role.as_str()) {
        Ok(role)
    } else {
        Err(ProjectError::InvalidInput(
            "role 必须是 manager/member/viewer".to_owned(),
        ))
    }
}

fn map_unique_project_key(error: sea_orm::DbErr) -> ProjectError {
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("project_key") || text.contains("projects_project_key_key") {
        ProjectError::Conflict("项目 Key 已存在".to_owned())
    } else {
        ProjectError::Database(error)
    }
}

impl From<projects::Model> for ProjectView {
    fn from(value: projects::Model) -> Self {
        Self {
            id: value.id,
            project_key: value.project_key,
            name: value.name,
            description: value.description,
            primary_department_id: value.primary_department_id,
            archived_at: value.archived_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
            task_number_seed: value.task_number_seed,
        }
    }
}
