use sea_orm::{ActiveValue, EntityTrait};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use crate::entities::{gamification, prelude::Gamification};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum GamificationTable {
    #[sea_orm(iden = "gamification")]
    Table,
    Id,
    Xp,
    Level,
    CurrentStreak,
    LongestStreak,
    TotalCompleted,
    LastCompletionDate,
}

#[derive(DeriveIden)]
enum AchievementsUnlocked {
    Table,
    Name,
    UnlockedAt,
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
                    .table(GamificationTable::Table)
                    .if_not_exists()
                    .col(
                        big_integer(GamificationTable::Id)
                            .primary_key()
                            .check(Expr::col(GamificationTable::Id).eq(1)),
                    )
                    .col(big_integer(GamificationTable::Xp).default(0))
                    .col(big_integer(GamificationTable::Level).default(1))
                    .col(big_integer(GamificationTable::CurrentStreak).default(0))
                    .col(big_integer(GamificationTable::LongestStreak).default(0))
                    .col(big_integer(GamificationTable::TotalCompleted).default(0))
                    .col(string_null(GamificationTable::LastCompletionDate))
                    .to_owned(),
            )
            .await?;

        // Seed the singleton row through the entity API rather than INSERT SQL.
        let conn = manager.get_connection();
        Gamification::insert(gamification::ActiveModel {
            id: ActiveValue::Set(1),
            xp: ActiveValue::Set(0),
            level: ActiveValue::Set(1),
            current_streak: ActiveValue::Set(0),
            longest_streak: ActiveValue::Set(0),
            total_completed: ActiveValue::Set(0),
            last_completion_date: ActiveValue::Set(None),
        })
        .on_conflict(
            sea_query::OnConflict::column(gamification::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(conn)
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(AchievementsUnlocked::Table)
                    .if_not_exists()
                    .col(string(AchievementsUnlocked::Name).primary_key())
                    .col(string(AchievementsUnlocked::UnlockedAt).default(now_default()))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for tbl in [
            AchievementsUnlocked::Table.into_iden(),
            GamificationTable::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(tbl).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
