use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;

use super::{CreatePrRequest, ProviderApi, RemoteComment, RemotePr, RemoteReviewer};
use crate::error::{DevTodoError, Result};
use crate::models::Provider;

pub struct GithubProvider {
    client: Client,
    token: String,
}

impl GithubProvider {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
        }
    }

    fn headers(&self) -> Vec<(reqwest::header::HeaderName, String)> {
        vec![
            (AUTHORIZATION, format!("Bearer {}", self.token)),
            (ACCEPT, "application/vnd.github+json".to_string()),
            (USER_AGENT, "devtodo-cli".to_string()),
        ]
    }

    fn build_request(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.get(url);
        for (k, v) in self.headers() {
            req = req.header(k, v);
        }
        req
    }
}

// ── GitHub API response types ──

#[derive(Deserialize)]
struct GhPr {
    number: i64,
    title: String,
    body: Option<String>,
    state: String,
    draft: Option<bool>,
    merged: Option<bool>,
    html_url: String,
    head: GhRef,
    base: GhRef,
    user: GhUser,
    labels: Vec<GhLabel>,
    requested_reviewers: Vec<GhUser>,
}

#[derive(Deserialize)]
struct GhRef {
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
}

#[derive(Deserialize)]
struct GhReview {
    user: GhUser,
    state: String,
}

#[derive(Deserialize)]
struct GhComment {
    id: i64,
    user: GhUser,
    body: String,
    created_at: String,
}

impl GhPr {
    fn to_status_string(&self) -> String {
        if self.merged.unwrap_or(false) {
            "merged".to_string()
        } else if self.state == "closed" {
            "closed".to_string()
        } else if self.draft.unwrap_or(false) {
            "draft".to_string()
        } else if !self.requested_reviewers.is_empty() {
            "review".to_string()
        } else {
            "open".to_string()
        }
    }
}

fn map_gh_review_state(state: &str) -> String {
    match state {
        "APPROVED" => "approved".to_string(),
        "CHANGES_REQUESTED" => "changes_requested".to_string(),
        _ => "pending".to_string(),
    }
}

#[async_trait]
impl ProviderApi for GithubProvider {
    fn provider_type(&self) -> Provider {
        Provider::Github
    }

