use thiserror::Error;

pub type Result<T> = std::result::Result<T, DevTodoError>;

#[derive(Debug, Error)]
pub enum DevTodoError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("API error: {0}")]
    Api(#[from] reqwest::Error),

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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialization(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialization(#[from] toml::de::Error),
}
