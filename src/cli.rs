use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "devtodo",
    version,
    about = "Developer todolist where each task is a pull request"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize devtodo in the current directory
    Init,

    /// Create a new task
    Add {
        /// Task title
        title: String,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority: low, medium, high, critical
        #[arg(short, long)]
        priority: Option<String>,

        /// Git branch name
        #[arg(short, long)]
        branch: Option<String>,

        /// Target base branch (default: main)
        #[arg(long, default_value = "main")]
        base: String,

        /// Labels (repeatable)
        #[arg(short, long)]
        label: Vec<String>,

        /// Assignee username
        #[arg(short, long)]
        assignee: Option<String>,
    },

    /// List tasks
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by label
        #[arg(short, long)]
        label: Option<String>,

        /// Filter by priority
        #[arg(short, long)]
        priority: Option<String>,

        /// Filter by assignee
        #[arg(short, long)]
        assignee: Option<String>,

        /// Filter by developer role (e.g. backend, frontend, devops)
        #[arg(long)]
        role: Option<String>,

        /// Created on/after (ISO date or natural: "2025-01-01", "7d ago")
        #[arg(long)]
        created_from: Option<String>,

        /// Created on/before
        #[arg(long)]
        created_to: Option<String>,

        /// Updated on/after
        #[arg(long)]
        updated_from: Option<String>,

        /// Updated on/before
        #[arg(long)]
        updated_to: Option<String>,

        /// Only tasks that have at least one dependency
        #[arg(long, conflicts_with = "no_deps")]
        has_deps: bool,

        /// Only tasks that have no dependencies (independent)
        #[arg(long)]
        no_deps: bool,

        /// Only tasks blocked by an unmerged dependency
        #[arg(long, conflicts_with = "ready")]
        blocked: bool,

        /// Only tasks whose dependencies are all merged/closed
        #[arg(long)]
        ready: bool,

        /// Sort by: created, updated, priority
        #[arg(long, default_value = "created")]
        sort: String,

        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Show task details
    Show {
        /// Task ID
        id: i64,

        /// Show comments
        #[arg(long)]
        comments: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Edit a task
    Edit {
        /// Task ID
        id: i64,

        /// New title
        #[arg(short, long)]
        title: Option<String>,

        /// New description
        #[arg(short, long)]
        description: Option<String>,

        /// New priority
        #[arg(short, long)]
        priority: Option<String>,

        /// New branch
        #[arg(short, long)]
        branch: Option<String>,

        /// New assignee
        #[arg(short, long)]
        assignee: Option<String>,
    },

    /// Change task status
    Status {
        /// Task ID
        id: i64,

        /// New status: draft, open, review, merged, closed
        status: String,
    },

    /// Delete a task
    Delete {
        /// Task ID
        id: i64,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },

    /// Manage labels
    Label {
        #[command(subcommand)]
        command: LabelCommands,
    },

    /// Manage reviewers
    Review {
        #[command(subcommand)]
        command: ReviewCommands,
    },

    /// Sync with remote provider
    Sync {
        /// Provider: github, gitlab
        #[arg(long)]
        provider: Option<String>,

        /// Show changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Push a task as a PR to the remote
    Push {
        /// Task ID
        id: i64,
    },

    /// Import PRs from remote
    Pull {
        /// Provider: github, gitlab
        #[arg(long)]
        provider: Option<String>,

        /// Repository (owner/repo)
        #[arg(long)]
        repo: Option<String>,

        /// PR state: open, closed, all
        #[arg(long, default_value = "open")]
        state: String,
    },

    /// Show statistics
    Stats {
        /// Period: 7d, 30d, 90d, all
        #[arg(long, default_value = "all")]
        period: String,
    },

    /// Export tasks
    Export {
        /// Format: json, csv, markdown
        format: ExportFormat,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Show your hunter profile (level, XP, streaks, achievements)
    Profile,

    /// Manage task dependencies (DAG)
    Deps {
        #[command(subcommand)]
        command: DepsCommands,
    },

    /// Manage developer roles (backend, frontend, ...)
    Role {
        #[command(subcommand)]
        command: RoleCommands,
    },

    /// Attach git commits as proof of work for a task
    Proof {
        #[command(subcommand)]
        command: ProofCommands,
    },

    /// Generate shell completions
    Completions {
        /// Shell: bash, zsh, fish
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum LabelCommands {
    /// Create a new label
    Add {
        /// Label name
        name: String,
        /// Label color (hex, e.g. #ff0000)
        #[arg(long)]
        color: Option<String>,
    },
    /// Remove a label
    Remove {
        /// Label name
        name: String,
    },
    /// List all labels
    List,
    /// Assign a label to a task
    Assign {
        /// Task ID
        task_id: i64,
        /// Label name
        label: String,
    },
    /// Unassign a label from a task
    Unassign {
        /// Task ID
        task_id: i64,
        /// Label name
        label: String,
    },
}

#[derive(Subcommand)]
pub enum ReviewCommands {
    /// Assign a reviewer to a task
    Assign {
        /// Task ID
        task_id: i64,
        /// Reviewer username
        username: String,
    },
    /// Remove a reviewer from a task
    Remove {
        /// Task ID
        task_id: i64,
        /// Reviewer username
        username: String,
    },
    /// Update review status
    Status {
        /// Task ID
        task_id: i64,
        /// Reviewer username
        username: String,
        /// Review status: approved, changes_requested
        status: String,
    },
    /// List reviewers for a task
    List {
        /// Task ID
        task_id: i64,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Set a configuration value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
    /// Get a configuration value
    Get {
        /// Config key
        key: String,
    },
    /// List all configuration
    List,
}

#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    Markdown,
}

#[derive(Subcommand)]
pub enum DepsCommands {
    /// Add a dependency: <task_id> depends on <on>
    Add {
        /// Task that gains a dependency
        task_id: i64,
        /// Task it depends on
        on: i64,
    },
    /// Remove a dependency
    Remove { task_id: i64, on: i64 },
    /// List direct dependencies of a task
    List { task_id: i64 },
    /// List tasks that depend on this one (its dependents)
    Dependents { task_id: i64 },
    /// Show the full dependency tree of a task
    Tree { task_id: i64 },
}

#[derive(Subcommand)]
pub enum RoleCommands {
    /// Set a developer's role
    Set {
        username: String,
        /// Role name (free-form, e.g. backend, frontend, devops, qa)
        role: String,
    },
    /// Remove a developer's role
    Remove { username: String },
    /// Get a developer's role
    Get { username: String },
    /// List all developer roles
    List,
}

#[derive(Subcommand)]
pub enum ProofCommands {
    /// Attach a commit as proof for a task
    Add {
        task_id: i64,
        /// Commit hash (full or short). Resolved via `git`.
        commit: String,
    },
    /// Auto-import commits from the task's branch (HEAD..base)
    Auto { task_id: i64 },
    /// List proofs attached to a task
    List { task_id: i64 },
    /// Remove a proof
    Remove { task_id: i64, commit: String },
    /// Verify all proofs still resolve in the local git repo
    Verify { task_id: i64 },
}
