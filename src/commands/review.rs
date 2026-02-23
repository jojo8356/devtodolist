use colored::Colorize;

use crate::cli::ReviewCommands;
use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;
use crate::models::ReviewStatus;

pub fn run(command: &ReviewCommands) -> Result<()> {
    let db = find_db()?;

    match command {
        ReviewCommands::Assign { task_id, username } => {
            let _ = db.get_task(*task_id)?;
            db.assign_reviewer(*task_id, username)?;
            println!(
                "{} Assigned reviewer '{}' to task #{}",
                "✓".green().bold(),
                username.bold(),
                task_id
            );
        }
        ReviewCommands::Remove { task_id, username } => {
            db.remove_reviewer(*task_id, username)?;
            println!(
                "{} Removed reviewer '{}' from task #{}",
                "✓".green().bold(),
                username.bold(),
                task_id
            );
        }
        ReviewCommands::Status {
            task_id,
            username,
            status,
        } => {
            let review_status: ReviewStatus = status.parse()?;
            db.update_review_status(*task_id, username, &review_status)?;
            println!(
                "{} Updated review for '{}' on task #{}: {}",
                "✓".green().bold(),
                username.bold(),
                task_id,
                display::colorize_review_status(&review_status)
            );
        }
        ReviewCommands::List { task_id } => {
            let _ = db.get_task(*task_id)?;
            let reviewers = db.list_reviewers(*task_id)?;
            display::print_reviewers_table(&reviewers);
        }
    }

    Ok(())
}
