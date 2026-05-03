use colored::Colorize;
use comfy_table::{Cell, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};

use crate::cli::RoleCommands;
use crate::commands::init::find_db;
use crate::error::Result;

pub async fn run(command: &RoleCommands) -> Result<()> {
    let db = find_db().await?;
    match command {
        RoleCommands::Set { username, role } => {
            db.set_role(username, role).await?;
            println!(
                "{} {} is now {}",
                "✓".green().bold(),
                username.bold(),
                role
            );
        }
        RoleCommands::Remove { username } => {
            db.remove_role(username).await?;
            println!("{} Removed role for {}", "✓".green().bold(), username);
        }
        RoleCommands::Get { username } => match db.get_role(username).await? {
            Some(role) => println!("{} = {}", username.bold(), role),
            None => println!("{} No role set for {}", "!".yellow().bold(), username),
        },
        RoleCommands::List => {
            let roles = db.list_roles().await?;
            if roles.is_empty() {
                println!("{}", "No roles configured.".dimmed());
                return Ok(());
            }
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec![Cell::new("Username"), Cell::new("Role")]);
            for r in roles {
                table.add_row(vec![Cell::new(r.username), Cell::new(r.role)]);
            }
            println!("{table}");
        }
    }
    Ok(())
}
