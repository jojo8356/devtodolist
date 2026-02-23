use colored::Colorize;
use dialoguer::Confirm;

use crate::commands::init::find_db;
use crate::error::Result;

pub fn run(id: i64, force: bool) -> Result<()> {
    let db = find_db()?;

    let task = db.get_task(id)?;

    if !force {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete task #{} \"{}\"?", id, task.title))
            .default(false)
            .interact()
            .unwrap_or(false);

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    db.delete_task(id)?;

    println!(
        "{} Deleted task #{} — {}",
        "✓".green().bold(),
        id.to_string().bold(),
        task.title
    );

    Ok(())
}
