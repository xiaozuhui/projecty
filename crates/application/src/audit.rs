use projecty_domain::audit::OperationAction;
pub fn task_action_requires_operation_log(action: OperationAction) -> bool {
    action.must_be_transactional_for_task()
}
pub fn export_is_unmasked() -> bool {
    true
}
