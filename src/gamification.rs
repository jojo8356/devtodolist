//! Gamification layer: levels, XP, streaks and achievements.
//!
//! Each time a task is merged, XP is awarded based on priority. Levels
//! are derived from total XP with a quadratic curve, topping out at 100.
//! Streaks reward consecutive days of merged tasks. Achievements are
//! persistent milestone badges stored in the database.

use chrono::{Local, NaiveDate};

use crate::db::Database;
use crate::error::Result;
use crate::models::Priority;

/// Maximum level achievable.
pub const MAX_LEVEL: u32 = 100;

/// Profile snapshot held in the `gamification` single-row table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Total XP accumulated across all task completions.
    pub xp: i64,
    /// Current level derived from [`xp`], clamped to [`MAX_LEVEL`].
    ///
    /// [`xp`]: Self::xp
    pub level: u32,
    /// Number of consecutive days with at least one merge, ending today.
    pub current_streak: u32,
    /// Largest value [`current_streak`] has ever reached. Never decreases.
    ///
    /// [`current_streak`]: Self::current_streak
    pub longest_streak: u32,
    /// Cumulative count of merged tasks that have been rewarded.
    pub total_completed: u64,
    /// Date of the most recent rewarded merge, used to compute streaks.
    pub last_completion_date: Option<NaiveDate>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            xp: 0,
            level: 1,
            current_streak: 0,
            longest_streak: 0,
            total_completed: 0,
            last_completion_date: None,
        }
    }
}

/// All achievements the player can unlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// Awarded on the very first merged task.
    FirstBlood,
    /// Awarded once 10 tasks have been merged.
    Grinder,
    /// Awarded once 50 tasks have been merged.
    Workaholic,
    /// Awarded once 100 tasks have been merged.
    Centurion,
    /// Awarded upon reaching level 10.
    Awakened,
    /// Awarded upon reaching level 50.
    SRank,
    /// Awarded upon reaching the level cap (100).
    MonarchOfShadows,
    /// Awarded for a 7-day active streak.
    WeekWarrior,
    /// Awarded for a 30-day active streak.
    Unstoppable,
    /// Awarded for a 100-day active streak.
    ShadowArmy,
}

impl Achievement {
    /// All achievements, in unlock-check order.
    pub const ALL: &'static [Achievement] = &[
        Achievement::FirstBlood,
        Achievement::Grinder,
        Achievement::Workaholic,
        Achievement::Centurion,
        Achievement::Awakened,
        Achievement::SRank,
        Achievement::MonarchOfShadows,
        Achievement::WeekWarrior,
        Achievement::Unstoppable,
        Achievement::ShadowArmy,
    ];

    /// Stable identifier used as the primary key in the DB.
    pub fn key(&self) -> &'static str {
        match self {
            Achievement::FirstBlood => "first_blood",
            Achievement::Grinder => "grinder",
            Achievement::Workaholic => "workaholic",
            Achievement::Centurion => "centurion",
            Achievement::Awakened => "awakened",
            Achievement::SRank => "s_rank",
            Achievement::MonarchOfShadows => "monarch_of_shadows",
            Achievement::WeekWarrior => "week_warrior",
            Achievement::Unstoppable => "unstoppable",
            Achievement::ShadowArmy => "shadow_army",
        }
    }

    /// Human-readable title shown in the profile screen.
    pub fn title(&self) -> &'static str {
        match self {
            Achievement::FirstBlood => "First Blood",
            Achievement::Grinder => "Grinder",
            Achievement::Workaholic => "Workaholic",
            Achievement::Centurion => "Centurion",
            Achievement::Awakened => "Awakened",
            Achievement::SRank => "S-Rank Hunter",
            Achievement::MonarchOfShadows => "Monarch of Shadows",
            Achievement::WeekWarrior => "Week Warrior",
            Achievement::Unstoppable => "Unstoppable",
            Achievement::ShadowArmy => "Shadow Army",
        }
    }

    /// One-line description of the unlock condition.
    pub fn description(&self) -> &'static str {
        match self {
            Achievement::FirstBlood => "Complete your first task",
            Achievement::Grinder => "Merge 10 tasks",
            Achievement::Workaholic => "Merge 50 tasks",
            Achievement::Centurion => "Merge 100 tasks",
            Achievement::Awakened => "Reach level 10",
            Achievement::SRank => "Reach level 50",
            Achievement::MonarchOfShadows => "Reach level 100",
            Achievement::WeekWarrior => "Maintain a 7-day streak",
            Achievement::Unstoppable => "Maintain a 30-day streak",
            Achievement::ShadowArmy => "Maintain a 100-day streak",
        }
    }

    /// Returns true if this achievement's unlock condition is met by the
    /// given profile.
    pub fn is_earned(&self, profile: &Profile) -> bool {
        match self {
            Achievement::FirstBlood => profile.total_completed >= 1,
            Achievement::Grinder => profile.total_completed >= 10,
            Achievement::Workaholic => profile.total_completed >= 50,
            Achievement::Centurion => profile.total_completed >= 100,
            Achievement::Awakened => profile.level >= 10,
            Achievement::SRank => profile.level >= 50,
            Achievement::MonarchOfShadows => profile.level >= MAX_LEVEL,
            Achievement::WeekWarrior => profile.current_streak >= 7,
            Achievement::Unstoppable => profile.current_streak >= 30,
            Achievement::ShadowArmy => profile.current_streak >= 100,
        }
    }
}

