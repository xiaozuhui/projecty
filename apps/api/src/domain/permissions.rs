use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemRole {
    SuperAdmin,
    User,
}
impl SystemRole {
    pub fn is_super_admin(self) -> bool {
        matches!(self, Self::SuperAdmin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Manager,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveProjectRole {
    SuperAdmin,
    Manager,
    Member,
    Viewer,
    None,
}
impl EffectiveProjectRole {
    pub fn strength(self) -> u8 {
        match self {
            Self::SuperAdmin => 4,
            Self::Manager => 3,
            Self::Member => 2,
            Self::Viewer => 1,
            Self::None => 0,
        }
    }
    pub fn can_read_project(self) -> bool {
        self.strength() >= Self::Viewer.strength()
    }
    pub fn can_create_task(self) -> bool {
        self.strength() >= Self::Member.strength()
    }
    pub fn can_delete_task(self) -> bool {
        self.strength() >= Self::Member.strength()
    }
    pub fn can_change_task_status(self) -> bool {
        self.strength() >= Self::Member.strength()
    }
    pub fn can_manage_project(self) -> bool {
        self.strength() >= Self::Manager.strength()
    }
    pub fn strongest(a: Self, b: Self) -> Self {
        if a.strength() >= b.strength() {
            a
        } else {
            b
        }
    }
}
impl From<ProjectRole> for EffectiveProjectRole {
    fn from(value: ProjectRole) -> Self {
        match value {
            ProjectRole::Manager => Self::Manager,
            ProjectRole::Member => Self::Member,
            ProjectRole::Viewer => Self::Viewer,
        }
    }
}
