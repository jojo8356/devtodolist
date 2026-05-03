use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub async fn run(_period: &str) -> Result<()> {
    let db = find_db().await?;

    let by_status = db.count_by_status().await?;
    let by_priority = db.count_by_priority().await?;
    let by_label = db.count_by_label().await?;
    let avg_merge = db.avg_merge_time_hours().await?;
    let oldest = db.oldest_open_tasks(5).await?;

    display::print_stats(&by_status, &by_priority, &by_label, avg_merge, &oldest);

    Ok(())
}
