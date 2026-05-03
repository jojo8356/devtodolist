use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<String>,
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub provider: Option<String>,
    pub remote_id: Option<i64>,
    pub source_url: Option<String>,
    pub assignee: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    /// Bridge to many-to-many `labels` via `task_labels`.
    TaskLabel,
    /// `task.assignee == dev_roles.username`. Logical relation, no FK.
    DevRole,
    /// Reviewers attached to this task.
    Reviewer,
    /// Comments on this task.
    Comment,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::TaskLabel => Entity::has_many(super::task_label::Entity).into(),
            Self::DevRole => Entity::belongs_to(super::dev_role::Entity)
                .from(Column::Assignee)
                .to(super::dev_role::Column::Username)
                .into(),
            Self::Reviewer => Entity::has_many(super::reviewer::Entity).into(),
            Self::Comment => Entity::has_many(super::comment::Entity).into(),
        }
    }
}

impl Related<super::task_label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TaskLabel.def()
    }
}

impl Related<super::reviewer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Reviewer.def()
    }
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comment.def()
    }
}

// `inner_join(task_dependency::Entity)` picks this default relation.
// We use it in two opposite directions (deps and dependents); for the second
// direction we add the WHERE clause manually in db.rs.
impl Related<super::task_dependency::Entity> for Entity {
    fn to() -> RelationDef {
        super::task_dependency::Relation::Task.def().rev()
    }
}

impl ActiveModelBehavior for ActiveModel {}
