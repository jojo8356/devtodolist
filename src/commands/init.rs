use std::path::Path;
use std::process::Command;

use colored::Colorize;

use crate::db::Database;
use crate::error::Result;

const DB_FILE: &str = ".devtodo.db";

pub fn run() -> Result<()> {
    if Path::new(DB_FILE).exists() {
        println!(
            "{} {} already exists in this directory.",
            "Warning:".yellow().bold(),
            DB_FILE
        );
    }

    let db = Database::open(DB_FILE)?;
    db.init()?;

    println!(
        "{} Initialized devtodo database ({})",
        "✓".green().bold(),
        DB_FILE
    );

    // Detect git remote
    if let Ok(output) = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        && output.status.success()
    {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("  {} {}", "Git remote detected:".dimmed(), url);
    }

    Ok(())
}

pub fn find_db() -> Result<Database> {
    if !Path::new(DB_FILE).exists() {
        return Err(crate::error::DevTodoError::Config(
            "No .devtodo.db found. Run `devtodo init` first.".into(),
        ));
    }
    Database::open(DB_FILE)
}
