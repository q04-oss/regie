use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regie_shared::types::{CommitSummary, DeferredItem, Repo, ScorecardEntry};
use serde::Serialize;
use sqlx::PgPool;

use crate::db;
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
    pool: PgPool,
}

impl IngestionService {
    pub fn new(github: GitHubClient, pool: PgPool) -> Self {
        Self { github, pool }
    }

    /// Pull SCORECARD.md / HARDENING.md / CLAUDE.md plus the last 10 commits
    /// from `repo` (e.g. `"q04-oss/box-fraise-platform"`), parse them, persist
    /// to Postgres, and return the freshly-ingested bundle.
    ///
    /// Persistence runs in a single transaction so a failure mid-write leaves
    /// the previous snapshot intact. Commit-summary upserts use `DO NOTHING`
    /// so re-running ingestion never overwrites an existing semantic summary
    /// produced by Claude.
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

        let ingested_at = Utc::now();
        let repo_row = build_repo_row(repo, &scorecard_entries, &recent_commits, ingested_at);
        let commit_rows: Vec<CommitSummary> = recent_commits
            .iter()
            .map(|c| commit_to_summary(repo, c))
            .collect();

        // One transaction so failed mid-write leaves the previous snapshot intact.
        let mut tx = self.pool.begin().await.context("begin transaction")?;
        db::upsert_repo(&mut tx, &repo_row).await?;
        if !scorecard_entries.is_empty() {
            db::upsert_scorecard_entries(&mut tx, &scorecard_entries).await?;
        }
        if !deferred_items.is_empty() {
            db::upsert_deferred_items(&mut tx, &deferred_items).await?;
        }
        if !commit_rows.is_empty() {
            db::upsert_commit_summaries(&mut tx, &commit_rows).await?;
        }
        tx.commit().await.context("commit transaction")?;

        Ok(IngestedRepo {
            repo_id: repo.to_owned(),
            scorecard_entries,
            deferred_items,
            recent_commits,
            claude_md,
            ingested_at,
        })
    }
}

fn build_repo_row(
    repo: &str,
    scorecard: &[ScorecardEntry],
    commits: &[GitHubCommit],
    ingested_at: DateTime<Utc>,
) -> Repo {
    // Latest = last entry after parser's ascending sort.
    let latest_grade = scorecard.last().map(|e| e.grade.clone());
    let last_commit = commits.first();
    Repo {
        id: repo.to_owned(),
        name: repo
            .rsplit_once('/')
            .map(|(_, n)| n.to_owned())
            .unwrap_or_else(|| repo.to_owned()),
        grade: latest_grade,
        // test_count would have to come from a separate parser; the scorecard
        // doesn't carry it as a structured field. Left None until a future pass.
        test_count: None,
        last_commit_sha: last_commit.map(|c| c.sha.clone()),
        last_commit_at: last_commit.map(|c| c.committed_at),
        last_ingested_at: Some(ingested_at),
    }
}

fn commit_to_summary(repo: &str, c: &GitHubCommit) -> CommitSummary {
    CommitSummary {
        repo_id: repo.to_owned(),
        sha: c.sha.clone(),
        message: c.message.clone(),
        author: c.author.clone(),
        committed_at: c.committed_at,
        semantic_summary: None,
        files_changed: c.files_changed,
    }
}
