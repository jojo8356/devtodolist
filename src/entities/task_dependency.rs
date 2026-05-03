use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "task_dependencies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub depends_on: i64,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    /// The dependent (`task_id` side).
    Task,
    /// The blocker (`depends_on` side).
    Blocker,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Task => Entity::belongs_to(super::task::Entity)
                .from(Column::TaskId)
                .to(super::task::Column::Id)
                .into(),
            Self::Blocker => Entity::belongs_to(super::task::Entity)
                .from(Column::DependsOn)
                .to(super::task::Column::Id)
                .into(),
        }
    }
}

// Default `Related<task::Entity>` chooses the "Task" side; for the blocker side
// we pass an explicit relation in queries.
impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
