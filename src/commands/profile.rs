use colored::Colorize;

use crate::commands::init::find_db;
use crate::error::Result;
use crate::gamification::{self, Achievement, MAX_LEVEL};

const BAR_WIDTH: usize = 20;

pub fn run() -> Result<()> {
    let db = find_db()?;
    let profile = db.get_profile()?;
    let unlocked: std::collections::HashSet<String> = db
        .list_unlocked_achievements()?
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    let (into, span) = gamification::progress_within_level(profile.xp);
    let next_xp = gamification::xp_to_next_level(profile.xp);

    println!("{}", "╔══════════════════════════════════════╗".purple());
    println!(
        "{}       {}       {}",
        "║".purple(),
        "devtodo — Hunter Profile".bold(),
        "║".purple()
    );
    println!("{}", "╚══════════════════════════════════════╝".purple());
    println!();

    // Level line
    println!(
        "{}   {}  /  {}",
        "Level".bold(),
        profile.level.to_string().yellow().bold(),
        MAX_LEVEL
    );

    // XP bar
    let filled = if span > 0 {
        ((into as f64 / span as f64) * BAR_WIDTH as f64).round() as usize
    } else {
        BAR_WIDTH
    };
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;
    let bar: String = format!(
        "[{}{}]",
        "█".repeat(filled).green(),
        "░".repeat(empty).dimmed()
    );
    if profile.level >= MAX_LEVEL {
        println!(
            "{}   {} XP   {}",
            bar,
            profile.xp.to_string().bold(),
            "MONARCH".purple().bold()
        );
    } else {
        let ceil = gamification::xp_for_level(profile.level + 1);
        println!(
            "{}   {} / {} XP   ({} to next)",
            bar,
            profile.xp.to_string().bold(),
            ceil,
            next_xp
        );
    }
    println!();

    // Streaks & totals
    println!(
        "{}  {} 🔥    {}  {} 🔥",
        "Current streak".bold(),
        profile.current_streak.to_string().red().bold(),
        "Longest".bold(),
        profile.longest_streak.to_string().red()
    );
    println!(
        "{}    {}",
        "Tasks merged".bold(),
        profile.total_completed.to_string().bold()
    );
    println!();

    // Achievements
    let total = Achievement::ALL.len();
    let earned = Achievement::ALL
        .iter()
        .filter(|a| unlocked.contains(a.key()))
        .count();
    println!(
        "{}  {} / {}",
        "Achievements".bold(),
        earned.to_string().yellow().bold(),
        total
    );
    for ach in Achievement::ALL {
        let has_it = unlocked.contains(ach.key());
        let mark = if has_it {
            "✓".green().bold().to_string()
        } else {
            "✗".dimmed().to_string()
        };
        let title = if has_it {
            format!("{:<20}", ach.title()).white().bold().to_string()
        } else {
            format!("{:<20}", ach.title()).dimmed().to_string()
        };
        println!("  {} {} — {}", mark, title, ach.description().dimmed());
    }

    Ok(())
}
