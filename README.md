# devtodo

[![Build](https://github.com/jojo8356/devtodolist/actions/workflows/build.yml/badge.svg)](https://github.com/jojo8356/devtodolist/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

A developer todolist CLI where **every task is a Pull Request**.

Manage your dev tasks directly from the terminal with the same lifecycle as a real PR — draft, open, review, merged, closed — and sync them bidirectionally with GitHub and GitLab.

---

## Features

- **PR-based workflow** — each task follows a pull request lifecycle
- **SQLite storage** — lightweight, portable, no server needed
- **GitHub & GitLab sync** — bidirectional sync, push PRs, import remote MRs
- **Labels, reviewers, priorities** — full metadata per task
- **Filters & sorting** — by status, label, priority, assignee
- **Statistics** — time to merge, breakdown by status/priority/label
- **Export** — JSON, CSV, Markdown
- **Shell completions** — bash, zsh, fish
- **Colored output** — status and priority highlighting in terminal

## Installation

### From source

```bash
git clone https://github.com/jojo8356/devtodolist.git
cd devtodolist
cargo build --release
```

The binary will be at `target/release/devtodo`.

### Add to PATH

```bash
cp target/release/devtodo ~/.local/bin/
```

## Quick Start

```bash
# Initialize in your project
cd my-project
devtodo init

# Create a task
devtodo add "Implement JWT authentication" \
  -d "Add JWT with refresh tokens" \
  -p high \
  -b feature/jwt-auth \
  -l feature -l security \
  -a alice

# List tasks
devtodo list

# Change status
devtodo status 1 review

# Assign a reviewer
devtodo review assign 1 bob
devtodo review status 1 bob approved

# View details
devtodo show 1
devtodo show 1 --json

# Merge
devtodo status 1 merged

# View stats
devtodo stats
```

## Commands

| Command | Description |
|---------|-------------|
| `devtodo init` | Initialize database in current directory |
| `devtodo add <title>` | Create a new task |
| `devtodo list` | List tasks (with filters) |
| `devtodo show <id>` | Show task details |
| `devtodo edit <id>` | Edit a task |
| `devtodo status <id> <status>` | Change status (draft/open/review/merged/closed) |
| `devtodo delete <id>` | Delete a task |
| `devtodo label <subcommand>` | Manage labels (add/remove/list/assign/unassign) |
| `devtodo review <subcommand>` | Manage reviewers (assign/remove/status/list) |
| `devtodo sync` | Sync with remote provider |
| `devtodo push <id>` | Push task as PR to remote |
| `devtodo pull` | Import PRs from remote |
| `devtodo stats` | Show statistics |
| `devtodo export <format>` | Export tasks (json/csv/markdown) |
| `devtodo config <subcommand>` | Manage configuration (set/get/list) |
| `devtodo completions <shell>` | Generate shell completions |

## GitHub & GitLab Integration

```bash
# Configure tokens
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
devtodo sync --dry-run  # preview changes
```

## Task Lifecycle

```
draft  -->  open  -->  review  -->  merged
                  \              \
                   -->  closed    -->  closed
```

## Filtering & Sorting

```bash
# Filter by status
devtodo list --status open

# Filter by label
devtodo list --label bug

# Filter by priority
devtodo list --priority critical

# Filter by assignee
devtodo list --assignee alice

# Sort and limit
devtodo list --sort updated --limit 10

# Combine filters
devtodo list --status review --priority high --label feature
```

## Export

```bash
# To stdout
devtodo export json
devtodo export csv
devtodo export markdown

# To file
devtodo export markdown -o TASKS.md

# With filter
devtodo export json --status open
```

## Shell Completions

```bash
# Bash
devtodo completions bash > ~/.local/share/bash-completion/completions/devtodo

# Zsh
devtodo completions zsh > ~/.zfunc/_devtodo

# Fish
devtodo completions fish > ~/.config/fish/completions/devtodo.fish
```

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust |
| CLI | clap (derive) |
| Database | SQLite (rusqlite) |
| HTTP | reqwest + tokio |
| Serialization | serde + serde_json |
| Display | comfy-table + colored |
| Config | TOML (XDG-compliant) |

## Project Structure

```
src/
├── main.rs           Entry point & command dispatch
├── cli.rs            CLI definition (clap derive)
├── db.rs             SQLite layer (schema, CRUD, stats)
├── models.rs         Data types (Task, Label, Reviewer, etc.)
├── error.rs          Unified error type (thiserror)
├── display.rs        Terminal formatting (tables, colors)
├── commands/         Command implementations
│   ├── init.rs       Database initialization
│   ├── add.rs        Task creation
│   ├── list.rs       Task listing with filters
│   ├── show.rs       Task detail view
│   ├── edit.rs       Task modification
│   ├── status.rs     Status transitions
│   ├── delete.rs     Task deletion
│   ├── label.rs      Label management
│   ├── review.rs     Reviewer management
│   ├── sync_cmd.rs   Sync, push, pull
│   ├── stats.rs      Statistics
│   ├── export.rs     JSON/CSV/Markdown export
│   └── config.rs     Configuration management
└── providers/        Remote API integrations
    ├── mod.rs        Provider trait
    ├── github.rs     GitHub REST API v3
    └── gitlab.rs     GitLab REST API v4
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run
cargo run -- init
cargo run -- add "My task" -p high
cargo run -- list
```

## License

MIT
