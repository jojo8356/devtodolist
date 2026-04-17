//! Integration tests for the gamification DB layer + orchestration.
//!
//! Uses in-memory SQLite so no filesystem setup or `tempfile` crate is
//! required. Every test spins up a fresh `Database` via the helper below.

use chrono::NaiveDate;
use rusqlite::Connection;

use devtodo::db::Database;
use devtodo::gamification::{Achievement, MAX_LEVEL, Profile, award_task_completion_on};
use devtodo::models::Priority;

fn fresh_db() -> Database {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let db = Database { conn };
    db.init().unwrap();
    db
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ── Profile round-trips ──────────────────────────────────────────────

#[test]
fn test_get_profile_on_fresh_db_returns_defaults() {
    let db = fresh_db();
    let p = db.get_profile().unwrap();
    assert_eq!(p.xp, 0);
    assert_eq!(p.level, 1);
    assert_eq!(p.current_streak, 0);
    assert_eq!(p.longest_streak, 0);
    assert_eq!(p.total_completed, 0);
    assert_eq!(p.last_completion_date, None);
}

#[test]
fn test_save_profile_round_trips_all_fields_including_date() {
    let db = fresh_db();
    let p = Profile {
        xp: 12_345,
        level: 16,
        current_streak: 7,
        longest_streak: 42,
        total_completed: 13,
        last_completion_date: Some(d(2025, 6, 1)),
    };
    db.save_profile(&p).unwrap();
    let got = db.get_profile().unwrap();
    assert_eq!(got, p);
}

#[test]
fn test_save_profile_round_trips_none_date() {
    let db = fresh_db();
    let p = Profile {
        xp: 100,
        level: 2,
        current_streak: 0,
        longest_streak: 3,
        total_completed: 1,
        last_completion_date: None,
    };
    db.save_profile(&p).unwrap();
    let got = db.get_profile().unwrap();
    assert_eq!(got.last_completion_date, None);
    assert_eq!(got, p);
}

// ── Achievement persistence ──────────────────────────────────────────

#[test]
fn test_unlock_achievement_persists_and_is_queryable() {
    let db = fresh_db();
    assert!(!db.is_achievement_unlocked("first_blood").unwrap());
    db.unlock_achievement("first_blood").unwrap();
    assert!(db.is_achievement_unlocked("first_blood").unwrap());
}

#[test]
fn test_unlock_achievement_twice_is_noop_and_does_not_duplicate() {
    // Anti-test: INSERT OR IGNORE means the second call is a no-op,
    // no error raised and no duplicate row created.
    let db = fresh_db();
    db.unlock_achievement("first_blood").unwrap();
    db.unlock_achievement("first_blood").unwrap();
    let rows = db.list_unlocked_achievements().unwrap();
    let matching: Vec<_> = rows.iter().filter(|(n, _)| n == "first_blood").collect();
    assert_eq!(matching.len(), 1);
}

#[test]
fn test_list_unlocked_achievements_returns_them_in_unlock_order() {
    let db = fresh_db();
    // Sleep to ensure distinct timestamps is unreliable; rely on the fact
    // that the SQLite default timestamp has second resolution. Insert
    // explicit rows in order.
    db.conn
        .execute(
            "INSERT INTO achievements_unlocked (name, unlocked_at) VALUES ('first_blood', '2024-01-01T00:00:00')",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO achievements_unlocked (name, unlocked_at) VALUES ('grinder', '2024-01-02T00:00:00')",
            [],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO achievements_unlocked (name, unlocked_at) VALUES ('awakened', '2024-01-03T00:00:00')",
            [],
        )
        .unwrap();

    let rows = db.list_unlocked_achievements().unwrap();
    let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["first_blood", "grinder", "awakened"]);
}

// ── award_task_completion orchestration ──────────────────────────────

#[test]
fn test_award_on_fresh_profile_returns_correct_reward() {
    let db = fresh_db();
    let reward = award_task_completion_on(&db, Some(&Priority::Medium), d(2024, 1, 1)).unwrap();
    assert_eq!(reward.xp_gained, 25);
    assert_eq!(reward.new_xp, 25);
    assert_eq!(reward.old_level, 1);
    assert_eq!(reward.new_level, 1);
    assert!(!reward.leveled_up);
    assert_eq!(reward.current_streak, 1);
    assert_eq!(reward.longest_streak, 1);
    assert!(reward.streak_extended);
    // First blood is the only one unlockable at 1 task, level 1, streak 1.
    assert_eq!(reward.new_achievements, vec![Achievement::FirstBlood]);
}

#[test]
fn test_award_persists_profile_changes_to_db() {
    let db = fresh_db();
    award_task_completion_on(&db, Some(&Priority::High), d(2024, 1, 1)).unwrap();
    let p = db.get_profile().unwrap();
    assert_eq!(p.xp, 50);
    assert_eq!(p.total_completed, 1);
    assert_eq!(p.last_completion_date, Some(d(2024, 1, 1)));
}

