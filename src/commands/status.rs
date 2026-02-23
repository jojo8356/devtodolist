use colored::Colorize;

use crate::commands::init::find_db;
use crate::display::colorize_status;
use crate::error::Result;
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

    Ok(())
}
