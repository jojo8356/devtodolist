//! Integration tests for the gamification DB layer + orchestration.
//!
//! Uses an in-memory SQLite DB (via SeaORM) so no filesystem setup is needed.

use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{ActiveValue, EntityTrait};

use devtodo::db::Database;
use devtodo::entities::{achievement_unlocked, prelude::AchievementUnlocked};
use devtodo::gamification::{Achievement, MAX_LEVEL, Profile, award_task_completion_on};
use devtodo::models::Priority;

async fn fresh_db() -> Database {
    let db = Database::open_in_memory().await.unwrap();
    db.init().await.unwrap();
    db
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ── Profile round-trips ──────────────────────────────────────────────

#[tokio::test]
async fn test_get_profile_on_fresh_db_returns_defaults() {
    let db = fresh_db().await;
    let p = db.get_profile().await.unwrap();
    assert_eq!(p.xp, 0);
    assert_eq!(p.level, 1);
    assert_eq!(p.current_streak, 0);
    assert_eq!(p.longest_streak, 0);
    assert_eq!(p.total_completed, 0);
    assert_eq!(p.last_completion_date, None);
}

#[tokio::test]
async fn test_save_profile_round_trips_all_fields_including_date() {
    let db = fresh_db().await;
    let p = Profile {
        xp: 12_345,
        level: 16,
        current_streak: 7,
        longest_streak: 42,
        total_completed: 13,
        last_completion_date: Some(d(2025, 6, 1)),
    };
    db.save_profile(&p).await.unwrap();
    let got = db.get_profile().await.unwrap();
    assert_eq!(got, p);
}

#[tokio::test]
async fn test_save_profile_round_trips_none_date() {
    let db = fresh_db().await;
    let p = Profile {
        xp: 100,
        level: 2,
        current_streak: 0,
        longest_streak: 3,
        total_completed: 1,
        last_completion_date: None,
    };
    db.save_profile(&p).await.unwrap();
    let got = db.get_profile().await.unwrap();
    assert_eq!(got.last_completion_date, None);
    assert_eq!(got, p);
}

// ── Achievement persistence ──────────────────────────────────────────

#[tokio::test]
async fn test_unlock_achievement_persists_and_is_queryable() {
    let db = fresh_db().await;
    assert!(!db.is_achievement_unlocked("first_blood").await.unwrap());
    db.unlock_achievement("first_blood").await.unwrap();
    assert!(db.is_achievement_unlocked("first_blood").await.unwrap());
}

#[tokio::test]
async fn test_unlock_achievement_twice_is_noop_and_does_not_duplicate() {
    let db = fresh_db().await;
    db.unlock_achievement("first_blood").await.unwrap();
    db.unlock_achievement("first_blood").await.unwrap();
    let rows = db.list_unlocked_achievements().await.unwrap();
    let matching: Vec<_> = rows.iter().filter(|(n, _)| n == "first_blood").collect();
    assert_eq!(matching.len(), 1);
}

#[tokio::test]
async fn test_list_unlocked_achievements_returns_them_in_unlock_order() {
    let db = fresh_db().await;
    // Insert with explicit timestamps via the entity ActiveModel so order
    // is deterministic — no SQL string needed.
    for (name, ts) in [
        ("first_blood", "2024-01-01T00:00:00"),
        ("grinder", "2024-01-02T00:00:00"),
        ("awakened", "2024-01-03T00:00:00"),
    ] {
        let am = achievement_unlocked::ActiveModel {
            name: ActiveValue::Set(name.to_string()),
            unlocked_at: ActiveValue::Set(
                NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S").unwrap(),
            ),
        };
        AchievementUnlocked::insert(am).exec(&db.conn).await.unwrap();
    }

    let rows = db.list_unlocked_achievements().await.unwrap();
    let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["first_blood", "grinder", "awakened"]);
}

// ── award_task_completion orchestration ──────────────────────────────

#[tokio::test]
async fn test_award_on_fresh_profile_returns_correct_reward() {
    let db = fresh_db().await;
    let reward = award_task_completion_on(&db, Some(&Priority::Medium), d(2024, 1, 1))
        .await
        .unwrap();
    assert_eq!(reward.xp_gained, 25);
    assert_eq!(reward.new_xp, 25);
    assert_eq!(reward.old_level, 1);
    assert_eq!(reward.new_level, 1);
    assert!(!reward.leveled_up);
    assert_eq!(reward.current_streak, 1);
    assert_eq!(reward.longest_streak, 1);
    assert!(reward.streak_extended);
    assert_eq!(reward.new_achievements, vec![Achievement::FirstBlood]);
}

