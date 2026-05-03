//! SeaORM migrator. Each migration is append-only — never edit one once shipped.

pub use sea_orm_migration::prelude::*;

mod m20240101_000001_initial;
mod m20240101_000002_gamification;
mod m20240101_000003_deps_roles_proofs;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_initial::Migration),
            Box::new(m20240101_000002_gamification::Migration),
            Box::new(m20240101_000003_deps_roles_proofs::Migration),
        ]
    }
}
