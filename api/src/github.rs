use anyhow::{anyhow, Context, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const BASE: &str = "https://api.github.com";

pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub committed_at: DateTime<Utc>,
    pub files_changed: Option<i32>,
}

#[derive(Deserialize)]
struct ContentsResponse {
    content: String,
    encoding: String,
}

#[derive(Deserialize)]
struct CommitListItem {
    sha: String,
    commit: CommitInner,
}

#[derive(Deserialize)]
struct CommitInner {
    message: String,
    author: CommitAuthor,
}

#[derive(Deserialize)]
struct CommitAuthor {
    name: String,
    date: DateTime<Utc>,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("regie/0.1")
            .build()
            .expect("reqwest client is infallible");
        Self { client, token }
    }

    /// Fetch the raw content of a file from a repo. Returns `Ok(None)` on 404
    /// so callers can distinguish "file not present" from "GitHub failed".
    pub async fn get_file_content(
        &self,
        repo: &str,
        path: &str,
    ) -> Result<Option<String>> {
        let url = format!("{BASE}/repos/{repo}/contents/{path}");
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("GitHub request failed")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow!(
                "GitHub returned {} for {}",
                resp.status(),
                url
            ));
        }

        let body: ContentsResponse =
            resp.json().await.context("invalid contents JSON")?;
        if body.encoding != "base64" {
            return Err(anyhow!(
                "unexpected encoding {} for {}",
                body.encoding,
                path
            ));
        }
        // GitHub wraps base64 with newlines.
        let cleaned: String =
            body.content.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(cleaned.as_bytes())
            .context("base64 decode")?;
        let text = String::from_utf8(bytes).context("utf-8 decode")?;
        Ok(Some(text))
    }

    /// Fetch the last `count` commits on the repo's default branch.
    pub async fn get_recent_commits(
        &self,
        repo: &str,
        count: u32,
    ) -> Result<Vec<GitHubCommit>> {
        let url = format!("{BASE}/repos/{repo}/commits?per_page={count}");
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("GitHub request failed")?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "GitHub returned {} for {}",
                resp.status(),
                url
            ));
        }

        let body: Vec<CommitListItem> =
            resp.json().await.context("invalid commits JSON")?;

        Ok(body
            .into_iter()
            .map(|c| GitHubCommit {
                sha: c.sha,
                message: c.commit.message,
                author: c.commit.author.name,
                committed_at: c.commit.author.date,
                files_changed: None,
            })
            .collect())
    }
}
