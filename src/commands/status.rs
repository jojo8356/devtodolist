use colored::Colorize;

use crate::commands::init::find_db;
use crate::display::colorize_status;
use crate::error::Result;
use crate::gamification;
use crate::models::TaskStatus;

pub fn run(id: i64, status: &str) -> Result<()> {
    let db = find_db()?;

    // Validate status
    let new_status: TaskStatus = status.parse()?;

    // Verify task exists
    let task = db.get_task(id)?;

    db.update_task_field(id, "status", Some(new_status.as_str()))?;

    println!(
        "{} Task #{}: {} -> {}",
        "✓".green().bold(),
        id.to_string().bold(),
        colorize_status(&task.status),
        colorize_status(&new_status),
    );

    // Gamification: award XP when a task is merged for the first time.
    if new_status == TaskStatus::Merged && task.status != TaskStatus::Merged {
        let reward = gamification::award_task_completion(&db, task.priority.as_ref())?;
        print_reward_banner(&reward);
    }

    Ok(())
}

fn print_reward_banner(reward: &gamification::CompletionReward) {
    println!();
    println!(
        "  {}  {}",
        "✦".yellow().bold(),
        format!("+{} XP", reward.xp_gained).yellow().bold()
    );
    println!(
        "  {}  Total {} XP  —  Level {}",
        "✦".cyan(),
        reward.new_xp.to_string().bold(),
        reward.new_level.to_string().bold()
    );
    if reward.streak_extended {
        println!(
            "  {}  Streak: {} day(s)  (longest {})",
            "🔥".red(),
            reward.current_streak.to_string().bold(),
            reward.longest_streak
        );
    }

    if reward.leveled_up {
        // \x07 = ASCII BEL, a classic terminal SFX.
        println!(
            "\x07  {}  {} {} -> {}",
            "★".yellow().bold(),
            "LEVEL UP!".yellow().bold(),
            reward.old_level,
            reward.new_level.to_string().bold()
        );
    }

    if !reward.new_achievements.is_empty() {
        println!("  {}  Achievements unlocked:", "🏆".yellow());
        for ach in &reward.new_achievements {
            println!(
                "     {} {} — {}",
                "✓".green().bold(),
                ach.title().bold(),
                ach.description().dimmed()
            );
        }
    }
}
