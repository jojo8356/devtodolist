//! Async data layer built on SeaORM.
//!
//! `Database` is a thin facade that wraps a `DatabaseConnection` and exposes
//! the same task/label/reviewer/comment/dep/role/proof operations the rest of
//! the codebase already calls. The public method shapes are kept close to the
//! pre-SeaORM API to minimize churn in command modules.
//!
//! Migrations are managed by the `migration::Migrator` (see `src/migration/`),
//! which replaces the previous PRAGMA `user_version` watermark.

use chrono::NaiveDate;
use sea_orm::sea_query::{
    self, Alias, CommonTableExpression, Expr, Func, JoinType, Query, SelectStatement, UnionType,
    WithClause,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, IntoSimpleExpr, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use sea_orm_migration::MigratorTrait;

use crate::entities::prelude::*;
use crate::entities::{
    achievement_unlocked, comment, dev_role, gamification, label, reviewer, task, task_commit,
    task_dependency, task_label,
};
use crate::error::{DevTodoError, Result};
use crate::gamification::{Profile, level_for_xp};
use crate::migration::Migrator;
use crate::models::{
    self as m, CommitProof, DevRole as DomainDevRole, Label as DomainLabel, Priority, Reviewer as DomainReviewer,
    Task as DomainTask, TaskStatus,
};

pub struct Database {
    pub conn: DatabaseConnection,
}

/// Statuses that count as "still open" for the dep-blocking check. Centralised
/// so the SQL builder paths and the dep-tree pretty-printer agree.
const FINISHED_STATUSES: &[&str] = &["merged", "closed"];

/// `EXISTS (SELECT 1 FROM task_dependencies d WHERE d.task_id = t.id)` —
/// "this task has at least one dependency".
fn deps_exists_subquery() -> SelectStatement {
    Query::select()
        .expr(Expr::val(1))
        .from(task_dependency::Entity)
        .and_where(
            Expr::col((task_dependency::Entity, task_dependency::Column::TaskId))
                .equals((task::Entity, task::Column::Id)),
        )
        .to_owned()
}

/// `EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id = d.depends_on
///          WHERE d.task_id = t.id AND p.status NOT IN ('merged','closed'))` —
/// "this task is blocked by at least one unfinished dep".
fn blocked_exists_subquery() -> SelectStatement {
    let p = Alias::new("p");
    Query::select()
        .expr(Expr::val(1))
        .from(task_dependency::Entity)
        .join_as(
            JoinType::InnerJoin,
            task::Entity,
            p.clone(),
            Expr::col((p.clone(), task::Column::Id)).equals((
                task_dependency::Entity,
                task_dependency::Column::DependsOn,
            )),
        )
        .and_where(
            Expr::col((task_dependency::Entity, task_dependency::Column::TaskId))
                .equals((task::Entity, task::Column::Id)),
        )
        .and_where(Expr::col((p, task::Column::Status)).is_not_in(FINISHED_STATUSES.iter().copied()))
        .to_owned()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DepsFilter {
    #[default]
    Any,
    HasDeps,
    NoDeps,
    /// Has at least one unfinished (not merged/closed) dependency.
    Blocked,
    /// All dependencies (if any) are merged/closed.
    Ready,
}

#[derive(Debug, Default, Clone)]
pub struct TaskFilter<'a> {
    pub status: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub assignee: Option<&'a str>,
    pub label: Option<&'a str>,
    pub role: Option<&'a str>,
    pub created_from: Option<&'a str>,
    pub created_to: Option<&'a str>,
    pub updated_from: Option<&'a str>,
    pub updated_to: Option<&'a str>,
    pub deps_filter: DepsFilter,
    pub sort: Option<&'a str>,
    pub limit: Option<u32>,
}

impl Database {
    /// Open (or create) a SQLite database file at `path`. `path` may be the
    /// magic value `":memory:"`.
    pub async fn open(path: &str) -> Result<Self> {
        let url = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite://{path}?mode=rwc")
        };
        let conn = sea_orm::Database::connect(&url).await?;
        // SQLite needs FK enforcement explicitly enabled. PRAGMAs aren't part
        // of the data model, so they stay as a one-off `execute_unprepared`.
        conn.execute_unprepared("PRAGMA foreign_keys = ON;").await?;
        Ok(Self { conn })
    }

    pub async fn open_in_memory() -> Result<Self> {
        Self::open(":memory:").await
    }

    /// Apply any pending migrations.
    pub async fn init(&self) -> Result<()> {
        Migrator::up(&self.conn, None).await?;
        Ok(())
    }

    // ── Tasks ──

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_task(
        &self,
        title: &str,
        description: Option<&str>,
        status: &TaskStatus,
        priority: Option<&Priority>,
        branch: Option<&str>,
        base_branch: Option<&str>,
        assignee: Option<&str>,
    ) -> Result<i64> {
        let now = chrono::Local::now().naive_local();
        let am = task::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(title.to_string()),
            description: ActiveValue::Set(description.map(str::to_string)),
            status: ActiveValue::Set(status.as_str().to_string()),
            priority: ActiveValue::Set(priority.map(|p| p.as_str().to_string())),
            branch: ActiveValue::Set(branch.map(str::to_string)),
            base_branch: ActiveValue::Set(base_branch.map(str::to_string)),
            provider: ActiveValue::Set(None),
            remote_id: ActiveValue::Set(None),
            source_url: ActiveValue::Set(None),
            assignee: ActiveValue::Set(assignee.map(str::to_string)),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };
        let res = Task::insert(am).exec(&self.conn).await?;
        Ok(res.last_insert_id)
    }

    pub async fn get_task(&self, id: i64) -> Result<DomainTask> {
        let row = Task::find_by_id(id)
            .one(&self.conn)
            .await?
            .ok_or_else(|| DevTodoError::NotFound("Task".into(), id.to_string()))?;
        Ok(model_to_task(row))
    }

    pub async fn list_tasks(
        &self,
        status: Option<&str>,
        priority: Option<&str>,
        assignee: Option<&str>,
        label_name: Option<&str>,
        sort: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<DomainTask>> {
        self.list_tasks_filtered(TaskFilter {
            status,
            priority,
            assignee,
            label: label_name,
            sort,
            limit,
            ..Default::default()
        })
        .await
    }

    /// Filtered list, built entirely with SeaORM's query builder. JOINs to
    /// `task_labels`/`labels` and `dev_roles` are conditional. Dep filters use
    /// correlated `EXISTS` subqueries built with `Query::select` so we don't
    /// touch raw SQL.
    pub async fn list_tasks_filtered(&self, f: TaskFilter<'_>) -> Result<Vec<DomainTask>> {
        use sea_orm::QueryTrait;

        let mut q = Task::find();

        if f.label.is_some() {
            q = q
                .join(JoinType::InnerJoin, task::Relation::TaskLabel.def())
                .join(JoinType::InnerJoin, task_label::Relation::Label.def())
                .distinct();
        }
        if f.role.is_some() {
            q = q.join(JoinType::InnerJoin, task::Relation::DevRole.def());
        }

        if let Some(s) = f.status {
            q = q.filter(task::Column::Status.eq(s));
        }
        if let Some(p) = f.priority {
            q = q.filter(task::Column::Priority.eq(p));
        }
        if let Some(a) = f.assignee {
            q = q.filter(task::Column::Assignee.eq(a));
        }
        if let Some(l) = f.label {
            q = q.filter(label::Column::Name.eq(l));
        }
        if let Some(r) = f.role {
            q = q.filter(dev_role::Column::Role.eq(r));
        }
        if let Some(d) = f.created_from {
            q = q.filter(task::Column::CreatedAt.gte(d));
        }
        if let Some(d) = f.created_to {
            q = q.filter(task::Column::CreatedAt.lte(d));
        }
        if let Some(d) = f.updated_from {
            q = q.filter(task::Column::UpdatedAt.gte(d));
        }
        if let Some(d) = f.updated_to {
            q = q.filter(task::Column::UpdatedAt.lte(d));
        }

        match f.deps_filter {
            DepsFilter::Any => {}
            DepsFilter::HasDeps => {
                q = q.filter(Expr::exists(deps_exists_subquery()));
            }
            DepsFilter::NoDeps => {
                q = q.filter(Expr::exists(deps_exists_subquery()).not());
            }
            DepsFilter::Blocked => {
                q = q.filter(Expr::exists(blocked_exists_subquery()));
            }
            DepsFilter::Ready => {
                q = q.filter(Expr::exists(blocked_exists_subquery()).not());
            }
        }

        q = match f.sort {
            Some("updated") => q.order_by_desc(task::Column::UpdatedAt),
            Some("priority") => q.order_by_desc(task::Column::Priority),
            _ => q.order_by_desc(task::Column::CreatedAt),
        };

        if let Some(n) = f.limit {
            q = q.limit(n as u64);
        }

        let _ = q.as_query(); // touch the QueryTrait so the import is meaningful
        Ok(q.all(&self.conn).await?.into_iter().map(model_to_task).collect())
    }

    /// Dynamic field update kept for backwards compatibility with the older
    /// rusqlite-era API. The whitelist becomes a `match` over typed columns,
    /// so there is no SQL string and no possibility of column injection.
    pub async fn update_task_field(&self, id: i64, field: &str, value: Option<&str>) -> Result<()> {
        let now = chrono::Local::now().naive_local();
        let v = value.map(str::to_string);

        let mut update = Task::update_many()
            .col_expr(task::Column::UpdatedAt, Expr::value(now))
            .filter(task::Column::Id.eq(id));

        update = match field {
            "title" => update.col_expr(task::Column::Title, Expr::value(v.clone().unwrap_or_default())),
            "description" => update.col_expr(task::Column::Description, Expr::value(v.clone())),
            "status" => update.col_expr(task::Column::Status, Expr::value(v.clone().unwrap_or_default())),
            "priority" => update.col_expr(task::Column::Priority, Expr::value(v.clone())),
            "branch" => update.col_expr(task::Column::Branch, Expr::value(v.clone())),
            "base_branch" => update.col_expr(task::Column::BaseBranch, Expr::value(v.clone())),
            "assignee" => update.col_expr(task::Column::Assignee, Expr::value(v.clone())),
            "provider" => update.col_expr(task::Column::Provider, Expr::value(v.clone())),
            "remote_id" => update.col_expr(
                task::Column::RemoteId,
                Expr::value(v.as_ref().and_then(|s| s.parse::<i64>().ok())),
            ),
            "source_url" => update.col_expr(task::Column::SourceUrl, Expr::value(v.clone())),
            other => {
                return Err(DevTodoError::Config(format!(
                    "Cannot update field: {other}"
                )));
            }
        };

        let res = update.exec(&self.conn).await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound("Task".into(), id.to_string()));
        }
        Ok(())
    }

    pub async fn delete_task(&self, id: i64) -> Result<()> {
        let res = Task::delete_by_id(id).exec(&self.conn).await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound("Task".into(), id.to_string()));
        }
        Ok(())
    }

    // ── Labels ──

    pub async fn insert_label(&self, name: &str, color: Option<&str>) -> Result<i64> {
        let am = label::ActiveModel {
            id: ActiveValue::NotSet,
            name: ActiveValue::Set(name.to_string()),
            color: ActiveValue::Set(color.map(str::to_string)),
        };
        let res = Label::insert(am).exec(&self.conn).await?;
        Ok(res.last_insert_id)
    }

    pub async fn get_label_by_name(&self, name: &str) -> Result<DomainLabel> {
        let row = Label::find()
            .filter(label::Column::Name.eq(name))
            .one(&self.conn)
            .await?
            .ok_or_else(|| DevTodoError::NotFound("Label".into(), name.to_string()))?;
        Ok(DomainLabel {
            id: row.id,
            name: row.name,
            color: row.color,
        })
    }

    pub async fn list_labels(&self) -> Result<Vec<DomainLabel>> {
        let rows = Label::find()
            .order_by_asc(label::Column::Name)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DomainLabel {
                id: r.id,
                name: r.name,
                color: r.color,
            })
            .collect())
    }

    pub async fn delete_label(&self, name: &str) -> Result<()> {
        let res = Label::delete_many()
            .filter(label::Column::Name.eq(name))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound("Label".into(), name.to_string()));
        }
        Ok(())
    }

    pub async fn assign_label(&self, task_id: i64, label_name: &str) -> Result<()> {
        let l = self.get_label_by_name(label_name).await?;
        let am = task_label::ActiveModel {
            task_id: ActiveValue::Set(task_id),
            label_id: ActiveValue::Set(l.id),
        };
        TaskLabel::insert(am)
            .on_conflict(
                sea_query::OnConflict::columns([
                    task_label::Column::TaskId,
                    task_label::Column::LabelId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn unassign_label(&self, task_id: i64, label_name: &str) -> Result<()> {
        let l = self.get_label_by_name(label_name).await?;
        TaskLabel::delete_many()
            .filter(task_label::Column::TaskId.eq(task_id))
            .filter(task_label::Column::LabelId.eq(l.id))
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn get_labels_for_task(&self, task_id: i64) -> Result<Vec<DomainLabel>> {
        // Many-to-many traversal: labels reachable from this task via task_labels.
        let rows = Label::find()
            .inner_join(task_label::Entity)
            .filter(task_label::Column::TaskId.eq(task_id))
            .order_by_asc(label::Column::Name)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DomainLabel {
                id: r.id,
                name: r.name,
                color: r.color,
            })
            .collect())
    }

    // ── Reviewers ──

    pub async fn assign_reviewer(&self, task_id: i64, username: &str) -> Result<i64> {
        let am = reviewer::ActiveModel {
            id: ActiveValue::NotSet,
            task_id: ActiveValue::Set(task_id),
            username: ActiveValue::Set(username.to_string()),
            status: ActiveValue::Set("pending".into()),
            reviewed_at: ActiveValue::Set(None),
        };
        let res = Reviewer::insert(am).exec(&self.conn).await?;
        Ok(res.last_insert_id)
    }

    pub async fn remove_reviewer(&self, task_id: i64, username: &str) -> Result<()> {
        let res = Reviewer::delete_many()
            .filter(reviewer::Column::TaskId.eq(task_id))
            .filter(reviewer::Column::Username.eq(username))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound(
                "Reviewer".into(),
                format!("{username} on task {task_id}"),
            ));
        }
        Ok(())
    }

    pub async fn update_review_status(
        &self,
        task_id: i64,
        username: &str,
        status: &m::ReviewStatus,
    ) -> Result<()> {
        let now = chrono::Local::now().naive_local();
        let res = Reviewer::update_many()
            .col_expr(reviewer::Column::Status, Expr::value(status.as_str()))
            .col_expr(reviewer::Column::ReviewedAt, Expr::value(now))
            .filter(reviewer::Column::TaskId.eq(task_id))
            .filter(reviewer::Column::Username.eq(username))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound(
                "Reviewer".into(),
                format!("{username} on task {task_id}"),
            ));
        }
        Ok(())
    }

    pub async fn list_reviewers(&self, task_id: i64) -> Result<Vec<DomainReviewer>> {
        let rows = Reviewer::find()
            .filter(reviewer::Column::TaskId.eq(task_id))
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DomainReviewer {
                id: r.id,
                task_id: r.task_id,
                username: r.username,
                status: r.status.parse().unwrap_or(m::ReviewStatus::Pending),
                reviewed_at: r.reviewed_at,
            })
            .collect())
    }

    // ── Comments ──

    pub async fn insert_comment(&self, task_id: i64, author: &str, body: &str) -> Result<i64> {
        let am = comment::ActiveModel {
            id: ActiveValue::NotSet,
            task_id: ActiveValue::Set(task_id),
            author: ActiveValue::Set(author.to_string()),
            body: ActiveValue::Set(body.to_string()),
            remote_id: ActiveValue::Set(None),
            created_at: ActiveValue::Set(chrono::Local::now().naive_local()),
        };
        let res = Comment::insert(am).exec(&self.conn).await?;
        Ok(res.last_insert_id)
    }

    pub async fn list_comments(&self, task_id: i64) -> Result<Vec<m::Comment>> {
        let rows = Comment::find()
            .filter(comment::Column::TaskId.eq(task_id))
            .order_by_asc(comment::Column::CreatedAt)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| m::Comment {
                id: r.id,
                task_id: r.task_id,
                author: r.author,
                body: r.body,
                remote_id: r.remote_id,
                created_at: r.created_at,
            })
            .collect())
    }

    // ── Stats helpers ──

    pub async fn count_by_status(&self) -> Result<Vec<(String, i64)>> {
        let rows: Vec<GroupCount> = Task::find()
            .select_only()
            .column_as(task::Column::Status, "key")
            .column_as(task::Column::Id.count(), "n")
            .group_by(task::Column::Status)
            .into_model::<GroupCount>()
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(|r| (r.key, r.n)).collect())
    }

    pub async fn count_by_priority(&self) -> Result<Vec<(String, i64)>> {
        // COALESCE(priority, 'none') so the "no priority" bucket is reported.
        let coalesced = Expr::expr(Func::coalesce([
            task::Column::Priority.into_simple_expr(),
            Expr::val("none").into(),
        ]));
        let rows: Vec<GroupCount> = Task::find()
            .select_only()
            .column_as(coalesced, "key")
            .column_as(task::Column::Id.count(), "n")
            .group_by(task::Column::Priority)
            .into_model::<GroupCount>()
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(|r| (r.key, r.n)).collect())
    }

    pub async fn count_by_label(&self) -> Result<Vec<(String, i64)>> {
        let rows: Vec<GroupCount> = TaskLabel::find()
            .select_only()
            .inner_join(label::Entity)
            .column_as(label::Column::Name, "key")
            .column_as(label::Column::Id.count(), "n")
            .group_by(label::Column::Name)
            .into_model::<GroupCount>()
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(|r| (r.key, r.n)).collect())
    }

    pub async fn avg_merge_time_hours(&self) -> Result<Option<f64>> {
        // SQLite-only `julianday()`; we wrap it in a typed Expr rather than
        // emitting raw SQL so the rest of the query is built by the ORM.
        let avg_expr = Expr::cust_with_exprs(
            "AVG((julianday($1) - julianday($2)) * 24)",
            [
                Expr::col(task::Column::UpdatedAt).into(),
                Expr::col(task::Column::CreatedAt).into(),
            ],
        );
        let row: Option<AvgHours> = Task::find()
            .select_only()
            .column_as(avg_expr, "avg_h")
            .filter(task::Column::Status.eq("merged"))
            .into_model::<AvgHours>()
            .one(&self.conn)
            .await?;
        Ok(row.and_then(|r| r.avg_h))
    }

    pub async fn oldest_open_tasks(&self, limit: u32) -> Result<Vec<DomainTask>> {
        let rows = Task::find()
            .filter(task::Column::Status.is_in(["open", "review", "draft"]))
            .order_by_asc(task::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(model_to_task).collect())
    }

    // ── Task dependencies ──

    pub async fn add_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        if task_id == depends_on {
            return Err(DevTodoError::SelfDependency(task_id));
        }
        self.get_task(task_id).await?;
        self.get_task(depends_on).await?;

        if self.depends_transitively(depends_on, task_id).await? {
            return Err(DevTodoError::DependencyCycle {
                from: task_id,
                to: depends_on,
            });
        }

        let am = task_dependency::ActiveModel {
            task_id: ActiveValue::Set(task_id),
            depends_on: ActiveValue::Set(depends_on),
            created_at: ActiveValue::Set(chrono::Local::now().naive_local()),
        };
        TaskDependency::insert(am)
            .on_conflict(
                sea_query::OnConflict::columns([
                    task_dependency::Column::TaskId,
                    task_dependency::Column::DependsOn,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn remove_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        let res = TaskDependency::delete_many()
            .filter(task_dependency::Column::TaskId.eq(task_id))
            .filter(task_dependency::Column::DependsOn.eq(depends_on))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound(
                "Dependency".into(),
                format!("#{task_id} -> #{depends_on}"),
            ));
        }
        Ok(())
    }

    pub async fn list_dependencies(&self, task_id: i64) -> Result<Vec<DomainTask>> {
        // Tasks that `task_id` depends on: join on `task.id = task_dependency.depends_on`,
        // i.e. follow the Blocker relation in reverse.
        let rows = Task::find()
            .join(
                JoinType::InnerJoin,
                task_dependency::Relation::Blocker.def().rev(),
            )
            .filter(task_dependency::Column::TaskId.eq(task_id))
            .order_by_asc(task::Column::Id)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(model_to_task).collect())
    }

    pub async fn list_dependents(&self, task_id: i64) -> Result<Vec<DomainTask>> {
        // Tasks that depend on `task_id`: join on `task.id = task_dependency.task_id`,
        // i.e. follow the Task relation in reverse.
        let rows = Task::find()
            .join(
                JoinType::InnerJoin,
                task_dependency::Relation::Task.def().rev(),
            )
            .filter(task_dependency::Column::DependsOn.eq(task_id))
            .order_by_asc(task::Column::Id)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(model_to_task).collect())
    }

    /// Returns true if `from` (transitively) depends on `target`. Built with
    /// sea_query's `WithClause` / `CommonTableExpression` so the recursive CTE
    /// is composed by the ORM rather than written as SQL string.
    async fn depends_transitively(&self, from: i64, target: i64) -> Result<bool> {
        let chain = Alias::new("chain");
        let chain_id = Alias::new("id");

        // Anchor: depends_on rows directly attached to `from`.
        let mut anchor = Query::select()
            .column(task_dependency::Column::DependsOn)
            .from(task_dependency::Entity)
            .and_where(task_dependency::Column::TaskId.eq(from))
            .to_owned();

        // Recursive step: walk one more edge from any node already in `chain`.
        let recursive = Query::select()
            .column((task_dependency::Entity, task_dependency::Column::DependsOn))
            .from(task_dependency::Entity)
            .inner_join(
                chain.clone(),
                Expr::col((task_dependency::Entity, task_dependency::Column::TaskId))
                    .equals((chain.clone(), chain_id.clone())),
            )
            .to_owned();

        let cte = CommonTableExpression::new()
            .query(anchor.union(UnionType::Distinct, recursive).to_owned())
            .columns([chain_id.clone()])
            .table_name(chain.clone())
            .to_owned();

        let with_clause = WithClause::new().recursive(true).cte(cte).to_owned();

        let select = Query::select()
            .expr(Expr::val(1))
            .from(chain.clone())
            .and_where(Expr::col(chain_id).eq(target))
            .limit(1)
            .to_owned();

        // Compile the WithQuery via the SeaORM connection's backend.
        let with_query = select.with(with_clause);
        let stmt = self.conn.get_database_backend().build(&with_query);
        Ok(self.conn.query_one(stmt).await?.is_some())
    }

    // ── Dev roles ──

    pub async fn set_role(&self, username: &str, role: &str) -> Result<()> {
        let am = dev_role::ActiveModel {
            username: ActiveValue::Set(username.to_string()),
            role: ActiveValue::Set(role.to_string()),
        };
        DevRole::insert(am)
            .on_conflict(
                sea_query::OnConflict::column(dev_role::Column::Username)
                    .update_column(dev_role::Column::Role)
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn remove_role(&self, username: &str) -> Result<()> {
        let res = DevRole::delete_many()
            .filter(dev_role::Column::Username.eq(username))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound("Role".into(), username.to_string()));
        }
        Ok(())
    }

    pub async fn get_role(&self, username: &str) -> Result<Option<String>> {
        let row = DevRole::find_by_id(username.to_string())
            .one(&self.conn)
            .await?;
        Ok(row.map(|r| r.role))
    }

    pub async fn list_roles(&self) -> Result<Vec<DomainDevRole>> {
        let rows = DevRole::find()
            .order_by_asc(dev_role::Column::Role)
            .order_by_asc(dev_role::Column::Username)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DomainDevRole {
                username: r.username,
                role: r.role,
            })
            .collect())
    }

    // ── Commit proofs ──

    pub async fn add_proof(
        &self,
        task_id: i64,
        commit_hash: &str,
        short_hash: Option<&str>,
        author: Option<&str>,
        message: Option<&str>,
        committed_at: Option<&str>,
    ) -> Result<()> {
        self.get_task(task_id).await?;
        let am = task_commit::ActiveModel {
            task_id: ActiveValue::Set(task_id),
            commit_hash: ActiveValue::Set(commit_hash.to_string()),
            short_hash: ActiveValue::Set(short_hash.map(str::to_string)),
            author: ActiveValue::Set(author.map(str::to_string)),
            message: ActiveValue::Set(message.map(str::to_string)),
            committed_at: ActiveValue::Set(committed_at.map(str::to_string)),
            added_at: ActiveValue::Set(chrono::Local::now().naive_local()),
        };
        TaskCommit::insert(am)
            .on_conflict(
                sea_query::OnConflict::columns([
                    task_commit::Column::TaskId,
                    task_commit::Column::CommitHash,
                ])
                .update_columns([
                    task_commit::Column::ShortHash,
                    task_commit::Column::Author,
                    task_commit::Column::Message,
                    task_commit::Column::CommittedAt,
                ])
                .to_owned(),
            )
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn remove_proof(&self, task_id: i64, commit_hash: &str) -> Result<()> {
        let res = TaskCommit::delete_many()
            .filter(task_commit::Column::TaskId.eq(task_id))
            .filter(task_commit::Column::CommitHash.eq(commit_hash))
            .exec(&self.conn)
            .await?;
        if res.rows_affected == 0 {
            return Err(DevTodoError::NotFound(
                "Commit proof".into(),
                format!("#{task_id} {commit_hash}"),
            ));
        }
        Ok(())
    }

    pub async fn list_proofs(&self, task_id: i64) -> Result<Vec<CommitProof>> {
        let rows = TaskCommit::find()
            .filter(task_commit::Column::TaskId.eq(task_id))
            .order_by_asc(task_commit::Column::AddedAt)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| CommitProof {
                task_id: r.task_id,
                commit_hash: r.commit_hash,
                short_hash: r.short_hash,
                author: r.author,
                message: r.message,
                committed_at: r.committed_at,
                added_at: r.added_at,
            })
            .collect())
    }

    // ── Gamification ──

    pub async fn get_profile(&self) -> Result<Profile> {
        // Lazy seed: the migration creates the row, but be defensive in case
        // someone wipes/restores the table independently.
        let row = Gamification::find_by_id(1).one(&self.conn).await?;
        let row = match row {
            Some(r) => r,
            None => {
                let seed = gamification::ActiveModel {
                    id: ActiveValue::Set(1),
                    xp: ActiveValue::Set(0),
                    level: ActiveValue::Set(1),
                    current_streak: ActiveValue::Set(0),
                    longest_streak: ActiveValue::Set(0),
                    total_completed: ActiveValue::Set(0),
                    last_completion_date: ActiveValue::Set(None),
                };
                Gamification::insert(seed).exec(&self.conn).await?;
                Gamification::find_by_id(1).one(&self.conn).await?.unwrap()
            }
        };

        let last_completion_date = row
            .last_completion_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        Ok(Profile {
            xp: row.xp,
            level: level_for_xp(row.xp).max(row.level.max(1) as u32),
            current_streak: row.current_streak.max(0) as u32,
            longest_streak: row.longest_streak.max(0) as u32,
            total_completed: row.total_completed.max(0) as u64,
            last_completion_date,
        })
    }

    pub async fn save_profile(&self, profile: &Profile) -> Result<()> {
        let last = profile
            .last_completion_date
            .map(|d| d.format("%Y-%m-%d").to_string());
        let am = gamification::ActiveModel {
            id: ActiveValue::Unchanged(1),
            xp: ActiveValue::Set(profile.xp),
            level: ActiveValue::Set(profile.level as i64),
            current_streak: ActiveValue::Set(profile.current_streak as i64),
            longest_streak: ActiveValue::Set(profile.longest_streak as i64),
            total_completed: ActiveValue::Set(profile.total_completed as i64),
            last_completion_date: ActiveValue::Set(last),
        };
        // Insert if missing, otherwise update by primary key.
        match Gamification::find_by_id(1).one(&self.conn).await? {
            Some(_) => {
                am.update(&self.conn).await?;
            }
            None => {
                Gamification::insert(am).exec(&self.conn).await?;
            }
        }
        Ok(())
    }

    pub async fn is_achievement_unlocked(&self, name: &str) -> Result<bool> {
        let row = AchievementUnlocked::find_by_id(name.to_string())
            .one(&self.conn)
            .await?;
        Ok(row.is_some())
    }

    pub async fn unlock_achievement(&self, name: &str) -> Result<()> {
        let am = achievement_unlocked::ActiveModel {
            name: ActiveValue::Set(name.to_string()),
            unlocked_at: ActiveValue::Set(chrono::Local::now().naive_local()),
        };
        AchievementUnlocked::insert(am)
            .on_conflict(
                sea_query::OnConflict::column(achievement_unlocked::Column::Name)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn list_unlocked_achievements(&self) -> Result<Vec<(String, String)>> {
        let rows = AchievementUnlocked::find()
            .order_by_asc(achievement_unlocked::Column::UnlockedAt)
            .all(&self.conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.name,
                    r.unlocked_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                )
            })
            .collect())
    }
}

