use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TaskDependencies {
    Table,
    TaskId,
    DependsOn,
    CreatedAt,
}

#[derive(DeriveIden)]
enum DevRoles {
    Table,
    Username,
    Role,
}

#[derive(DeriveIden)]
enum TaskCommits {
    Table,
    TaskId,
    CommitHash,
    ShortHash,
    Author,
    Message,
    CommittedAt,
    AddedAt,
}

#[derive(DeriveIden)]
enum DepsIdx {
    #[sea_orm(iden = "idx_deps_task")]
    Task,
    #[sea_orm(iden = "idx_deps_blocking")]
    Blocking,
}

#[derive(DeriveIden)]
enum CommitsIdx {
    #[sea_orm(iden = "idx_commits_task")]
    Task,
}

fn now_default() -> sea_query::SimpleExpr {
    Expr::cust("(strftime('%Y-%m-%dT%H:%M:%S', 'now'))")
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskDependencies::Table)
                    .if_not_exists()
                    .col(big_integer(TaskDependencies::TaskId))
                    .col(big_integer(TaskDependencies::DependsOn))
                    .col(string(TaskDependencies::CreatedAt).default(now_default()))
                    .primary_key(
                        Index::create()
                            .col(TaskDependencies::TaskId)
                            .col(TaskDependencies::DependsOn),
                    )
                    .check(
                        Expr::col(TaskDependencies::TaskId)
                            .ne(Expr::col(TaskDependencies::DependsOn)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskDependencies::Table, TaskDependencies::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskDependencies::Table, TaskDependencies::DependsOn)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(DepsIdx::Task.to_string())
                    .table(TaskDependencies::Table)
                    .col(TaskDependencies::TaskId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(DepsIdx::Blocking.to_string())
                    .table(TaskDependencies::Table)
                    .col(TaskDependencies::DependsOn)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(DevRoles::Table)
                    .if_not_exists()
                    .col(string(DevRoles::Username).primary_key())
                    .col(string(DevRoles::Role))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TaskCommits::Table)
                    .if_not_exists()
                    .col(big_integer(TaskCommits::TaskId))
                    .col(string(TaskCommits::CommitHash))
                    .col(string_null(TaskCommits::ShortHash))
                    .col(string_null(TaskCommits::Author))
                    .col(string_null(TaskCommits::Message))
                    .col(string_null(TaskCommits::CommittedAt))
                    .col(string(TaskCommits::AddedAt).default(now_default()))
                    .primary_key(
                        Index::create()
                            .col(TaskCommits::TaskId)
                            .col(TaskCommits::CommitHash),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskCommits::Table, TaskCommits::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(CommitsIdx::Task.to_string())
                    .table(TaskCommits::Table)
                    .col(TaskCommits::TaskId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for tbl in [
            TaskCommits::Table.into_iden(),
            DevRoles::Table.into_iden(),
            TaskDependencies::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(tbl).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
