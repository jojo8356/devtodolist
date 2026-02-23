use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, ContentArrangement, Table};

use crate::models::*;

pub fn colorize_status(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Draft => "draft".dimmed().to_string(),
        TaskStatus::Open => "open".green().to_string(),
        TaskStatus::Review => "review".yellow().to_string(),
        TaskStatus::Merged => "merged".purple().to_string(),
        TaskStatus::Closed => "closed".red().to_string(),
    }
}

pub fn colorize_priority(priority: &Priority) -> String {
    match priority {
        Priority::Low => "low".dimmed().to_string(),
        Priority::Medium => "medium".white().to_string(),
        Priority::High => "high".yellow().bold().to_string(),
        Priority::Critical => "critical".red().bold().to_string(),
    }
}

pub fn colorize_review_status(status: &ReviewStatus) -> String {
    match status {
        ReviewStatus::Pending => "pending".yellow().to_string(),
        ReviewStatus::Approved => "approved".green().to_string(),
        ReviewStatus::ChangesRequested => "changes_requested".red().to_string(),
    }
}

pub fn print_task_table(tasks: &[Task]) {
    if tasks.is_empty() {
        println!("{}", "No tasks found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID"),
            Cell::new("Title"),
            Cell::new("Status"),
            Cell::new("Priority"),
            Cell::new("Branch"),
            Cell::new("Assignee"),
        ]);

    for task in tasks {
        table.add_row(vec![
            Cell::new(task.id),
            Cell::new(&task.title),
            Cell::new(colorize_status(&task.status)),
            Cell::new(
                task.priority
                    .as_ref()
                    .map(|p| colorize_priority(p))
                    .unwrap_or_else(|| "-".dimmed().to_string()),
            ),
            Cell::new(task.branch.as_deref().unwrap_or("-")),
            Cell::new(task.assignee.as_deref().unwrap_or("-")),
        ]);
    }

    println!("{table}");
}

pub fn print_task_detail(task: &Task, labels: &[Label], reviewers: &[Reviewer]) {
    println!("{}", format!("PR #{}", task.id).bold());
    println!("  {} {}", "Title:".bold(), task.title);
    println!("  {} {}", "Status:".bold(), colorize_status(&task.status));

    if let Some(ref p) = task.priority {
        println!("  {} {}", "Priority:".bold(), colorize_priority(p));
    }
    if let Some(ref desc) = task.description {
        println!("  {} {}", "Description:".bold(), desc);
    }
    if let Some(ref branch) = task.branch {
        println!(
            "  {} {} -> {}",
            "Branch:".bold(),
            branch.cyan(),
            task.base_branch.as_deref().unwrap_or("main")
        );
    }
    if let Some(ref assignee) = task.assignee {
        println!("  {} {}", "Assignee:".bold(), assignee);
    }
    if let Some(ref url) = task.source_url {
        println!("  {} {}", "URL:".bold(), url.underline());
    }

    if !labels.is_empty() {
        let label_str: Vec<String> = labels.iter().map(|l| format!("[{}]", l.name)).collect();
        println!("  {} {}", "Labels:".bold(), label_str.join(" "));
    }

    if !reviewers.is_empty() {
        println!("  {}", "Reviewers:".bold());
        for r in reviewers {
            println!(
                "    - {} ({})",
                r.username,
                colorize_review_status(&r.status)
            );
        }
    }

    println!(
        "  {} {}",
        "Created:".bold(),
        task.created_at.format("%Y-%m-%d %H:%M")
    );
    println!(
        "  {} {}",
        "Updated:".bold(),
        task.updated_at.format("%Y-%m-%d %H:%M")
    );
}

pub fn print_comments(comments: &[Comment]) {
    if comments.is_empty() {
        println!("{}", "  No comments.".dimmed());
        return;
    }
    println!("  {}", "Comments:".bold());
    for c in comments {
        println!(
            "    {} {} ({})",
            c.author.bold(),
            c.created_at.format("%Y-%m-%d %H:%M").to_string().dimmed(),
            if c.remote_id.is_some() { "remote" } else { "local" }
        );
        println!("      {}", c.body);
    }
}

pub fn print_labels_table(labels: &[Label]) {
    if labels.is_empty() {
        println!("{}", "No labels found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("ID"),
            Cell::new("Name"),
            Cell::new("Color"),
        ]);

    for label in labels {
        table.add_row(vec![
            Cell::new(label.id),
            Cell::new(&label.name),
            Cell::new(label.color.as_deref().unwrap_or("-")),
        ]);
    }

    println!("{table}");
}

pub fn print_reviewers_table(reviewers: &[Reviewer]) {
    if reviewers.is_empty() {
        println!("{}", "No reviewers assigned.".dimmed());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Username"),
            Cell::new("Status"),
            Cell::new("Reviewed at"),
        ]);

    for r in reviewers {
        table.add_row(vec![
            Cell::new(&r.username),
            Cell::new(colorize_review_status(&r.status)),
            Cell::new(
                r.reviewed_at
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ]);
    }

    println!("{table}");
}

pub fn print_stats(
    by_status: &[(String, i64)],
    by_priority: &[(String, i64)],
    by_label: &[(String, i64)],
    avg_merge_hours: Option<f64>,
    oldest: &[Task],
) {
    println!("{}", "=== Task Statistics ===".bold());
    println!();

    println!("{}", "By status:".bold());
    for (status, count) in by_status {
        println!("  {:<15} {}", status, count);
    }
    println!();

    println!("{}", "By priority:".bold());
    for (priority, count) in by_priority {
        println!("  {:<15} {}", priority, count);
    }
    println!();

    if !by_label.is_empty() {
        println!("{}", "By label:".bold());
        for (label, count) in by_label {
            println!("  {:<15} {}", label, count);
        }
        println!();
    }

    if let Some(hours) = avg_merge_hours {
        if hours < 24.0 {
            println!("{} {:.1}h", "Avg time to merge:".bold(), hours);
        } else {
            println!("{} {:.1}d", "Avg time to merge:".bold(), hours / 24.0);
        }
        println!();
    }

    if !oldest.is_empty() {
        println!("{}", "Oldest open tasks:".bold());
        for t in oldest {
            println!(
                "  #{} {} ({})",
                t.id,
                t.title,
                t.created_at.format("%Y-%m-%d")
            );
        }
    }
}
