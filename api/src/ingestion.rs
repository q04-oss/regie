use anyhow::Result;
use chrono::{DateTime, Utc};
use regie_shared::types::{DeferredItem, ScorecardEntry};
use serde::Serialize;

use crate::github::{GitHubClient, GitHubCommit};
use crate::parsers;

#[derive(Debug, Serialize)]
pub struct IngestedRepo {
    pub repo_id: String,
    pub scorecard_entries: Vec<ScorecardEntry>,
    pub deferred_items: Vec<DeferredItem>,
    pub recent_commits: Vec<GitHubCommit>,
    pub claude_md: Option<String>,
    pub ingested_at: DateTime<Utc>,
}

pub struct IngestionService {
    github: GitHubClient,
}

impl IngestionService {
    pub fn new(github: GitHubClient) -> Self {
        Self { github }
    }

    /// Pull SCORECARD.md / HARDENING.md / CLAUDE.md plus the last 10 commits
    /// from `repo` (e.g. `"q04-oss/box-fraise-platform"`) and return the
    /// parsed bundle. Each fetch is independent — a missing file is `None`,
    /// not an error, so a repo that has only some of these markdown files
    /// still returns a useful response.
    pub async fn ingest_repo(&self, repo: &str) -> Result<IngestedRepo> {
        let scorecard_md = self.github.get_file_content(repo, "SCORECARD.md").await?;
        let hardening_md = self.github.get_file_content(repo, "HARDENING.md").await?;
        let claude_md = self.github.get_file_content(repo, "CLAUDE.md").await?;
        let recent_commits = self.github.get_recent_commits(repo, 10).await?;

        let scorecard_entries = scorecard_md
            .as_deref()
            .map(|s| parsers::scorecard::parse(repo, s))
            .unwrap_or_default();
        let deferred_items = hardening_md
            .as_deref()
            .map(|s| parsers::hardening::parse(repo, s))
            .unwrap_or_default();

        Ok(IngestedRepo {
            repo_id: repo.to_owned(),
            scorecard_entries,
            deferred_items,
            recent_commits,
            claude_md,
            ingested_at: Utc::now(),
        })
    }
}
