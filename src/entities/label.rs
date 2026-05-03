use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "labels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub name: String,
    pub color: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    TaskLabel,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::TaskLabel => Entity::has_many(super::task_label::Entity).into(),
        }
    }
}

impl Related<super::task_label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TaskLabel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
