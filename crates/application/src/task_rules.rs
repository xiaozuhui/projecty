use projecty_domain::tasks::{
    validate_two_level_task_rule, NewTaskParent, TaskKind, TaskRuleError,
};

pub fn classify_new_task(parent: NewTaskParent) -> Result<TaskKind, TaskRuleError> {
    validate_two_level_task_rule(&parent)
}
pub fn can_delete_parent_task(non_deleted_subtask_count: u64) -> bool {
    non_deleted_subtask_count == 0
}
