use std::fmt;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

// ── Task Status ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Draft,
    Open,
    Review,
    Merged,
    Closed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Review => "review",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = crate::error::DevTodoError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "open" => Ok(Self::Open),
            "review" => Ok(Self::Review),
            "merged" => Ok(Self::Merged),
            "closed" => Ok(Self::Closed),
            other => Err(crate::error::DevTodoError::InvalidStatus(other.to_string())),
        }
    }
}

// ── Priority ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Priority {
    type Err = crate::error::DevTodoError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            other => Err(crate::error::DevTodoError::InvalidPriority(other.to_string())),
        }
    }
}

// ── Review Status ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
        }
    }
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ReviewStatus {
    type Err = crate::error::DevTodoError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "changes_requested" => Ok(Self::ChangesRequested),
            other => Err(crate::error::DevTodoError::InvalidStatus(other.to_string())),
        }
    }
}

// ── Provider ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Gitlab,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Provider {
    type Err = crate::error::DevTodoError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github" => Ok(Self::Github),
            "gitlab" => Ok(Self::Gitlab),
            other => Err(crate::error::DevTodoError::Config(format!(
                "Unknown provider: {other}"
            ))),
        }
    }
}

// ── Task ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: Option<Priority>,
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub provider: Option<Provider>,
    pub remote_id: Option<i64>,
    pub source_url: Option<String>,
    pub assignee: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ── Label ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

// ── Reviewer ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reviewer {
    pub id: i64,
    pub task_id: i64,
    pub username: String,
    pub status: ReviewStatus,
    pub reviewed_at: Option<NaiveDateTime>,
}

// ── Comment ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: i64,
    pub task_id: i64,
    pub author: String,
    pub body: String,
    pub remote_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_task_status() {
        assert_eq!("draft".parse::<TaskStatus>().unwrap(), TaskStatus::Draft);
        assert_eq!("OPEN".parse::<TaskStatus>().unwrap(), TaskStatus::Open);
        assert_eq!("Review".parse::<TaskStatus>().unwrap(), TaskStatus::Review);
        assert_eq!("merged".parse::<TaskStatus>().unwrap(), TaskStatus::Merged);
        assert_eq!("closed".parse::<TaskStatus>().unwrap(), TaskStatus::Closed);
    }

    #[test]
    fn parse_task_status_invalid() {
        assert!("invalid".parse::<TaskStatus>().is_err());
        assert!("".parse::<TaskStatus>().is_err());
    }

    #[test]
    fn task_status_display() {
        assert_eq!(TaskStatus::Draft.to_string(), "draft");
        assert_eq!(TaskStatus::Merged.to_string(), "merged");
    }

    #[test]
    fn parse_priority() {
        assert_eq!("low".parse::<Priority>().unwrap(), Priority::Low);
        assert_eq!("CRITICAL".parse::<Priority>().unwrap(), Priority::Critical);
        assert_eq!("Medium".parse::<Priority>().unwrap(), Priority::Medium);
        assert_eq!("high".parse::<Priority>().unwrap(), Priority::High);
    }

    #[test]
    fn parse_priority_invalid() {
        assert!("urgent".parse::<Priority>().is_err());
    }

    #[test]
    fn parse_review_status() {
        assert_eq!(
            "approved".parse::<ReviewStatus>().unwrap(),
            ReviewStatus::Approved
        );
        assert_eq!(
            "changes_requested".parse::<ReviewStatus>().unwrap(),
            ReviewStatus::ChangesRequested
        );
        assert_eq!(
            "pending".parse::<ReviewStatus>().unwrap(),
            ReviewStatus::Pending
        );
    }

    #[test]
    fn parse_provider() {
        assert_eq!("github".parse::<Provider>().unwrap(), Provider::Github);
        assert_eq!("GITLAB".parse::<Provider>().unwrap(), Provider::Gitlab);
        assert!("bitbucket".parse::<Provider>().is_err());
    }

    #[test]
    fn task_status_roundtrip() {
        for status in [
            TaskStatus::Draft,
            TaskStatus::Open,
            TaskStatus::Review,
            TaskStatus::Merged,
            TaskStatus::Closed,
        ] {
            let s = status.as_str();
            let parsed: TaskStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn priority_roundtrip() {
        for prio in [
            Priority::Low,
            Priority::Medium,
            Priority::High,
            Priority::Critical,
        ] {
            let s = prio.as_str();
            let parsed: Priority = s.parse().unwrap();
            assert_eq!(parsed, prio);
        }
    }

    #[test]
    fn task_status_serde_roundtrip() {
        let status = TaskStatus::Review;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"review\"");
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn priority_serde_roundtrip() {
        let prio = Priority::Critical;
        let json = serde_json::to_string(&prio).unwrap();
        assert_eq!(json, "\"critical\"");
        let parsed: Priority = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, prio);
    }
}
