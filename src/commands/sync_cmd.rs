use std::process::Command;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::commands::config::get_value;
use crate::commands::init::find_db;
use crate::error::{DevTodoError, Result};
use crate::models::{Provider, TaskStatus};
use crate::providers::{self, CreatePrRequest, ProviderApi, RemotePr};

fn detect_repo() -> Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| DevTodoError::Git("Failed to run git".into()))?;

    if !output.status.success() {
        return Err(DevTodoError::Git(
            "No git remote 'origin' found. Use --repo to specify.".into(),
        ));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Parse owner/repo from various URL formats
    let repo = url
        .trim_end_matches(".git")
        .rsplit('/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    if repo.len() == 2 {
        // Handle SSH format: extract owner from "host:owner"
        let owner = repo[0].rsplit(':').next().unwrap_or(repo[0]);
        Ok(format!("{}/{}", owner, repo[1]))
    } else {
        Err(DevTodoError::Git(format!(
            "Cannot parse repo from remote URL: {url}"
        )))
    }
}

fn resolve_provider(provider_arg: Option<&str>) -> Result<(Provider, String)> {
    let provider: Provider = if let Some(p) = provider_arg {
        p.parse()?
    } else if let Some(p) = get_value("default.provider")? {
        p.parse()?
    } else {
        return Err(DevTodoError::Config(
            "No provider specified. Use --provider or set default.provider in config.".into(),
        ));
    };

    let token_key = match provider {
        Provider::Github => "github.token",
        Provider::Gitlab => "gitlab.token",
    };

    let token = get_value(token_key)?.ok_or_else(|| {
        DevTodoError::Config(format!(
            "No token found for {provider}. Run: devtodo config set {token_key} <token>"
        ))
    })?;

    Ok((provider, token))
}

fn build_api(provider: &Provider, token: &str) -> Result<Box<dyn ProviderApi>> {
    let base_url = get_value("gitlab.url")?;
    Ok(providers::build_provider(
        provider,
        token,
        base_url.as_deref(),
    ))
}

fn map_remote_status(status: &str) -> TaskStatus {
    match status {
        "draft" => TaskStatus::Draft,
        "review" => TaskStatus::Review,
        "merged" => TaskStatus::Merged,
        "closed" => TaskStatus::Closed,
        _ => TaskStatus::Open,
    }
}

pub fn run_sync(provider_arg: Option<&str>, dry_run: bool) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async_sync(provider_arg, dry_run))
}

async fn async_sync(provider_arg: Option<&str>, dry_run: bool) -> Result<()> {
    let db = find_db()?;
    let (provider, token) = resolve_provider(provider_arg)?;
    let api = build_api(&provider, &token)?;
    let repo = detect_repo()?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Syncing with {} ({})...", provider, repo));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let remote_prs = api.list_prs(&repo, "all").await?;

    let mut created = 0u32;
    let mut updated = 0u32;

    for rpr in &remote_prs {
        // Check if we already track this PR
        let existing = db.list_tasks(
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        let local = existing.iter().find(|t| {
            t.provider.as_ref() == Some(&provider) && t.remote_id == Some(rpr.remote_id)
        });

        if let Some(task) = local {
            let new_status = map_remote_status(&rpr.status);
            if task.status != new_status || task.title != rpr.title {
                if dry_run {
                    println!(
                        "  {} Would update #{}: {} -> {}",
                        "~".yellow(),
                        task.id,
                        task.status,
                        new_status
                    );
                } else {
                    db.update_task_field(task.id, "status", Some(new_status.as_str()))?;
                    db.update_task_field(task.id, "title", Some(&rpr.title))?;
                    if let Some(ref desc) = rpr.description {
                        db.update_task_field(task.id, "description", Some(desc))?;
                    }
                }
                updated += 1;
            }
        } else {
            if dry_run {
                println!(
                    "  {} Would import PR #{}: {}",
                    "+".green(),
                    rpr.remote_id,
                    rpr.title
                );
            } else {
                import_remote_pr(&db, &provider, rpr)?;
            }
            created += 1;
        }
    }

    pb.finish_and_clear();

    if dry_run {
        println!(
            "{} Dry run: {} to create, {} to update",
            "!".yellow().bold(),
            created,
            updated
        );
    } else {
        println!(
            "{} Synced with {}: {} imported, {} updated",
            "✓".green().bold(),
            provider,
            created,
            updated
        );
    }

    Ok(())
}

fn import_remote_pr(
    db: &crate::db::Database,
    provider: &Provider,
    rpr: &RemotePr,
) -> Result<()> {
    let status = map_remote_status(&rpr.status);

    let id = db.insert_task(
        &rpr.title,
        rpr.description.as_deref(),
        &status,
        None,
        rpr.branch.as_deref(),
        rpr.base_branch.as_deref(),
        rpr.author.as_deref(),
    )?;

    db.update_task_field(id, "provider", Some(provider.as_str()))?;
    db.update_task_field(id, "remote_id", Some(&rpr.remote_id.to_string()))?;
    db.update_task_field(id, "source_url", Some(&rpr.source_url))?;

    // Import labels
    for label_name in &rpr.labels {
        if db.get_label_by_name(label_name).is_err() {
            db.insert_label(label_name, None)?;
        }
        db.assign_label(id, label_name)?;
    }

    // Import reviewers
    for reviewer in &rpr.reviewers {
        db.assign_reviewer(id, &reviewer.username)?;
        if reviewer.status != "pending" {
            if let Ok(rs) = reviewer.status.parse() {
                db.update_review_status(id, &reviewer.username, &rs)?;
            }
        }
    }

    // Import comments
    for comment in &rpr.comments {
        db.insert_comment(id, &comment.author, &comment.body)?;
    }

    Ok(())
}

pub fn run_push(id: i64) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async_push(id))
}

