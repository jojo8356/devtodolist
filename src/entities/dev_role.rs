use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "dev_roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub username: String,
    pub role: String,
}

// dev_roles is a leaf table from the ORM's relation graph: tasks reference it
// via `assignee == username`, but the `task::Relation::DevRole` definition
// already encodes that direction with `belongs_to`. Defining a `has_many` here
// would force a mirror `Related` impl on `task::Entity`, which we don't need.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