/// XP awarded when a task of the given priority is merged.
pub fn xp_for_priority(priority: Option<&Priority>) -> i64 {
    match priority {
        Some(Priority::Low) => 10,
        Some(Priority::Medium) => 25,
        Some(Priority::High) => 50,
        Some(Priority::Critical) => 100,
        None => 15,
    }
}

/// Total XP required to *reach* a given level.
///
/// Level 1 is the starting level (0 XP). Each level N thereafter needs
/// `50 * (N - 1)^2` cumulative XP. Level 100 caps at `490_050` XP.
pub fn xp_for_level(n: u32) -> i64 {
    if n <= 1 {
        return 0;
    }
    let step = (n - 1) as i64;
    50 * step * step
}

/// Derive the level from a total XP value, clamped to `MAX_LEVEL`.
pub fn level_for_xp(xp: i64) -> u32 {
    if xp <= 0 {
        return 1;
    }
    let lvl = ((xp as f64 / 50.0).sqrt().floor() as u32) + 1;
    lvl.min(MAX_LEVEL)
}

/// XP still required to reach the next level. Returns `0` at max level.
pub fn xp_to_next_level(current_xp: i64) -> i64 {
    let lvl = level_for_xp(current_xp);
    if lvl >= MAX_LEVEL {
        return 0;
    }
    let next = xp_for_level(lvl + 1);
    (next - current_xp).max(0)
}

/// Progress within the current level, as `(xp_into_level, xp_span_of_level)`.
/// At max level, returns `(1, 1)` so UIs render a full bar.
pub fn progress_within_level(current_xp: i64) -> (i64, i64) {
    let lvl = level_for_xp(current_xp);
    if lvl >= MAX_LEVEL {
        return (1, 1);
    }
    let floor = xp_for_level(lvl);
    let ceil = xp_for_level(lvl + 1);
    let into = (current_xp - floor).max(0);
    let span = (ceil - floor).max(1);
    (into, span)
}

/// Outcome of awarding a task completion, returned so callers can render
/// banners/SFX for the user.
#[derive(Debug, Clone)]
pub struct CompletionReward {
    /// XP added by this single completion.
    pub xp_gained: i64,
    /// Profile XP total after the award was applied.
    pub new_xp: i64,
    /// Level before the award.
    pub old_level: u32,
    /// Level after the award.
    pub new_level: u32,
    /// True iff this award crossed a level threshold.
    pub leveled_up: bool,
    /// Current streak after the award (days).
    pub current_streak: u32,
    /// All-time longest streak after the award (days).
    pub longest_streak: u32,
    /// True iff this award advanced (started or extended) the streak;
    /// false for same-day duplicate awards.
    pub streak_extended: bool,
    /// Achievements that were freshly unlocked by this award.
    pub new_achievements: Vec<Achievement>,
}

