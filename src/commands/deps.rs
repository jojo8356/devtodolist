use colored::Colorize;

use crate::cli::DepsCommands;
use crate::commands::init::find_db;
use crate::db::Database;
use crate::display;
use crate::error::Result;

pub async fn run(command: &DepsCommands) -> Result<()> {
    let db = find_db().await?;
    match command {
        DepsCommands::Add { task_id, on } => {
            db.add_dependency(*task_id, *on).await?;
            println!("{} #{} now depends on #{}", "✓".green().bold(), task_id, on);
        }
        DepsCommands::Remove { task_id, on } => {
            db.remove_dependency(*task_id, *on).await?;
            println!(
                "{} Removed dependency #{} -> #{}",
                "✓".green().bold(),
                task_id,
                on
            );
        }
        DepsCommands::List { task_id } => {
            let deps = db.list_dependencies(*task_id).await?;
            if deps.is_empty() {
                println!(
                    "{}",
                    format!("Task #{task_id} has no dependencies.").dimmed()
                );
            } else {
                println!("{}", format!("Task #{task_id} depends on:").bold());
                display::print_task_table(&deps);
            }
        }
        DepsCommands::Dependents { task_id } => {
            let deps = db.list_dependents(*task_id).await?;
            if deps.is_empty() {
                println!("{}", format!("No tasks depend on #{task_id}.").dimmed());
            } else {
                println!("{}", format!("Tasks blocked by #{task_id}:").bold());
                display::print_task_table(&deps);
            }
        }
        DepsCommands::Tree { task_id } => {
            let task = db.get_task(*task_id).await?;
            println!("#{} {}", task.id, task.title.bold());
            print_tree_recursive(&db, *task_id, 1, &mut Vec::new()).await?;
        }
    }
    Ok(())
}

/// Recursively prints the dependency tree.
/// Returns a boxed future because Rust async fns can't recurse directly.
fn print_tree_recursive<'a>(
    db: &'a Database,
    task_id: i64,
    depth: usize,
    visited: &'a mut Vec<i64>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if visited.contains(&task_id) {
            println!("{}↻ #{} (cycle)", "  ".repeat(depth), task_id);
            return Ok(());
        }
        visited.push(task_id);

        let deps = db.list_dependencies(task_id).await?;
        for dep in deps {
            let marker = match dep.status.as_str() {
                "merged" | "closed" => "✓".green().to_string(),
                _ => "·".yellow().to_string(),
            };
            println!(
                "{}{} #{} {} [{}]",
                "  ".repeat(depth),
                marker,
                dep.id,
                dep.title,
                dep.status
            );
            print_tree_recursive(db, dep.id, depth + 1, visited).await?;
        }

        visited.pop();
        Ok(())
    })
}
