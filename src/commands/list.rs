use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub fn run(
    status: Option<&str>,
    label: Option<&str>,
    priority: Option<&str>,
    assignee: Option<&str>,
    sort: &str,
    limit: Option<u32>,
) -> Result<()> {
    let db = find_db()?;
    let tasks = db.list_tasks(status, priority, assignee, label, Some(sort), limit)?;
    display::print_task_table(&tasks);
    Ok(())
}
