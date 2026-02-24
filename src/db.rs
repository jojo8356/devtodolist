use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{DevTodoError, Result};
use crate::models::*;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'draft',
    priority    TEXT,
    branch      TEXT,
    base_branch TEXT,
    provider    TEXT,
    remote_id   INTEGER,
    source_url  TEXT,
    assignee    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
);

CREATE TABLE IF NOT EXISTS labels (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT UNIQUE NOT NULL,
    color TEXT
);

CREATE TABLE IF NOT EXISTS task_labels (
    task_id  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    UNIQUE(task_id, label_id)
);

CREATE TABLE IF NOT EXISTS reviewers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    username    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    reviewed_at TEXT
);

CREATE TABLE IF NOT EXISTS comments (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    author     TEXT NOT NULL,
    body       TEXT NOT NULL,
    remote_id  INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
);
"#;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn init(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    // ── Tasks ──

    #[allow(clippy::too_many_arguments)]
    pub fn insert_task(
        &self,
        title: &str,
        description: Option<&str>,
        status: &TaskStatus,
        priority: Option<&Priority>,
        branch: Option<&str>,
        base_branch: Option<&str>,
        assignee: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO tasks (title, description, status, priority, branch, base_branch, assignee)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                title,
                description,
                status.as_str(),
                priority.map(|p| p.as_str()),
                branch,
                base_branch,
                assignee,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_task(&self, id: i64) -> Result<Task> {
        self.conn
            .query_row("SELECT * FROM tasks WHERE id = ?1", params![id], |row| {
                Ok(row_to_task(row))
            })?
            .map_err(|_| DevTodoError::NotFound("Task".into(), id.to_string()))
    }

    pub fn list_tasks(
        &self,
        status: Option<&str>,
        priority: Option<&str>,
        assignee: Option<&str>,
        label: Option<&str>,
        sort: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<Task>> {
        let mut sql = String::from("SELECT DISTINCT t.* FROM tasks t");
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if label.is_some() {
            sql.push_str(
                " JOIN task_labels tl ON t.id = tl.task_id JOIN labels l ON tl.label_id = l.id",
            );
        }

        if let Some(s) = status {
            conditions.push(format!("t.status = ?{param_idx}"));
            param_values.push(Box::new(s.to_string()));
            param_idx += 1;
        }
        if let Some(p) = priority {
            conditions.push(format!("t.priority = ?{param_idx}"));
            param_values.push(Box::new(p.to_string()));
            param_idx += 1;
        }
        if let Some(a) = assignee {
            conditions.push(format!("t.assignee = ?{param_idx}"));
            param_values.push(Box::new(a.to_string()));
            param_idx += 1;
        }
        if let Some(l) = label {
            conditions.push(format!("l.name = ?{param_idx}"));
            param_values.push(Box::new(l.to_string()));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        let sort_col = match sort {
            Some("updated") => "t.updated_at",
            Some("priority") => "t.priority",
            _ => "t.created_at",
        };
        sql.push_str(&format!(" ORDER BY {sort_col} DESC"));

        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let _ = param_idx; // suppress unused warning
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| Ok(row_to_task(row)))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?.map_err(|_| DevTodoError::Db(rusqlite::Error::QueryReturnedNoRows))?);
        }
        Ok(tasks)
    }

    pub fn update_task_field(&self, id: i64, field: &str, value: Option<&str>) -> Result<()> {
        let allowed = [
            "title",
            "description",
            "status",
            "priority",
            "branch",
            "base_branch",
            "assignee",
            "provider",
            "remote_id",
            "source_url",
        ];
        if !allowed.contains(&field) {
            return Err(DevTodoError::Config(format!(
                "Cannot update field: {field}"
            )));
        }

        let sql = format!(
            "UPDATE tasks SET {field} = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?2"
        );
        let affected = self.conn.execute(&sql, params![value, id])?;
        if affected == 0 {
            return Err(DevTodoError::NotFound("Task".into(), id.to_string()));
        }
        Ok(())
    }

    pub fn delete_task(&self, id: i64) -> Result<()> {
        let affected = self
            .conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DevTodoError::NotFound("Task".into(), id.to_string()));
        }
        Ok(())
    }

    // ── Labels ──

    pub fn insert_label(&self, name: &str, color: Option<&str>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO labels (name, color) VALUES (?1, ?2)",
            params![name, color],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_label_by_name(&self, name: &str) -> Result<Label> {
        self.conn
            .query_row(
                "SELECT id, name, color FROM labels WHERE name = ?1",
                params![name],
                |row| {
                    Ok(Label {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        color: row.get(2)?,
                    })
                },
            )
            .map_err(|_| DevTodoError::NotFound("Label".into(), name.to_string()))
    }

    pub fn list_labels(&self) -> Result<Vec<Label>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color FROM labels ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete_label(&self, name: &str) -> Result<()> {
        let affected = self
            .conn
            .execute("DELETE FROM labels WHERE name = ?1", params![name])?;
        if affected == 0 {
            return Err(DevTodoError::NotFound("Label".into(), name.to_string()));
        }
        Ok(())
    }

    pub fn assign_label(&self, task_id: i64, label_name: &str) -> Result<()> {
        let label = self.get_label_by_name(label_name)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?1, ?2)",
            params![task_id, label.id],
        )?;
        Ok(())
    }

    pub fn unassign_label(&self, task_id: i64, label_name: &str) -> Result<()> {
        let label = self.get_label_by_name(label_name)?;
        self.conn.execute(
            "DELETE FROM task_labels WHERE task_id = ?1 AND label_id = ?2",
            params![task_id, label.id],
        )?;
        Ok(())
    }

    pub fn get_labels_for_task(&self, task_id: i64) -> Result<Vec<Label>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.id, l.name, l.color FROM labels l
             JOIN task_labels tl ON l.id = tl.label_id
             WHERE tl.task_id = ?1 ORDER BY l.name",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Reviewers ──

    pub fn assign_reviewer(&self, task_id: i64, username: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO reviewers (task_id, username) VALUES (?1, ?2)",
            params![task_id, username],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_reviewer(&self, task_id: i64, username: &str) -> Result<()> {
        let affected = self.conn.execute(
            "DELETE FROM reviewers WHERE task_id = ?1 AND username = ?2",
            params![task_id, username],
        )?;
        if affected == 0 {
            return Err(DevTodoError::NotFound(
                "Reviewer".into(),
                format!("{username} on task {task_id}"),
            ));
        }
        Ok(())
    }

    pub fn update_review_status(
        &self,
        task_id: i64,
        username: &str,
        status: &ReviewStatus,
    ) -> Result<()> {
        let affected = self.conn.execute(
            "UPDATE reviewers SET status = ?1, reviewed_at = strftime('%Y-%m-%dT%H:%M:%S', 'now')
             WHERE task_id = ?2 AND username = ?3",
            params![status.as_str(), task_id, username],
        )?;
        if affected == 0 {
            return Err(DevTodoError::NotFound(
                "Reviewer".into(),
                format!("{username} on task {task_id}"),
            ));
        }
        Ok(())
    }

    pub fn list_reviewers(&self, task_id: i64) -> Result<Vec<Reviewer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, username, status, reviewed_at FROM reviewers WHERE task_id = ?1",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            let status_str: String = row.get(3)?;
            let reviewed_at_str: Option<String> = row.get(4)?;
            Ok(Reviewer {
                id: row.get(0)?,
                task_id: row.get(1)?,
                username: row.get(2)?,
                status: status_str.parse().unwrap_or(ReviewStatus::Pending),
                reviewed_at: reviewed_at_str
                    .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S").ok()),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Comments ──

    pub fn insert_comment(&self, task_id: i64, author: &str, body: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO comments (task_id, author, body) VALUES (?1, ?2, ?3)",
            params![task_id, author, body],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_comments(&self, task_id: i64) -> Result<Vec<Comment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, author, body, remote_id, created_at
             FROM comments WHERE task_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            let created_str: String = row.get(5)?;
            Ok(Comment {
                id: row.get(0)?,
                task_id: row.get(1)?,
                author: row.get(2)?,
                body: row.get(3)?,
                remote_id: row.get(4)?,
                created_at: NaiveDateTime::parse_from_str(&created_str, "%Y-%m-%dT%H:%M:%S")
                    .unwrap_or_default(),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Stats helpers ──

    pub fn count_by_status(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM tasks GROUP BY status")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn count_by_priority(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(priority, 'none'), COUNT(*) FROM tasks GROUP BY priority")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn count_by_label(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.name, COUNT(*) FROM task_labels tl
             JOIN labels l ON tl.label_id = l.id GROUP BY l.name",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn avg_merge_time_hours(&self) -> Result<Option<f64>> {
        self.conn
            .query_row(
                "SELECT AVG((julianday(updated_at) - julianday(created_at)) * 24)
                 FROM tasks WHERE status = 'merged'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map(|o| o.flatten())
            .map_err(Into::into)
    }

    pub fn oldest_open_tasks(&self, limit: u32) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tasks WHERE status IN ('open', 'review', 'draft')
             ORDER BY created_at ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?.map_err(|_| DevTodoError::Db(rusqlite::Error::QueryReturnedNoRows))?);
        }
        Ok(tasks)
    }
}

/// Helper to create an in-memory database for testing.
#[cfg(test)]
fn test_db() -> Database {
    let db = Database {
        conn: Connection::open_in_memory().unwrap(),
    };
    db.conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    db.init().unwrap();
    db
}

fn row_to_task(row: &rusqlite::Row<'_>) -> std::result::Result<Task, DevTodoError> {
    let status_str: String = row.get(3)?;
    let priority_str: Option<String> = row.get(4)?;
    let provider_str: Option<String> = row.get(7)?;
    let created_str: String = row.get(11)?;
    let updated_str: String = row.get(12)?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: status_str.parse().unwrap_or(TaskStatus::Draft),
        priority: priority_str.and_then(|s| s.parse().ok()),
        branch: row.get(5)?,
        base_branch: row.get(6)?,
        provider: provider_str.and_then(|s| s.parse().ok()),
        remote_id: row.get(8)?,
        source_url: row.get(9)?,
        assignee: row.get(10)?,
        created_at: NaiveDateTime::parse_from_str(&created_str, "%Y-%m-%dT%H:%M:%S")
            .unwrap_or_default(),
        updated_at: NaiveDateTime::parse_from_str(&updated_str, "%Y-%m-%dT%H:%M:%S")
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Task CRUD ──

    #[test]
    fn insert_and_get_task() {
        let db = test_db();
        let id = db
            .insert_task(
                "Fix login bug",
                Some("Details here"),
                &TaskStatus::Open,
                Some(&Priority::High),
                Some("fix/login"),
                Some("main"),
                Some("alice"),
            )
            .unwrap();
        assert_eq!(id, 1);

        let task = db.get_task(id).unwrap();
        assert_eq!(task.title, "Fix login bug");
        assert_eq!(task.description.as_deref(), Some("Details here"));
        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(task.priority, Some(Priority::High));
        assert_eq!(task.branch.as_deref(), Some("fix/login"));
        assert_eq!(task.base_branch.as_deref(), Some("main"));
        assert_eq!(task.assignee.as_deref(), Some("alice"));
    }

    #[test]
    fn get_task_not_found() {
        let db = test_db();
        assert!(db.get_task(999).is_err());
    }

    #[test]
    fn update_task_field() {
        let db = test_db();
        let id = db
            .insert_task("Task", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();

        db.update_task_field(id, "title", Some("Updated title"))
            .unwrap();
        db.update_task_field(id, "status", Some("review")).unwrap();

        let task = db.get_task(id).unwrap();
        assert_eq!(task.title, "Updated title");
        assert_eq!(task.status, TaskStatus::Review);
    }

    #[test]
    fn update_invalid_field_rejected() {
        let db = test_db();
        let id = db
            .insert_task("Task", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        assert!(db.update_task_field(id, "evil_field", Some("x")).is_err());
    }

    #[test]
    fn delete_task() {
        let db = test_db();
        let id = db
            .insert_task(
                "To delete",
                None,
                &TaskStatus::Draft,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        db.delete_task(id).unwrap();
        assert!(db.get_task(id).is_err());
    }

    #[test]
    fn delete_task_not_found() {
        let db = test_db();
        assert!(db.delete_task(999).is_err());
    }

    #[test]
    fn delete_task_cascades_labels() {
        let db = test_db();
        let id = db
            .insert_task("Task", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        db.insert_label("bug", None).unwrap();
        db.assign_label(id, "bug").unwrap();

        db.delete_task(id).unwrap();
        // Label still exists but association is gone
        let labels = db.list_labels().unwrap();
        assert_eq!(labels.len(), 1);
    }

    // ── List & Filters ──

    #[test]
    fn list_tasks_empty() {
        let db = test_db();
        let tasks = db.list_tasks(None, None, None, None, None, None).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn list_tasks_filter_by_status() {
        let db = test_db();
        db.insert_task("A", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();
        db.insert_task("B", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        db.insert_task("C", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();

        let open = db
            .list_tasks(Some("open"), None, None, None, None, None)
            .unwrap();
        assert_eq!(open.len(), 2);

        let draft = db
            .list_tasks(Some("draft"), None, None, None, None, None)
            .unwrap();
        assert_eq!(draft.len(), 1);
    }

    #[test]
    fn list_tasks_with_limit() {
        let db = test_db();
        for i in 0..10 {
            db.insert_task(
                &format!("Task {i}"),
                None,
                &TaskStatus::Open,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        let tasks = db
            .list_tasks(None, None, None, None, None, Some(3))
            .unwrap();
        assert_eq!(tasks.len(), 3);
    }

    // ── Labels ──

    #[test]
    fn label_crud() {
        let db = test_db();
        db.insert_label("bug", Some("#ff0000")).unwrap();
        db.insert_label("feature", None).unwrap();

        let labels = db.list_labels().unwrap();
        assert_eq!(labels.len(), 2);

        let bug = db.get_label_by_name("bug").unwrap();
        assert_eq!(bug.color.as_deref(), Some("#ff0000"));

        db.delete_label("bug").unwrap();
        let labels = db.list_labels().unwrap();
        assert_eq!(labels.len(), 1);
    }

    #[test]
    fn label_not_found() {
        let db = test_db();
        assert!(db.get_label_by_name("nonexistent").is_err());
        assert!(db.delete_label("nonexistent").is_err());
    }

    #[test]
    fn assign_and_unassign_label() {
        let db = test_db();
        let id = db
            .insert_task("T", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        db.insert_label("bug", None).unwrap();
        db.insert_label("feature", None).unwrap();

        db.assign_label(id, "bug").unwrap();
        db.assign_label(id, "feature").unwrap();

        let labels = db.get_labels_for_task(id).unwrap();
        assert_eq!(labels.len(), 2);

        db.unassign_label(id, "bug").unwrap();
        let labels = db.get_labels_for_task(id).unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].name, "feature");
    }

    #[test]
    fn assign_label_idempotent() {
        let db = test_db();
        let id = db
            .insert_task("T", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        db.insert_label("bug", None).unwrap();
        db.assign_label(id, "bug").unwrap();
        db.assign_label(id, "bug").unwrap(); // INSERT OR IGNORE
        let labels = db.get_labels_for_task(id).unwrap();
        assert_eq!(labels.len(), 1);
    }

    #[test]
    fn filter_tasks_by_label() {
        let db = test_db();
        let id1 = db
            .insert_task("A", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();
        let _id2 = db
            .insert_task("B", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();

        db.insert_label("bug", None).unwrap();
        db.assign_label(id1, "bug").unwrap();

        let tasks = db
            .list_tasks(None, None, None, Some("bug"), None, None)
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "A");
    }

    // ── Reviewers ──

    #[test]
    fn reviewer_crud() {
        let db = test_db();
        let id = db
            .insert_task("T", None, &TaskStatus::Review, None, None, None, None)
            .unwrap();

        db.assign_reviewer(id, "alice").unwrap();
        db.assign_reviewer(id, "bob").unwrap();

        let reviewers = db.list_reviewers(id).unwrap();
        assert_eq!(reviewers.len(), 2);
        assert_eq!(reviewers[0].status, ReviewStatus::Pending);

        db.update_review_status(id, "alice", &ReviewStatus::Approved)
            .unwrap();
        let reviewers = db.list_reviewers(id).unwrap();
        let alice = reviewers.iter().find(|r| r.username == "alice").unwrap();
        assert_eq!(alice.status, ReviewStatus::Approved);
        assert!(alice.reviewed_at.is_some());

        db.remove_reviewer(id, "bob").unwrap();
        let reviewers = db.list_reviewers(id).unwrap();
        assert_eq!(reviewers.len(), 1);
    }

    #[test]
    fn remove_reviewer_not_found() {
        let db = test_db();
        let id = db
            .insert_task("T", None, &TaskStatus::Draft, None, None, None, None)
            .unwrap();
        assert!(db.remove_reviewer(id, "nobody").is_err());
    }

    // ── Comments ──

    #[test]
    fn comment_crud() {
        let db = test_db();
        let id = db
            .insert_task("T", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();

        db.insert_comment(id, "alice", "Looks good").unwrap();
        db.insert_comment(id, "bob", "Needs changes").unwrap();

        let comments = db.list_comments(id).unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].author, "alice");
        assert_eq!(comments[1].body, "Needs changes");
    }

    // ── Stats ──

    #[test]
    fn stats_count_by_status() {
        let db = test_db();
        db.insert_task("A", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();
        db.insert_task("B", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();
        db.insert_task("C", None, &TaskStatus::Merged, None, None, None, None)
            .unwrap();

        let counts = db.count_by_status().unwrap();
        let open_count = counts
            .iter()
            .find(|(s, _)| s == "open")
            .map(|(_, c)| *c)
            .unwrap_or(0);
        assert_eq!(open_count, 2);
        let merged_count = counts
            .iter()
            .find(|(s, _)| s == "merged")
            .map(|(_, c)| *c)
            .unwrap_or(0);
        assert_eq!(merged_count, 1);
    }

    #[test]
    fn stats_oldest_open() {
        let db = test_db();
        db.insert_task("Old", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();
        db.insert_task("Closed", None, &TaskStatus::Closed, None, None, None, None)
            .unwrap();
        db.insert_task("New", None, &TaskStatus::Open, None, None, None, None)
            .unwrap();

        let oldest = db.oldest_open_tasks(5).unwrap();
        assert_eq!(oldest.len(), 2); // Only open/review/draft
        assert_eq!(oldest[0].title, "Old");
    }
}
