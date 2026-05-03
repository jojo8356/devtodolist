use std::io;

use clap::{CommandFactory, Parser};
use clap_complete::generate;

use devtodo::cli::{Cli, Commands};
use devtodo::commands;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init => commands::init::run().await,

        Commands::Add {
            title,
            description,
            priority,
            branch,
            base,
            label,
            assignee,
        } => {
            commands::add::run(
                title,
                description.as_deref(),
                priority.as_deref(),
                branch.as_deref(),
                base,
                label,
                assignee.as_deref(),
            )
            .await
        }

        Commands::List {
            status,
            label,
            priority,
            assignee,
            role,
            created_from,
            created_to,
            updated_from,
            updated_to,
            has_deps,
            no_deps,
            blocked,
            ready,
            sort,
            limit,
        } => {
            commands::list::run(
                status.as_deref(),
                label.as_deref(),
                priority.as_deref(),
                assignee.as_deref(),
                role.as_deref(),
                created_from.as_deref(),
                created_to.as_deref(),
                updated_from.as_deref(),
                updated_to.as_deref(),
                *has_deps,
                *no_deps,
                *blocked,
                *ready,
                sort,
                *limit,
            )
            .await
        }

        Commands::Show { id, comments, json } => commands::show::run(*id, *comments, *json).await,

        Commands::Edit {
            id,
            title,
            description,
            priority,
            branch,
            assignee,
        } => {
            commands::edit::run(
                *id,
                title.as_deref(),
                description.as_deref(),
                priority.as_deref(),
                branch.as_deref(),
                assignee.as_deref(),
            )
            .await
        }

        Commands::Status { id, status } => commands::status::run(*id, status).await,

        Commands::Delete { id, force } => commands::delete::run(*id, *force).await,

        Commands::Label { command } => commands::label::run(command).await,

        Commands::Review { command } => commands::review::run(command).await,

        Commands::Sync { provider, dry_run } => {
            commands::sync_cmd::run_sync(provider.as_deref(), *dry_run).await
        }

        Commands::Push { id } => commands::sync_cmd::run_push(*id).await,

        Commands::Pull {
            provider,
            repo,
            state,
        } => commands::sync_cmd::run_pull(provider.as_deref(), repo.as_deref(), state).await,

        Commands::Stats { period } => commands::stats::run(period).await,

        Commands::Export {
            format,
            output,
            status,
        } => commands::export::run(format, output.as_deref(), status.as_deref()).await,

        Commands::Config { command } => commands::config::run(command),

        Commands::Profile => commands::profile::run().await,

        Commands::Deps { command } => commands::deps::run(command).await,

        Commands::Role { command } => commands::role::run(command).await,

        Commands::Proof { command } => commands::proof::run(command).await,

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
