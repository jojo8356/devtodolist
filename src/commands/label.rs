use colored::Colorize;

use crate::cli::LabelCommands;
use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub fn run(command: &LabelCommands) -> Result<()> {
    let db = find_db()?;

    match command {
        LabelCommands::Add { name, color } => {
            db.insert_label(name, color.as_deref())?;
            println!(
                "{} Created label '{}'",
                "✓".green().bold(),
                name.bold()
            );
        }
        LabelCommands::Remove { name } => {
            db.delete_label(name)?;
            println!(
                "{} Removed label '{}'",
                "✓".green().bold(),
                name.bold()
            );
        }
        LabelCommands::List => {
            let labels = db.list_labels()?;
            display::print_labels_table(&labels);
        }
        LabelCommands::Assign { task_id, label } => {
            // Verify task exists
            let _ = db.get_task(*task_id)?;
            db.assign_label(*task_id, label)?;
            println!(
                "{} Assigned label '{}' to task #{}",
                "✓".green().bold(),
                label,
                task_id
            );
        }
        LabelCommands::Unassign { task_id, label } => {
            db.unassign_label(*task_id, label)?;
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