fn model_to_task(row: task::Model) -> DomainTask {
    DomainTask {
        id: row.id,
        title: row.title,
        description: row.description,
        status: row.status.parse().unwrap_or(TaskStatus::Draft),
        priority: row.priority.and_then(|s| s.parse().ok()),
        branch: row.branch,
        base_branch: row.base_branch,
        provider: row.provider.and_then(|s| s.parse().ok()),
        remote_id: row.remote_id,
        source_url: row.source_url,
        assignee: row.assignee,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// Generic `(key, count)` projection used by all the GROUP BY counts.
#[derive(Debug, FromQueryResult)]
struct GroupCount {
    key: String,
    n: i64,
}

/// Single-column projection for `avg_merge_time_hours`.
#[derive(Debug, FromQueryResult)]
struct AvgHours {
    avg_h: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        Database::open_in_memory().await.unwrap().tap_init().await
    }

    /// Tiny extension to chain `init` after `open_in_memory` in tests.
    impl Database {
        async fn tap_init(self) -> Self {
            self.init().await.unwrap();
            self
        }
    }

    async fn mk_task(db: &Database, name: &str) -> i64 {
        db.insert_task(name, None, &TaskStatus::Open, None, None, None, None)
            .await
            .unwrap()
    }

    // ── Tasks CRUD ──

    #[tokio::test]
    async fn insert_and_get_task() {
        let db = test_db().await;
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
            .await
            .unwrap();
        assert_eq!(id, 1);

        let task = db.get_task(id).await.unwrap();
        assert_eq!(task.title, "Fix login bug");
        assert_eq!(task.description.as_deref(), Some("Details here"));
        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(task.priority, Some(Priority::High));
        assert_eq!(task.branch.as_deref(), Some("fix/login"));
        assert_eq!(task.base_branch.as_deref(), Some("main"));
        assert_eq!(task.assignee.as_deref(), Some("alice"));
    }

    #[tokio::test]
    async fn get_task_not_found_returns_typed_notfound() {
        let db = test_db().await;
        let err = db.get_task(999).await.unwrap_err();
        assert!(matches!(&err, DevTodoError::NotFound(k, _) if k == "Task"));
    }

    #[tokio::test]
    async fn update_task_field_and_reject_unknown_field() {
        let db = test_db().await;
        let id = db
            .insert_task("T", None, &TaskStatus::Draft, None, None, None, None)
            .await
            .unwrap();
        db.update_task_field(id, "title", Some("Updated")).await.unwrap();
        db.update_task_field(id, "status", Some("review")).await.unwrap();
        let task = db.get_task(id).await.unwrap();
        assert_eq!(task.title, "Updated");
        assert_eq!(task.status, TaskStatus::Review);

        assert!(db.update_task_field(id, "evil_field", Some("x")).await.is_err());
    }

    #[tokio::test]
    async fn delete_task_and_cascades() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.insert_label("bug", None).await.unwrap();
        db.assign_label(id, "bug").await.unwrap();
        db.delete_task(id).await.unwrap();
        assert!(db.get_task(id).await.is_err());
        // Label survives, association is cascaded.
        assert_eq!(db.list_labels().await.unwrap().len(), 1);
        assert!(db.delete_task(999).await.is_err());
    }

    // ── Filters ──

    #[tokio::test]
    async fn list_filter_by_status_and_label() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let _b = db
            .insert_task("B", None, &TaskStatus::Draft, None, None, None, None)
            .await
            .unwrap();
        let _c = mk_task(&db, "C").await;
        db.insert_label("bug", None).await.unwrap();
        db.assign_label(a, "bug").await.unwrap();

        let opens = db.list_tasks(Some("open"), None, None, None, None, None).await.unwrap();
        assert_eq!(opens.len(), 2);

        let with_bug = db.list_tasks(None, None, None, Some("bug"), None, None).await.unwrap();
        assert_eq!(with_bug.len(), 1);
        assert_eq!(with_bug[0].title, "A");
    }

    #[tokio::test]
    async fn list_with_limit() {
        let db = test_db().await;
        for i in 0..10 {
            mk_task(&db, &format!("T{i}")).await;
        }
        let three = db.list_tasks(None, None, None, None, None, Some(3)).await.unwrap();
        assert_eq!(three.len(), 3);
    }

    // ── Labels ──

    #[tokio::test]
    async fn label_crud() {
        let db = test_db().await;
        db.insert_label("bug", Some("#ff0000")).await.unwrap();
        db.insert_label("feat", None).await.unwrap();
        let labels = db.list_labels().await.unwrap();
        assert_eq!(labels.len(), 2);
        let bug = db.get_label_by_name("bug").await.unwrap();
        assert_eq!(bug.color.as_deref(), Some("#ff0000"));
        db.delete_label("bug").await.unwrap();
        assert_eq!(db.list_labels().await.unwrap().len(), 1);
        assert!(db.get_label_by_name("nope").await.is_err());
    }

    #[tokio::test]
    async fn assign_label_is_idempotent() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.insert_label("bug", None).await.unwrap();
        db.assign_label(id, "bug").await.unwrap();
        db.assign_label(id, "bug").await.unwrap();
        let labels = db.get_labels_for_task(id).await.unwrap();
        assert_eq!(labels.len(), 1);
    }

    // ── Reviewers ──

    #[tokio::test]
    async fn reviewer_lifecycle() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.assign_reviewer(id, "alice").await.unwrap();
        db.assign_reviewer(id, "bob").await.unwrap();

        let reviewers = db.list_reviewers(id).await.unwrap();
        assert_eq!(reviewers.len(), 2);
        assert_eq!(reviewers[0].status, m::ReviewStatus::Pending);

        db.update_review_status(id, "alice", &m::ReviewStatus::Approved)
            .await
            .unwrap();
        let reviewers = db.list_reviewers(id).await.unwrap();
        let alice = reviewers.iter().find(|r| r.username == "alice").unwrap();
        assert_eq!(alice.status, m::ReviewStatus::Approved);
        assert!(alice.reviewed_at.is_some());

        db.remove_reviewer(id, "bob").await.unwrap();
        assert_eq!(db.list_reviewers(id).await.unwrap().len(), 1);
        assert!(db.remove_reviewer(id, "ghost").await.is_err());
    }

    // ── Comments ──

    #[tokio::test]
    async fn comment_crud() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.insert_comment(id, "alice", "Looks good").await.unwrap();
        db.insert_comment(id, "bob", "Needs changes").await.unwrap();
        let comments = db.list_comments(id).await.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].author, "alice");
        assert_eq!(comments[1].body, "Needs changes");
    }

    // ── Stats ──

    #[tokio::test]
    async fn stats_count_by_status() {
        let db = test_db().await;
        mk_task(&db, "A").await;
        mk_task(&db, "B").await;
        db.insert_task("C", None, &TaskStatus::Merged, None, None, None, None)
            .await
            .unwrap();

        let counts = db.count_by_status().await.unwrap();
        let open_count = counts.iter().find(|(s, _)| s == "open").map(|(_, c)| *c).unwrap_or(0);
        assert_eq!(open_count, 2);
        let merged_count = counts.iter().find(|(s, _)| s == "merged").map(|(_, c)| *c).unwrap_or(0);
        assert_eq!(merged_count, 1);
    }

    #[tokio::test]
    async fn stats_oldest_open() {
        let db = test_db().await;
        mk_task(&db, "Old").await;
        db.insert_task("Closed", None, &TaskStatus::Closed, None, None, None, None)
            .await
            .unwrap();
        mk_task(&db, "New").await;
        let oldest = db.oldest_open_tasks(5).await.unwrap();
        assert_eq!(oldest.len(), 2);
        assert_eq!(oldest[0].title, "Old");
    }

    // ── Dependencies ──

    #[tokio::test]
    async fn dep_add_and_list() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        db.add_dependency(a, b).await.unwrap();
        let deps = db.list_dependencies(a).await.unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, b);
        let dependents = db.list_dependents(b).await.unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, a);
    }

    #[tokio::test]
    async fn dep_self_loop_returns_typed_self_dependency() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let err = db.add_dependency(a, a).await.unwrap_err();
        assert!(matches!(err, DevTodoError::SelfDependency(id) if id == a));
    }

    #[tokio::test]
    async fn dep_direct_cycle_returns_typed_cycle() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        db.add_dependency(a, b).await.unwrap();
        let err = db.add_dependency(b, a).await.unwrap_err();
        assert!(matches!(err, DevTodoError::DependencyCycle { from, to } if from == b && to == a));
    }

    #[tokio::test]
    async fn dep_transitive_cycle_returns_typed_cycle() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        let c = mk_task(&db, "C").await;
        db.add_dependency(a, b).await.unwrap();
        db.add_dependency(b, c).await.unwrap();
        let err = db.add_dependency(c, a).await.unwrap_err();
        assert!(matches!(err, DevTodoError::DependencyCycle { .. }));
    }

    #[tokio::test]
    async fn dep_to_missing_task_returns_notfound() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let err = db.add_dependency(a, 999).await.unwrap_err();
        assert!(matches!(&err, DevTodoError::NotFound(k, _) if k == "Task"));
    }

    #[tokio::test]
    async fn dep_remove_then_remove_again_errors() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        db.add_dependency(a, b).await.unwrap();
        db.remove_dependency(a, b).await.unwrap();
        assert!(db.remove_dependency(a, b).await.is_err());
    }

    #[tokio::test]
    async fn list_filter_has_no_deps() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        let _c = mk_task(&db, "C").await;
        db.add_dependency(a, b).await.unwrap();

        let with = db
            .list_tasks_filtered(TaskFilter {
                deps_filter: DepsFilter::HasDeps,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(with.len(), 1);
        assert_eq!(with[0].id, a);

        let without = db
            .list_tasks_filtered(TaskFilter {
                deps_filter: DepsFilter::NoDeps,
                ..Default::default()
            })
            .await
            .unwrap();
        let ids: Vec<i64> = without.iter().map(|t| t.id).collect();
        assert!(ids.contains(&b));
        assert!(!ids.contains(&a));
    }

    #[tokio::test]
    async fn list_filter_blocked_then_ready_after_merge() {
        let db = test_db().await;
        let parent = mk_task(&db, "parent").await;
        let child = mk_task(&db, "child").await;
        db.add_dependency(child, parent).await.unwrap();

        let blocked = db
            .list_tasks_filtered(TaskFilter {
                deps_filter: DepsFilter::Blocked,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(blocked.iter().map(|t| t.id).collect::<Vec<_>>(), vec![child]);

        db.update_task_field(parent, "status", Some("merged"))
            .await
            .unwrap();
        let ready = db
            .list_tasks_filtered(TaskFilter {
                deps_filter: DepsFilter::Ready,
                ..Default::default()
            })
            .await
            .unwrap();
        let ready_ids: Vec<i64> = ready.iter().map(|t| t.id).collect();
        assert!(ready_ids.contains(&child));
    }

    // ── Roles ──

    #[tokio::test]
    async fn role_set_get_remove() {
        let db = test_db().await;
        db.set_role("alice", "backend").await.unwrap();
        assert_eq!(db.get_role("alice").await.unwrap().as_deref(), Some("backend"));
        db.set_role("alice", "fullstack").await.unwrap();
        assert_eq!(db.get_role("alice").await.unwrap().as_deref(), Some("fullstack"));
        db.remove_role("alice").await.unwrap();
        assert!(db.get_role("alice").await.unwrap().is_none());
        assert!(db.remove_role("alice").await.is_err());
    }

    #[tokio::test]
    async fn list_filter_by_role() {
        let db = test_db().await;
        let _ = db
            .insert_task("Backend", None, &TaskStatus::Open, None, None, None, Some("alice"))
            .await
            .unwrap();
        let _ = db
            .insert_task("Frontend", None, &TaskStatus::Open, None, None, None, Some("bob"))
            .await
            .unwrap();
        db.set_role("alice", "backend").await.unwrap();
        db.set_role("bob", "frontend").await.unwrap();

        let backend = db
            .list_tasks_filtered(TaskFilter {
                role: Some("backend"),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(backend.len(), 1);
        assert_eq!(backend[0].title, "Backend");
    }

    // ── Date range ──

    #[tokio::test]
    async fn list_filter_by_date_range() {
        let db = test_db().await;
        let id1 = mk_task(&db, "old").await;
        let id2 = mk_task(&db, "recent").await;

        // Backdate task 1 via the entity Update API — no SQL string.
        let backdated = chrono::NaiveDateTime::parse_from_str("2020-01-01T00:00:00", "%Y-%m-%dT%H:%M:%S")
            .unwrap();
        Task::update_many()
            .col_expr(task::Column::CreatedAt, Expr::value(backdated))
            .filter(task::Column::Id.eq(id1))
            .exec(&db.conn)
            .await
            .unwrap();

        let recent = db
            .list_tasks_filtered(TaskFilter {
                created_from: Some("2024-01-01T00:00:00"),
                ..Default::default()
            })
            .await
            .unwrap();
        let recent_ids: Vec<i64> = recent.iter().map(|t| t.id).collect();
        assert!(recent_ids.contains(&id2));
        assert!(!recent_ids.contains(&id1));

        let old = db
            .list_tasks_filtered(TaskFilter {
                created_to: Some("2021-01-01T00:00:00"),
                ..Default::default()
            })
            .await
            .unwrap();
        let old_ids: Vec<i64> = old.iter().map(|t| t.id).collect();
        assert!(old_ids.contains(&id1));
        assert!(!old_ids.contains(&id2));
    }

    // ── Commit proofs ──

    #[tokio::test]
    async fn proof_add_list_remove() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.add_proof(id, "abc1234567890", Some("abc1234"), Some("alice"), Some("Fix"), Some("2025-01-01T10:00:00"))
            .await
            .unwrap();
        db.add_proof(id, "def9876543210", Some("def9876"), Some("bob"), Some("Refactor"), Some("2025-01-02T10:00:00"))
            .await
            .unwrap();
        assert_eq!(db.list_proofs(id).await.unwrap().len(), 2);
        db.remove_proof(id, "abc1234567890").await.unwrap();
        assert_eq!(db.list_proofs(id).await.unwrap().len(), 1);
        assert!(db.remove_proof(id, "nope").await.is_err());
    }

    #[tokio::test]
    async fn proof_for_missing_task_errors_with_notfound() {
        let db = test_db().await;
        let err = db.add_proof(999, "abc", None, None, None, None).await.unwrap_err();
        assert!(matches!(&err, DevTodoError::NotFound(k, _) if k == "Task"));
    }

    #[tokio::test]
    async fn proof_replace_same_hash_is_idempotent() {
        let db = test_db().await;
        let id = mk_task(&db, "T").await;
        db.add_proof(id, "abc", None, None, Some("first"), None).await.unwrap();
        db.add_proof(id, "abc", None, None, Some("second"), None).await.unwrap();
        let proofs = db.list_proofs(id).await.unwrap();
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].message.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn cascading_delete_removes_deps() {
        let db = test_db().await;
        let a = mk_task(&db, "A").await;
        let b = mk_task(&db, "B").await;
        db.add_dependency(a, b).await.unwrap();
        db.add_proof(a, "abc", None, None, None, None).await.unwrap();
        db.delete_task(a).await.unwrap();
        assert!(db.list_dependents(b).await.unwrap().is_empty());
    }

    // ── Migrations (SeaORM) ──

    #[tokio::test]
    async fn fresh_db_has_all_tables_after_init() {
        let db = test_db().await;
        // Sanity-check by querying every table — none should error.
        db.list_tasks(None, None, None, None, None, None).await.unwrap();
        db.list_labels().await.unwrap();
        db.list_roles().await.unwrap();
        db.get_profile().await.unwrap();
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let db = test_db().await;
        db.init().await.unwrap();
        db.init().await.unwrap();
        let id = mk_task(&db, "still works").await;
        assert_eq!(db.get_task(id).await.unwrap().title, "still works");
    }
}
