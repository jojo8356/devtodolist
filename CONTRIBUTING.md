# Contributing to devtodo

Thanks for considering a contribution! This guide covers everything you need to set up the project, run the test suite, and ship a clean PR.

For a high-level tour of the codebase and the design decisions behind it, read [DOC.md](DOC.md) first.

---

## Table of contents

- [Quick start](#quick-start)
- [Development setup](#development-setup)
- [Running tests](#running-tests)
- [Code style](#code-style)
- [Commit & PR conventions](#commit--pr-conventions)
- [How-to guides](#how-to-guides)
  - [Adding a new CLI command](#adding-a-new-cli-command)
  - [Adding a new database column or table](#adding-a-new-database-column-or-table)
  - [Adding a new error variant](#adding-a-new-error-variant)
  - [Adding integration tests](#adding-integration-tests)
- [Reporting bugs](#reporting-bugs)

---

## Quick start

```bash
git clone https://github.com/jojo8356/devtodolist.git
cd devtodolist
cargo build
cargo test
```

If everything is green, you're set.

---

## Development setup

### Prerequisites

- **Rust 1.75+** (the project pins `edition = "2024"`). Install via [rustup](https://rustup.rs/).
- **git** in `$PATH` — the `proof` and `sync` commands shell out to it; integration tests build temporary git repos.
- **No native dependencies** — TLS goes through `rustls`, SQLite is bundled by `sqlx`. No `pkg-config`/`openssl-dev` needed.

### First build

```bash
cargo build
```

The first build downloads SeaORM's procedural macros and takes ~75 s. Incremental rebuilds are sub-second.

### Running locally

```bash
cargo run -- init
cargo run -- add "My first task" --priority high
cargo run -- list
```

Each `cargo run` creates `.devtodo.db` in your current working directory. Delete it to reset state.

---

## Running tests

```bash
cargo test                    # full suite (~2 min on cold cache)
cargo test --lib              # unit tests only (sub-second)
cargo test --test cli_deps    # one integration file
cargo test deps_self_loop     # by name substring
```

The suite is **152 tests** across 9 binaries:

| File | Type | What it covers |
|---|---|---|
| `src/db.rs` | unit (`#[tokio::test]`) | SeaORM CRUD, filters, cycle detection, migrations |
| `src/gamification.rs` | unit | XP/level math, streak rules, achievement thresholds |
| `src/models.rs` | unit | Status/Priority parsing & serde roundtrips |
| `src/commands/dateparse.rs` | unit | Date input parsing |
| `tests/gamification_db.rs` | integration | Gamification orchestration against an in-memory DB |
| `tests/cli_smoke.rs` | CLI integration (assert_cmd) | init, help, NotFound errors, migration idempotency |
| `tests/cli_deps.rs` | CLI integration | deps CRUD + cycles + list filters |
| `tests/cli_role.rs` | CLI integration | role CRUD + `--role` filter |
| `tests/cli_dates.rs` | CLI integration | every date format the parser accepts |
| `tests/cli_proof.rs` | CLI integration | proof attach/auto/verify, NoBranch error |

CLI integration tests use [`assert_cmd`](https://docs.rs/assert_cmd) to spawn the binary in a fresh `tempfile::TempDir`, so tests don't share state.

### Adding a test

Most new features need both:

1. **A unit test** in the relevant `src/*.rs` covering the lowest-level path (DB, parser, math).
2. **A CLI integration test** in `tests/cli_*.rs` that exercises the user-facing command end-to-end and asserts on stderr for typed error variants.

See [Adding integration tests](#adding-integration-tests).

---

## Code style

### Formatting & lints

CI gates on these — please run them locally before pushing.

```bash
cargo fmt              # rustfmt with default settings
cargo clippy --all-targets  # zero warnings expected
```

### Style conventions

- **No raw SQL strings.** All queries go through SeaORM's typed builder. The single exception is `PRAGMA foreign_keys = ON;` since pragmas aren't part of the data model.
- **Typed errors.** Prefer specific `DevTodoError` variants (`SelfDependency`, `DependencyCycle`, `NoBranch`, `CommitNotFound`, `InvalidDate`, …) over `DevTodoError::Config(format!(...))`. Add a new variant when a generic message would lose information.
- **No `unwrap()` in production code paths.** Tests are fine. In production, propagate via `?`.
- **Async all the way down.** Every DB call is `async`; commands are `async`; `main` is `#[tokio::main]`. Don't introduce `block_on` inside the runtime.
- **Comments explain *why*, not *what*.** A short doc-comment on non-obvious functions is better than a paragraph above an obvious one.

### Module layout

- `src/cli.rs` — `clap` derive only; no logic
- `src/commands/<name>.rs` — one file per top-level command, each with `pub async fn run(...)`
- `src/db.rs` — the only place where SeaORM is imported by application code
- `src/entities/<table>.rs` — one file per table, with relations
- `src/migration/m<date>_<slug>.rs` — append-only

---

## Commit & PR conventions

### Commits

Follow a [Conventional Commits](https://www.conventionalcommits.org/) flavor — short imperative subject, optional body. The repo's history (`git log`) is the canonical reference. Examples:

```
feat: add task dependencies with cycle detection
fix: get_task returns NotFound instead of generic Db error
test: add CLI integration tests for proof.auto
refactor: replace raw SQL counts with sea_query GROUP BY
docs: expand README date filter examples
```

Subject lines: ≤ 72 chars, no trailing period, lowercase verb.

### Pull requests

A good PR includes:

1. **Why this change?** — the user-visible problem or architectural goal.
2. **What changed at a high level** — bullet list.
3. **Tests** — what you added and how to run them.
4. **Screenshots/output** — for any user-visible output change (table layout, error message, profile screen).

CI runs `cargo build`, `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`. PRs that don't pass CI won't be reviewed until they're green.

---

## How-to guides

### Adding a new CLI command

Suppose you want `devtodo archive <id>`.

1. **Declare the command in `src/cli.rs`:**
   ```rust
   /// Archive a task (soft-delete; keeps stats but hides from list)
   Archive {
       id: i64,
       /// Skip the confirmation prompt
       #[arg(long)]
       force: bool,
   },
   ```

2. **Create `src/commands/archive.rs`:**
   ```rust
   use colored::Colorize;
   use crate::commands::init::find_db;
   use crate::error::Result;

   pub async fn run(id: i64, force: bool) -> Result<()> {
       let db = find_db().await?;
       let task = db.get_task(id).await?;
       // ... do the thing
       println!("{} Archived #{}", "✓".green().bold(), id);
       Ok(())
   }
   ```

3. **Register the module in `src/commands/mod.rs`:**
   ```rust
   pub mod archive;
   ```

4. **Wire it up in `src/main.rs`:**
   ```rust
   Commands::Archive { id, force } => commands::archive::run(*id, *force).await,
   ```

5. **Add tests** — a CLI integration test in `tests/cli_smoke.rs` (or a new `tests/cli_archive.rs`) that uses `TestProject` to verify the happy path and at least one error path.

### Adding a new database column or table

The schema is **append-only**. Never edit a shipped migration — add a new one.

1. **Add a new migration file** under `src/migration/`, named `m<YYYYMMDD>_<NNNNNN>_<slug>.rs`:
   ```rust
   use sea_orm_migration::prelude::*;
   use sea_orm_migration::schema::*;

   #[derive(DeriveMigrationName)]
   pub struct Migration;

   #[derive(DeriveIden)]
   enum Tasks {
       Table,
       ArchivedAt,  // new column
   }

   #[async_trait::async_trait]
   impl MigrationTrait for Migration {
       async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
           manager
               .alter_table(
                   Table::alter()
                       .table(Tasks::Table)
                       .add_column(string_null(Tasks::ArchivedAt))
                       .to_owned(),
               )
               .await
       }

       async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
           manager
               .alter_table(
                   Table::alter()
                       .table(Tasks::Table)
                       .drop_column(Tasks::ArchivedAt)
                       .to_owned(),
               )
               .await
       }
   }
   ```

2. **Register it in `src/migration/mod.rs`:**
   ```rust
   mod m20260601_000004_archived_at;
   // ...
   Box::new(m20260601_000004_archived_at::Migration),
   ```

3. **Update the entity** under `src/entities/<table>.rs` to expose the new field on `Model` and `ActiveModel`.

4. **Use the new field** through `Entity::find()` / `ActiveModel::insert()` — never via raw SQL.

5. **Add a unit test** in `src/db.rs` that exercises the new column round-trip.

### Adding a new error variant

In `src/error.rs`, add the variant:

```rust
#[error("Task #{0} is archived")]
TaskArchived(i64),
```

Then return it from the relevant call site instead of a generic `Config(format!(...))`. Add an integration test that asserts on the message:

```rust
.stderr(predicate::str::contains("is archived"));
```

### Adding integration tests

CLI tests use the `TestProject` helper in `tests/common/mod.rs`:

```rust
mod common;
use common::TestProject;
use predicates::prelude::*;

#[test]
fn my_command_does_the_thing() {
    let p = TestProject::new();          // tempdir + `devtodo init`
    p.cmd().args(["add", "T"]).assert().success();
    p.cmd()
        .args(["my-command", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("expected output"));
}
```

For commands that need a real git repo (proof tests):

```rust
let p = TestProject::new_with_git();
let hash = p.commit("first commit");
p.cmd().args(["proof", "add", "1", &hash]).assert().success();
```

For parameterized tests, use [`rstest`](https://docs.rs/rstest):

```rust
#[rstest]
#[case::iso("2025-01-01")]
#[case::natural("yesterday")]
fn list_accepts_date_input(#[case] input: &str) {
    let p = TestProject::new();
    p.cmd().args(["list", "--created-from", input]).assert().success();
}
```

> ⚠️ When asserting on table output with `predicate::str::contains`, beware of substring collisions with column headers. `"A"` matches `"Assignee"`. Prefer distinctive titles like `task-alpha`.

---

## Reporting bugs

Please open an issue at <https://github.com/jojo8356/devtodolist/issues> with:

- `devtodo --version`
- The exact command you ran
- The full output (stdout + stderr)
- What you expected vs. what happened
- A minimal repro if you can

For security issues (e.g., token handling), prefer emailing the maintainer directly rather than filing a public issue.

---

By contributing, you agree that your contributions will be licensed under the project's [MIT License](LICENSE).
