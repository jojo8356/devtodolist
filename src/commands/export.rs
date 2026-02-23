use std::fs;
use std::io::Write;

use colored::Colorize;

use crate::cli::ExportFormat;
use crate::commands::init::find_db;
use crate::error::Result;

pub fn run(format: &ExportFormat, output: Option<&str>, status: Option<&str>) -> Result<()> {
    let db = find_db()?;
    let tasks = db.list_tasks(status, None, None, None, Some("created"), None)?;

    let content = match format {
        ExportFormat::Json => {
            serde_json::to_string_pretty(&tasks)?
        }
        ExportFormat::Csv => {
            let mut out = String::from("id,title,status,priority,branch,base_branch,assignee,created_at,updated_at\n");
            for t in &tasks {
                out.push_str(&format!(
                    "{},\"{}\",{},{},{},{},{},{},{}\n",
                    t.id,
                    t.title.replace('"', "\"\""),
                    t.status,
                    t.priority.as_ref().map(|p| p.as_str()).unwrap_or(""),
                    t.branch.as_deref().unwrap_or(""),
                    t.base_branch.as_deref().unwrap_or(""),
                    t.assignee.as_deref().unwrap_or(""),
                    t.created_at.format("%Y-%m-%dT%H:%M:%S"),
                    t.updated_at.format("%Y-%m-%dT%H:%M:%S"),
                ));
            }
            out
        }
        ExportFormat::Markdown => {
            let mut out = String::from("# Tasks\n\n");
            out.push_str("| ID | Title | Status | Priority | Branch | Assignee |\n");
            out.push_str("|---|---|---|---|---|---|\n");
            for t in &tasks {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    t.id,
                    t.title,
                    t.status,
                    t.priority.as_ref().map(|p| p.as_str()).unwrap_or("-"),
                    t.branch.as_deref().unwrap_or("-"),
                    t.assignee.as_deref().unwrap_or("-"),
                ));
            }
            out
        }
    };

    match output {
        Some(path) => {
            fs::write(path, &content)?;
            println!(
                "{} Exported {} tasks to {}",
                "✓".green().bold(),
                tasks.len(),
                path.bold()
            );
        }
        None => {
            std::io::stdout().write_all(content.as_bytes())?;
        }
    }

    Ok(())
}
