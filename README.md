# devtodo

[![Build](https://github.com/jojo8356/devtodolist/actions/workflows/build.yml/badge.svg)](https://github.com/jojo8356/devtodolist/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

A developer todolist CLI where **every task is a Pull Request**.

Manage your dev tasks directly from the terminal with the same lifecycle as a real PR — draft, open, review, merged, closed — and sync them bidirectionally with GitHub and GitLab.

> Want the deep dive? See [DOC.md](DOC.md). Want to contribute? See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Features

- **PR-based workflow** — each task follows a pull request lifecycle (`draft → open → review → merged|closed`)
- **GitHub & GitLab sync** — bidirectional sync, `push` local tasks as PRs, `pull` remote PRs
- **Task dependencies** — DAG with cycle detection (`devtodo deps add 2 1`); filter blocked vs. ready tasks
- **Developer roles** — tag assignees as `backend`/`frontend`/`devops`/…, then filter `--role backend`
- **Commit proofs** — attach git commits to tasks (`devtodo proof add` / `proof auto` / `proof verify`)
- **Powerful filters** — by status, label, priority, assignee, role, deps state, *date range* (`--created-from "1w ago"`)
- **Gamification** — level up from 1 to 100, daily streaks, 10 achievements
- **SQLite storage via SeaORM** — typed entities, versioned migrations, in-memory tests
- **Statistics** — time to merge, breakdown by status/priority/label
- **Export** — JSON, CSV, Markdown
- **Shell completions** — bash, zsh, fish

---

## Gamification

Shipping code should feel like hunting. Every merged task levels you up — from a nameless E-rank hunter to the **Monarch of Shadows**.

- **Levels 1 → 100** with a quadratic XP curve (`XP for level N = 50 × (N−1)²`; level 100 at 490,050 XP)
- **XP per merge** scales with priority:

  | Priority | XP |
  |----------|----|
  | Critical | 100 |
  | High     | 50 |
  | Medium   | 25 |
  | None     | 15 |
  | Low      | 10 |

- **Daily streaks** — close a task every day and the fire keeps burning. Miss a day and the streak resets (but your longest ever stays on record).
- **Achievements** — ten badges from *First Blood* to *Monarch of Shadows*, unlocked automatically.
- **Level-up SFX** — a terminal bell fires when you rank up.

```bash
devtodo status 42 merged   # gain XP, maybe level up
devtodo profile            # see your hunter card
```

```
╔══════════════════════════════════════╗
║       devtodo — Hunter Profile       ║
╚══════════════════════════════════════╝

Level   42  /  100
[████████░░░░░░░░░░]   8450 / 19600 XP   (150 to next)

Current streak  12 🔥    Longest  28 🔥
Tasks merged    127

Achievements  6 / 10
  ✓ First Blood          — Complete your first task
  ✓ Grinder              — Merge 10 tasks
  ✗ S-Rank Hunter        — Reach level 50
  ✗ Monarch of Shadows   — Reach level 100
```

---

## Installation

### From source

```bash
git clone https://github.com/jojo8356/devtodolist.git
cd devtodolist
cargo build --release
cp target/release/devtodo ~/.local/bin/
```

> First build takes ~75 s (SeaORM macros). Incremental rebuilds are sub-second.

---

## Quick Start

```bash
# Initialize in your project
cd my-project
devtodo init

# Create a task
devtodo add "Implement JWT auth" \
  -d "Add JWT with refresh tokens" \
  -p high \
  -b feature/jwt-auth \
  -l feature -l security \
  -a alice

# List & filter
devtodo list
devtodo list --status review --priority high
devtodo list --created-from "1w ago" --role backend

# Dependencies
devtodo deps add 2 1            # task #2 depends on #1
devtodo deps tree 2             # show full dep tree
devtodo list --blocked          # only tasks blocked by an unmerged dep
devtodo list --ready            # tasks whose deps are all merged

# Roles
devtodo role set alice backend
devtodo list --role backend

# Commit proofs (must be in a git repo)
devtodo proof add 1 a373e29     # attach a single commit
devtodo proof auto 1            # import every commit on the task's branch
devtodo proof verify 1          # check all attached commits still resolve

# Lifecycle
devtodo status 1 review
devtodo review assign 1 bob
devtodo review status 1 bob approved
devtodo status 1 merged         # ★ XP gained ★

# Stats & export
devtodo stats
devtodo export markdown -o TASKS.md
```

---

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize database in current directory |
| `add <title>` | Create a new task |
| `list` | List tasks (filters: `--status`, `--label`, `--priority`, `--assignee`, `--role`, `--has-deps`, `--no-deps`, `--blocked`, `--ready`, `--created-from`/`--created-to`, `--updated-from`/`--updated-to`, `--sort`, `--limit`) |
| `show <id>` | Show task details (`--comments`, `--json`) |
| `edit <id>` | Edit a task |
| `status <id> <status>` | Change status (`draft`/`open`/`review`/`merged`/`closed`) |
| `delete <id>` | Delete a task |
| `label <subcommand>` | Manage labels (`add`/`remove`/`list`/`assign`/`unassign`) |
| `review <subcommand>` | Manage reviewers (`assign`/`remove`/`status`/`list`) |
| `deps <subcommand>` | Task DAG (`add`/`remove`/`list`/`dependents`/`tree`) |
| `role <subcommand>` | Developer roles (`set`/`get`/`remove`/`list`) |
| `proof <subcommand>` | Commit proofs (`add`/`auto`/`list`/`remove`/`verify`) |
| `sync` | Bidirectional sync with the configured provider |
| `push <id>` | Push a local task as a PR |
| `pull` | Import PRs from a remote |
| `stats` | Show statistics |
| `export <format>` | Export tasks (`json`/`csv`/`markdown`) |
| `config <subcommand>` | Manage configuration (`set`/`get`/`list`) |
| `profile` | Show your hunter profile |
| `completions <shell>` | Generate shell completions (`bash`/`zsh`/`fish`) |

See [DOC.md](DOC.md) for every flag, every error variant, and the full data model.

---

## Filtering & dates

```bash
# Date range (ISO + natural language)
devtodo list --created-from "2025-01-01" --created-to "2025-06-30"
devtodo list --updated-from "yesterday"
devtodo list --created-from "1w ago" --no-deps

# Combine deps + role + dates
devtodo list --ready --role backend --updated-from "3 days ago"
```

Accepted date formats:

- ISO: `2025-01-15`, `2025-01-15T10:30:00`
- Relative: `today`, `yesterday`, `now`, `7d`, `2w`, `1m`, `1y`, `3 days ago`, `1 week ago`, `1 year ago`

---

## GitHub & GitLab integration

```bash
# Configure tokens (stored in $XDG_CONFIG_HOME/devtodo/config.toml, chmod 600)
devtodo config set github.token ghp_xxxxxxxxxxxx
devtodo config set gitlab.token glpat-xxxxxxxxxxxx
devtodo config set default.provider github

# For self-hosted GitLab
devtodo config set gitlab.url https://gitlab.mycompany.com

# Push a local task as a PR
devtodo push 1

# Import remote PRs
devtodo pull --provider github

# Bidirectional sync
devtodo sync
devtodo sync --dry-run
```

---

## Task lifecycle

```
draft  -->  open  -->  review  -->  merged
                  \              \
                   -->  closed    -->  closed
```

Status transitions are free-form (no enforced state machine). Setting status to `merged` for the first time triggers the gamification reward.

---

## Tech stack

| Component | Choice |
|-----------|--------|
| Language | Rust 2024 edition |
| CLI | clap (derive) |
| Database | SQLite via **SeaORM** (sqlx-sqlite + tokio-rustls) |
| Migrations | sea-orm-migration (typed `SchemaManager`) |
| HTTP | reqwest + tokio (rustls only, no native-tls) |
| Display | comfy-table + colored |
| Tests | tokio + assert_cmd + predicates + rstest + tempfile |

The whole data layer is **SQL-string-free**: every query (including the recursive CTE for cycle detection) is built with SeaORM's typed query builder. Only the SQLite `PRAGMA foreign_keys` is left as a raw statement, since pragmas aren't part of the data model. See [DOC.md § Data layer](DOC.md#data-layer) for the rationale.

---

## Project structure

```
src/
├── main.rs              Entry point & command dispatch (#[tokio::main])
├── lib.rs               Library re-exports for integration tests
├── cli.rs               clap derive: every command + every flag
├── db.rs                SeaORM facade — async API, no raw SQL
├── models.rs            Domain types: Task, Label, Reviewer, ...
├── error.rs             DevTodoError (typed variants)
├── display.rs           comfy-table + colored output
├── gamification.rs      Levels, XP curve, streaks, achievements
├── entities/            SeaORM entities (one per table)
├── migration/           Versioned migrations via sea-orm-migration
├── commands/            One module per top-level command
└── providers/           GitHub & GitLab REST API clients

tests/
├── common/              Shared TestProject helper
├── cli_smoke.rs         init / help / NotFound / migration idempotency
├── cli_deps.rs          dependency CRUD, cycle detection, filters
├── cli_role.rs          role CRUD + filter
├── cli_dates.rs         every accepted date format + range filtering
├── cli_proof.rs         commit attach / auto-import / verify
└── gamification_db.rs   profile persistence, achievement orchestration
```

---

## Development

```bash
cargo build                 # debug build
cargo test                  # run the full suite (152 tests)
cargo clippy --all-targets  # zero warnings expected
cargo fmt                   # rustfmt
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full contributor workflow, commit conventions, and how to add a new entity, migration, or command.

---

## License

[MIT](LICENSE) © 2026 Johan Polsinelli
