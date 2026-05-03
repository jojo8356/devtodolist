# devtodo — Technical Documentation

This document is the **deep-dive companion** to the [README](README.md). It documents every command, every flag, the on-disk schema, the error model, the gamification math, the sync semantics, and the configuration surface.

If you want to quickly use `devtodo`, the README is enough. If you want to **modify** it, **integrate** with it, or understand **why** something is shaped the way it is — read on.

---

## Table of contents

- [Architecture overview](#architecture-overview)
- [Data layer](#data-layer)
  - [Schema](#schema)
  - [Migrations](#migrations)
  - [Why no raw SQL?](#why-no-raw-sql)
- [Error model](#error-model)
- [Command reference](#command-reference)
  - [Task lifecycle](#task-lifecycle)
  - [List filters in detail](#list-filters-in-detail)
  - [Date input grammar](#date-input-grammar)
  - [Dependencies (DAG)](#dependencies-dag)
  - [Roles](#roles)
  - [Commit proofs](#commit-proofs)
  - [Sync, push, pull](#sync-push-pull)
- [Gamification math](#gamification-math)
- [Configuration](#configuration)
- [Output formats](#output-formats)
- [Storage & file layout](#storage--file-layout)
- [Performance notes](#performance-notes)

---

## Architecture overview

```
┌─────────────────────────────────────────────────────────────┐
│  main.rs   #[tokio::main]                                   │
│   └── dispatches to commands::<name>::run(...).await        │
├─────────────────────────────────────────────────────────────┤
│  commands/*  one async fn per CLI command                   │
│   └── all DB calls go through Database (db.rs)              │
├─────────────────────────────────────────────────────────────┤
│  db.rs  Database = facade over sea_orm::DatabaseConnection  │
│   └── uses entities/* for typed CRUD, migration/* on init   │
├─────────────────────────────────────────────────────────────┤
│  entities/*  one Model+ActiveModel+Relations per table      │
│  migration/* sea-orm-migration; append-only                 │
│  providers/* GitHub & GitLab REST clients (reqwest)         │
├─────────────────────────────────────────────────────────────┤
│  SQLite via sqlx-sqlite (rustls TLS)                        │
└─────────────────────────────────────────────────────────────┘
```

Key design rules:

- **Single async runtime** (`tokio` multi-thread). No blocking I/O on async paths.
- **No raw SQL strings** in application code (one PRAGMA exception, see below).
- **Typed errors** at every boundary; the CLI converts them to `stderr: Error: <message>` and exits non-zero.
- **Config & DB are local** — config in `$XDG_CONFIG_HOME/devtodo/config.toml`, DB in the project's working directory.

---

## Data layer

### Schema

10 tables, each backed by a SeaORM entity in `src/entities/`:

| Table | PK | Purpose |
|---|---|---|
| `tasks` | `id` (autoincr) | The PR-equivalent record |
| `labels` | `id` (autoincr) | Reusable tags |
| `task_labels` | `(task_id, label_id)` | Many-to-many bridge |
| `reviewers` | `id` (autoincr) | One row per reviewer per task |
| `comments` | `id` (autoincr) | Local + imported comments |
| `gamification` | `id = 1` (singleton) | Global hunter profile |
| `achievements_unlocked` | `name` | Persistent badge state |
| `task_dependencies` | `(task_id, depends_on)` | Directed acyclic graph |
| `dev_roles` | `username` | `username → role` mapping |
| `task_commits` | `(task_id, commit_hash)` | Commit "proofs of work" |

Plus `seaql_migrations` — managed by `sea-orm-migration` to track applied versions. You shouldn't touch it manually.

### Tasks columns (the wide one)

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | autoincrement |
| `title` | TEXT NOT NULL | |
| `description` | TEXT | nullable |
| `status` | TEXT | `draft` \| `open` \| `review` \| `merged` \| `closed`; default `draft` |
| `priority` | TEXT | `low` \| `medium` \| `high` \| `critical`; nullable |
| `branch` | TEXT | local git branch |
| `base_branch` | TEXT | usually `main` |
| `provider` | TEXT | `github` \| `gitlab`; set after first push |
| `remote_id` | INTEGER | PR number on the remote |
| `source_url` | TEXT | direct URL to the remote PR |
| `assignee` | TEXT | username; joins `dev_roles.username` |
| `created_at` | TEXT (ISO 8601) | seeded by SQLite `strftime` default |
| `updated_at` | TEXT (ISO 8601) | bumped on every `update_task_field` |

### Migrations

`src/migration/` contains versioned migrations applied by `Migrator::up()` on every `Database::init()` (called by `init` and `find_db`).

Three migrations are shipped today:

1. `m20240101_000001_initial` — tasks, labels, task_labels, reviewers, comments
2. `m20240101_000002_gamification` — gamification (singleton + seed), achievements_unlocked
3. `m20240101_000003_deps_roles_proofs` — task_dependencies, dev_roles, task_commits

Each migration uses **`SchemaManager` + typed `ColumnDef`/`ForeignKey`/`Index`** — no SQL strings. The default-timestamp expression is the one place where SQLite-specific syntax leaks (`Expr::cust("(strftime('%Y-%m-%dT%H:%M:%S', 'now'))")`); this is contained behind the `now_default()` helper in each migration file.

> Migrations are **append-only**. Never edit a shipped migration — see [CONTRIBUTING § Adding a new database column or table](CONTRIBUTING.md#adding-a-new-database-column-or-table).

### Why no raw SQL?

SeaORM's typed query builder gives us:

- **Compile-time safety** for column names — typos become errors, not runtime failures.
- **No injection surface** — every value goes through bind parameters.
- **Refactorable joins** — relations are first-class enums; renaming a column in an entity propagates to every query.

The two interesting cases we kept in pure-builder form:

1. **Recursive CTE for cycle detection** (`db::Database::depends_transitively`). Built with `CommonTableExpression` + `WithClause::recursive(true)` + `anchor.union(UnionType::Distinct, recursive)`. Compiles to `WITH RECURSIVE chain(id) AS (...)`.

2. **Conditional `EXISTS` subqueries** for `--blocked` / `--ready`. Built as `Expr::exists(SelectStatement)` — see `deps_exists_subquery()` and `blocked_exists_subquery()` in `db.rs`.

The single intentional raw-SQL exception is `PRAGMA foreign_keys = ON;`, fired once per connection in `Database::open`. Pragmas configure the engine, not the data, and SeaORM has no typed API for them.

---

## Error model

`src/error.rs` defines a single `DevTodoError` enum. CLI commands return `Result<(), DevTodoError>`; `main.rs` prints `Error: {e}` to stderr and exits 1.

| Variant | When | Example message |
|---|---|---|
| `Db(sea_orm::DbErr)` | DB driver failure | `Database error: …` |
| `Api(reqwest::Error)` | provider HTTP failure | `API error: …` |
| `Config(String)` | unstructured config / input error | `Configuration error: Cannot update field: foo` |
| `NotFound(String, String)` | entity lookup miss | `Task not found: 42` |
| `InvalidStatus(String)` | bad status string | `Invalid status: revieq` |
| `InvalidPriority(String)` | bad priority string | `Invalid priority: urgent` |
| `Git(String)` | git CLI returned non-zero | `Git error: …` |
| **`DependencyCycle { from, to }`** | adding a dep would close a cycle | `Adding dependency would create a cycle: #2 already (transitively) depends on #1` |
| **`SelfDependency(i64)`** | task depending on itself | `A task cannot depend on itself (#3)` |
| **`NoBranch(i64)`** | proof/push without a branch set | `Task #1 has no branch set. Use \`devtodo edit 1 --branch <name>\`` |
| **`CommitNotFound { commit, reason }`** | git couldn't resolve the hash | `Commit not found: deadbeef (...)` |
| **`GitNotAvailable(String)`** | `git` binary missing | `Git is not available on this system: …` |
| **`InvalidDate { input, reason }`** | bad date input | `Cannot parse date 'next thursday': …` |
| `Io(std::io::Error)` | filesystem error | `IO error: …` |
| `Serialization(serde_json::Error)` | JSON encoding error | `Serialization error: …` |
| `TomlSerialization` / `TomlDeserialization` | config file error | `TOML serialization error: …` |

Variants in **bold** are the typed domain errors introduced when adding deps/proofs/dates. Tests assert on these via `matches!(err, DevTodoError::SelfDependency(_))`.

---

## Command reference

### Task lifecycle

```
draft  ──►  open  ──►  review  ──►  merged
           │ ▲         │  ▲         │
           ▼ └─ status ┘  ▼ status  ▼
          closed ◄──── closed   closed
```

`devtodo status <id> <new>` accepts any transition — there is no enforced state machine. Setting status to `merged` for the **first time** triggers gamification rewards. Setting it from `merged` to `merged` is a no-op for XP (intentional, asserted by `test_status_merged_to_merged_does_not_double_award`).

### List filters in detail

```bash
devtodo list \
  [--status <s>] [--priority <p>] [--assignee <u>] [--label <l>] [--role <r>] \
  [--has-deps | --no-deps] [--blocked | --ready] \
  [--created-from <date>] [--created-to <date>] \
  [--updated-from <date>] [--updated-to <date>] \
  [--sort created|updated|priority] [--limit <N>]
```

- `--has-deps` and `--no-deps` are mutually exclusive (clap-enforced)
- `--blocked` and `--ready` are mutually exclusive (clap-enforced)
- `--blocked` ⇒ task has at least one dep whose status ∉ {`merged`, `closed`}
- `--ready` ⇒ task is either dep-less OR all deps are merged/closed
- All `*-from` are inclusive; bare-date `*-to` is set to `23:59:59` of that day so an inclusive bound on a calendar day "just works"
- `--sort` defaults to `created` (descending)

### Date input grammar

Both `--created-from`/`-to` and `--updated-from`/`-to` accept:

| Form | Examples |
|---|---|
| ISO date | `2025-01-15` |
| ISO datetime | `2025-01-15T10:30:00` |
| Keywords | `now`, `today`, `yesterday` |
| Compact relative | `7d`, `2w`, `1m`, `1y`, `12h` |
| Natural relative | `3 days ago`, `1 week ago`, `1 year ago`, `2 hours ago` |
| Anything `dateparser` accepts | `Jan 5 2025`, `2025-01-15 10:30 UTC`, etc. |

Garbage input returns `DevTodoError::InvalidDate { input, reason }`.

> Implementation: `src/commands/dateparse.rs`. The relative parser is hand-rolled (~30 lines, chrono only); absolute dates fall through to the [`dateparser`](https://crates.io/crates/dateparser) crate.

### Dependencies (DAG)

```bash
devtodo deps add <task_id> <on_id>     # task_id depends on on_id
devtodo deps remove <task_id> <on_id>
devtodo deps list <task_id>            # what task_id depends on
devtodo deps dependents <task_id>      # what depends on task_id
devtodo deps tree <task_id>            # recursive tree, shows cycle marker
```

**Cycle detection** uses a SQLite recursive CTE (built via `WithClause::recursive(true)`) — it walks the existing graph from the proposed *blocker* and rejects the insertion if the proposed *dependent* is reachable. This catches both direct (A↔B) and transitive (A→B→C→A) cycles in a single query, no recursion in Rust.

The check is **before** the insert, so the DB never sees a cycle. The `task_dependencies` table also has a `CHECK (task_id != depends_on)` guard for the trivial case.

### Roles

Roles are a free-form string per username:

```bash
devtodo role set alice backend
devtodo role set bob frontend
devtodo role set carol devops
devtodo role get alice
devtodo role list
devtodo role remove alice
```

`set` is an upsert. There's no enum constraint — `backend`, `frontend`, `devops`, `qa`, `data`, anything works. Filtering uses `--role <r>`:

```bash
devtodo list --role backend
```

The implementation is a JOIN on `task.assignee = dev_roles.username`. Tasks whose assignee has no role row are excluded from `--role` results.

### Commit proofs

A "proof" is a git commit attached to a task as evidence that the work was done. Proofs are stored in `task_commits` keyed by `(task_id, commit_hash)`.

```bash
devtodo proof add <task_id> <commit>      # attach one commit (full or short hash)
devtodo proof auto <task_id>              # attach every commit in (base..branch)
devtodo proof list <task_id>
devtodo proof verify <task_id>            # check every attached commit still resolves
devtodo proof remove <task_id> <commit>
```

`add` resolves the commit-ish via `git show -s --format=...` (separator `\x1f` for safe field splitting). On success it stores the full hash, short hash, author name, message subject, and `committed_at` (ISO 8601 from `%aI`).

`auto` requires the task to have a `branch` set — otherwise returns `DevTodoError::NoBranch(id)`. It reads `git log --format=… base..branch` and inserts each commit. Re-attaching the same hash is **idempotent** (`ON CONFLICT … UPDATE`) — useful for refreshing an outdated message.

`verify` re-runs `git show -s` per attached commit and reports `N valid, M missing`. This catches commits that were rebased or garbage-collected.

`remove` accepts either the full or short hash — it tries to resolve through git first, falls back to the literal string.

### Sync, push, pull

```bash
devtodo sync                        # bidirectional with the configured provider
devtodo sync --dry-run              # preview, no writes
devtodo push <id>                   # push a local task as a PR
devtodo pull --provider github      # import remote PRs as local tasks
devtodo pull --repo owner/repo --state open
```

`sync` walks the configured provider's PRs:

- If a local task has matching `(provider, remote_id)`, it updates `status`, `title`, `description` if they drift.
- Otherwise it imports the remote PR via `import_remote_pr`, including labels (creating them if missing), reviewers, and comments.

`push` either creates a new remote PR (if `remote_id` is None) or updates the existing one's status. Status mapping:

| Local status | GitHub action |
|---|---|
| `draft` / `open` / `review` | `PATCH /pulls/{n}` with `state: open` |
| `closed` | `PATCH /pulls/{n}` with `state: closed` |
| `merged` | `PUT /pulls/{n}/merge` |

GitLab follows the same conceptual mapping for merge requests.

The provider abstraction is the `ProviderApi` trait in `src/providers/mod.rs`. Adding a Bitbucket/Gitea client means implementing `list_prs`, `get_pr`, `create_pr`, `update_pr_status`.

---

## Gamification math

All math lives in `src/gamification.rs` — no DB calls; pure functions composable in tests.

### XP per merge

```
xp_for_priority(Some(Critical))  = 100
xp_for_priority(Some(High))      = 50
xp_for_priority(Some(Medium))    = 25
xp_for_priority(None)            = 15   (neutral default)
xp_for_priority(Some(Low))       = 10
```

### Level curve

Quadratic: total XP needed to *reach* level N is

```
xp_for_level(N) = 50 × (N - 1)²    for N ≥ 1
xp_for_level(1) = 0
xp_for_level(2) = 50
xp_for_level(10) = 4 050
xp_for_level(50) = 120 050
xp_for_level(100) = 490 050         ← max level (clamped)
```

Inverse:

```
level_for_xp(xp) = floor(sqrt(xp / 50)) + 1   clamped to [1, 100]
```

`level_for_xp(-1)` returns 1 (negative XP treated as 0 — anti-test in place).

### Streaks

Streaks count consecutive **calendar days** with at least one merge:

| Previous date | Today | Result |
|---|---|---|
| `None` | first ever | `streak = 1`, `extended = true` |
| same as today | duplicate | `streak = max(current, 1)`, `extended = false` |
| `today.pred()` (yesterday) | continuation | `streak += 1`, `extended = true` |
| earlier than yesterday | gap | `streak = 1`, `extended = true` |
| **future** (clock skew) | defensive | `streak = max(current, 1)`, `last_completion_date` *preserved*, `extended = false` |

`longest_streak = max(longest_streak, current_streak)` — never decreases.

### Achievements

Ten badges, evaluated after each award:

| Badge | Condition |
|---|---|
| First Blood | `total_completed ≥ 1` |
| Grinder | `total_completed ≥ 10` |
| Workaholic | `total_completed ≥ 50` |
| Centurion | `total_completed ≥ 100` |
| Awakened | `level ≥ 10` |
| S-Rank Hunter | `level ≥ 50` |
| Monarch of Shadows | `level ≥ 100` |
| Week Warrior | `current_streak ≥ 7` |
| Unstoppable | `current_streak ≥ 30` |
| Shadow Army | `current_streak ≥ 100` |

Already-unlocked badges aren't re-emitted (DB guard + idempotent `INSERT … ON CONFLICT DO NOTHING`).

---

## Configuration

Config lives at `$XDG_CONFIG_HOME/devtodo/config.toml` (typically `~/.config/devtodo/config.toml` on Linux). The file is `chmod 600` after every write — token-grade security.

```toml
[default]
provider = "github"

[github]
token = "ghp_xxxxxxxxxxxxx"

[gitlab]
token = "glpat-xxxxxxxxxxxxx"
url = "https://gitlab.mycompany.com"   # optional, defaults to gitlab.com
```

| Key | Effect |
|---|---|
| `default.provider` | Used by `sync`/`pull` when `--provider` is omitted |
| `github.token` | GitHub PAT (Bearer auth) |
| `gitlab.token` | GitLab PAT (header `PRIVATE-TOKEN`) |
| `gitlab.url` | Self-hosted GitLab base URL |

Manage via:

```bash
devtodo config set <key> <value>
devtodo config get <key>          # tokens are masked: "ghp_xxxx...yyyy"
devtodo config list
```

The DB itself is **per-project** — `.devtodo.db` in your current working directory. `init` creates it; `find_db` looks for it (and runs pending migrations on every open).

---

## Output formats

### Tables (default `list`, `show`, etc.)

Built with [`comfy-table`](https://docs.rs/comfy-table) using the `UTF8_FULL` preset and `UTF8_ROUND_CORNERS` modifier. Status and priority are colorized via [`colored`](https://docs.rs/colored).

### JSON (`show --json`, `export json`)

```json
{
  "task": { "id": 1, "title": "...", "status": "open", ... },
  "labels": [ { "id": 1, "name": "bug", ... } ],
  "reviewers": [ { "username": "alice", "status": "approved", ... } ],
  "comments": [ ]
}
```

### CSV (`export csv`)

Header: `id,title,status,priority,branch,base_branch,assignee,created_at,updated_at`. Quotes are doubled (`"`→`""`) inside title.

### Markdown (`export markdown`)

Standard pipe table with `| ID | Title | Status | Priority | Branch | Assignee |`.

---

## Storage & file layout

```
~/.config/devtodo/
└── config.toml          chmod 600 (tokens)

<project>/
└── .devtodo.db          SQLite, WAL mode (sqlx default for sqlite::file:)
```

`.devtodo.db` is the SQLite file. With `sqlx-sqlite` it uses WAL by default — safe for concurrent reads while a write is in flight. Add `.devtodo.db` and `.devtodo.db-*` to `.gitignore` if you don't want to commit your task list.

---

## Performance notes

- **First-run cold compile**: ~75 s (SeaORM proc macros). Subsequent debug builds: sub-second.
- **In-memory DBs for tests**: every `Database::open_in_memory().await` is a fresh isolated DB; tests run in parallel without interference (~0.2 s for the 98 unit tests).
- **CLI integration tests**: ~75 s total (they spawn the binary in `tempfile::TempDir`s; some create real git repos). Run them in parallel with `cargo test` — they tolerate it.
- **Recursive CTE for cycle detection**: scales linearly in DAG size; for a project with thousands of tasks and dependencies, still sub-millisecond.
- **JSON exports**: serialized via `serde_json`; not streamed — fine up to ~100 k tasks.
