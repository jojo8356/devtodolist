use thiserror::Error;

pub type Result<T> = std::result::Result<T, DevTodoError>;

#[derive(Debug, Error)]
pub enum DevTodoError {
    #[error("Database error: {0}")]
    Db(#[from] sea_orm::DbErr),

    #[error("API error: {0}")]
    Api(#[from] reqwest::Error),

    /// Generic configuration / user-input error with no more specific variant.
    /// New code should prefer one of the typed variants below.
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("{0} not found: {1}")]
    NotFound(String, String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Invalid priority: {0}")]
    InvalidPriority(String),

    #[error("Git error: {0}")]
    Git(String),

    // ── Typed domain errors (introduced for deps / proofs / dates) ──
    /// Adding the edge {from} -> {to} would close a cycle in the DAG.
    #[error("Adding dependency would create a cycle: #{to} already (transitively) depends on #{from}")]
    DependencyCycle { from: i64, to: i64 },

    /// A task is not allowed to depend on itself.
    #[error("A task cannot depend on itself (#{0})")]
    SelfDependency(i64),

    /// A proof or push command needs a branch on the task but none is set.
    #[error("Task #{0} has no branch set. Use `devtodo edit {0} --branch <name>`")]
    NoBranch(i64),

    /// The given commit-ish could not be resolved by git.
    #[error("Commit not found: {commit} ({reason})")]
    CommitNotFound { commit: String, reason: String },

    /// `git` binary is missing or not invokable.
    #[error("Git is not available on this system: {0}")]
    GitNotAvailable(String),

    /// User-supplied date string could not be parsed.
    #[error("Cannot parse date '{input}': {reason}")]
    InvalidDate { input: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialization(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialization(#[from] toml::de::Error),
}
