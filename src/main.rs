pub mod cli;
pub mod commands;
pub mod db;
pub mod display;
pub mod error;
pub mod models;
pub mod providers;

use std::io;

use clap::{CommandFactory, Parser};
use clap_complete::generate;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init => commands::init::run(),

        Commands::Add {
            title,
            description,
            priority,
            branch,
            base,
            label,
            assignee,
        } => commands::add::run(
            title,
            description.as_deref(),
            priority.as_deref(),
            branch.as_deref(),
            base,
            label,
            assignee.as_deref(),
        ),

        Commands::List {
            status,
            label,
            priority,
            assignee,
            sort,
            limit,
        } => commands::list::run(
            status.as_deref(),
            label.as_deref(),
            priority.as_deref(),
            assignee.as_deref(),
            sort,
            *limit,
        ),

        Commands::Show { id, comments, json } => commands::show::run(*id, *comments, *json),

        Commands::Edit {
            id,
            title,
            description,
            priority,
            branch,
            assignee,
        } => commands::edit::run(
            *id,
            title.as_deref(),
            description.as_deref(),
            priority.as_deref(),
            branch.as_deref(),
            assignee.as_deref(),
        ),

        Commands::Status { id, status } => commands::status::run(*id, status),

        Commands::Delete { id, force } => commands::delete::run(*id, *force),

        Commands::Label { command } => commands::label::run(command),

        Commands::Review { command } => commands::review::run(command),

        Commands::Sync { provider, dry_run } => {
            commands::sync_cmd::run_sync(provider.as_deref(), *dry_run)
        }

        Commands::Push { id } => commands::sync_cmd::run_push(*id),

        Commands::Pull {
            provider,
            repo,
            state,
        } => commands::sync_cmd::run_pull(provider.as_deref(), repo.as_deref(), state),

        Commands::Stats { period } => commands::stats::run(period),

        Commands::Export {
            format,
            output,
            status,
        } => commands::export::run(format, output.as_deref(), status.as_deref()),

        Commands::Config { command } => commands::config::run(command),

        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(*shell, &mut cmd, "devtodo", &mut io::stdout());
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{}: {}", colored::Colorize::red("Error"), e);
        std::process::exit(1);
    }
}
