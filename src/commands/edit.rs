use colored::Colorize;

use crate::commands::init::find_db;
use crate::error::Result;

pub fn run(
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    branch: Option<&str>,
    assignee: Option<&str>,
) -> Result<()> {
    let db = find_db()?;

    // Verify task exists
    let _ = db.get_task(id)?;

    let mut updated = false;

    if let Some(v) = title {
        db.update_task_field(id, "title", Some(v))?;
        updated = true;
    }
    if let Some(v) = description {
        db.update_task_field(id, "description", Some(v))?;
        updated = true;
    }
    if let Some(v) = priority {
        // Validate priority
        let _ = v.parse::<crate::models::Priority>()?;
        db.update_task_field(id, "priority", Some(v))?;
        updated = true;
    }
    if let Some(v) = branch {
        db.update_task_field(id, "branch", Some(v))?;
        updated = true;
    }
    if let Some(v) = assignee {
        db.update_task_field(id, "assignee", Some(v))?;
        updated = true;
    }

    if updated {
        println!(
            "{} Updated task #{}",
            "✓".green().bold(),
            id.to_string().bold()
        );
    } else {
        println!("Nothing to update. Use --title, --description, --priority, --branch, or --assignee.");
    }

    Ok(())
}
