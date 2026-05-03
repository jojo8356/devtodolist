use crate::commands::init::find_db;
use crate::display;
use crate::error::Result;

pub async fn run(id: i64, show_comments: bool, json: bool) -> Result<()> {
    let db = find_db().await?;
    let task = db.get_task(id).await?;

    if json {
        let labels = db.get_labels_for_task(id).await?;
        let reviewers = db.list_reviewers(id).await?;
        let comments = if show_comments {
            db.list_comments(id).await?
        } else {
            vec![]
        };

        let output = serde_json::json!({
            "task": task,
            "labels": labels,
            "reviewers": reviewers,
            "comments": comments,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let labels = db.get_labels_for_task(id).await?;
    let reviewers = db.list_reviewers(id).await?;
    display::print_task_detail(&task, &labels, &reviewers);

    if show_comments {
        let comments = db.list_comments(id).await?;
        display::print_comments(&comments);
    }

    Ok(())
}