/// Apply the streak rule for a completion on `today` given the previous
/// completion date.
///
/// Handles clock skew: if the previously recorded completion date is in the
/// future relative to `today`, the award is treated like a same-day
/// duplicate — neither incremented nor reset — and `last_completion_date`
/// is preserved so we don't rewrite history backwards.
fn update_streak(profile: &mut Profile, today: NaiveDate) -> bool {
    let prev = profile.last_completion_date;
    let mut extended = false;
    let mut keep_prev_date = false;
    profile.current_streak = match prev {
        Some(d) if d == today => profile.current_streak.max(1),
        // Clock skew: stored date is in the future. Treat as same-day.
        Some(d) if d > today => {
            keep_prev_date = true;
            profile.current_streak.max(1)
        }
        Some(d) if d.succ_opt() == Some(today) => {
            extended = true;
            profile.current_streak.saturating_add(1)
        }
        _ => {
            extended = true;
            1
        }
    };
    if profile.current_streak > profile.longest_streak {
        profile.longest_streak = profile.current_streak;
    }
    if !keep_prev_date {
        profile.last_completion_date = Some(today);
    }
    extended
}

/// Award XP, update streak and unlock any newly-earned achievements for a
/// completed task. Persists the updated profile and achievement rows.
pub fn award_task_completion(
    db: &Database,
    priority: Option<&Priority>,
) -> Result<CompletionReward> {
    award_task_completion_on(db, priority, Local::now().date_naive())
}