    async fn list_prs(&self, repo: &str, state: &str) -> Result<Vec<RemotePr>> {
        let gh_state = match state {
            "merged" | "closed" => "closed",
            "all" => "all",
            _ => "open",
        };

        let url =
            format!("https://api.github.com/repos/{repo}/pulls?state={gh_state}&per_page=100");

        let resp = self
            .build_request(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;

        let prs: Vec<GhPr> = resp.json().await?;

        Ok(prs
            .into_iter()
            .map(|pr| {
                let status = pr.to_status_string();
                RemotePr {
                    remote_id: pr.number,
                    title: pr.title,
                    description: pr.body,
                    status,
                    branch: Some(pr.head.ref_name),
                    base_branch: Some(pr.base.ref_name),
                    source_url: pr.html_url,
                    author: Some(pr.user.login),
                    labels: pr.labels.into_iter().map(|l| l.name).collect(),
                    reviewers: pr
                        .requested_reviewers
                        .into_iter()
                        .map(|u| RemoteReviewer {
                            username: u.login,
                            status: "pending".to_string(),
                        })
                        .collect(),
                    comments: vec![],
                }
            })
            .collect())
    }

    async fn get_pr(&self, repo: &str, pr_number: i64) -> Result<RemotePr> {
        // Fetch PR
        let url = format!("https://api.github.com/repos/{repo}/pulls/{pr_number}");
        let resp = self
            .build_request(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;
        let pr: GhPr = resp.json().await?;

        // Fetch reviews
        let reviews_url = format!("https://api.github.com/repos/{repo}/pulls/{pr_number}/reviews");
        let reviews_resp = self
            .build_request(&reviews_url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;
        let reviews: Vec<GhReview> = reviews_resp.json().await?;

        // Fetch comments
        let comments_url =
            format!("https://api.github.com/repos/{repo}/issues/{pr_number}/comments");
        let comments_resp = self
            .build_request(&comments_url)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;
        let comments: Vec<GhComment> = comments_resp.json().await?;

        let status = pr.to_status_string();
        Ok(RemotePr {
            remote_id: pr.number,
            title: pr.title,
            description: pr.body,
            status,
            branch: Some(pr.head.ref_name),
            base_branch: Some(pr.base.ref_name),
            source_url: pr.html_url,
            author: Some(pr.user.login),
            labels: pr.labels.into_iter().map(|l| l.name).collect(),
            reviewers: reviews
                .into_iter()
                .map(|r| RemoteReviewer {
                    username: r.user.login,
                    status: map_gh_review_state(&r.state),
                })
                .collect(),
            comments: comments
                .into_iter()
                .map(|c| RemoteComment {
                    remote_id: c.id,
                    author: c.user.login,
                    body: c.body,
                    created_at: c.created_at,
                })
                .collect(),
        })
    }

    async fn create_pr(&self, repo: &str, req: &CreatePrRequest) -> Result<RemotePr> {
        let url = format!("https://api.github.com/repos/{repo}/pulls");

        let body = serde_json::json!({
            "title": req.title,
            "body": req.description,
            "head": req.branch,
            "base": req.base_branch,
            "draft": req.draft,
        });

        let mut http_req = self.client.post(&url);
        for (k, v) in self.headers() {
            http_req = http_req.header(k, v);
        }

        let resp = http_req
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;

        let pr: GhPr = resp.json().await?;

        // Assign labels if any
        if !req.labels.is_empty() {
            let labels_url = format!(
                "https://api.github.com/repos/{repo}/issues/{}/labels",
                pr.number
            );
            let mut label_req = self.client.post(&labels_url);
            for (k, v) in self.headers() {
                label_req = label_req.header(k, v);
            }
            let _ = label_req
                .json(&serde_json::json!({"labels": req.labels}))
                .send()
                .await;
        }

        // Request reviewers if any
        if !req.reviewers.is_empty() {
            let rev_url = format!(
                "https://api.github.com/repos/{repo}/pulls/{}/requested_reviewers",
                pr.number
            );
            let mut rev_req = self.client.post(&rev_url);
            for (k, v) in self.headers() {
                rev_req = rev_req.header(k, v);
            }
            let _ = rev_req
                .json(&serde_json::json!({"reviewers": req.reviewers}))
                .send()
                .await;
        }

        let status = pr.to_status_string();
        Ok(RemotePr {
            remote_id: pr.number,
            title: pr.title,
            description: pr.body,
            status,
            branch: Some(pr.head.ref_name),
            base_branch: Some(pr.base.ref_name),
            source_url: pr.html_url,
            author: Some(pr.user.login),
            labels: pr.labels.into_iter().map(|l| l.name).collect(),
            reviewers: vec![],
            comments: vec![],
        })
    }

    async fn update_pr_status(&self, repo: &str, pr_number: i64, status: &str) -> Result<()> {
        let url = format!("https://api.github.com/repos/{repo}/pulls/{pr_number}");

        let body = match status {
            "closed" => serde_json::json!({"state": "closed"}),
            "open" | "draft" | "review" => serde_json::json!({"state": "open"}),
            "merged" => {
                let merge_url =
                    format!("https://api.github.com/repos/{repo}/pulls/{pr_number}/merge");
                let mut req = self.client.put(&merge_url);
                for (k, v) in self.headers() {
                    req = req.header(k, v);
                }
                req.send()
                    .await?
                    .error_for_status()
                    .map_err(DevTodoError::Api)?;
                return Ok(());
            }
            _ => return Err(DevTodoError::InvalidStatus(status.to_string())),
        };

        let mut req = self.client.patch(&url);
        for (k, v) in self.headers() {
            req = req.header(k, v);
        }
        req.json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(DevTodoError::Api)?;

        Ok(())
    }
}
