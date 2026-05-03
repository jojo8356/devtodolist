use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "task_labels")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub label_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Task,
    Label,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Task => Entity::belongs_to(super::task::Entity)
                .from(Column::TaskId)
                .to(super::task::Column::Id)
                .into(),
            Self::Label => Entity::belongs_to(super::label::Entity)
                .from(Column::LabelId)
                .to(super::label::Column::Id)
                .into(),
        }
    }
}

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Task.def()
    }
}

impl Related<super::label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Label.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
