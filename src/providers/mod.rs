pub mod github;
pub mod gitlab;

use async_trait::async_trait;

use crate::error::Result;
use crate::models::Provider;

/// A remote PR as returned by a provider API.
#[derive(Debug, Clone)]
pub struct RemotePr {
    pub remote_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String,       // provider-native status string
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub source_url: String,
    pub author: Option<String>,
    pub labels: Vec<String>,
    pub reviewers: Vec<RemoteReviewer>,
    pub comments: Vec<RemoteComment>,
}

#[derive(Debug, Clone)]
pub struct RemoteReviewer {
    pub username: String,
    pub status: String, // "pending" | "approved" | "changes_requested"
}

#[derive(Debug, Clone)]
pub struct RemoteComment {
    pub remote_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

/// Data needed to create a PR on a remote.
#[derive(Debug)]
pub struct CreatePrRequest {
    pub title: String,
    pub description: Option<String>,
    pub branch: String,
    pub base_branch: String,
    pub draft: bool,
    pub labels: Vec<String>,
    pub reviewers: Vec<String>,
}

/// Trait that all providers must implement.
#[async_trait]
pub trait ProviderApi: Send + Sync {
    fn provider_type(&self) -> Provider;

    async fn list_prs(&self, repo: &str, state: &str) -> Result<Vec<RemotePr>>;

    async fn get_pr(&self, repo: &str, pr_number: i64) -> Result<RemotePr>;

    async fn create_pr(&self, repo: &str, req: &CreatePrRequest) -> Result<RemotePr>;

    async fn update_pr_status(&self, repo: &str, pr_number: i64, status: &str) -> Result<()>;
}

/// Build a provider from its name and token.
pub fn build_provider(
    provider: &Provider,
    token: &str,
    base_url: Option<&str>,
) -> Box<dyn ProviderApi> {
    match provider {
        Provider::Github => Box::new(github::GithubProvider::new(token)),
        Provider::Gitlab => Box::new(gitlab::GitlabProvider::new(
            token,
            base_url.unwrap_or("https://gitlab.com"),
        )),
    }
}
