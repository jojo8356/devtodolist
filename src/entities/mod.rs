//! SeaORM entity definitions for every persisted table.
//!
//! Schema-of-record lives here: each entity declares its columns, primary key,
//! and relations. The DB-side schema is created by the `migration` module, and
//! `db.rs` orchestrates the actual queries against these entities.

pub mod achievement_unlocked;
pub mod comment;
pub mod dev_role;
pub mod gamification;
pub mod label;
pub mod reviewer;
pub mod task;
pub mod task_commit;
pub mod task_dependency;
pub mod task_label;

pub mod prelude {
    pub use super::achievement_unlocked::Entity as AchievementUnlocked;
    pub use super::comment::Entity as Comment;
    pub use super::dev_role::Entity as DevRole;
    pub use super::gamification::Entity as Gamification;
    pub use super::label::Entity as Label;
    pub use super::reviewer::Entity as Reviewer;
    pub use super::task::Entity as Task;
    pub use super::task_commit::Entity as TaskCommit;
    pub use super::task_dependency::Entity as TaskDependency;
    pub use super::task_label::Entity as TaskLabel;
}
