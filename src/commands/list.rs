use crate::commands::dateparse::{parse_to_db_format, parse_to_db_format_end};
use crate::commands::init::find_db;
use crate::db::{DepsFilter, TaskFilter};
use crate::display;
use crate::error::Result;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    status: Option<&str>,
    label: Option<&str>,
    priority: Option<&str>,
    assignee: Option<&str>,
    role: Option<&str>,
    created_from: Option<&str>,
    created_to: Option<&str>,
    updated_from: Option<&str>,
    updated_to: Option<&str>,
    has_deps: bool,
    no_deps: bool,
    blocked: bool,
    ready: bool,
    sort: &str,
    limit: Option<u32>,
) -> Result<()> {
    let db = find_db().await?;

    let created_from = created_from.map(parse_to_db_format).transpose()?;
    let created_to = created_to.map(parse_to_db_format_end).transpose()?;
    let updated_from = updated_from.map(parse_to_db_format).transpose()?;
    let updated_to = updated_to.map(parse_to_db_format_end).transpose()?;

    let deps_filter = if blocked {
        DepsFilter::Blocked
    } else if ready {
        DepsFilter::Ready
    } else if has_deps {
        DepsFilter::HasDeps
    } else if no_deps {
        DepsFilter::NoDeps
    } else {
        DepsFilter::Any
    };

    let tasks = db
        .list_tasks_filtered(TaskFilter {
            status,
            priority,
            assignee,
            label,
            role,
            created_from: created_from.as_deref(),
            created_to: created_to.as_deref(),
            updated_from: updated_from.as_deref(),
            updated_to: updated_to.as_deref(),
            deps_filter,
            sort: Some(sort),
            limit,
        })
        .await?;
    display::print_task_table(&tasks);
    Ok(())
}
