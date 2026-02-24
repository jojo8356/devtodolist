use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;

use super::{CreatePrRequest, ProviderApi, RemoteComment, RemotePr, RemoteReviewer};
use crate::error::{DevTodoError, Result};
use crate::models::Provider;

pub struct GitlabProvider {
    client: Client,
    token: String,
    base_url: String,
}

impl GitlabProvider {
    pub fn new(token: &str, base_url: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn build_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header(USER_AGENT, "devtodo-cli")
    }

    fn build_post(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header(USER_AGENT, "devtodo-cli")
    }

    fn build_put(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .put(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header(USER_AGENT, "devtodo-cli")
    }

    /// Encode project path for GitLab API (owner/repo -> owner%2Frepo).
    fn encode_project(&self, repo: &str) -> String {
        repo.replace('/', "%2F")
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/api/v4{}", self.base_url, path)
    }
}

// ── GitLab API response types ──

#[derive(Deserialize)]
struct GlMr {
    iid: i64,
    title: String,
    description: Option<String>,
    state: String,
    work_in_progress: Option<bool>,
    draft: Option<bool>,
    web_url: String,
    source_branch: String,
    target_branch: String,
    author: GlUser,
    labels: Vec<String>,
    merged_at: Option<String>,
}

#[derive(Deserialize)]
struct GlUser {
    username: String,
}

#[derive(Deserialize)]
struct GlApproval {
    approved_by: Vec<GlApprover>,
}

#[derive(Deserialize)]
struct GlApprover {
    user: GlUser,
}

#[derive(Deserialize)]
struct GlNote {
    id: i64,
    author: GlUser,
    body: String,
    created_at: String,
    system: bool,
}

impl GlMr {
    fn to_status_string(&self) -> String {
        if self.state == "merged" || self.merged_at.is_some() {
            "merged".to_string()
        } else if self.state == "closed" {
            "closed".to_string()
        } else if self.draft.unwrap_or(false) || self.work_in_progress.unwrap_or(false) {
            "draft".to_string()
        } else {
            "open".to_string()
        }
    }
}

#[async_trait]
impl ProviderApi for GitlabProvider {
    fn provider_type(&self) -> Provider {
        Provider::Gitlab
    }

    async fn list_prs(&self, repo: &str, state: &str) -> Result<Vec<RemotePr>> {
        let project = self.encode_project(repo);

        let gl_state = match state {
            "merged" => "merged",
            "closed" => "closed",
            "all" => "all",
            _ => "opened",
        };

        let url = self.api_url(&format!(
            "/projects/{project}/merge_requests?state={gl_state}&per_page=100"
        ));

        let resp = self
            .build_get(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;

        let mrs: Vec<GlMr> = resp.json().await?;

        Ok(mrs
            .into_iter()
            .map(|mr| {
                let status = mr.to_status_string();
                RemotePr {
                    remote_id: mr.iid,
                    title: mr.title,
                    description: mr.description,
                    status,
                    branch: Some(mr.source_branch),
                    base_branch: Some(mr.target_branch),
                    source_url: mr.web_url,
                    author: Some(mr.author.username),
                    labels: mr.labels,
                    reviewers: vec![],
                    comments: vec![],
                }
            })
            .collect())
    }

    async fn get_pr(&self, repo: &str, mr_iid: i64) -> Result<RemotePr> {
        let project = self.encode_project(repo);

        // Fetch MR
        let url = self.api_url(&format!("/projects/{project}/merge_requests/{mr_iid}"));
        let resp = self
            .build_get(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;
        let mr: GlMr = resp.json().await?;

        // Fetch approvals
        let approvals_url = self.api_url(&format!(
            "/projects/{project}/merge_requests/{mr_iid}/approvals"
        ));
        let approvals: Vec<RemoteReviewer> =
            if let Ok(resp) = self.build_get(&approvals_url).send().await {
                if let Ok(approval) = resp.json::<GlApproval>().await {
                    approval
                        .approved_by
                        .into_iter()
                        .map(|a| RemoteReviewer {
                            username: a.user.username,
                            status: "approved".to_string(),
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

        // Fetch notes (comments), filter out system notes
        let notes_url = self.api_url(&format!(
            "/projects/{project}/merge_requests/{mr_iid}/notes?per_page=100"
        ));
        let comments: Vec<RemoteComment> = if let Ok(resp) = self.build_get(&notes_url).send().await
        {
            if let Ok(notes) = resp.json::<Vec<GlNote>>().await {
                notes
                    .into_iter()
                    .filter(|n| !n.system)
                    .map(|n| RemoteComment {
                        remote_id: n.id,
                        author: n.author.username,
                        body: n.body,
                        created_at: n.created_at,
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let status = mr.to_status_string();
        Ok(RemotePr {
            remote_id: mr.iid,
            title: mr.title,
            description: mr.description,
            status,
            branch: Some(mr.source_branch),
            base_branch: Some(mr.target_branch),
            source_url: mr.web_url,
            author: Some(mr.author.username),
            labels: mr.labels,
            reviewers: approvals,
            comments,
        })
    }

    async fn create_pr(&self, repo: &str, req: &CreatePrRequest) -> Result<RemotePr> {
        let project = self.encode_project(repo);
        let url = self.api_url(&format!("/projects/{project}/merge_requests"));

        let mut title = req.title.clone();
        if req.draft {
            title = format!("Draft: {title}");
        }

        let mut body = serde_json::json!({
            "title": title,
            "description": req.description,
            "source_branch": req.branch,
            "target_branch": req.base_branch,
        });

        if !req.labels.is_empty() {
            body["labels"] = serde_json::Value::String(req.labels.join(","));
        }

        if !req.reviewers.is_empty() {
            // GitLab requires user IDs for reviewers, not usernames.
            // For simplicity we set them as assignees instead.
            // A full implementation would resolve usernames to IDs first.
        }

        let resp = self
            .build_post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;

        let mr: GlMr = resp.json().await?;

        let status = mr.to_status_string();
        Ok(RemotePr {
            remote_id: mr.iid,
            title: mr.title,
            description: mr.description,
            status,
            branch: Some(mr.source_branch),
            base_branch: Some(mr.target_branch),
            source_url: mr.web_url,
            author: Some(mr.author.username),
            labels: mr.labels,
            reviewers: vec![],
            comments: vec![],
        })
    }

    async fn update_pr_status(&self, repo: &str, mr_iid: i64, status: &str) -> Result<()> {
        let project = self.encode_project(repo);

        match status {
            "merged" => {
                let url = self.api_url(&format!(
                    "/projects/{project}/merge_requests/{mr_iid}/merge"
                ));
                self.build_put(&url)
                    .send()
                    .await?
                    .error_for_status()
                    .map_err(DevTodoError::Api)?;
            }
            "closed" => {
                let url = self.api_url(&format!("/projects/{project}/merge_requests/{mr_iid}"));
                self.build_put(&url)
                    .json(&serde_json::json!({"state_event": "close"}))
                    .send()
                    .await?
                    .error_for_status()
                    .map_err(DevTodoError::Api)?;
            }
            "open" | "review" => {
                let url = self.api_url(&format!("/projects/{project}/merge_requests/{mr_iid}"));
                self.build_put(&url)
                    .json(&serde_json::json!({"state_event": "reopen"}))
                    .send()
                    .await?
                    .error_for_status()
                    .map_err(DevTodoError::Api)?;
            }
            _ => return Err(DevTodoError::InvalidStatus(status.to_string())),
        }

        Ok(())
    }
}
