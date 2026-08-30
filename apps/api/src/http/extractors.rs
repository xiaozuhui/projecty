use crate::domain::permissions::SystemRole;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub account: String,
    pub system_role: SystemRole,
}
impl CurrentUser {
    pub fn dev_user() -> Self {
        Self {
            user_id: Uuid::nil(),
            account: "dev".to_owned(),
            system_role: SystemRole::SuperAdmin,
        }
    }
}