async fn async_push(id: i64) -> Result<()> {
    let db = find_db()?;
    let task = db.get_task(id)?;

    let provider = if let Some(ref p) = task.provider {
        (p.clone(), get_value(&format!("{}.token", p.as_str()))?.ok_or_else(|| {
            DevTodoError::Config(format!("No token for {p}"))
        })?)
    } else {
        resolve_provider(None)?
    };

    let api = build_api(&provider.0, &provider.1)?;
    let repo = detect_repo()?;

    let branch = task.branch.as_deref().ok_or_else(|| {
        DevTodoError::Config(format!("Task #{id} has no branch set. Use `devtodo edit {id} --branch <name>`"))
    })?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );

    if let Some(remote_id) = task.remote_id {
        // Update existing PR
        pb.set_message(format!("Updating PR #{remote_id} on {}...", provider.0));
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        api.update_pr_status(&repo, remote_id, task.status.as_str()).await?;
        pb.finish_and_clear();

        println!(
            "{} Updated PR #{} on {} — {}",
            "✓".green().bold(),
            remote_id,
            provider.0,
            task.source_url.as_deref().unwrap_or("")
        );
    } else {
        // Create new PR
        pb.set_message(format!("Creating PR on {}...", provider.0));
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        let labels = db
            .get_labels_for_task(id)?
            .into_iter()
            .map(|l| l.name)
            .collect();
        let reviewers = db
            .list_reviewers(id)?
            .into_iter()
            .map(|r| r.username)
            .collect();

        let req = CreatePrRequest {
            title: task.title.clone(),
            description: task.description.clone(),
            branch: branch.to_string(),
            base_branch: task.base_branch.clone().unwrap_or_else(|| "main".to_string()),
            draft: task.status == TaskStatus::Draft,
            labels,
            reviewers,
        };

        let remote_pr = api.create_pr(&repo, &req).await?;
        pb.finish_and_clear();

        // Update local task with remote info
        db.update_task_field(id, "provider", Some(provider.0.as_str()))?;
        db.update_task_field(id, "remote_id", Some(&remote_pr.remote_id.to_string()))?;
        db.update_task_field(id, "source_url", Some(&remote_pr.source_url))?;

        println!(
            "{} Created PR #{} on {} — {}",
            "✓".green().bold(),
            remote_pr.remote_id,
            provider.0,
            remote_pr.source_url.underline()
        );
    }

    Ok(())
}

pub fn run_pull(provider_arg: Option<&str>, repo_arg: Option<&str>, state: &str) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async_pull(provider_arg, repo_arg, state))
}

async fn async_pull(
    provider_arg: Option<&str>,
    repo_arg: Option<&str>,
    state: &str,
) -> Result<()> {
    let db = find_db()?;
    let (provider, token) = resolve_provider(provider_arg)?;
    let api = build_api(&provider, &token)?;

    let repo = if let Some(r) = repo_arg {
        r.to_string()
    } else {
        detect_repo()?
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Pulling PRs from {} ({})...", provider, repo));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let remote_prs = api.list_prs(&repo, state).await?;
    let mut imported = 0u32;
    let mut skipped = 0u32;

    for rpr in &remote_prs {
        // Check if already tracked
        let existing = db.list_tasks(None, None, None, None, None, None)?;
        let already = existing.iter().any(|t| {
            t.provider.as_ref() == Some(&provider) && t.remote_id == Some(rpr.remote_id)
        });

        if already {
            skipped += 1;
        } else {
            import_remote_pr(&db, &provider, rpr)?;
            imported += 1;
        }
    }

    pb.finish_and_clear();

    println!(
        "{} Pulled from {}: {} imported, {} already tracked",
        "✓".green().bold(),
        provider,
        imported,
        skipped
    );

    Ok(())
}
