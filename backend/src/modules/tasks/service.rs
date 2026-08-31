//! 任务用例：分页查询、两层子任务、逻辑删除和操作日志事务。

use chrono::{DateTime, Utc};
use projecty_entity::{operation_logs, project_members, project_statuses, projects, tasks, users};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, FromQueryResult, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    application::{authz, task_rules},
    domain::{
        permissions::{EffectiveProjectRole, ProjectRole, SystemRole},
        tasks::NewTaskParent,
    },
    http::extractors::CurrentUser,
};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("任务不存在")]
    NotFound,
    #[error("没有项目任务操作权限")]
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
pub struct ListTasksQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub status_id: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
}

impl ListTasksQuery {
    fn normalized(&self) -> (u64, u64) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self.page_size.unwrap_or(30).clamp(1, 100);
        (page, page_size)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub status_id: Option<Uuid>,
    pub assignee_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub parent_task_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
    // 双层 Option:字段缺省=不修改,显式 null=清空,配合 double_option 反序列化。
    #[serde(default, deserialize_with = "double_option")]
    pub assignee_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    pub due_at: Option<Option<DateTime<Utc>>>,
}

/// 把 JSON null 与字段缺失区分开:缺失走 serde default 得 None(不改),null 得 Some(None)(清空)。
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Deserialize)]
pub struct TransitionTaskRequest {
    pub status_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct MoveTaskRequest {
    pub status_id: Uuid,
    /// 目标列内下标,从 0 开始,超出列长时落尾。
    pub position: u64,
}

#[derive(Debug, Deserialize)]
pub struct DeleteTaskRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskView {
    pub id: Uuid,
    pub task_key: String,
    pub project_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub task_number: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub status_id: Uuid,
    pub position: i64,
    pub assignee_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub reporter_id: Uuid,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TaskListResponse {
    pub items: Vec<TaskView>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct CrossProjectTasksQuery {
    /// assignee(默认)=我负责的,reporter=我创建的,all=可见项目全部。
    pub scope: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TaskListItem {
    #[serde(flatten)]
    pub task: TaskView,
    pub project_key: String,
    pub project_name: String,
    pub status_name: String,
}

#[derive(Debug, Serialize)]
pub struct CrossProjectTaskListResponse {
    pub items: Vec<TaskListItem>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
}

/// 跨项目任务列表:可见项目集 = 直接成员 ∪ 部门授权闭包,再叠加 scope 过滤。
pub async fn list_cross_project_tasks(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &CrossProjectTasksQuery,
) -> Result<CrossProjectTaskListResponse, TaskError> {
    let scope = query.scope.as_deref().unwrap_or("assignee");
    let scope_column = match scope {
        "assignee" => Some(tasks::Column::AssigneeId),
        "reporter" => Some(tasks::Column::ReporterId),
        "all" => None,
        _ => {
            return Err(TaskError::InvalidInput(
                "scope 必须是 assignee/reporter/all".to_owned(),
            ))
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
    let mut statement = tasks::Entity::find()
        .filter(tasks::Column::DeletedAt.is_null())
        .order_by_desc(tasks::Column::UpdatedAt)
        .order_by_desc(tasks::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1);
    if let Some(column) = scope_column {
        statement = statement.filter(column.eq(current_user.user_id));
    }
    if !current_user.system_role.is_super_admin() {
        let visible = visible_project_ids(db, current_user.user_id).await?;
        if visible.is_empty() {
            return Ok(CrossProjectTaskListResponse {
                items: Vec::new(),
                page,
                page_size,
                has_more: false,
            });
        }
        statement = statement.filter(tasks::Column::ProjectId.is_in(visible));
    }
    let mut models = statement.all(db).await?;
    let has_more = models.len() > page_size as usize;
    models.truncate(page_size as usize);
    let mut items: Vec<TaskView> = models.into_iter().map(TaskView::from).collect();
    hydrate_assignees(db, &mut items).await?;
    let items = hydrate_project_context(db, items).await?;
    Ok(CrossProjectTaskListResponse {
        items,
        page,
        page_size,
        has_more,
    })
}

/// 一次查询展开视图的 project_key/project_name/status_name,批量回填避免逐行查询。
async fn hydrate_project_context(
    db: &DatabaseConnection,
    views: Vec<TaskView>,
) -> Result<Vec<TaskListItem>, TaskError> {
    let project_ids: HashSet<Uuid> = views.iter().map(|view| view.project_id).collect();
    let status_ids: HashSet<Uuid> = views.iter().map(|view| view.status_id).collect();
    let projects: HashMap<Uuid, (String, String)> = if project_ids.is_empty() {
        HashMap::new()
    } else {
        projects::Entity::find()
            .filter(projects::Column::Id.is_in(project_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|project| (project.id, (project.project_key, project.name)))
            .collect()
    };
    let statuses: HashMap<Uuid, String> = if status_ids.is_empty() {
        HashMap::new()
    } else {
        project_statuses::Entity::find()
            .filter(project_statuses::Column::Id.is_in(status_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|status| (status.id, status.name))
            .collect()
    };
    Ok(views
        .into_iter()
        .map(|view| {
            let (project_key, project_name) =
                projects.get(&view.project_id).cloned().unwrap_or_default();
            let status_name = statuses.get(&view.status_id).cloned().unwrap_or_default();
            TaskListItem {
                task: view,
                project_key,
                project_name,
                status_name,
            }
        })
        .collect())
}

#[derive(Debug, FromQueryResult)]
struct VisibleProjectId {
    id: Uuid,
}

/// 当前用户可见项目 id 集:直接成员或其部门(含祖先)被授权的项目。
async fn visible_project_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, TaskError> {
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT DISTINCT p.id FROM projects p WHERE p.deleted_at IS NULL AND (p.id IN (SELECT pm.project_id FROM project_members pm WHERE pm.user_id = $1 AND pm.revoked_at IS NULL) OR p.id IN (SELECT pdg.project_id FROM project_department_grants pdg JOIN department_closure dc ON dc.ancestor_id = pdg.department_id JOIN user_departments ud ON ud.department_id = dc.descendant_id WHERE ud.user_id = $1 AND pdg.revoked_at IS NULL AND ud.revoked_at IS NULL))",
        [user_id.into()],
    );
    Ok(VisibleProjectId::find_by_statement(statement)
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect())
}

pub async fn list_project_tasks(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    query: &ListTasksQuery,
) -> Result<TaskListResponse, TaskError> {
    let project = find_project(db, project_key).await?;
    require_read_role(db, current_user, project.id).await?;
    let (page, page_size) = query.normalized();
    let mut statement = tasks::Entity::find()
        .filter(tasks::Column::ProjectId.eq(project.id))
        .filter(tasks::Column::DeletedAt.is_null())
        .order_by_desc(tasks::Column::CreatedAt)
        .order_by_desc(tasks::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1);
    if let Some(status_id) = query.status_id {
        statement = statement.filter(tasks::Column::StatusId.eq(status_id));
    }
    if let Some(parent_task_id) = query.parent_task_id {
        statement = statement.filter(tasks::Column::ParentTaskId.eq(parent_task_id));
    }
    let mut models = statement.all(db).await?;
    let has_more = models.len() > page_size as usize;
    models.truncate(page_size as usize);
    let mut items: Vec<TaskView> = models.into_iter().map(TaskView::from).collect();
    hydrate_assignees(db, &mut items).await?;
    Ok(TaskListResponse {
        items,
        page,
        page_size,
        has_more,
    })
}

pub async fn detail(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_read_role(db, current_user, task.project_id).await?;
    let mut view = TaskView::from(task);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn create_project_task(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    mut request: CreateTaskRequest,
) -> Result<TaskView, TaskError> {
    let project = find_project(db, project_key).await?;
    ensure_project_open(&project)?;
    require_write_role(db, current_user, project.id).await?;
    request.parent_task_id = None;
    create_task(db, current_user, project, request).await
}

pub async fn create_subtask(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    parent_task_key: &str,
    mut request: CreateTaskRequest,
) -> Result<TaskView, TaskError> {
    let parent = find_task(db, parent_task_key, false).await?;
    require_write_role(db, current_user, parent.project_id).await?;
    request.parent_task_id = Some(parent.id);
    let project = projects::Entity::find_by_id(parent.project_id)
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    ensure_project_open(&project)?;
    create_task(db, current_user, project, request).await
}

async fn create_task(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project: projects::Model,
    request: CreateTaskRequest,
) -> Result<TaskView, TaskError> {
    let title = required_title(request.title)?;
    let priority = normalize_priority(request.priority)?;
    if let Some(assignee_id) = request.assignee_id {
        validate_assignee(db, project.id, assignee_id).await?;
    }
    let txn = db.begin().await?;

    let parent = if let Some(parent_id) = request.parent_task_id {
        let parent = tasks::Entity::find_by_id(parent_id)
            .filter(tasks::Column::ProjectId.eq(project.id))
            .filter(tasks::Column::DeletedAt.is_null())
            .one(&txn)
            .await?
            .ok_or(TaskError::NotFound)?;
        task_rules::classify_new_task(NewTaskParent {
            parent_task_id: Some(parent.id),
            parent_already_has_parent: parent.parent_task_id.is_some(),
        })
        .map_err(|error| TaskError::Conflict(error.to_string()))?;
        Some(parent)
    } else {
        task_rules::classify_new_task(NewTaskParent {
            parent_task_id: None,
            parent_already_has_parent: false,
        })
        .map_err(|error| TaskError::Conflict(error.to_string()))?;
        None
    };

    let status = find_status_for_create(&txn, project.id, request.status_id).await?;
    let task_number = next_task_number(&txn, project.id).await?;
    // 新任务追加到目标列末尾,保证列内 position 连续。
    let position = next_position_in_column(&txn, project.id, status.id).await?;
    let now = Utc::now();
    let task = tasks::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_key: Set(format!("{}-{}", project.project_key, task_number)),
        project_id: Set(project.id),
        parent_task_id: Set(parent.map(|value| value.id)),
        status_id: Set(status.id),
        position: Set(position),
        milestone_id: Set(None),
        title: Set(title),
        description: Set(request.description),
        priority: Set(priority),
        reporter_id: Set(current_user.user_id),
        assignee_id: Set(request.assignee_id),
        due_at: Set(request.due_at),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
        task_number: Set(task_number),
    }
    .insert(&txn)
    .await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &task,
        "create",
        format!("创建任务 {}", task.task_key),
        json!({ "task_key": task.task_key, "status_id": task.status_id, "parent_task_id": task.parent_task_id }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(task);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn update(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: UpdateTaskRequest,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let old = serde_json::to_value(&task)?;
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    let mut diff = serde_json::Map::new();
    if let Some(title) = request.title {
        active.title = Set(required_title(title)?);
        diff.insert("title".to_owned(), json!(active.title.as_ref()));
    }
    if let Some(description) = request.description {
        active.description = Set(Some(description.clone()));
        diff.insert("description".to_owned(), json!(description));
    }
    if let Some(priority) = request.priority {
        let priority = normalize_priority(Some(priority))?;
        active.priority = Set(priority.clone());
        diff.insert("priority".to_owned(), json!(priority));
    }
    if let Some(assignee_id) = request.assignee_id {
        if let Some(assignee_id) = assignee_id {
            validate_assignee(db, task.project_id, assignee_id).await?;
        }
        active.assignee_id = Set(assignee_id);
        diff.insert("assignee_id".to_owned(), json!(assignee_id));
    }
    if let Some(due_at) = request.due_at {
        active.due_at = Set(due_at);
        diff.insert("due_at".to_owned(), json!(due_at));
    }
    if diff.is_empty() {
        let mut view = TaskView::from(task);
        hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
        return Ok(view);
    }
    active.updated_at = Set(Utc::now());
    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &updated,
        "update",
        format!("更新任务 {}", updated.task_key),
        serde_json::Value::Object(diff),
        Some(old),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(updated);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn transition(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: TransitionTaskRequest,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let status = project_statuses::Entity::find_by_id(request.status_id)
        .filter(project_statuses::Column::ProjectId.eq(task.project_id))
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput("目标状态不属于当前项目".to_owned()))?;
    if status.id == task.status_id {
        let mut view = TaskView::from(task);
        hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
        return Ok(view);
    }
    let txn = db.begin().await?;
    // 换列后追加到新列末尾,旧列收口补齐空档。
    let position = next_position_in_column(&txn, task.project_id, status.id).await?;
    let old_status_id = task.status_id;
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    active.status_id = Set(status.id);
    active.position = Set(position);
    active.updated_at = Set(Utc::now());
    let updated = active.update(&txn).await?;
    compact_column(&txn, task.project_id, old_status_id).await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &updated,
        "status_transition",
        format!("变更任务 {} 状态", updated.task_key),
        json!({ "from_status_id": task.status_id, "to_status_id": status.id }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(updated);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

/// 看板拖拽落点:换列与列内重排共用,目标列按给定下标插入并整体重编号。
pub async fn move_task(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: MoveTaskRequest,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let status = project_statuses::Entity::find_by_id(request.status_id)
        .filter(project_statuses::Column::ProjectId.eq(task.project_id))
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput("目标状态不属于当前项目".to_owned()))?;
    let txn = db.begin().await?;
    // 项目级排他:防止并发移动交错读写同一列造成重复或空洞 position。
    txn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtext($1))",
        [task.project_id.to_string().into()],
    ))
    .await?;
    let same_column = status.id == task.status_id;
    let mut column = column_task_ids(&txn, task.project_id, status.id, None).await?;
    let current_index = column.iter().position(|id| *id == task.id);
    column.retain(|id| *id != task.id);
    let insert_index = (request.position as usize).min(column.len());
    if same_column && current_index == Some(insert_index) {
        txn.rollback().await?;
        let mut view = TaskView::from(task);
        hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
        return Ok(view);
    }
    column.insert(insert_index, task.id);
    for (index, id) in column.iter().enumerate() {
        let mut active = tasks::ActiveModel {
            id: Set(*id),
            ..Default::default()
        };
        active.position = Set(index as i64);
        active.update(&txn).await?;
    }
    let old_status_id = task.status_id;
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    if !same_column {
        active.status_id = Set(status.id);
    }
    active.updated_at = Set(Utc::now());
    let updated = active.update(&txn).await?;
    if !same_column {
        compact_column(&txn, task.project_id, old_status_id).await?;
    }
    write_task_log(
        &txn,
        current_user.user_id,
        &updated,
        "move",
        format!("移动任务 {} 到列下标 {insert_index}", updated.task_key),
        json!({ "from_status_id": old_status_id, "to_status_id": status.id, "position": insert_index }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(updated);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn delete(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: DeleteTaskRequest,
) -> Result<(), TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let active_subtasks = tasks::Entity::find()
        .filter(tasks::Column::ParentTaskId.eq(task.id))
        .filter(tasks::Column::DeletedAt.is_null())
        .count(db)
        .await?;
    if !task_rules::can_delete_parent_task(active_subtasks) {
        return Err(TaskError::Conflict("父任务仍有未删除的子任务".to_owned()));
    }
    let txn = db.begin().await?;
    let now = Utc::now();
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    active.deleted_at = Set(Some(now));
    active.deleted_by = Set(Some(current_user.user_id));
    active.delete_reason = Set(request.reason.clone());
    active.updated_at = Set(now);
    let deleted = active.update(&txn).await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &deleted,
        "logical_delete",
        format!("逻辑删除任务 {}", deleted.task_key),
        json!({ "reason": request.reason }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

pub async fn restore(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, true).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let txn = db.begin().await?;
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    active.deleted_at = Set(None);
    active.deleted_by = Set(None);
    active.delete_reason = Set(None);
    active.updated_at = Set(Utc::now());
    let restored = active.update(&txn).await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &restored,
        "restore",
        format!("恢复任务 {}", restored.task_key),
        json!({ "restored": true }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(restored);
    hydrate_assignees(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn subtasks(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<Vec<TaskView>, TaskError> {
    let parent = find_task(db, task_key, false).await?;
    require_read_role(db, current_user, parent.project_id).await?;
    let mut items: Vec<TaskView> = tasks::Entity::find()
        .filter(tasks::Column::ParentTaskId.eq(parent.id))
        .filter(tasks::Column::DeletedAt.is_null())
        .order_by_asc(tasks::Column::TaskNumber)
        .all(db)
        .await?
        .into_iter()
        .map(TaskView::from)
        .collect();
    hydrate_assignees(db, &mut items).await?;
    Ok(items)
}

fn ensure_project_open(project: &projects::Model) -> Result<(), TaskError> {
    if project.archived_at.is_some() {
        Err(TaskError::Conflict("归档项目不能新增或修改任务".to_owned()))
    } else {
        Ok(())
    }
}

async fn ensure_project_open_by_id(
    db: &DatabaseConnection,
    project_id: Uuid,
) -> Result<(), TaskError> {
    let project = projects::Entity::find_by_id(project_id)
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    ensure_project_open(&project)
}

async fn find_project(
    db: &DatabaseConnection,
    project_key: &str,
) -> Result<projects::Model, TaskError> {
    projects::Entity::find()
        .filter(projects::Column::ProjectKey.eq(project_key))
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)
}

async fn find_task(
    db: &DatabaseConnection,
    task_key: &str,
    include_deleted: bool,
) -> Result<tasks::Model, TaskError> {
    let mut query = tasks::Entity::find().filter(tasks::Column::TaskKey.eq(task_key));
    if !include_deleted {
        query = query.filter(tasks::Column::DeletedAt.is_null());
    }
    query.one(db).await?.ok_or(TaskError::NotFound)
}

pub(crate) async fn user_can_read_project(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<bool, sea_orm::DbErr> {
    match effective_role(db, current_user, project_id).await {
        Ok(role) => Ok(role.can_read_project()),
        Err(TaskError::Database(error)) => Err(error),
        Err(_) => Ok(false),
    }
}

async fn require_read_role(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<EffectiveProjectRole, TaskError> {
    let role = effective_role(db, current_user, project_id).await?;
    authz::require_project_read(role).map_err(|_| TaskError::Forbidden)?;
    Ok(role)
}

async fn require_write_role(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<(), TaskError> {
    let role = effective_role(db, current_user, project_id).await?;
    authz::require_task_write(role).map_err(|_| TaskError::Forbidden)
}

async fn effective_role(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_id: Uuid,
) -> Result<EffectiveProjectRole, TaskError> {
    if current_user.system_role.is_super_admin() {
        return Ok(EffectiveProjectRole::SuperAdmin);
    }
    let member = project_members::Entity::find()
        .filter(project_members::Column::ProjectId.eq(project_id))
        .filter(project_members::Column::UserId.eq(current_user.user_id))
        .filter(project_members::Column::RevokedAt.is_null())
        .one(db)
        .await?;
    let direct = member.and_then(|value| match value.role.as_str() {
        "manager" => Some(ProjectRole::Manager),
        "member" => Some(ProjectRole::Member),
        "viewer" => Some(ProjectRole::Viewer),
        _ => None,
    });
    let department = department_grant_role(db, current_user.user_id, project_id).await?;
    Ok(authz::compute_effective_project_role(
        &authz::ProjectRoleInputs {
            system_role: current_user.system_role,
            direct_project_role: direct,
            department_grant_role: department,
        },
    ))
}

fn parse_project_role(value: &str) -> Option<ProjectRole> {
    match value {
        "manager" => Some(ProjectRole::Manager),
        "member" => Some(ProjectRole::Member),
        "viewer" => Some(ProjectRole::Viewer),
        _ => None,
    }
}

#[derive(Debug, FromQueryResult)]
struct DepartmentGrantRole {
    role: String,
}

async fn department_grant_role(
    db: &DatabaseConnection,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<Option<ProjectRole>, TaskError> {
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT pdg.role FROM project_department_grants pdg JOIN department_closure dc ON dc.ancestor_id = pdg.department_id JOIN user_departments ud ON ud.department_id = dc.descendant_id WHERE pdg.project_id = $1 AND ud.user_id = $2 AND pdg.revoked_at IS NULL AND ud.revoked_at IS NULL ORDER BY CASE pdg.role WHEN 'member' THEN 2 WHEN 'viewer' THEN 1 ELSE 0 END DESC LIMIT 1",
        [project_id.into(), user_id.into()],
    );
    Ok(DepartmentGrantRole::find_by_statement(statement)
        .one(db)
        .await?
        .and_then(|value| parse_project_role(&value.role)))
}

async fn find_status_for_create<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    project_id: Uuid,
    status_id: Option<Uuid>,
) -> Result<project_statuses::Model, TaskError> {
    let query =
        project_statuses::Entity::find().filter(project_statuses::Column::ProjectId.eq(project_id));
    if let Some(status_id) = status_id {
        return query
            .filter(project_statuses::Column::Id.eq(status_id))
            .one(conn)
            .await?
            .ok_or_else(|| TaskError::InvalidInput("目标状态不属于当前项目".to_owned()));
    }
    query
        .order_by_asc(project_statuses::Column::SortOrder)
        .one(conn)
        .await?
        .ok_or_else(|| TaskError::Conflict("当前项目还没有任务状态".to_owned()))
}

#[derive(Debug, FromQueryResult)]
struct NextTaskNumber {
    task_number_seed: i64,
}

async fn next_task_number<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    project_id: Uuid,
) -> Result<i64, TaskError> {
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE projects SET task_number_seed = task_number_seed + 1, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND deleted_at IS NULL RETURNING task_number_seed",
        [project_id.into()],
    );
    NextTaskNumber::find_by_statement(statement)
        .one(conn)
        .await?
        .map(|value| value.task_number_seed)
        .ok_or(TaskError::NotFound)
}

/// 目标列的下一个顺位:MAX(position)+1,空列返回 0。
async fn next_position_in_column<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    project_id: Uuid,
    status_id: Uuid,
) -> Result<i64, TaskError> {
    let max = tasks::Entity::find()
        .filter(tasks::Column::ProjectId.eq(project_id))
        .filter(tasks::Column::StatusId.eq(status_id))
        .filter(tasks::Column::DeletedAt.is_null())
        .select_only()
        .column_as(tasks::Column::Position.max(), "max_position")
        .into_tuple::<Option<i64>>()
        .one(conn)
        .await?;
    Ok(max.flatten().unwrap_or(-1) + 1)
}

/// 状态列的任务 id 顺序:position 优先,task_number 兜底排序。
async fn column_task_ids<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    project_id: Uuid,
    status_id: Uuid,
    exclude_task_id: Option<Uuid>,
) -> Result<Vec<Uuid>, TaskError> {
    let mut query = tasks::Entity::find()
        .filter(tasks::Column::ProjectId.eq(project_id))
        .filter(tasks::Column::StatusId.eq(status_id))
        .filter(tasks::Column::DeletedAt.is_null())
        .order_by_asc(tasks::Column::Position)
        .order_by_asc(tasks::Column::TaskNumber);
    if let Some(exclude_task_id) = exclude_task_id {
        query = query.filter(tasks::Column::Id.ne(exclude_task_id));
    }
    Ok(query
        .all(conn)
        .await?
        .into_iter()
        .map(|task| task.id)
        .collect())
}

/// 列内重编号为 0..n 连续值,用于移出任务后收口空档。
async fn compact_column<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    project_id: Uuid,
    status_id: Uuid,
) -> Result<(), TaskError> {
    let ids = column_task_ids(conn, project_id, status_id, None).await?;
    for (index, id) in ids.iter().enumerate() {
        let mut active = tasks::ActiveModel {
            id: Set(*id),
            ..Default::default()
        };
        active.position = Set(index as i64);
        active.update(conn).await?;
    }
    Ok(())
}

/// 批量回填负责人显示名:不去重 assignee 集合后单查 users,避免逐任务 N+1。
async fn hydrate_assignees(
    db: &DatabaseConnection,
    views: &mut [TaskView],
) -> Result<(), TaskError> {
    let ids: HashSet<Uuid> = views.iter().filter_map(|view| view.assignee_id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let names: HashMap<Uuid, String> = users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|user| (user.id, user.display_name))
        .collect();
    for view in views {
        view.assignee_name = view.assignee_id.and_then(|id| names.get(&id).cloned());
    }
    Ok(())
}

/// 负责人合法性:存在、未停用、且按其真实系统角色可读该项目。
async fn validate_assignee(
    db: &DatabaseConnection,
    project_id: Uuid,
    assignee_id: Uuid,
) -> Result<(), TaskError> {
    let user = users::Entity::find_by_id(assignee_id)
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput("负责人不存在".to_owned()))?;
    if !user.is_active {
        return Err(TaskError::InvalidInput("负责人已被停用".to_owned()));
    }
    let as_current = CurrentUser {
        user_id: user.id,
        account: user.account,
        system_role: match user.system_role.as_str() {
            "super_admin" => SystemRole::SuperAdmin,
            _ => SystemRole::User,
        },
    };
    if !user_can_read_project(db, &as_current, project_id).await? {
        return Err(TaskError::InvalidInput(
            "负责人没有当前项目的访问权限".to_owned(),
        ));
    }
    Ok(())
}

async fn write_task_log<C: ConnectionTrait + Send + Sync>(
    conn: &C,
    actor_user_id: Uuid,
    task: &tasks::Model,
    action: &str,
    summary: String,
    diff: serde_json::Value,
    snapshot: Option<serde_json::Value>,
) -> Result<(), TaskError> {
    operation_logs::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_user_id: Set(actor_user_id),
        module: Set("task".to_owned()),
        action: Set(action.to_owned()),
        project_id: Set(Some(task.project_id)),
        task_id: Set(Some(task.id)),
        target_type: Set(if task.parent_task_id.is_some() {
            "subtask"
        } else {
            "task"
        }
        .to_owned()),
        target_id: Set(Some(task.id)),
        summary: Set(summary),
        diff: Set(Some(diff)),
        snapshot: Set(snapshot),
        created_at: Set(Utc::now()),
    }
    .insert(conn)
    .await?;
    Ok(())
}

fn required_title(title: String) -> Result<String, TaskError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(TaskError::InvalidInput("任务标题不能为空".to_owned()));
    }
    if title.chars().count() > 240 {
        return Err(TaskError::InvalidInput(
            "任务标题不能超过 240 个字符".to_owned(),
        ));
    }
    Ok(title.to_owned())
}

fn normalize_priority(priority: Option<String>) -> Result<String, TaskError> {
    let priority = priority.unwrap_or_else(|| "medium".to_owned());
    match priority.as_str() {
        "urgent" | "high" | "medium" | "low" | "none" => Ok(priority),
        _ => Err(TaskError::InvalidInput(
            "priority 必须是 urgent/high/medium/low/none".to_owned(),
        )),
    }
}

impl From<tasks::Model> for TaskView {
    fn from(value: tasks::Model) -> Self {
        Self {
            id: value.id,
            task_key: value.task_key,
            project_id: value.project_id,
            parent_task_id: value.parent_task_id,
            task_number: value.task_number,
            title: value.title,
            description: value.description,
            priority: value.priority,
            status_id: value.status_id,
            position: value.position,
            assignee_id: value.assignee_id,
            assignee_name: None,
            reporter_id: value.reporter_id,
            due_at: value.due_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
