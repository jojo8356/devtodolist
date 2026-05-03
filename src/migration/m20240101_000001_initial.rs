use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
    Title,
    Description,
    Status,
    Priority,
    Branch,
    BaseBranch,
    Provider,
    RemoteId,
    SourceUrl,
    Assignee,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Labels {
    Table,
    Id,
    Name,
    Color,
}

#[derive(DeriveIden)]
enum TaskLabels {
    Table,
    TaskId,
    LabelId,
}

#[derive(DeriveIden)]
enum Reviewers {
    Table,
    Id,
    TaskId,
    Username,
    Status,
    ReviewedAt,
}

#[derive(DeriveIden)]
enum Comments {
    Table,
    Id,
    TaskId,
    Author,
    Body,
    RemoteId,
    CreatedAt,
}

/// SQLite needs `strftime` to seed default timestamps; this returns the
/// Expr we plug into `.default()` for the relevant columns.
fn now_default() -> sea_query::SimpleExpr {
    Expr::cust("(strftime('%Y-%m-%dT%H:%M:%S', 'now'))")
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tasks::Table)
                    .if_not_exists()
                    .col(pk_auto(Tasks::Id))
                    .col(string(Tasks::Title))
                    .col(string_null(Tasks::Description))
                    .col(string(Tasks::Status).default("draft"))
                    .col(string_null(Tasks::Priority))
                    .col(string_null(Tasks::Branch))
                    .col(string_null(Tasks::BaseBranch))
                    .col(string_null(Tasks::Provider))
                    .col(big_integer_null(Tasks::RemoteId))
                    .col(string_null(Tasks::SourceUrl))
                    .col(string_null(Tasks::Assignee))
                    .col(string(Tasks::CreatedAt).default(now_default()))
                    .col(string(Tasks::UpdatedAt).default(now_default()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Labels::Table)
                    .if_not_exists()
                    .col(pk_auto(Labels::Id))
                    .col(string_uniq(Labels::Name))
                    .col(string_null(Labels::Color))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TaskLabels::Table)
                    .if_not_exists()
                    .col(big_integer(TaskLabels::TaskId))
                    .col(big_integer(TaskLabels::LabelId))
                    .primary_key(
                        Index::create()
                            .col(TaskLabels::TaskId)
                            .col(TaskLabels::LabelId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskLabels::Table, TaskLabels::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskLabels::Table, TaskLabels::LabelId)
                            .to(Labels::Table, Labels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Reviewers::Table)
                    .if_not_exists()
                    .col(pk_auto(Reviewers::Id))
                    .col(big_integer(Reviewers::TaskId))
                    .col(string(Reviewers::Username))
                    .col(string(Reviewers::Status).default("pending"))
                    .col(string_null(Reviewers::ReviewedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Reviewers::Table, Reviewers::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Comments::Table)
                    .if_not_exists()
                    .col(pk_auto(Comments::Id))
                    .col(big_integer(Comments::TaskId))
                    .col(string(Comments::Author))
                    .col(string(Comments::Body))
                    .col(big_integer_null(Comments::RemoteId))
                    .col(string(Comments::CreatedAt).default(now_default()))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Comments::Table, Comments::TaskId)
                            .to(Tasks::Table, Tasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for tbl in [
            Comments::Table.into_iden(),
            Reviewers::Table.into_iden(),
            TaskLabels::Table.into_iden(),
            Labels::Table.into_iden(),
            Tasks::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(tbl).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
