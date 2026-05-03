use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "gamification")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub xp: i64,
    pub level: i64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub total_completed: i64,
    /// Stored as `YYYY-MM-DD` string to keep migration semantics identical
    /// to the original rusqlite schema.
    pub last_completion_date: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        unreachable!()
    }
}

impl ActiveModelBehavior for ActiveModel {}
