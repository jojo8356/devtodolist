use colored::Colorize;

use crate::commands::init::find_db;
use crate::error::Result;
use crate::models::{Priority, TaskStatus};

pub async fn run(
    title: &str,
    description: Option<&str>,
    priority: Option<&str>,
    branch: Option<&str>,
    base: &str,
    labels: &[String],
    assignee: Option<&str>,
) -> Result<()> {
    let db = find_db().await?;

    let prio = priority.map(|p| p.parse::<Priority>()).transpose()?;

    let id = db
        .insert_task(
            title,
            description,
            &TaskStatus::Draft,
            prio.as_ref(),
            branch,
            Some(base),
            assignee,
        )
        .await?;

    for label_name in labels {
        if db.get_label_by_name(label_name).await.is_err() {
            db.insert_label(label_name, None).await?;
            println!("  {} Created label '{}'", "+".dimmed(), label_name);
        }
        db.assign_label(id, label_name).await?;
    }

    println!(
        "{} Created task #{} — {}",
        "✓".green().bold(),
        id.to_string().bold(),
        title
    );

    Ok(())
}
