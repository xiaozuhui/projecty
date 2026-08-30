use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "department_closure")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ancestor_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub descendant_id: Uuid,
    pub depth: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
