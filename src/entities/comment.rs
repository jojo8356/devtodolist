use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub task_id: i64,
    pub author: String,
    pub body: String,
    pub remote_id: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
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

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