/// Variant of [`award_task_completion`] that takes the completion date
/// explicitly, to keep the logic deterministic in tests.
pub fn award_task_completion_on(
    db: &Database,
    priority: Option<&Priority>,
    today: NaiveDate,
) -> Result<CompletionReward> {
    let mut profile = db.get_profile()?;

    let xp_gained = xp_for_priority(priority);
    let old_level = profile.level;

    profile.xp = profile.xp.saturating_add(xp_gained);
    profile.total_completed = profile.total_completed.saturating_add(1);
    profile.level = level_for_xp(profile.xp);

    let streak_extended = update_streak(&mut profile, today);

    db.save_profile(&profile)?;

    // Evaluate achievements based on the freshly updated profile.
    let mut new_achievements = Vec::new();
    for ach in Achievement::ALL {
        if ach.is_earned(&profile) && !db.is_achievement_unlocked(ach.key())? {
            db.unlock_achievement(ach.key())?;
            new_achievements.push(*ach);
        }
    }

    Ok(CompletionReward {
        xp_gained,
        new_xp: profile.xp,
        old_level,
        new_level: profile.level,
        leveled_up: profile.level > old_level,
        current_streak: profile.current_streak,
        longest_streak: profile.longest_streak,
        streak_extended,
        new_achievements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    // ── xp_for_priority ───────────────────────────────────────────────

    #[test]
    fn test_xp_for_priority_low_awards_10() {
        assert_eq!(xp_for_priority(Some(&Priority::Low)), 10);
    }

    #[test]
    fn test_xp_for_priority_medium_awards_25() {
        assert_eq!(xp_for_priority(Some(&Priority::Medium)), 25);
    }

    #[test]
    fn test_xp_for_priority_high_awards_50() {
        assert_eq!(xp_for_priority(Some(&Priority::High)), 50);
    }

    #[test]
    fn test_xp_for_priority_critical_awards_100() {
        assert_eq!(xp_for_priority(Some(&Priority::Critical)), 100);
    }

    #[test]
    fn test_xp_for_priority_none_returns_neutral_15_and_does_not_collapse_to_zero() {
        // Anti-test: documents that None is a neutral default (15), not 0.
        assert_eq!(xp_for_priority(None), 15);
        assert_ne!(xp_for_priority(None), 0);
    }

    // ── xp_for_level ─────────────────────────────────────────────────

    #[test]
    fn test_xp_for_level_one_is_zero() {
        assert_eq!(xp_for_level(1), 0);
    }

    #[test]
    fn test_xp_for_level_two_is_fifty() {
        assert_eq!(xp_for_level(2), 50);
    }

    #[test]
    fn test_xp_for_level_five_follows_quadratic_curve() {
        // 50 * (5-1)^2 = 50 * 16 = 800
        assert_eq!(xp_for_level(5), 800);
    }

    #[test]
    fn test_xp_for_level_ten_follows_quadratic_curve() {
        // 50 * 9^2 = 4050
        assert_eq!(xp_for_level(10), 4050);
    }

    #[test]
    fn test_xp_for_level_fifty_follows_quadratic_curve() {
        // 50 * 49^2 = 120_050
        assert_eq!(xp_for_level(50), 120_050);
    }

    #[test]
    fn test_xp_for_level_hundred_is_490050() {
        assert_eq!(xp_for_level(100), 490_050);
    }

    #[test]
    fn test_xp_for_level_zero_does_not_panic_and_returns_zero() {
        // Anti-test: level 0 is not valid, but the fn must not panic.
        assert_eq!(xp_for_level(0), 0);
    }

    // ── level_for_xp ─────────────────────────────────────────────────

    #[test]
    fn test_level_for_xp_at_level_one_threshold() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(49), 1);
    }

    #[test]
    fn test_level_for_xp_at_level_two_threshold() {
        assert_eq!(level_for_xp(50), 2);
        assert_eq!(level_for_xp(199), 2);
    }

    #[test]
    fn test_level_for_xp_at_level_three_threshold() {
        assert_eq!(level_for_xp(200), 3);
    }

    #[test]
    fn test_level_for_xp_at_max_threshold() {
        assert_eq!(level_for_xp(490_050), MAX_LEVEL);
    }

    #[test]
    fn test_level_clamps_at_100_with_huge_xp() {
        // Anti-test: level never exceeds 100 even with astronomical XP.
        assert_eq!(level_for_xp(10_000_000), MAX_LEVEL);
        assert_eq!(level_for_xp(i64::MAX), MAX_LEVEL);
    }

    #[test]
    fn test_level_for_xp_rejects_negative_xp_by_treating_as_zero() {
        // Anti-test: negative XP is clamped to level 1 (treated as 0).
        assert_eq!(level_for_xp(-1), 1);
        assert_eq!(level_for_xp(-1_000_000), 1);
        assert_eq!(level_for_xp(i64::MIN), 1);
    }

    // ── xp_to_next_level ─────────────────────────────────────────────

    #[test]
    fn test_xp_to_next_level_at_start_is_fifty() {
        // Level 1 with 0 XP: need 50 to reach level 2.
        assert_eq!(xp_to_next_level(0), 50);
    }

    #[test]
    fn test_xp_to_next_level_mid_curve() {
        // At 100 XP (level 2), next threshold is 200, so 100 remain.
        assert_eq!(xp_to_next_level(100), 100);
    }

    #[test]
    fn test_xp_to_next_level_never_exceeds_remaining_xp() {
        // Anti-test: the "to next" value should never be negative.
        for xp in [0, 1, 50, 123, 4050, 490_050, 1_000_000] {
            assert!(xp_to_next_level(xp) >= 0, "xp={xp}");
        }
    }

    #[test]
    fn test_xp_to_next_level_saturates_to_zero_at_max_level() {
        // Anti-test: at max level, "to next" is 0, not negative or underflowed.
        assert_eq!(xp_to_next_level(490_050), 0);
        assert_eq!(xp_to_next_level(i64::MAX), 0);
    }

    // ── progress_within_level ────────────────────────────────────────

    #[test]
    fn test_progress_within_level_is_zero_at_threshold() {
        // At exactly the threshold for level 2, `into` is 0.
        let (into, span) = progress_within_level(50);
        assert_eq!(into, 0);
        assert_eq!(span, xp_for_level(3) - xp_for_level(2));
    }

    #[test]
    fn test_progress_within_level_is_consistent_mid_level() {
        let (into, span) = progress_within_level(75);
        // into + floor == current xp
        assert_eq!(into + xp_for_level(2), 75);
        assert_eq!(span, xp_for_level(3) - xp_for_level(2));
    }

    #[test]
    fn test_progress_within_level_never_exceeds_one() {
        // Anti-test: into/span ratio is always in [0, 1].
        for xp in [
            0, 1, 49, 50, 199, 200, 4049, 4050, 490_049, 490_050, 1_000_000,
        ] {
            let (into, span) = progress_within_level(xp);
            assert!(span > 0, "span must be positive, xp={xp}");
            assert!(into <= span, "into {into} <= span {span} (xp={xp})");
            assert!(into >= 0, "into must be non-negative (xp={xp})");
        }
    }

    #[test]
    fn test_progress_within_level_returns_full_bar_at_max() {
        // At max level, UI shows a full bar.
        let (into, span) = progress_within_level(490_050);
        assert_eq!(into, span);
    }

    // ── Achievement::is_earned ───────────────────────────────────────

    fn profile_with(total: u64, level: u32, current_streak: u32) -> Profile {
        Profile {
            xp: 0,
            level,
            current_streak,
            longest_streak: current_streak,
            total_completed: total,
            last_completion_date: None,
        }
    }

    #[test]
    fn test_first_blood_earned_at_one_task() {
        assert!(Achievement::FirstBlood.is_earned(&profile_with(1, 1, 0)));
    }

    #[test]
    fn test_grinder_earned_at_exactly_ten_tasks() {
        assert!(Achievement::Grinder.is_earned(&profile_with(10, 1, 0)));
    }

    #[test]
    fn test_grinder_does_not_unlock_at_nine_tasks() {
        // Anti-test: one below the threshold must not trigger.
        assert!(!Achievement::Grinder.is_earned(&profile_with(9, 1, 0)));
    }

    #[test]
    fn test_workaholic_earned_at_fifty_tasks() {
        assert!(Achievement::Workaholic.is_earned(&profile_with(50, 1, 0)));
    }

    #[test]
    fn test_workaholic_does_not_unlock_at_forty_nine_tasks() {
        // Anti-test
        assert!(!Achievement::Workaholic.is_earned(&profile_with(49, 1, 0)));
    }

    #[test]
    fn test_centurion_earned_at_one_hundred_tasks() {
        assert!(Achievement::Centurion.is_earned(&profile_with(100, 1, 0)));
    }

    #[test]
    fn test_centurion_does_not_unlock_at_ninety_nine_tasks() {
        // Anti-test
        assert!(!Achievement::Centurion.is_earned(&profile_with(99, 1, 0)));
    }

    #[test]
    fn test_awakened_earned_at_level_ten() {
        assert!(Achievement::Awakened.is_earned(&profile_with(0, 10, 0)));
    }

    #[test]
    fn test_awakened_does_not_unlock_below_level_ten() {
        // Anti-test
        assert!(!Achievement::Awakened.is_earned(&profile_with(0, 9, 0)));
    }

    #[test]
    fn test_s_rank_earned_at_level_fifty() {
        assert!(Achievement::SRank.is_earned(&profile_with(0, 50, 0)));
    }

    #[test]
    fn test_monarch_earned_at_level_one_hundred() {
        assert!(Achievement::MonarchOfShadows.is_earned(&profile_with(0, 100, 0)));
    }

    #[test]
    fn test_monarch_does_not_unlock_at_level_ninety_nine() {
        // Anti-test
        assert!(!Achievement::MonarchOfShadows.is_earned(&profile_with(0, 99, 0)));
    }

    #[test]
    fn test_week_warrior_earned_at_seven_day_streak() {
        assert!(Achievement::WeekWarrior.is_earned(&profile_with(0, 1, 7)));
    }

    #[test]
    fn test_week_warrior_does_not_unlock_at_six_day_streak() {
        // Anti-test
        assert!(!Achievement::WeekWarrior.is_earned(&profile_with(0, 1, 6)));
    }

    #[test]
    fn test_unstoppable_earned_at_thirty_day_streak() {
        assert!(Achievement::Unstoppable.is_earned(&profile_with(0, 1, 30)));
    }

    #[test]
    fn test_shadow_army_earned_at_hundred_day_streak() {
        assert!(Achievement::ShadowArmy.is_earned(&profile_with(0, 1, 100)));
    }

    #[test]
    fn test_shadow_army_does_not_unlock_at_ninety_nine_day_streak() {
        // Anti-test
        assert!(!Achievement::ShadowArmy.is_earned(&profile_with(0, 1, 99)));
    }

    #[test]
    fn test_achievement_keys_are_unique() {
        let mut keys: Vec<&str> = Achievement::ALL.iter().map(|a| a.key()).collect();
        keys.sort_unstable();
        let len = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), len, "achievement keys must be unique");
    }

    // ── update_streak ────────────────────────────────────────────────

    #[test]
    fn test_streak_first_completion_sets_one() {
        let mut p = Profile::default();
        let extended = update_streak(&mut p, d(2024, 1, 1));
        assert_eq!(p.current_streak, 1);
        assert_eq!(p.longest_streak, 1);
        assert!(extended);
    }

    #[test]
    fn test_streak_same_day_does_not_increment() {
        let mut p = Profile::default();
        update_streak(&mut p, d(2024, 1, 1));
        let extended = update_streak(&mut p, d(2024, 1, 1));
        assert_eq!(p.current_streak, 1);
        assert!(!extended);
    }

    #[test]
    fn test_streak_next_day_increments_to_two() {
        let mut p = Profile::default();
        update_streak(&mut p, d(2024, 1, 1));
        update_streak(&mut p, d(2024, 1, 2));
        assert_eq!(p.current_streak, 2);
        assert_eq!(p.longest_streak, 2);
    }

    #[test]
    fn test_streak_resets_after_gap() {
        let mut p = Profile::default();
        update_streak(&mut p, d(2024, 1, 1));
        update_streak(&mut p, d(2024, 1, 2));
        update_streak(&mut p, d(2024, 1, 5));
        assert_eq!(p.current_streak, 1);
    }

    #[test]
    fn test_longest_streak_never_decreases_after_reset() {
        // Anti-test: after a gap, longest is preserved.
        let mut p = Profile::default();
        update_streak(&mut p, d(2024, 1, 1));
        update_streak(&mut p, d(2024, 1, 2));
        assert_eq!(p.longest_streak, 2);
        update_streak(&mut p, d(2024, 1, 5));
        assert_eq!(p.longest_streak, 2);
    }

    #[test]
    fn test_longest_streak_never_decreases_after_thirty_then_gap() {
        // Anti-test from spec: streak=30 then 7-day gap → current=1,
        // longest stays at 30.
        let mut p = Profile::default();
        let mut day = d(2024, 1, 1);
        for _ in 0..30 {
            update_streak(&mut p, day);
            day = day.succ_opt().unwrap();
        }
        assert_eq!(p.current_streak, 30);
        assert_eq!(p.longest_streak, 30);

        // 7-day gap
        let gap_day = day
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap()
            .succ_opt()
            .unwrap();
        update_streak(&mut p, gap_day);
        assert_eq!(p.current_streak, 1);
        assert_eq!(p.longest_streak, 30);
    }

    #[test]
    fn test_streak_does_not_crash_with_future_last_date() {
        // Anti-test: clock skew where stored date is in the future.
        // Must not panic and must not reset the streak.
        let mut p = Profile {
            current_streak: 5,
            longest_streak: 5,
            last_completion_date: Some(d(2099, 12, 31)),
            ..Profile::default()
        };
        let extended = update_streak(&mut p, d(2024, 1, 1));
        assert!(!extended, "future-date skew should not count as extended");
        assert_eq!(p.current_streak, 5, "streak must not reset on clock skew");
        assert_eq!(
            p.last_completion_date,
            Some(d(2099, 12, 31)),
            "must not rewrite last date backwards"
        );
    }
}
