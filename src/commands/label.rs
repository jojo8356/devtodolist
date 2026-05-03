use colored::Colorize;

use crate::cli::LabelCommands;
use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub async fn run(command: &LabelCommands) -> Result<()> {
    let db = find_db().await?;

    match command {
        LabelCommands::Add { name, color } => {
            db.insert_label(name, color.as_deref()).await?;
            println!("{} Created label '{}'", "✓".green().bold(), name.bold());
        }
        LabelCommands::Remove { name } => {
            db.delete_label(name).await?;
            println!("{} Removed label '{}'", "✓".green().bold(), name.bold());
        }
        LabelCommands::List => {
            let labels = db.list_labels().await?;
            display::print_labels_table(&labels);
        }
        LabelCommands::Assign { task_id, label } => {
            let _ = db.get_task(*task_id).await?;
            db.assign_label(*task_id, label).await?;
            println!(
                "{} Assigned label '{}' to task #{}",
                "✓".green().bold(),
                label,
                task_id
            );
        }
        LabelCommands::Unassign { task_id, label } => {
            db.unassign_label(*task_id, label).await?;
            println!(
                "{} Unassigned label '{}' from task #{}",
                "✓".green().bold(),
                label,
                task_id
            );
        }
    }

    Ok(())
}