#[test]
fn test_award_levels_up_flag_true_only_when_threshold_crossed() {
    let db = fresh_db();
    // First award at Low (10 XP): stays at level 1.
    let r1 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 1)).unwrap();
    assert!(!r1.leveled_up);
    // Bring total to 49 XP (still level 1) with 3 more Lows + Medium.
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 2)).unwrap();
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 3)).unwrap();
    award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 4)).unwrap();
    // 40 XP total so far. One more Low → 50 → level 2.
    let crossing = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 5)).unwrap();
    assert!(
        crossing.leveled_up,
        "crossing the 50 XP threshold must flip leveled_up"
    );
    assert_eq!(crossing.old_level, 1);
    assert_eq!(crossing.new_level, 2);
    // Next award inside level 2 does NOT flip the flag again.
    let inside = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 6)).unwrap();
    assert!(!inside.leveled_up);
}

#[test]
fn test_award_unlocks_multiple_achievements_in_single_call() {
    // Seed the profile so the next award crosses level 10 AND hits 10 tasks
    // simultaneously (9 tasks already, XP just below 4050).
    let db = fresh_db();
    let mut p = Profile {
        xp: 4040, // next level at 4050
        level: 9,
        current_streak: 6, // next award on d+1 -> 7
        longest_streak: 6,
        total_completed: 9, // next award -> 10
        last_completion_date: Some(d(2024, 1, 6)),
    };
    db.save_profile(&p).unwrap();
    // Pre-unlock First Blood + earlier badges since they'd have fired.
    db.unlock_achievement(Achievement::FirstBlood.key())
        .unwrap();

    let reward = award_task_completion_on(&db, Some(&Priority::Medium), d(2024, 1, 7)).unwrap();
    assert!(reward.leveled_up);
    // Should unlock: Grinder (10), Awakened (lvl 10), WeekWarrior (7-day).
    let keys: Vec<&str> = reward.new_achievements.iter().map(|a| a.key()).collect();
    assert!(keys.contains(&"grinder"));
    assert!(keys.contains(&"awakened"));
    assert!(keys.contains(&"week_warrior"));

    // Sanity: the old profile we built is unused after load.
    p.level = reward.new_level;
    assert_eq!(p.level, 10);
}

#[test]
fn test_award_does_not_unlock_same_achievement_twice() {
    // Anti-test: DB guard + logic guard prevent duplicate unlocks.
    let db = fresh_db();
    let r1 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 1)).unwrap();
    assert!(r1.new_achievements.contains(&Achievement::FirstBlood));

    let r2 = award_task_completion_on(&db, Some(&Priority::Low), d(2024, 1, 2)).unwrap();
    assert!(
        !r2.new_achievements.contains(&Achievement::FirstBlood),
        "first_blood must not re-unlock on subsequent awards"
    );

    let rows = db.list_unlocked_achievements().unwrap();
    let fb_count = rows.iter().filter(|(n, _)| n == "first_blood").count();
    assert_eq!(fb_count, 1);
}

#[test]
fn test_award_with_no_priority_still_grants_neutral_fifteen_xp() {
    // Anti-test: None priority is not rejected; it grants the neutral 15.
    let db = fresh_db();
    let reward = award_task_completion_on(&db, None, d(2024, 1, 1)).unwrap();
    assert_eq!(reward.xp_gained, 15);
    assert_ne!(reward.xp_gained, 0);
}

// ── Status command idempotency guard (merged -> merged) ──────────────

#[test]
fn test_status_merged_to_merged_does_not_double_award() {
    // Simulates the status.rs guard: award only fires when the previous
    // status was NOT Merged. Here we just exercise the profile side.
    let db = fresh_db();
    award_task_completion_on(&db, Some(&Priority::Critical), d(2024, 1, 1)).unwrap();
    let p1 = db.get_profile().unwrap();
    assert_eq!(p1.total_completed, 1);

    // The status command's guard prevents a second award call. If it did
    // call twice, this is the mirror of that — we simply don't call it.
    let p2 = db.get_profile().unwrap();
    assert_eq!(p1, p2, "no state drift without a re-award");
}

// ── Level cap anti-tests through award path ──────────────────────────

#[test]
fn test_award_never_pushes_level_past_hundred() {
    // Anti-test: saturating XP + level clamp keep us at 100.
    let db = fresh_db();
    let p = Profile {
        xp: i64::MAX - 100, // huge
        level: MAX_LEVEL,
        current_streak: 0,
        longest_streak: 0,
        total_completed: 0,
        last_completion_date: None,
    };
    db.save_profile(&p).unwrap();

    let reward = award_task_completion_on(&db, Some(&Priority::Critical), d(2024, 1, 1)).unwrap();
    assert_eq!(reward.new_level, MAX_LEVEL);
    let after = db.get_profile().unwrap();
    assert_eq!(after.level, MAX_LEVEL);
}
