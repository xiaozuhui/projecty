//! 任务用例：分页查询、两层子任务、逻辑删除和操作日志事务。

use chrono::{DateTime, Utc};
use projecty_entity::{
    labels, milestones, operation_logs, project_members, project_statuses, projects,
    task_dependencies, task_labels, tasks, users,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, FromQueryResult, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Select, Set, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::{
    application::{authz, task_rules},
    domain::{
        permissions::{EffectiveProjectRole, ProjectRole, SystemRole},
        tasks::NewTaskParent,
    },
    http::extractors::CurrentUser,
    modules::notifications::service as notification_service,
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
    #[error("{0}")]
    ForbiddenAction(String),
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
    pub task_type: Option<String>,
    /// 标题或任务编号模糊匹配。
    pub keyword: Option<String>,
    pub assignee_id: Option<Uuid>,
    /// true=只看未分配负责人的任务。
    pub unassigned: Option<bool>,
    pub priority: Option<String>,
    pub milestone_id: Option<Uuid>,
    pub label_id: Option<Uuid>,
    /// true=只看已逾期(截止时间已过且状态未到完成类)。
    pub overdue: Option<bool>,
    /// true=只看 7 天内到期且未完成的任务。
    pub due_soon: Option<bool>,
}

impl ListTasksQuery {
    fn normalized(&self) -> (u64, u64) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self.page_size.unwrap_or(30).clamp(1, 100);
        (page, page_size)
    }

    fn keyword(&self) -> Option<String> {
        let keyword = self.keyword.as_deref()?.trim().to_owned();
        if keyword.is_empty() {
            return None;
        }
        Some(keyword.chars().take(100).collect())
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub task_type: Option<String>,
    pub status_id: Option<Uuid>,
    pub assignee_id: Option<Uuid>,
    pub reviewer_id: Option<Uuid>,
    pub start_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub parent_task_id: Option<Uuid>,
    pub milestone_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    // 描述同样用双层 Option:缺省=不修改,显式 null=清空。
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    pub priority: Option<String>,
    pub task_type: Option<String>,
    // 双层 Option:字段缺省=不修改,显式 null=清空,配合 double_option 反序列化。
    #[serde(default, deserialize_with = "double_option")]
    pub assignee_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    pub reviewer_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "double_option")]
    pub start_at: Option<Option<DateTime<Utc>>>,
    #[serde(default, deserialize_with = "double_option")]
    pub due_at: Option<Option<DateTime<Utc>>>,
    /// 变更任务归属:null=脱离父任务转为主任务,Some=挂靠到指定根任务。
    #[serde(default, deserialize_with = "double_option")]
    pub parent_task_id: Option<Option<Uuid>>,
    /// 里程碑关联:null=解除关联,Some=挂到指定里程碑。
    #[serde(default, deserialize_with = "double_option")]
    pub milestone_id: Option<Option<Uuid>>,
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
    pub task_type: String,
    pub status_id: Uuid,
    pub position: i64,
    pub assignee_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub reviewer_id: Option<Uuid>,
    pub reviewer_name: Option<String>,
    pub reporter_id: Uuid,
    pub start_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub milestone_id: Option<Uuid>,
    pub labels: Vec<LabelLite>,
    /// 子任务统计(看板卡片进度展示用),根任务才有意义,子任务恒为 0。
    pub subtask_total: i64,
    pub subtask_done: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LabelLite {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TaskListResponse {
    pub items: Vec<TaskView>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
    pub total: u64,
}

#[derive(Debug, Deserialize)]
pub struct CrossProjectTasksQuery {
    /// assignee(默认)=我负责的,reporter=我创建的,reviewer=我评审的,all=可见项目全部。
    pub scope: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub keyword: Option<String>,
    /// true=只看已逾期(截止时间已过且状态未到完成类)。
    pub overdue: Option<bool>,
    /// true=只看 7 天内到期且未完成的任务。
    pub due_soon: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TaskListItem {
    #[serde(flatten)]
    pub task: TaskView,
    pub project_key: String,
    pub project_name: String,
    pub status_name: String,
    /// 状态类别(todo/in_progress/done),前端据此给状态胶囊着色。
    pub status_category: String,
}

#[derive(Debug, Serialize)]
pub struct CrossProjectTaskListResponse {
    pub items: Vec<TaskListItem>,
    pub page: u64,
    pub page_size: u64,
    pub has_more: bool,
    pub total: u64,
}

/// 跨项目任务列表:可见项目集 = 直接成员 ∪ 部门授权闭包,再叠加 scope/关键词/逾期过滤。
pub async fn list_cross_project_tasks(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    query: &CrossProjectTasksQuery,
) -> Result<CrossProjectTaskListResponse, TaskError> {
    let scope = query.scope.as_deref().unwrap_or("assignee");
    let scope_column = match scope {
        "assignee" => Some(tasks::Column::AssigneeId),
        "reporter" => Some(tasks::Column::ReporterId),
        "reviewer" => Some(tasks::Column::ReviewerId),
        "all" => None,
        _ => {
            return Err(TaskError::InvalidInput(
                "scope 必须是 assignee/reporter/reviewer/all".to_owned(),
            ))
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
    let keyword = query
        .keyword
        .as_deref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(100).collect::<String>());
    let mut statement = tasks::Entity::find().filter(tasks::Column::DeletedAt.is_null());
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
                total: 0,
            });
        }
        statement = statement.filter(tasks::Column::ProjectId.is_in(visible));
    }
    if let Some(keyword) = keyword {
        let pattern = format!("%{}%", escape_like(&keyword));
        statement = statement.filter(
            Condition::any()
                .add(tasks::Column::Title.like(&pattern))
                .add(tasks::Column::TaskKey.ilike(&pattern)),
        );
    }
    if query.overdue == Some(true) {
        statement = apply_overdue_cross_project(db, statement).await?;
    }
    if query.due_soon == Some(true) {
        statement = apply_due_soon_cross_project(db, statement).await?;
    }
    let total = statement.clone().count(db).await?;
    let models = statement
        .order_by_desc(tasks::Column::UpdatedAt)
        .order_by_desc(tasks::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1)
        .all(db)
        .await?;
    let has_more = models.len() > page_size as usize;
    let models = models.into_iter().take(page_size as usize).collect::<Vec<_>>();
    let mut items: Vec<TaskView> = models.into_iter().map(TaskView::from).collect();
    hydrate_views(db, &mut items).await?;
    let items = hydrate_project_context(db, items).await?;
    Ok(CrossProjectTaskListResponse {
        items,
        page,
        page_size,
        has_more,
        total,
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
    let statuses: HashMap<Uuid, (String, String)> = if status_ids.is_empty() {
        HashMap::new()
    } else {
        project_statuses::Entity::find()
            .filter(project_statuses::Column::Id.is_in(status_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|status| (status.id, (status.name, status.category)))
            .collect()
    };
    Ok(views
        .into_iter()
        .map(|view| {
            let (project_key, project_name) =
                projects.get(&view.project_id).cloned().unwrap_or_default();
            let (status_name, status_category) =
                statuses.get(&view.status_id).cloned().unwrap_or_default();
            TaskListItem {
                task: view,
                project_key,
                project_name,
                status_name,
                status_category,
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
    let statement = build_task_statement(db, &project, query).await?;
    let (page, page_size) = query.normalized();
    let total = statement.clone().count(db).await?;
    let models = statement
        .order_by_desc(tasks::Column::CreatedAt)
        .order_by_desc(tasks::Column::Id)
        .offset((page - 1) * page_size)
        .limit(page_size + 1)
        .all(db)
        .await?;
    let has_more = models.len() > page_size as usize;
    let models = models.into_iter().take(page_size as usize).collect::<Vec<_>>();
    let mut items: Vec<TaskView> = models.into_iter().map(TaskView::from).collect();
    hydrate_views(db, &mut items).await?;
    Ok(TaskListResponse {
        items,
        page,
        page_size,
        has_more,
        total,
    })
}

/// 项目内任务查询语句组装:列表分页与 CSV 导出共用同一套过滤维度。
async fn build_task_statement(
    db: &DatabaseConnection,
    project: &projects::Model,
    query: &ListTasksQuery,
) -> Result<Select<tasks::Entity>, TaskError> {
    let mut statement = tasks::Entity::find()
        .filter(tasks::Column::ProjectId.eq(project.id))
        .filter(tasks::Column::DeletedAt.is_null());
    if let Some(status_id) = query.status_id {
        statement = statement.filter(tasks::Column::StatusId.eq(status_id));
    }
    if let Some(parent_task_id) = query.parent_task_id {
        statement = statement.filter(tasks::Column::ParentTaskId.eq(parent_task_id));
    }
    if let Some(task_type) = query.task_type.as_deref() {
        let task_type = normalize_task_type(Some(task_type.to_owned()))?;
        statement = statement.filter(tasks::Column::TaskType.eq(task_type));
    }
    if let Some(keyword) = query.keyword() {
        let pattern = format!("%{}%", escape_like(&keyword));
        statement = statement.filter(
            Condition::any()
                .add(tasks::Column::Title.ilike(&pattern))
                .add(tasks::Column::TaskKey.ilike(&pattern)),
        );
    }
    if let Some(assignee_id) = query.assignee_id {
        statement = statement.filter(tasks::Column::AssigneeId.eq(assignee_id));
    }
    if query.unassigned == Some(true) {
        statement = statement.filter(tasks::Column::AssigneeId.is_null());
    }
    if let Some(priority) = query.priority.as_deref() {
        let priority = normalize_priority(Some(priority.to_owned()))?;
        statement = statement.filter(tasks::Column::Priority.eq(priority));
    }
    if let Some(milestone_id) = query.milestone_id {
        statement = statement.filter(tasks::Column::MilestoneId.eq(milestone_id));
    }
    if let Some(label_id) = query.label_id {
        statement = statement
            .join(
                sea_orm::sea_query::JoinType::InnerJoin,
                task_labels::Entity::belongs_to(tasks::Entity)
                    .from(task_labels::Column::TaskId)
                    .to(tasks::Column::Id)
                    .into(),
            )
            .filter(task_labels::Column::LabelId.eq(label_id));
    }
    if query.overdue == Some(true) {
        statement = apply_overdue_filter(db, statement, project.id).await?;
    }
    if query.due_soon == Some(true) {
        statement = apply_due_soon_filter(db, statement, project.id).await?;
    }
    Ok(statement)
}

/// 逾期过滤:截止时间已过,且状态未到完成类(项目内 done 状态集先查后排除)。
async fn apply_overdue_filter(
    db: &DatabaseConnection,
    statement: Select<tasks::Entity>,
    project_id: Uuid,
) -> Result<Select<tasks::Entity>, TaskError> {
    let done_ids = done_status_ids(db, Some(project_id)).await?;
    let statement = statement.filter(tasks::Column::DueAt.lt(Utc::now()));
    if done_ids.is_empty() {
        return Ok(statement);
    }
    Ok(statement.filter(tasks::Column::StatusId.is_not_in(done_ids)))
}

/// 逾期过滤的跨项目版本:排除所有项目的 done 类状态。
async fn apply_overdue_cross_project(
    db: &DatabaseConnection,
    statement: Select<tasks::Entity>,
) -> Result<Select<tasks::Entity>, TaskError> {
    let done_ids = done_status_ids(db, None).await?;
    let statement = statement.filter(tasks::Column::DueAt.lt(Utc::now()));
    if done_ids.is_empty() {
        return Ok(statement);
    }
    Ok(statement.filter(tasks::Column::StatusId.is_not_in(done_ids)))
}

/// done 类状态 id 集;project_id 为 None 时取全部项目。
async fn done_status_ids(
    db: &DatabaseConnection,
    project_id: Option<Uuid>,
) -> Result<Vec<Uuid>, TaskError> {
    let mut query = project_statuses::Entity::find()
        .filter(project_statuses::Column::Category.eq("done"));
    if let Some(project_id) = project_id {
        query = query.filter(project_statuses::Column::ProjectId.eq(project_id));
    }
    Ok(query
        .all(db)
        .await?
        .into_iter()
        .map(|status| status.id)
        .collect())
}

/// 7 天内到期过滤:due_at ∈ [now, now+7d] 且状态未到完成类(项目内)。
async fn apply_due_soon_filter(
    db: &DatabaseConnection,
    statement: Select<tasks::Entity>,
    project_id: Uuid,
) -> Result<Select<tasks::Entity>, TaskError> {
    let done_ids = done_status_ids(db, Some(project_id)).await?;
    let now = Utc::now();
    let until = now + chrono::TimeDelta::try_days(7).unwrap_or_default();
    let statement = statement
        .filter(tasks::Column::DueAt.gte(now))
        .filter(tasks::Column::DueAt.lte(until));
    if done_ids.is_empty() {
        return Ok(statement);
    }
    Ok(statement.filter(tasks::Column::StatusId.is_not_in(done_ids)))
}

/// 7 天内到期过滤的跨项目版本:排除所有项目的 done 类状态。
async fn apply_due_soon_cross_project(
    db: &DatabaseConnection,
    statement: Select<tasks::Entity>,
) -> Result<Select<tasks::Entity>, TaskError> {
    let done_ids = done_status_ids(db, None).await?;
    let now = Utc::now();
    let until = now + chrono::TimeDelta::try_days(7).unwrap_or_default();
    let statement = statement
        .filter(tasks::Column::DueAt.gte(now))
        .filter(tasks::Column::DueAt.lte(until));
    if done_ids.is_empty() {
        return Ok(statement);
    }
    Ok(statement.filter(tasks::Column::StatusId.is_not_in(done_ids)))
}

/// LIKE 模式转义:百分号、下划线与反斜杠按字面匹配。
fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub async fn detail(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_read_role(db, current_user, task.project_id).await?;
    let mut view = TaskView::from(task);
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
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
    let role = require_write_role(db, current_user, project.id).await?;
    request.parent_task_id = None;
    create_task(db, current_user, project, role, request).await
}

pub async fn create_subtask(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    parent_task_key: &str,
    mut request: CreateTaskRequest,
) -> Result<TaskView, TaskError> {
    let parent = find_task(db, parent_task_key, false).await?;
    let role = require_write_role(db, current_user, parent.project_id).await?;
    request.parent_task_id = Some(parent.id);
    let project = projects::Entity::find_by_id(parent.project_id)
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    ensure_project_open(&project)?;
    create_task(db, current_user, project, role, request).await
}

async fn create_task(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project: projects::Model,
    role: EffectiveProjectRole,
    request: CreateTaskRequest,
) -> Result<TaskView, TaskError> {
    let title = required_title(request.title)?;
    let priority = normalize_priority(request.priority)?;
    let task_type = normalize_task_type(request.task_type)?;
    validate_task_schedule(request.start_at, request.due_at)?;
    if let Some(assignee_id) = request.assignee_id {
        validate_task_user(db, project.id, assignee_id, "负责人").await?;
    }
    if let Some(reviewer_id) = request.reviewer_id {
        validate_task_user(db, project.id, reviewer_id, "评审人").await?;
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
    // 新任务尚无评审人,直接建在完成列只有项目管理员及以上可以,防止 member 绕过评审流转。
    if status.category == "done" && !role.can_manage_project() {
        return Err(TaskError::ForbiddenAction(
            "已完成状态的任务仅项目管理员可以创建".to_owned(),
        ));
    }
    if let Some(milestone_id) = request.milestone_id {
        validate_milestone(&txn, project.id, milestone_id).await?;
    }
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
        milestone_id: Set(request.milestone_id),
        title: Set(title),
        description: Set(request.description),
        priority: Set(priority),
        task_type: Set(task_type),
        reporter_id: Set(current_user.user_id),
        assignee_id: Set(request.assignee_id),
        reviewer_id: Set(request.reviewer_id),
        start_at: Set(request.start_at),
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
    // 分配/评审通知与任务写入同事务,失败即整体回滚。
    let actor_name = actor_display_name(db, current_user.user_id).await?;
    if let Some(assignee_id) = task.assignee_id {
        notify_task_event(
            &txn,
            &[assignee_id],
            current_user,
            &actor_name,
            &task,
            &project.project_key,
            notification_service::KIND_ASSIGNED,
        )
        .await?;
    }
    if let Some(reviewer_id) = task.reviewer_id {
        notify_task_event(
            &txn,
            &[reviewer_id],
            current_user,
            &actor_name,
            &task,
            &project.project_key,
            notification_service::KIND_REVIEW_REQUESTED,
        )
        .await?;
    }
    write_task_log(
        &txn,
        current_user.user_id,
        &task,
        "create",
        format!("创建任务 {}", task.task_key),
        json!({ "task_key": task.task_key, "status_id": task.status_id, "parent_task_id": task.parent_task_id, "milestone_id": task.milestone_id }),
        Some(serde_json::to_value(&task)?),
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(task);
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
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
    let next_start_at = request.start_at.clone().unwrap_or(task.start_at);
    let next_due_at = request.due_at.clone().unwrap_or(task.due_at);
    validate_task_schedule(next_start_at, next_due_at)?;
    let mut active: tasks::ActiveModel = task.clone().into_active_model();
    let mut diff = serde_json::Map::new();
    if let Some(title) = request.title {
        active.title = Set(required_title(title)?);
        diff.insert("title".to_owned(), json!(active.title.as_ref()));
    }
    if let Some(description) = request.description {
        active.description = Set(description.clone());
        diff.insert("description".to_owned(), json!(description));
    }
    if let Some(priority) = request.priority {
        let priority = normalize_priority(Some(priority))?;
        active.priority = Set(priority.clone());
        diff.insert("priority".to_owned(), json!(priority));
    }
    if let Some(task_type) = request.task_type {
        let task_type = normalize_task_type(Some(task_type))?;
        active.task_type = Set(task_type.clone());
        diff.insert("task_type".to_owned(), json!(task_type));
    }
    if let Some(assignee_id) = request.assignee_id {
        if let Some(assignee_id) = assignee_id {
            validate_task_user(db, task.project_id, assignee_id, "负责人").await?;
        }
        active.assignee_id = Set(assignee_id);
        diff.insert("assignee_id".to_owned(), json!(assignee_id));
    }
    if let Some(reviewer_id) = request.reviewer_id {
        if let Some(reviewer_id) = reviewer_id {
            validate_task_user(db, task.project_id, reviewer_id, "评审人").await?;
        }
        active.reviewer_id = Set(reviewer_id);
        diff.insert("reviewer_id".to_owned(), json!(reviewer_id));
    }
    if let Some(start_at) = request.start_at {
        active.start_at = Set(start_at);
        diff.insert("start_at".to_owned(), json!(start_at));
    }
    if let Some(due_at) = request.due_at {
        active.due_at = Set(due_at);
        diff.insert("due_at".to_owned(), json!(due_at));
    }
    if let Some(parent_task_id) = request.parent_task_id {
        let next_parent = match parent_task_id {
            Some(parent_id) => {
                if parent_id == task.id {
                    return Err(TaskError::InvalidInput("任务不能挂靠到自己".to_owned()));
                }
                let parent = tasks::Entity::find_by_id(parent_id)
                    .filter(tasks::Column::ProjectId.eq(task.project_id))
                    .filter(tasks::Column::DeletedAt.is_null())
                    .one(db)
                    .await?
                    .ok_or(TaskError::InvalidInput(
                        "父任务不存在或不属于当前项目".to_owned(),
                    ))?;
                // 复用两层规则:目标父任务自身必须是根任务,保证 任务→子任务 两层结构。
                task_rules::classify_new_task(NewTaskParent {
                    parent_task_id: Some(parent.id),
                    parent_already_has_parent: parent.parent_task_id.is_some(),
                })
                .map_err(|error| TaskError::Conflict(error.to_string()))?;
                Some(parent.id)
            }
            None => None,
        };
        if next_parent != task.parent_task_id {
            active.parent_task_id = Set(next_parent);
            diff.insert("parent_task_id".to_owned(), json!(next_parent));
        }
    }
    if let Some(milestone_id) = request.milestone_id {
        if let Some(milestone_id) = milestone_id {
            validate_milestone(db, task.project_id, milestone_id).await?;
        }
        active.milestone_id = Set(milestone_id);
        diff.insert("milestone_id".to_owned(), json!(milestone_id));
    }
    if diff.is_empty() {
        let mut view = TaskView::from(task);
        hydrate_user_names(db, std::slice::from_mut(&mut view)).await?;
        return Ok(view);
    }
    active.updated_at = Set(Utc::now());
    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    // 换了负责人/评审人才通知,清空与不变都不打扰。
    let actor_name = actor_display_name(db, current_user.user_id).await?;
    let project = projects::Entity::find_by_id(task.project_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    if let Some(new_assignee) = updated.assignee_id {
        if Some(new_assignee) != task.assignee_id {
            notify_task_event(
                &txn,
                &[new_assignee],
                current_user,
                &actor_name,
                &updated,
                &project.project_key,
                notification_service::KIND_ASSIGNED,
            )
            .await?;
        }
    }
    if let Some(new_reviewer) = updated.reviewer_id {
        if Some(new_reviewer) != task.reviewer_id {
            notify_task_event(
                &txn,
                &[new_reviewer],
                current_user,
                &actor_name,
                &updated,
                &project.project_key,
                notification_service::KIND_REVIEW_REQUESTED,
            )
            .await?;
        }
    }
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
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

pub async fn transition(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: TransitionTaskRequest,
) -> Result<TaskView, TaskError> {
    let task = find_task(db, task_key, false).await?;
    let role = require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let status = project_statuses::Entity::find_by_id(request.status_id)
        .filter(project_statuses::Column::ProjectId.eq(task.project_id))
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput("目标状态不属于当前项目".to_owned()))?;
    ensure_transition_allowed(&task, &status, current_user, role)?;
    if status.id == task.status_id {
        let mut view = TaskView::from(task);
        hydrate_user_names(db, std::slice::from_mut(&mut view)).await?;
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
    notify_status_changed(db, &txn, current_user, &task, &status.name).await?;
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
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
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
    let role = require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let status = project_statuses::Entity::find_by_id(request.status_id)
        .filter(project_statuses::Column::ProjectId.eq(task.project_id))
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput("目标状态不属于当前项目".to_owned()))?;
    // 跨列才算状态流转,同列仅重排维持既有写权限语义。
    if status.id != task.status_id {
        ensure_transition_allowed(&task, &status, current_user, role)?;
    }
    let txn = db.begin().await?;
    // 项目级排他:防止并发移动交错读写同一列造成重复或空洞 position。
    txn.execute_raw(Statement::from_sql_and_values(
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
        hydrate_user_names(db, std::slice::from_mut(&mut view)).await?;
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
        notify_status_changed(db, &txn, current_user, &task, &status.name).await?;
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
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
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
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
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
    hydrate_views(db, &mut items).await?;
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
) -> Result<EffectiveProjectRole, TaskError> {
    let role = effective_role(db, current_user, project_id).await?;
    authz::require_task_write(role).map_err(|_| TaskError::Forbidden)?;
    Ok(role)
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

/// 视图回填统一入口:负责人/评审人姓名、标签、子任务进度,一次批量补齐。
async fn hydrate_views(
    db: &DatabaseConnection,
    views: &mut [TaskView],
) -> Result<(), TaskError> {
    hydrate_user_names(db, views).await?;
    hydrate_task_labels(db, views).await?;
    hydrate_task_stats(db, views).await?;
    Ok(())
}

/// 子任务进度统计:一条聚合 SQL 按 parent 分组,done 类状态计完成。
async fn hydrate_task_stats(
    db: &DatabaseConnection,
    views: &mut [TaskView],
) -> Result<(), TaskError> {
    #[derive(Debug, FromQueryResult)]
    struct SubtaskStat {
        parent_task_id: Uuid,
        total: i64,
        done: i64,
    }
    let parent_ids: Vec<Uuid> = views
        .iter()
        .filter(|view| view.parent_task_id.is_none())
        .map(|view| view.id)
        .collect();
    if parent_ids.is_empty() {
        return Ok(());
    }
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT t.parent_task_id AS parent_task_id, COUNT(*) AS total, COUNT(*) FILTER (WHERE ps.category = 'done') AS done FROM tasks t JOIN project_statuses ps ON ps.id = t.status_id WHERE t.parent_task_id = ANY($1) AND t.deleted_at IS NULL GROUP BY t.parent_task_id",
        [parent_ids.into()],
    );
    let stats: HashMap<Uuid, (i64, i64)> = SubtaskStat::find_by_statement(statement)
        .all(db)
        .await?
        .into_iter()
        .map(|row| (row.parent_task_id, (row.total, row.done)))
        .collect();
    for view in views.iter_mut() {
        if view.parent_task_id.is_none() {
            let (total, done) = stats.get(&view.id).cloned().unwrap_or((0, 0));
            view.subtask_total = total;
            view.subtask_done = done;
        }
    }
    Ok(())
}

/// 批量回填负责人/评审人显示名:合并去重 id 集合后单查 users,避免逐任务 N+1。
async fn hydrate_user_names(
    db: &DatabaseConnection,
    views: &mut [TaskView],
) -> Result<(), TaskError> {
    let mut ids: HashSet<Uuid> = views.iter().filter_map(|view| view.assignee_id).collect();
    ids.extend(views.iter().filter_map(|view| view.reviewer_id));
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
        view.reviewer_name = view.reviewer_id.and_then(|id| names.get(&id).cloned());
    }
    Ok(())
}

/// 批量回填任务标签:先查关联行再查标签本体,两次查询组装,避免逐任务 N+1。
async fn hydrate_task_labels(
    db: &DatabaseConnection,
    views: &mut [TaskView],
) -> Result<(), TaskError> {
    let task_ids: Vec<Uuid> = views.iter().map(|view| view.id).collect();
    if task_ids.is_empty() {
        return Ok(());
    }
    let links = task_labels::Entity::find()
        .filter(task_labels::Column::TaskId.is_in(task_ids))
        .all(db)
        .await?;
    let label_ids: HashSet<Uuid> = links.iter().map(|link| link.label_id).collect();
    let names: HashMap<Uuid, String> = if label_ids.is_empty() {
        HashMap::new()
    } else {
        labels::Entity::find()
            .filter(labels::Column::Id.is_in(label_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|label| (label.id, label.name))
            .collect()
    };
    let mut by_task: HashMap<Uuid, Vec<LabelLite>> = HashMap::new();
    for link in links {
        if let Some(name) = names.get(&link.label_id) {
            by_task.entry(link.task_id).or_default().push(LabelLite {
                id: link.label_id,
                name: name.clone(),
            });
        }
    }
    for view in views {
        let mut attached = by_task.remove(&view.id).unwrap_or_default();
        attached.sort_by(|a, b| a.name.cmp(&b.name));
        view.labels = attached;
    }
    Ok(())
}

/// 操作者显示名,通知文案快照用。
async fn actor_display_name(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<String, TaskError> {
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    Ok(user.display_name)
}

/// 事务内写一条任务事件通知,文案按类型生成快照。
async fn notify_task_event<C>(
    conn: &C,
    recipient_ids: &[Uuid],
    actor: &CurrentUser,
    actor_name: &str,
    task: &tasks::Model,
    project_key: &str,
    kind: &str,
) -> Result<(), TaskError>
where
    C: ConnectionTrait + Send + Sync,
{
    let summary = match kind {
        notification_service::KIND_ASSIGNED => {
            format!("{actor_name} 将 {}「{}」分配给你", task.task_key, task.title)
        }
        notification_service::KIND_REVIEW_REQUESTED => {
            format!("{actor_name} 指定你评审 {}「{}」", task.task_key, task.title)
        }
        _ => return Ok(()),
    };
    Ok(notification_service::notify(
        conn,
        recipient_ids,
        actor,
        actor_name,
        task,
        project_key,
        kind,
        summary,
    )
    .await?)
}

/// 状态流转通知:负责人、创建人、评审人都收到(通知服务内部去重并排除操作者)。
async fn notify_status_changed<C>(
    db: &DatabaseConnection,
    conn: &C,
    actor: &CurrentUser,
    task: &tasks::Model,
    status_name: &str,
) -> Result<(), TaskError>
where
    C: ConnectionTrait + Send + Sync,
{
    let actor_name = actor_display_name(db, actor.user_id).await?;
    let project = projects::Entity::find_by_id(task.project_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    let audience = notification_service::task_audience(task);
    let summary = format!(
        "{actor_name} 将 {}「{}」流转为 {status_name}",
        task.task_key, task.title
    );
    Ok(notification_service::notify(
        conn,
        &audience,
        actor,
        &actor_name,
        task,
        &project.project_key,
        notification_service::KIND_STATUS_CHANGED,
        summary,
    )
    .await?)
}

/// 里程碑合法性:存在、未删除且属于当前项目。
async fn validate_milestone<C>(
    conn: &C,
    project_id: Uuid,
    milestone_id: Uuid,
) -> Result<(), TaskError>
where
    C: ConnectionTrait + Send + Sync,
{
    let found = milestones::Entity::find_by_id(milestone_id)
        .filter(milestones::Column::ProjectId.eq(project_id))
        .filter(milestones::Column::DeletedAt.is_null())
        .one(conn)
        .await?;
    if found.is_none() {
        return Err(TaskError::InvalidInput(
            "里程碑不存在或不属于当前项目".to_owned(),
        ));
    }
    Ok(())
}

/// 负责人/评审人合法性:存在、未停用、且按其真实系统角色可读该项目。
async fn validate_task_user(
    db: &DatabaseConnection,
    project_id: Uuid,
    user_id: Uuid,
    field: &str,
) -> Result<(), TaskError> {
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| TaskError::InvalidInput(format!("{field}不存在")))?;
    if !user.is_active {
        return Err(TaskError::InvalidInput(format!("{field}已被停用")));
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
        return Err(TaskError::InvalidInput(format!(
            "{field}没有当前项目的访问权限"
        )));
    }
    Ok(())
}

/// 状态流转权限:项目管理员/超管豁免;评审人任意流转(含改为已完成与打回);负责人仅限非完成列;其他成员拒绝。
fn ensure_transition_allowed(
    task: &tasks::Model,
    target_status: &project_statuses::Model,
    current_user: &CurrentUser,
    role: EffectiveProjectRole,
) -> Result<(), TaskError> {
    if role.can_manage_project() {
        return Ok(());
    }
    if task.reviewer_id == Some(current_user.user_id) {
        return Ok(());
    }
    if target_status.category == "done" {
        return Err(TaskError::ForbiddenAction(
            "只有评审人可以将任务改为已完成".to_owned(),
        ));
    }
    if task.assignee_id == Some(current_user.user_id) {
        return Ok(());
    }
    Err(TaskError::ForbiddenAction(
        "只有负责人或评审人可以变更任务状态".to_owned(),
    ))
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

fn validate_task_schedule(
    start_at: Option<DateTime<Utc>>,
    due_at: Option<DateTime<Utc>>,
) -> Result<(), TaskError> {
    if let (Some(start_at), Some(due_at)) = (start_at, due_at) {
        if start_at > due_at {
            return Err(TaskError::InvalidInput(
                "开始时间不能晚于结束时间".to_owned(),
            ));
        }
    }
    Ok(())
}

fn normalize_task_type(task_type: Option<String>) -> Result<String, TaskError> {
    let task_type = task_type.unwrap_or_else(|| "feature".to_owned());
    match task_type.as_str() {
        "feature" | "bug" | "design" | "revert" | "improvement" | "refactor" | "docs" | "chore" => Ok(task_type),
        _ => Err(TaskError::InvalidInput(
            "task_type 必须是 feature/bug/design/revert/improvement/refactor/docs/chore".to_owned(),
        )),
    }
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
            task_type: value.task_type,
            status_id: value.status_id,
            position: value.position,
            assignee_id: value.assignee_id,
            assignee_name: None,
            reviewer_id: value.reviewer_id,
            reviewer_name: None,
            reporter_id: value.reporter_id,
            start_at: value.start_at,
            due_at: value.due_at,
            milestone_id: value.milestone_id,
            labels: Vec::new(),
            subtask_total: 0,
            subtask_done: 0,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

// ---------- 标签 ----------

#[derive(Debug, Deserialize)]
pub struct AddLabelRequest {
    pub name: String,
}

pub async fn list_project_labels(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<Vec<LabelLite>, TaskError> {
    let project = find_project(db, project_key).await?;
    require_read_role(db, current_user, project.id).await?;
    Ok(labels::Entity::find()
        .filter(labels::Column::ProjectId.eq(project.id))
        .order_by_asc(labels::Column::Name)
        .all(db)
        .await?
        .into_iter()
        .map(|label| LabelLite {
            id: label.id,
            name: label.name,
        })
        .collect())
}

/// 打标签:项目内按名称幂等查找或创建标签,再关联任务(已关联则直接返回)。
pub async fn add_task_label(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: AddLabelRequest,
) -> Result<LabelLite, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(TaskError::InvalidInput("标签名称不能为空".to_owned()));
    }
    if name.chars().count() > 40 {
        return Err(TaskError::InvalidInput(
            "标签名称不能超过 40 个字符".to_owned(),
        ));
    }
    let name = name.to_owned();
    let txn = db.begin().await?;
    let label = match labels::Entity::find()
        .filter(labels::Column::ProjectId.eq(task.project_id))
        .filter(labels::Column::Name.eq(&name))
        .one(&txn)
        .await?
    {
        Some(label) => label,
        None => {
            labels::ActiveModel {
                id: Set(Uuid::now_v7()),
                project_id: Set(task.project_id),
                name: Set(name.clone()),
                created_at: Set(Utc::now()),
            }
            .insert(&txn)
            .await?
        }
    };
    let linked = task_labels::Entity::find_by_id((task.id, label.id))
        .one(&txn)
        .await?;
    if linked.is_none() {
        task_labels::ActiveModel {
            task_id: Set(task.id),
            label_id: Set(label.id),
            created_at: Set(Utc::now()),
        }
        .insert(&txn)
        .await?;
        write_task_log(
            &txn,
            current_user.user_id,
            &task,
            "label_added",
            format!("为任务 {} 添加标签 {name}", task.task_key),
            json!({ "label": name }),
            None,
        )
        .await?;
    }
    txn.commit().await?;
    Ok(LabelLite {
        id: label.id,
        name: label.name,
    })
}

/// 解除任务的标签关联;标签本体保留,其他任务仍可继续使用。
pub async fn remove_task_label(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    label_id: Uuid,
) -> Result<(), TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    if task_labels::Entity::find_by_id((task.id, label_id))
        .one(db)
        .await?
        .is_none()
    {
        return Err(TaskError::NotFound);
    }
    let label = labels::Entity::find_by_id(label_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    let txn = db.begin().await?;
    task_labels::Entity::delete_by_id((task.id, label_id))
        .exec(&txn)
        .await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &task,
        "label_removed",
        format!("移除任务 {} 的标签 {}", task.task_key, label.name),
        json!({ "label": label.name }),
        None,
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

// ---------- 依赖 ----------

#[derive(Debug, Serialize)]
pub struct TaskRefView {
    pub dependency_id: Uuid,
    pub task_id: Uuid,
    pub task_key: String,
    pub title: String,
    pub status_name: String,
    pub is_done: bool,
}

#[derive(Debug, Serialize)]
pub struct TaskDependenciesResponse {
    /// 阻塞当前任务的任务。
    pub blocked_by: Vec<TaskRefView>,
    /// 当前任务阻塞的任务。
    pub blocks: Vec<TaskRefView>,
}

#[derive(Debug, Deserialize)]
pub struct AddDependencyRequest {
    pub depends_on_task_key: String,
}

pub async fn list_dependencies(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<TaskDependenciesResponse, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_read_role(db, current_user, task.project_id).await?;
    let blocked_by_rows = task_dependencies::Entity::find()
        .filter(task_dependencies::Column::TaskId.eq(task.id))
        .all(db)
        .await?;
    let blocks_rows = task_dependencies::Entity::find()
        .filter(task_dependencies::Column::DependsOnTaskId.eq(task.id))
        .all(db)
        .await?;
    let mut related_ids: Vec<Uuid> = blocked_by_rows
        .iter()
        .map(|row| row.depends_on_task_id)
        .collect();
    related_ids.extend(blocks_rows.iter().map(|row| row.task_id));
    let related: HashMap<Uuid, tasks::Model> = if related_ids.is_empty() {
        HashMap::new()
    } else {
        tasks::Entity::find()
            .filter(tasks::Column::Id.is_in(related_ids))
            .filter(tasks::Column::DeletedAt.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(|model| (model.id, model))
            .collect()
    };
    let status_ids: HashSet<Uuid> = related.values().map(|model| model.status_id).collect();
    let statuses: HashMap<Uuid, (String, bool)> = if status_ids.is_empty() {
        HashMap::new()
    } else {
        project_statuses::Entity::find()
            .filter(project_statuses::Column::Id.is_in(status_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|status| {
                (
                    status.id,
                    (status.name, status.category == "done"),
                )
            })
            .collect()
    };
    let to_view = |dependency_id: Uuid, model: &tasks::Model| {
        let (status_name, is_done) = statuses
            .get(&model.status_id)
            .cloned()
            .unwrap_or((String::new(), false));
        TaskRefView {
            dependency_id,
            task_id: model.id,
            task_key: model.task_key.clone(),
            title: model.title.clone(),
            status_name,
            is_done,
        }
    };
    Ok(TaskDependenciesResponse {
        blocked_by: blocked_by_rows
            .iter()
            .filter_map(|row| related.get(&row.depends_on_task_id).map(|m| to_view(row.id, m)))
            .collect(),
        blocks: blocks_rows
            .iter()
            .filter_map(|row| related.get(&row.task_id).map(|m| to_view(row.id, m)))
            .collect(),
    })
}

/// 新增依赖:仅同项目、禁自依赖与重复,项目内全量边内存 BFS 防环。
pub async fn add_dependency(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    request: AddDependencyRequest,
) -> Result<TaskDependenciesResponse, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    ensure_project_open_by_id(db, task.project_id).await?;
    let depends_on = find_task(db, request.depends_on_task_key.trim(), false).await?;
    if task.id == depends_on.id {
        return Err(TaskError::InvalidInput("任务不能依赖自己".to_owned()));
    }
    if task.project_id != depends_on.project_id {
        return Err(TaskError::InvalidInput(
            "依赖的任务必须属于当前项目".to_owned(),
        ));
    }
    let duplicate = task_dependencies::Entity::find()
        .filter(task_dependencies::Column::TaskId.eq(task.id))
        .filter(task_dependencies::Column::DependsOnTaskId.eq(depends_on.id))
        .one(db)
        .await?;
    if duplicate.is_some() {
        return Err(TaskError::Conflict("依赖关系已存在".to_owned()));
    }
    if creates_cycle(db, task.id, depends_on.id).await? {
        return Err(TaskError::Conflict(
            "检测到循环依赖：该任务已(间接)阻塞当前任务".to_owned(),
        ));
    }
    let txn = db.begin().await?;
    task_dependencies::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_id: Set(task.id),
        depends_on_task_id: Set(depends_on.id),
        created_by: Set(current_user.user_id),
        created_at: Set(Utc::now()),
    }
    .insert(&txn)
    .await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &task,
        "dependency_added",
        format!("为任务 {} 添加依赖 {}", task.task_key, depends_on.task_key),
        json!({ "depends_on_task_key": depends_on.task_key }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_dependencies(db, current_user, task_key).await
}

pub async fn remove_dependency(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
    dependency_id: Uuid,
) -> Result<TaskDependenciesResponse, TaskError> {
    let task = find_task(db, task_key, false).await?;
    require_write_role(db, current_user, task.project_id).await?;
    let dependency = task_dependencies::Entity::find_by_id(dependency_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    if dependency.task_id != task.id {
        return Err(TaskError::NotFound);
    }
    let depends_on = tasks::Entity::find_by_id(dependency.depends_on_task_id)
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    let txn = db.begin().await?;
    task_dependencies::Entity::delete_by_id(dependency_id)
        .exec(&txn)
        .await?;
    write_task_log(
        &txn,
        current_user.user_id,
        &task,
        "dependency_removed",
        format!(
            "移除任务 {} 对 {} 的依赖",
            task.task_key, depends_on.task_key
        ),
        json!({ "depends_on_task_key": depends_on.task_key }),
        None,
    )
    .await?;
    txn.commit().await?;
    list_dependencies(db, current_user, task_key).await
}

/// 防环:从目标依赖出发沿 depends_on 方向遍历,若能回到当前任务说明成环。
/// 项目内全量依赖边一次拉取后在内存做 BFS,规模可控。
async fn creates_cycle(
    db: &DatabaseConnection,
    task_id: Uuid,
    depends_on_id: Uuid,
) -> Result<bool, TaskError> {
    #[derive(Debug, FromQueryResult)]
    struct DependencyEdge {
        task_id: Uuid,
        depends_on_task_id: Uuid,
    }
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT td.task_id, td.depends_on_task_id FROM task_dependencies td JOIN tasks t ON t.id = td.task_id WHERE t.project_id = (SELECT project_id FROM tasks WHERE id = $1)",
        [task_id.into()],
    );
    let edges = DependencyEdge::find_by_statement(statement)
        .all(db)
        .await?;
    let mut graph: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for edge in edges {
        graph
            .entry(edge.task_id)
            .or_default()
            .push(edge.depends_on_task_id);
    }
    let mut queue: VecDeque<Uuid> = VecDeque::from([depends_on_id]);
    let mut visited: HashSet<Uuid> = HashSet::from([depends_on_id]);
    while let Some(current) = queue.pop_front() {
        if current == task_id {
            return Ok(true);
        }
        if let Some(next_ids) = graph.get(&current) {
            for next in next_ids {
                if visited.insert(*next) {
                    queue.push_back(*next);
                }
            }
        }
    }
    Ok(false)
}

// ---------- 任务复制 ----------

/// 复制任务:字段 + 标签 + 里程碑关联随行,评论/附件/依赖/子任务不复制;
/// 副本总是根任务,落到项目默认状态列末尾,操作者成为创建人。
pub async fn copy_task(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    task_key: &str,
) -> Result<TaskView, TaskError> {
    let source = find_task(db, task_key, false).await?;
    let role = require_write_role(db, current_user, source.project_id).await?;
    let project = projects::Entity::find_by_id(source.project_id)
        .filter(projects::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(TaskError::NotFound)?;
    ensure_project_open(&project)?;
    let txn = db.begin().await?;
    let status = find_status_for_create(&txn, project.id, None).await?;
    if status.category == "done" && !role.can_manage_project() {
        return Err(TaskError::ForbiddenAction(
            "项目默认状态为已完成时,仅项目管理员可以复制".to_owned(),
        ));
    }
    let task_number = next_task_number(&txn, project.id).await?;
    let position = next_position_in_column(&txn, project.id, status.id).await?;
    let now = Utc::now();
    let copy = tasks::ActiveModel {
        id: Set(Uuid::now_v7()),
        task_key: Set(format!("{}-{}", project.project_key, task_number)),
        project_id: Set(project.id),
        parent_task_id: Set(None),
        status_id: Set(status.id),
        position: Set(position),
        milestone_id: Set(source.milestone_id),
        title: Set(source.title.clone()),
        description: Set(source.description.clone()),
        priority: Set(source.priority.clone()),
        task_type: Set(source.task_type.clone()),
        reporter_id: Set(current_user.user_id),
        assignee_id: Set(source.assignee_id),
        reviewer_id: Set(source.reviewer_id),
        start_at: Set(source.start_at),
        due_at: Set(source.due_at),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        deleted_by: Set(None),
        delete_reason: Set(None),
        task_number: Set(task_number),
    }
    .insert(&txn)
    .await?;
    let label_rows = task_labels::Entity::find()
        .filter(task_labels::Column::TaskId.eq(source.id))
        .all(&txn)
        .await?;
    for row in label_rows {
        task_labels::ActiveModel {
            task_id: Set(copy.id),
            label_id: Set(row.label_id),
            created_at: Set(now),
        }
        .insert(&txn)
        .await?;
    }
    // 副本沿用了原负责人,与其新建任务同等待遇,分配即通知。
    if let Some(assignee_id) = copy.assignee_id {
        let actor_name = actor_display_name(db, current_user.user_id).await?;
        notify_task_event(
            &txn,
            &[assignee_id],
            current_user,
            &actor_name,
            &copy,
            &project.project_key,
            notification_service::KIND_ASSIGNED,
        )
        .await?;
    }
    write_task_log(
        &txn,
        current_user.user_id,
        &copy,
        "copy",
        format!("复制任务 {} 为 {}", source.task_key, copy.task_key),
        json!({ "source_task_key": source.task_key }),
        None,
    )
    .await?;
    txn.commit().await?;
    let mut view = TaskView::from(copy);
    hydrate_views(db, std::slice::from_mut(&mut view)).await?;
    Ok(view)
}

// ---------- 回收站 ----------

#[derive(Debug, Serialize)]
pub struct DeletedTaskItem {
    pub id: Uuid,
    pub task_key: String,
    pub title: String,
    pub status_name: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_name: Option<String>,
    pub delete_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeletedTaskListResponse {
    pub items: Vec<DeletedTaskItem>,
}

/// 已逻辑删除的任务清单,供回收站展示与恢复(恢复走既有 restore 用例)。
pub async fn list_deleted_tasks(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<DeletedTaskListResponse, TaskError> {
    let project = find_project(db, project_key).await?;
    require_read_role(db, current_user, project.id).await?;
    let rows = tasks::Entity::find()
        .filter(tasks::Column::ProjectId.eq(project.id))
        .filter(tasks::Column::DeletedAt.is_not_null())
        .order_by_desc(tasks::Column::DeletedAt)
        .limit(200)
        .all(db)
        .await?;
    let user_ids: HashSet<Uuid> = rows.iter().filter_map(|row| row.deleted_by).collect();
    let status_ids: HashSet<Uuid> = rows.iter().map(|row| row.status_id).collect();
    let user_names: HashMap<Uuid, String> = if user_ids.is_empty() {
        HashMap::new()
    } else {
        users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|user| (user.id, user.display_name))
            .collect()
    };
    let status_names: HashMap<Uuid, String> = if status_ids.is_empty() {
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
    let items = rows
        .into_iter()
        .map(|row| DeletedTaskItem {
            status_name: status_names.get(&row.status_id).cloned().unwrap_or_default(),
            deleted_by_name: row.deleted_by.and_then(|id| user_names.get(&id).cloned()),
            id: row.id,
            task_key: row.task_key,
            title: row.title,
            deleted_at: row.deleted_at,
            delete_reason: row.delete_reason,
        })
        .collect();
    Ok(DeletedTaskListResponse { items })
}

// ---------- 项目级依赖边(甘特图用) ----------

#[derive(Debug, Serialize)]
pub struct ProjectDependencyEdge {
    pub dependency_id: Uuid,
    /// 被阻塞任务。
    pub task_key: String,
    /// 阻塞方任务。
    pub depends_on_task_key: String,
    pub depends_on_title: String,
    pub is_done: bool,
}

#[derive(Debug, Serialize)]
pub struct ProjectDependencyListResponse {
    pub items: Vec<ProjectDependencyEdge>,
}

/// 项目内全部依赖边一次拉取,甘特图连线与列表都复用。
pub async fn list_project_dependencies(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
) -> Result<ProjectDependencyListResponse, TaskError> {
    let project = find_project(db, project_key).await?;
    require_read_role(db, current_user, project.id).await?;
    #[derive(Debug, FromQueryResult)]
    struct DependencyRow {
        id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    }
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT td.id, td.task_id, td.depends_on_task_id FROM task_dependencies td JOIN tasks t ON t.id = td.task_id WHERE t.project_id = $1",
        [project.id.into()],
    );
    let rows = DependencyRow::find_by_statement(statement).all(db).await?;
    let mut task_ids: Vec<Uuid> = rows.iter().map(|row| row.task_id).collect();
    task_ids.extend(rows.iter().map(|row| row.depends_on_task_id));
    let related: HashMap<Uuid, tasks::Model> = if task_ids.is_empty() {
        HashMap::new()
    } else {
        tasks::Entity::find()
            .filter(tasks::Column::Id.is_in(task_ids))
            .filter(tasks::Column::DeletedAt.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(|model| (model.id, model))
            .collect()
    };
    let status_ids: HashSet<Uuid> = related.values().map(|model| model.status_id).collect();
    let done_status: HashSet<Uuid> = if status_ids.is_empty() {
        HashSet::new()
    } else {
        project_statuses::Entity::find()
            .filter(project_statuses::Column::Id.is_in(status_ids))
            .filter(project_statuses::Column::Category.eq("done"))
            .all(db)
            .await?
            .into_iter()
            .map(|status| status.id)
            .collect()
    };
    let items = rows
        .into_iter()
        .filter_map(|row| {
            let blocked = related.get(&row.task_id)?;
            let blocker = related.get(&row.depends_on_task_id)?;
            Some(ProjectDependencyEdge {
                dependency_id: row.id,
                task_key: blocked.task_key.clone(),
                depends_on_task_key: blocker.task_key.clone(),
                depends_on_title: blocker.title.clone(),
                is_done: done_status.contains(&blocker.status_id),
            })
        })
        .collect();
    Ok(ProjectDependencyListResponse { items })
}

// ---------- 任务导出 ----------

fn csv_escape(value: String) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

/// 按当前筛选全量导出项目任务为 CSV,列含状态/负责人/里程碑/标签等展示名。
pub async fn export_project_tasks_csv(
    db: &DatabaseConnection,
    current_user: &CurrentUser,
    project_key: &str,
    query: &ListTasksQuery,
) -> Result<String, TaskError> {
    let project = find_project(db, project_key).await?;
    require_read_role(db, current_user, project.id).await?;
    let statement = build_task_statement(db, &project, query).await?;
    let mut views: Vec<TaskView> = statement
        .order_by_desc(tasks::Column::CreatedAt)
        .order_by_desc(tasks::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .map(TaskView::from)
        .collect();
    hydrate_views(db, &mut views).await?;
    // 里程碑名与状态名按 id 映射补齐。
    let milestone_ids: HashSet<Uuid> = views.iter().filter_map(|view| view.milestone_id).collect();
    let milestone_names: HashMap<Uuid, String> = if milestone_ids.is_empty() {
        HashMap::new()
    } else {
        milestones::Entity::find()
            .filter(milestones::Column::Id.is_in(milestone_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|milestone| (milestone.id, milestone.name))
            .collect()
    };
    let status_ids: HashSet<Uuid> = views.iter().map(|view| view.status_id).collect();
    let status_names: HashMap<Uuid, String> = if status_ids.is_empty() {
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
    let mut csv = String::from("task_key,title,status,assignee,reviewer,priority,task_type,milestone,labels,start_at,due_at,created_at,updated_at\n");
    for view in views {
        let labels = view
            .labels
            .iter()
            .map(|label| label.name.clone())
            .collect::<Vec<_>>()
            .join("|");
        csv.push_str(
            &[
                view.task_key,
                view.title,
                status_names.get(&view.status_id).cloned().unwrap_or_default(),
                view.assignee_name.unwrap_or_default(),
                view.reviewer_name.unwrap_or_default(),
                view.priority,
                view.task_type,
                view.milestone_id
                    .and_then(|id| milestone_names.get(&id).cloned())
                    .unwrap_or_default(),
                labels,
                view.start_at.map(|value| value.to_rfc3339()).unwrap_or_default(),
                view.due_at.map(|value| value.to_rfc3339()).unwrap_or_default(),
                view.created_at.to_rfc3339(),
                view.updated_at.to_rfc3339(),
            ]
            .into_iter()
            .map(csv_escape)
            .collect::<Vec<_>>()
            .join(","),
        );
        csv.push('\n');
    }
    Ok(csv)
}
