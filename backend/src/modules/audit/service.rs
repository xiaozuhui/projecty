//! 操作日志查询和不脱敏 CSV 导出服务。

use chrono::{DateTime, Utc};
use projecty_entity::{operation_logs, project_members, projects, tasks};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{http::extractors::CurrentUser, modules::tasks::service::user_can_read_project};

const EXPORT_BATCH_SIZE: u64 = 5_000;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("日志不存在")]
    NotFound,
    #[error("没有查看当前项目日志的权限")]
    Forbidden,
    #[error("只有超级管理员可以导出全局操作日志")]
    SuperAdminRequired,
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

impl AuditQuery {
    fn normalized(&self) -> (u64, u64) {
        (
            self.page.unwrap_or(1).max(1),
            self.page_size.unwrap_or(50).clamp(1, 100),
        )
    }
}

#[derive(Debug, Serialize)]
pub struct OperationLogView {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub module: String,
    pub action: String,
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub summary: String,
    pub diff: Option<Value>,
    pub snapshot: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuditListResponse {
    pub items: Vec<OperationLogView>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
}

pub async fn project_logs(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    query: &AuditQuery,
) -> Result<AuditListResponse, AuditError> {
    let project = find_project(db, project_key).await?;
    require_project_read(db, current_user, project.id).await?;
    let (page, page_size) = query.normalized();
    let mut rows = operation_logs::Entity::find()
        .filter(operation_logs::Column::ProjectId.eq(project.id))
        .order_by_desc(operation_logs::Column::CreatedAt)
        .order_by_desc(operation_logs::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1)
        .all(db)
        .await?;
    let has_more = rows.len() > page_size as usize;
    rows.truncate(page_size as usize);
    Ok(AuditListResponse {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        page_size,
        has_more,
    })
}

pub async fn task_logs(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    query: &AuditQuery,
) -> Result<AuditListResponse, AuditError> {
    let task = tasks::Entity::find()
        .filter(tasks::Column::TaskKey.eq(task_key))
        .one(db)
        .await?
        .ok_or(AuditError::NotFound)?;
    require_project_read(db, current_user, task.project_id).await?;
    let (page, page_size) = query.normalized();
    let mut rows = operation_logs::Entity::find()
        .filter(operation_logs::Column::TaskId.eq(task.id))
        .order_by_desc(operation_logs::Column::CreatedAt)
        .order_by_desc(operation_logs::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1)
        .all(db)
        .await?;
    let has_more = rows.len() > page_size as usize;
    rows.truncate(page_size as usize);
    Ok(AuditListResponse {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        page_size,
        has_more,
    })
}

pub async fn export_project_logs(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<String, AuditError> {
    let project = find_project(db, project_key).await?;
    require_project_export(db, current_user, project.id).await?;
    let (csv, exported_rows) = export_csv(
        operation_logs::Entity::find()
            .filter(operation_logs::Column::ProjectId.eq(project.id))
            .order_by_asc(operation_logs::Column::CreatedAt)
            .order_by_asc(operation_logs::Column::Id),
        db,
    )
    .await?;
    write_export_log(
        db,
        current_user.user_id,
        Some(project.id),
        None,
        "project_logs_export",
        format!("导出项目 {} 操作日志", project.project_key),
        exported_rows,
    )
    .await?;
    Ok(csv)
}

pub async fn export_task_logs(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<String, AuditError> {
    let task = tasks::Entity::find()
        .filter(tasks::Column::TaskKey.eq(task_key))
        .one(db)
        .await?
        .ok_or(AuditError::NotFound)?;
    require_project_export(db, current_user, task.project_id).await?;
    let (csv, exported_rows) = export_csv(
        operation_logs::Entity::find()
            .filter(operation_logs::Column::TaskId.eq(task.id))
            .order_by_asc(operation_logs::Column::CreatedAt)
            .order_by_asc(operation_logs::Column::Id),
        db,
    )
    .await?;
    write_export_log(
        db,
        current_user.user_id,
        Some(task.project_id),
        Some(task.id),
        "task_logs_export",
        format!("导出任务 {} 操作日志", task.task_key),
        exported_rows,
    )
    .await?;
    Ok(csv)
}

pub async fn export_admin_logs(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
) -> Result<String, AuditError> {
    if !current_user.system_role.is_super_admin() {
        return Err(AuditError::SuperAdminRequired);
    }
    let (csv, exported_rows) = export_csv(
        operation_logs::Entity::find()
            .order_by_asc(operation_logs::Column::CreatedAt)
            .order_by_asc(operation_logs::Column::Id),
        db,
    )
    .await?;
    write_export_log(
        db,
        current_user.user_id,
        None,
        None,
        "admin_logs_export",
        "导出全局操作日志".to_owned(),
        exported_rows,
    )
    .await?;
    Ok(csv)
}

async fn require_project_export(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), AuditError> {
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
        Err(AuditError::Forbidden)
    }
}

async fn require_project_read(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), AuditError> {
    if user_can_read_project(db, current_user, project_id).await? {
        Ok(())
    } else {
        Err(AuditError::Forbidden)
    }
}

async fn find_project(
    db: &DatabaseConnection,
    project_key: &str,
) -> Result<projects::Model, AuditError> {
    projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(project_key))
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(AuditError::NotFound)
}

async fn export_csv<S>(
    query: sea_orm::Select<S>,
    db: &DatabaseConnection,
) -> Result<(String, usize), AuditError>
where
    S: sea_orm::EntityTrait<Model = operation_logs::Model>,
{
    let mut csv = String::from("id,actor_user_id,module,action,project_id,task_id,target_type,target_id,summary,diff,snapshot,created_at\n");
    let mut offset = 0;
    let mut exported_rows = 0;
    loop {
        let rows = query
            .clone()
            .offset(offset)
            .limit(EXPORT_BATCH_SIZE)
            .all(db)
            .await?;
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for row in rows {
            let view: OperationLogView = row.into();
            csv.push_str(
                &[
                    view.id.to_string(),
                    view.actor_user_id.to_string(),
                    view.module,
                    view.action,
                    view.project_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    view.task_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    view.target_type,
                    view.target_id
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    view.summary,
                    view.diff.map(|value| value.to_string()).unwrap_or_default(),
                    view.snapshot
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    view.created_at.to_rfc3339(),
                ]
                .into_iter()
                .map(csv_escape)
                .collect::<Vec<_>>()
                .join(","),
            );
            csv.push('\n');
        }
        exported_rows += row_count;
        if row_count < EXPORT_BATCH_SIZE as usize {
            break;
        }
        offset += EXPORT_BATCH_SIZE;
    }
    Ok((csv, exported_rows))
}

async fn write_export_log<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor_user_id: Uuid,
    project_id: Option<Uuid>,
    task_id: Option<Uuid>,
    action: &str,
    summary: String,
    exported_rows: usize,
) -> Result<(), AuditError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor_user_id),
        module: Set("audit".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(project_id),
        task_id: Set(task_id),
        target_type: Set("export".to_owned()),
        target_id: Set(task_id.or(project_id)),
        summary: Set(summary),
        diff: Set(Some(
            json!({ "exported_rows": exported_rows, "format": "csv", "masked": false }),
        )),
        snapshot: Set(None),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await?;
    Ok(())
}

fn csv_escape(value: String) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

impl From<operation_logs::Model> for OperationLogView {
    fn from(value: operation_logs::Model) -> Self {
        Self {
            id: value.id,
            actor_user_id: value.actor_user_id,
            module: value.module,
            action: value.action,
            project_id: value.project_id,
            task_id: value.task_id,
            target_type: value.target_type,
            target_id: value.target_id,
            summary: value.summary,
            diff: value.diff,
            snapshot: value.snapshot,
            created_at: value.created_at,
        }
    }
}
