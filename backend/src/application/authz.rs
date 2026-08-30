use crate::domain::permissions::{EffectiveProjectRole, ProjectRole, SystemRole};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ProjectRoleInputs {
    pub system_role: SystemRole,
    pub direct_project_role: Option<ProjectRole>,
    /// 部门层级展开后命中的最高部门授权角色。部门授权只允许 member/viewer，不授予 manager。
    pub department_grant_role: Option<ProjectRole>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AuthorizationError {
    #[error("没有项目读取权限")]
    CannotReadProject,
    #[error("没有任务写入权限")]
    CannotWriteTask,
    #[error("没有项目管理权限")]
    CannotManageProject,
    #[error("只有超级管理员可以执行该操作")]
    SuperAdminRequired,
}

pub fn compute_effective_project_role(input: &ProjectRoleInputs) -> EffectiveProjectRole {
    if input.system_role.is_super_admin() {
        return EffectiveProjectRole::SuperAdmin;
    }
    let direct = input
        .direct_project_role
        .map(EffectiveProjectRole::from)
        .unwrap_or(EffectiveProjectRole::None);
    let department = input
        .department_grant_role
        .map(EffectiveProjectRole::from)
        .unwrap_or(EffectiveProjectRole::None);
    EffectiveProjectRole::strongest(direct, department)
}

pub fn require_project_read(role: EffectiveProjectRole) -> Result<(), AuthorizationError> {
    role.can_read_project()
        .then_some(())
        .ok_or(AuthorizationError::CannotReadProject)
}
pub fn require_task_write(role: EffectiveProjectRole) -> Result<(), AuthorizationError> {
    (role.can_create_task() && role.can_delete_task() && role.can_change_task_status())
        .then_some(())
        .ok_or(AuthorizationError::CannotWriteTask)
}
pub fn require_project_manager(role: EffectiveProjectRole) -> Result<(), AuthorizationError> {
    role.can_manage_project()
        .then_some(())
        .ok_or(AuthorizationError::CannotManageProject)
}
pub fn require_super_admin(system_role: SystemRole) -> Result<(), AuthorizationError> {
    system_role
        .is_super_admin()
        .then_some(())
        .ok_or(AuthorizationError::SuperAdminRequired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_admin_bypasses_project_membership() {
        let role = compute_effective_project_role(&ProjectRoleInputs {
            system_role: SystemRole::SuperAdmin,
            direct_project_role: None,
            department_grant_role: None,
        });
        assert_eq!(role, EffectiveProjectRole::SuperAdmin);
    }

    #[test]
    fn direct_manager_wins_over_department_viewer() {
        let role = compute_effective_project_role(&ProjectRoleInputs {
            system_role: SystemRole::User,
            direct_project_role: Some(ProjectRole::Manager),
            department_grant_role: Some(ProjectRole::Viewer),
        });
        assert_eq!(role, EffectiveProjectRole::Manager);
    }

    #[test]
    fn viewer_cannot_write_tasks() {
        assert_eq!(
            require_task_write(EffectiveProjectRole::Viewer),
            Err(AuthorizationError::CannotWriteTask)
        );
    }
}
