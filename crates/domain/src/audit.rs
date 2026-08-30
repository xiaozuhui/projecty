use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationModule {
    Auth,
    Department,
    Project,
    ProjectMember,
    Task,
    Comment,
    Milestone,
    Audit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationAction {
    Create,
    Update,
    LogicalDelete,
    Restore,
    Archive,
    Revoke,
    StatusTransition,
    Export,
    Login,
    Logout,
}
impl OperationAction {
    pub fn must_be_transactional_for_task(self) -> bool {
        matches!(
            self,
            Self::Create | Self::LogicalDelete | Self::Restore | Self::StatusTransition
        )
    }
}
