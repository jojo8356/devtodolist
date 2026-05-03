use std::process::Command;

use colored::Colorize;
use comfy_table::{
    Cell, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL,
};

use crate::cli::ProofCommands;
use crate::commands::init::find_db;
use crate::error::{DevTodoError, Result};

pub async fn run(command: &ProofCommands) -> Result<()> {
    let db = find_db().await?;
    match command {
        ProofCommands::Add { task_id, commit } => {
            let info = git_show(commit)?;
            db.add_proof(
                *task_id,
                &info.full_hash,
                Some(&info.short_hash),
                Some(&info.author),
                Some(&info.message),
                Some(&info.committed_at),
            )
            .await?;
            println!(
                "{} Attached commit {} ({}) to task #{}",
                "✓".green().bold(),
                info.short_hash.cyan(),
                info.message.lines().next().unwrap_or("").dimmed(),
                task_id
            );
        }
        ProofCommands::Auto { task_id } => {
            let task = db.get_task(*task_id).await?;
            let branch = task
                .branch
                .as_deref()
                .ok_or(DevTodoError::NoBranch(*task_id))?;
            let base = task.base_branch.as_deref().unwrap_or("main");
            let commits = git_log_range(base, branch)?;
            if commits.is_empty() {
                println!(
                    "{} No commits found in {}..{}",
                    "!".yellow().bold(),
                    base,
                    branch
                );
                return Ok(());
            }
            let mut imported = 0u32;
            for c in &commits {
                db.add_proof(
                    *task_id,
                    &c.full_hash,
                    Some(&c.short_hash),
                    Some(&c.author),
                    Some(&c.message),
                    Some(&c.committed_at),
                )
                .await?;
                imported += 1;
            }
            println!(
                "{} Imported {} commit(s) from {}..{} as proofs for #{}",
                "✓".green().bold(),
                imported,
                base,
                branch,
                task_id
            );
        }
        ProofCommands::List { task_id } => {
            let proofs = db.list_proofs(*task_id).await?;
            if proofs.is_empty() {
                println!(
                    "{}",
                    format!("No commit proofs for task #{task_id}.").dimmed()
                );
                return Ok(());
            }
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("Commit"),
                    Cell::new("Author"),
                    Cell::new("Message"),
                    Cell::new("Date"),
                ]);
            for p in proofs {
                table.add_row(vec![
                    Cell::new(
                        p.short_hash
                            .unwrap_or_else(|| p.commit_hash.chars().take(7).collect()),
                    ),
                    Cell::new(p.author.unwrap_or_else(|| "-".into())),
                    Cell::new(
                        p.message
                            .as_deref()
                            .and_then(|m| m.lines().next())
                            .unwrap_or("-"),
                    ),
                    Cell::new(p.committed_at.unwrap_or_else(|| "-".into())),
                ]);
            }
            println!("{table}");
        }
        ProofCommands::Remove { task_id, commit } => {
            let resolved = git_show(commit)
                .map(|i| i.full_hash)
                .unwrap_or_else(|_| commit.clone());
            // Try the resolved (full) hash first, then the user-provided form.
            if db.remove_proof(*task_id, &resolved).await.is_err() {
                db.remove_proof(*task_id, commit).await?;
            }
            println!(
                "{} Removed commit {} from task #{}",
                "✓".green().bold(),
                commit,
                task_id
            );
        }
        ProofCommands::Verify { task_id } => {
            let proofs = db.list_proofs(*task_id).await?;
            if proofs.is_empty() {
                println!("{}", format!("No proofs for #{task_id}.").dimmed());
                return Ok(());
            }
            let mut ok = 0u32;
            let mut missing = 0u32;
            for p in &proofs {
                if git_show(&p.commit_hash).is_ok() {
                    println!(
                        "  {} {}",
                        "✓".green(),
                        &p.commit_hash[..7.min(p.commit_hash.len())]
                    );
                    ok += 1;
                } else {
                    println!(
                        "  {} {} (not found in repo)",
                        "✗".red(),
                        &p.commit_hash[..7.min(p.commit_hash.len())]
                    );
                    missing += 1;
                }
            }
            println!("{} {} valid, {} missing", "Verified:".bold(), ok, missing);
        }
    }
    Ok(())
}

struct CommitInfo {
    full_hash: String,
    short_hash: String,
    author: String,
    message: String,
    committed_at: String,
}

fn git_show(commit: &str) -> Result<CommitInfo> {
    let format = "%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let output = Command::new("git")
        .args(["show", "-s", &format!("--format={format}"), commit])
        .output()
        .map_err(|e| DevTodoError::GitNotAvailable(e.to_string()))?;

    if !output.status.success() {
        return Err(DevTodoError::CommitNotFound {
            commit: commit.to_string(),
            reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = line.split('\x1f').collect();
    if parts.len() < 5 {
        return Err(DevTodoError::Git(format!(
            "Unexpected git output for {commit}"
        )));
    }
    Ok(CommitInfo {
        full_hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        author: parts[2].to_string(),
        message: parts[4].to_string(),
        committed_at: parts[3].to_string(),
    })
}

fn git_log_range(base: &str, head: &str) -> Result<Vec<CommitInfo>> {
    let format = "%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let range = format!("{base}..{head}");
    let output = Command::new("git")
        .args(["log", &format!("--format={format}"), &range])
        .output()
        .map_err(|e| DevTodoError::GitNotAvailable(e.to_string()))?;

    if !output.status.success() {
        return Err(DevTodoError::Git(format!(
            "git log failed for {range}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() >= 5 {
            out.push(CommitInfo {
                full_hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                author: parts[2].to_string(),
                message: parts[4].to_string(),
                committed_at: parts[3].to_string(),
            });
        }
    }
    Ok(out)
}
