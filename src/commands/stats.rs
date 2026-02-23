use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub fn run(_period: &str) -> Result<()> {
    let db = find_db()?;

    let by_status = db.count_by_status()?;
    let by_priority = db.count_by_priority()?;
    let by_label = db.count_by_label()?;
    let avg_merge = db.avg_merge_time_hours()?;
    let oldest = db.oldest_open_tasks(5)?;

    display::print_stats(&by_status, &by_priority, &by_label, avg_merge, &oldest);

    Ok(())
}