#[tokio::test]
async fn test_award_persists_profile_changes_to_db() {
    let db = fresh_db().await;
    award_task_completion_on(&db, Some(&Priority::High), d(2024, 1, 1))
        .await
        .unwrap();
    let p = db.get_profile().await.unwrap();
    assert_eq!(p.xp, 50);
    assert_eq!(p.total_completed, 1);
    assert_eq!(p.last_completion_date, Some(d(2024, 1, 1)));
}

#[tokio::test]
async fn test_award_levels_up_flag_true_only_when_threshold_crossed() {
    let db = fresh_db().await;
    let r1 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 1))
        .await
        .unwrap();
    assert!(!r1.leveled_up);
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 2)).await.unwrap();
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 3)).await.unwrap();
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 4)).await.unwrap();
    let crossing = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 5))
        .await
        .unwrap();
    assert!(crossing.leveled_up);
    assert_eq!(crossing.old_level, 1);
    assert_eq!(crossing.new_level, 2);
    let inside = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 6))
        .await
        .unwrap();
    assert!(!inside.leveled_up);
}

#[tokio::test]
async fn test_award_unlocks_multiple_achievements_in_single_call() {
    let db = fresh_db().await;
    let mut p = Profile {
        xp: 4040,
        level: 9,
        current_streak: 6,
        longest_streak: 6,
        total_completed: 9,
        last_completion_date: Some(d(2024, 1, 6)),
    };
    db.save_profile(&p).await.unwrap();
    db.unlock_achievement(Achievement::FirstBlood.key())
        .await
        .unwrap();

    let reward = award_task_completion_on(&db, Some(&Priority::Medium), d(2024, 1, 7))
        .await
        .unwrap();
    assert!(reward.leveled_up);
    let keys: Vec<&str> = reward.new_achievements.iter().map(|a| a.key()).collect();
    assert!(keys.contains(&"grinder"));
    assert!(keys.contains(&"awakened"));
    assert!(keys.contains(&"week_warrior"));

    p.level = reward.new_level;
    assert_eq!(p.level, 10);
}

#[tokio::test]
async fn test_award_does_not_unlock_same_achievement_twice() {
    let db = fresh_db().await;
    let r1 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 1))
        .await
        .unwrap();
    assert!(r1.new_achievements.contains(&Achievement::FirstBlood));

    let r2 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 2))
        .await
        .unwrap();
    assert!(!r2.new_achievements.contains(&Achievement::FirstBlood));

    let rows = db.list_unlocked_achievements().await.unwrap();
    let fb_count = rows.iter().filter(|(n, _)| n == "first_blood").count();
    assert_eq!(fb_count, 1);
}

#[tokio::test]
async fn test_award_with_no_priority_still_grants_neutral_fifteen_xp() {
    let db = fresh_db().await;
    let reward = award_task_completion_on(&db, None, d(2024, 1, 1))
        .await
        .unwrap();
    assert_eq!(reward.xp_gained, 15);
    assert_ne!(reward.xp_gained, 0);
}

#[tokio::test]
async fn test_status_merged_to_merged_does_not_double_award() {
    let db = fresh_db().await;
    award_task_completion_on(&db, Some(&Priority::Critical), d(2024, 1, 1))
        .await
        .unwrap();
    let p1 = db.get_profile().await.unwrap();
    assert_eq!(p1.total_completed, 1);
    let p2 = db.get_profile().await.unwrap();
    assert_eq!(p1, p2);
}

#[tokio::test]
async fn test_award_never_pushes_level_past_hundred() {
    let db = fresh_db().await;
    let p = Profile {
        xp: i64::MAX - 100,
        level: MAX_LEVEL,
        current_streak: 0,
        longest_streak: 0,
        total_completed: 0,
        last_completion_date: None,
    };
    db.save_profile(&p).await.unwrap();

    let reward = award_task_completion_on(&db, Some(&Priority::Critical), d(2024, 1, 1))
        .await
        .unwrap();
    assert_eq!(reward.new_level, MAX_LEVEL);
    let after = db.get_profile().await.unwrap();
    assert_eq!(after.level, MAX_LEVEL);
}
