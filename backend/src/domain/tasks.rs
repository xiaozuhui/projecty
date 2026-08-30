use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Task,
    Subtask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskParent {
    pub parent_task_id: Option<Uuid>,
    pub parent_already_has_parent: bool,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TaskRuleError {
    #[error("子任务不能再创建子任务")]
    SubtaskCannotHaveSubtask,
}

/// 验证 Projecty v1 的两层任务约束：任务 -> 子任务，子任务不能再有子任务。
pub fn validate_two_level_task_rule(parent: &NewTaskParent) -> Result<TaskKind, TaskRuleError> {
    match (parent.parent_task_id, parent.parent_already_has_parent) {
        (None, _) => Ok(TaskKind::Task),
        (Some(_), false) => Ok(TaskKind::Subtask),
        (Some(_), true) => Err(TaskRuleError::SubtaskCannotHaveSubtask),
    }
}
