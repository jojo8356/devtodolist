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
    pub xp: i64,
    pub level: u32,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub total_completed: u64,
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
    FirstBlood,
    Grinder,
    Workaholic,
    Centurion,
    Awakened,
    SRank,
    MonarchOfShadows,
    WeekWarrior,
    Unstoppable,
    ShadowArmy,
}

impl Achievement {
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
    pub xp_gained: i64,
    pub new_xp: i64,
    pub old_level: u32,
    pub new_level: u32,
    pub leveled_up: bool,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub streak_extended: bool,
    pub new_achievements: Vec<Achievement>,
}

/// Apply the streak rule for a completion on `today` given the previous
/// completion date.
fn update_streak(profile: &mut Profile, today: NaiveDate) -> bool {
    let prev = profile.last_completion_date;
    let mut extended = false;
    profile.current_streak = match prev {
        Some(d) if d == today => profile.current_streak.max(1),
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
    profile.last_completion_date = Some(today);
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

    #[test]
    fn xp_table_matches_spec() {
        assert_eq!(xp_for_priority(Some(&Priority::Low)), 10);
        assert_eq!(xp_for_priority(Some(&Priority::Medium)), 25);
        assert_eq!(xp_for_priority(Some(&Priority::High)), 50);
        assert_eq!(xp_for_priority(Some(&Priority::Critical)), 100);
        assert_eq!(xp_for_priority(None), 15);
    }

    #[test]
    fn level_curve_matches_spec() {
        assert_eq!(xp_for_level(1), 0);
        assert_eq!(xp_for_level(2), 50);
        assert_eq!(xp_for_level(3), 200);
        assert_eq!(xp_for_level(10), 50 * 81);
        assert_eq!(xp_for_level(100), 490_050);
    }

    #[test]
    fn level_for_xp_clamps_to_max() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(49), 1);
        assert_eq!(level_for_xp(50), 2);
        assert_eq!(level_for_xp(199), 2);
        assert_eq!(level_for_xp(200), 3);
        assert_eq!(level_for_xp(490_050), MAX_LEVEL);
        assert_eq!(level_for_xp(10_000_000), MAX_LEVEL);
    }

    #[test]
    fn progress_within_level_is_consistent() {
        let (into, span) = progress_within_level(75);
        assert_eq!(into + xp_for_level(2), 75);
        assert_eq!(span, xp_for_level(3) - xp_for_level(2));
    }

    #[test]
    fn streak_rules() {
        let mut p = Profile::default();
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        update_streak(&mut p, d1);
        assert_eq!(p.current_streak, 1);
        assert_eq!(p.longest_streak, 1);

        // Same day completion — unchanged.
        update_streak(&mut p, d1);
        assert_eq!(p.current_streak, 1);

        // Next day extends streak.
        let d2 = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        update_streak(&mut p, d2);
        assert_eq!(p.current_streak, 2);
        assert_eq!(p.longest_streak, 2);

        // Gap resets to 1 but longest_streak is preserved.
        let d5 = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();
        update_streak(&mut p, d5);
        assert_eq!(p.current_streak, 1);
        assert_eq!(p.longest_streak, 2);
    }
}
