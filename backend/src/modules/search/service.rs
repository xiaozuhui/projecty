use crate::http::extractors::CurrentUser;
use projecty_entity::{projects, tasks, users};
use sea_orm::{
    ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("数据库错误：{0}")]
    Database(#[from] sea_orm::DbErr),
}
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub projects: Vec<projects::Model>,
    pub tasks: Vec<tasks::Model>,
    pub users: Vec<UserView>,
}
#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub account: String,
    pub display_name: String,
}
#[derive(Debug, FromQueryResult)]
struct ProjectId {
    id: Uuid,
}

pub async fn search(
    db: &DatabaseConnection,
    user: &CurrentUser,
    query: &SearchQuery,
) -> Result<SearchResult, SearchError> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(SearchResult {
            projects: vec![],
            tasks: vec![],
            users: vec![],
        });
    }
    let pattern = format!("%{}%", q);
    let ids = if user.system_role.is_super_admin() {
        None
    } else {
        Some(visible_project_ids(db, user.user_id).await?)
    };
    if ids.as_ref().is_some_and(Vec::is_empty) {
        return Ok(SearchResult {
            projects: vec![],
            tasks: vec![],
            users: vec![],
        });
    }
    let mut project_query = projects::Entity::find()
        .filter(projects::Column::DeletedAt.is_null())
        .filter(
            projects::Column::ProjectKey
                .like(&pattern)
                .or(projects::Column::Name.like(&pattern)),
        )
        .order_by_desc(projects::Column::UpdatedAt)
        .limit(20);
    let mut task_query = tasks::Entity::find()
        .filter(tasks::Column::DeletedAt.is_null())
        .filter(
            tasks::Column::TaskKey
                .like(&pattern)
                .or(tasks::Column::Title.like(&pattern)),
        )
        .order_by_desc(tasks::Column::UpdatedAt)
        .limit(50);
    if let Some(project_ids) = &ids {
        project_query = project_query.filter(projects::Column::Id.is_in(project_ids.clone()));
        task_query = task_query.filter(tasks::Column::ProjectId.is_in(project_ids.clone()));
    }
    let projects = project_query.all(db).await?;
    let tasks = task_query.all(db).await?;
    let users = if user.system_role.is_super_admin() {
        users::Entity::find()
            .filter(users::Column::IsActive.eq(true))
            .filter(users::Column::DeletedAt.is_null())
            .filter(
                users::Column::Account
                    .like(&pattern)
                    .or(users::Column::DisplayName.like(&pattern)),
            )
            .limit(20)
            .all(db)
            .await?
            .into_iter()
            .map(|u| UserView {
                id: u.id,
                account: u.account,
                display_name: u.display_name,
            })
            .collect()
    } else {
        vec![]
    };
    Ok(SearchResult {
        projects,
        tasks,
        users,
    })
}
async fn visible_project_ids(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Vec<Uuid>, SearchError> {
    Ok(ProjectId::find_by_statement(Statement::from_sql_and_values(DatabaseBackend::Postgres,"SELECT DISTINCT project_id AS id FROM project_members WHERE user_id = $1 AND revoked_at IS NULL UNION SELECT DISTINCT pdg.project_id AS id FROM project_department_grants pdg JOIN department_closure dc ON dc.ancestor_id = pdg.department_id JOIN user_departments ud ON ud.department_id = dc.descendant_id WHERE ud.user_id = $1 AND ud.revoked_at IS NULL AND pdg.revoked_at IS NULL",[user_id.into()])).all(db).await?.into_iter().map(|r|r.id).collect())
}
