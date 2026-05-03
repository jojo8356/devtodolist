use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "task_commits")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub commit_hash: String,
    pub short_hash: Option<String>,
    pub author: Option<String>,
    pub message: Option<String>,
    pub committed_at: Option<String>,
    pub added_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Task,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Task => Entity::belongs_to(super::task::Entity)
                .from(Column::TaskId)
                .to(super::task::Column::Id)
                .into(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
