use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionMarker {
    pub deleted_at: DateTime<Utc>,
    pub deleted_by: Uuid,
    pub delete_reason: Option<String>,
}
impl DeletionMarker {
    pub fn now(deleted_by: Uuid, delete_reason: Option<String>) -> Self {
        Self {
            deleted_at: Utc::now(),
            deleted_by,
            delete_reason,
        }
    }
}
