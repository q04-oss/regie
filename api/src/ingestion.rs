use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use regie_shared::types::{
    CommitSummary, DeferredItem, Recommendation, Repo, ScorecardEntry,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;

use crate::anthropic::AnthropicClient;
use crate::db;
use crate::github::GitHubClient;
use crate::parsers;

#[derive(Debug, Serialize)]
pub struct IngestedRepo {
    pub repo_id: String,
    pub scorecard_entries: Vec<ScorecardEntry>,
    pub deferred_items: Vec<DeferredItem>,
    pub recent_commits: Vec<CommitSummary>,
    pub claude_md: Option<String>,
    pub ingested_at: DateTime<Utc>,
    pub recommendation: Option<Recommendation>,
}

pub struct IngestionService {
    github: GitHubClient,
    pool: PgPool,
    anthropic: AnthropicClient,
    /// When `true`, all Anthropic calls are skipped; commits get no semantic
    /// summary and recommendations are not generated. Set automatically when
    /// `ANTHROPIC_API_KEY` is missing so dev/test runs don't need a key.
    pub skip_ai: bool,
}

impl IngestionService {
    pub fn new(
        github: GitHubClient,
        pool: PgPool,
        anthropic: AnthropicClient,
        skip_ai: bool,
    ) -> Self {
        Self {
            github,
            pool,
            anthropic,
            skip_ai,
        }
    }

    /// Pull markdown + commits from `repo`, parse, summarise commits via
    /// Claude (skipping commits already summarised), persist everything in a
    /// single transaction, then generate a "what next" recommendation. The
    /// recommendation is best-effort: if Claude is unavailable or returns
    /// invalid JSON, the rest of the ingestion still succeeds.
    pub async fn ingest_repo(&self, repo: &str) -> Result<IngestedRepo> {
        // ── 1. Fetch from GitHub ─────────────────────────────────────────
        let scorecard_md = self.github.get_file_content(repo, "SCORECARD.md").await?;
        let hardening_md = self.github.get_file_content(repo, "HARDENING.md").await?;
        let claude_md = self.github.get_file_content(repo, "CLAUDE.md").await?;
        let raw_commits = self.github.get_recent_commits(repo, 10).await?;

        // ── 2. Parse markdown ────────────────────────────────────────────
        let scorecard_entries = scorecard_md
            .as_deref()
            .map(|s| parsers::scorecard::parse(repo, s))
            .unwrap_or_default();
        let deferred_items = hardening_md
            .as_deref()
            .map(|s| parsers::hardening::parse(repo, s))
            .unwrap_or_default();

        // ── 3. Build initial CommitSummary list ──────────────────────────
        let mut commits: Vec<CommitSummary> = raw_commits
            .iter()
            .map(|c| CommitSummary {
                repo_id: repo.to_owned(),
                sha: c.sha.clone(),
                message: c.message.clone(),
                author: c.author.clone(),
                committed_at: c.committed_at,
                semantic_summary: None,
                files_changed: c.files_changed,
            })
            .collect();

        // ── 4. Carry over existing summaries from DB ─────────────────────
        let existing_summaries = self.load_existing_summaries(repo).await?;
        for c in commits.iter_mut() {
            if let Some(Some(s)) = existing_summaries.get(&c.sha) {
                c.semantic_summary = Some(s.clone());
            }
        }

        // ── 5. Summarise commits without an existing summary ─────────────
        if !self.skip_ai {
            let repo_short = short_name(repo);
            for c in commits.iter_mut() {
                if c.semantic_summary.is_some() {
                    continue;
                }
                match self
                    .anthropic
                    .summarise_commit(&c.message, &repo_short)
                    .await
                {
                    Ok(s) if !s.is_empty() => c.semantic_summary = Some(s),
                    Ok(_) => {}
                    Err(e) => tracing::warn!(
                        error = %e,
                        sha = %c.sha,
                        "summarise_commit failed — leaving summary null",
                    ),
                }
            }
        }

        // ── 6. Persist scorecard / deferred / repo / commits in one tx ──
        let ingested_at = Utc::now();
        let repo_row = build_repo_row(
            repo,
            &scorecard_entries,
            &commits,
            claude_md.as_deref(),
            ingested_at,
        );
        let mut tx = self.pool.begin().await.context("begin transaction")?;
        db::upsert_repo(&mut tx, &repo_row).await?;
        if !scorecard_entries.is_empty() {
            db::upsert_scorecard_entries(&mut tx, &scorecard_entries).await?;
        }
        if !deferred_items.is_empty() {
            db::upsert_deferred_items(&mut tx, &deferred_items).await?;
        }
        if !commits.is_empty() {
            db::upsert_commit_summaries(&mut tx, &commits).await?;
        }
        tx.commit().await.context("commit transaction")?;

        // ── 7. Generate + persist a recommendation (best-effort) ────────
        let recommendation = if self.skip_ai {
            None
        } else {
            match self
                .build_recommendation(
                    repo,
                    scorecard_entries.last().map(|e| e.grade.as_str()),
                    &deferred_items,
                    &commits,
                    claude_md.as_deref(),
                )
                .await
            {
                Ok(rec) => {
                    let mut conn = self.pool.acquire().await?;
                    if let Err(e) = db::insert_recommendation(&mut conn, &rec).await {
                        tracing::warn!(error = %e, "persist recommendation failed");
                    }
                    Some(rec)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "recommendation generation failed");
                    None
                }
            }
        };

        Ok(IngestedRepo {
            repo_id: repo.to_owned(),
            scorecard_entries,
            deferred_items,
            recent_commits: commits,
            claude_md,
            ingested_at,
            recommendation,
        })
    }

    /// Force-regenerate a recommendation from current persisted state. Used
    /// by `POST /api/repos/:repo_id/recommendation` and as a fallback by
    /// `GET` when no row exists yet.
    pub async fn regenerate_recommendation(
        &self,
        repo: &str,
    ) -> Result<Recommendation> {
        if self.skip_ai {
            return Err(anyhow!("AI is disabled (SKIP_AI=true)"));
        }

        let (grade, claude_md, deferred, commits) = {
            let mut conn = self.pool.acquire().await.context("acquire connection")?;
            let repo_row = db::get_repo(&mut conn, repo)
                .await?
                .ok_or_else(|| anyhow!("repo {repo} not tracked"))?;
            let deferred = db::get_deferred_items(&mut conn, repo).await?;
            let commits = db::get_recent_commits(&mut conn, repo, 5).await?;
            (repo_row.grade, repo_row.claude_md, deferred, commits)
        };

        let rec = self
            .build_recommendation(
                repo,
                grade.as_deref(),
                &deferred,
                &commits,
                claude_md.as_deref(),
            )
            .await?;

        let mut conn = self.pool.acquire().await?;
        db::insert_recommendation(&mut conn, &rec).await?;
        Ok(rec)
    }

    async fn build_recommendation(
        &self,
        repo: &str,
        grade: Option<&str>,
        deferred: &[DeferredItem],
        commits: &[CommitSummary],
        claude_md: Option<&str>,
    ) -> Result<Recommendation> {
        // Truncate CLAUDE.md to ~500 chars on a UTF-8 char boundary. We
        // collect through chars() to avoid splitting a multi-byte codepoint.
        let excerpt: Option<String> =
            claude_md.map(|s| s.chars().take(500).collect());
        let grade = grade.unwrap_or("ungraded");
        self.anthropic
            .recommend_next_action(repo, grade, deferred, commits, excerpt.as_deref())
            .await
    }

    async fn load_existing_summaries(
        &self,
        repo: &str,
    ) -> Result<HashMap<String, Option<String>>> {
        let mut conn = self.pool.acquire().await?;
        let rows = db::get_recent_commits(&mut conn, repo, 100).await?;
        Ok(rows
            .into_iter()
            .map(|c| (c.sha, c.semantic_summary))
            .collect())
    }
}

fn build_repo_row(
    repo: &str,
    scorecard: &[ScorecardEntry],
    commits: &[CommitSummary],
    claude_md: Option<&str>,
    ingested_at: DateTime<Utc>,
) -> Repo {
    let latest_grade = scorecard.last().map(|e| e.grade.clone());
    let last_commit = commits.first();
    Repo {
        id: repo.to_owned(),
        name: short_name(repo),
        grade: latest_grade,
        // test_count would require a separate parser pass; the scorecard
        // doesn't carry it as a structured field.
        test_count: None,
        last_commit_sha: last_commit.map(|c| c.sha.clone()),
        last_commit_at: last_commit.map(|c| c.committed_at),
        last_ingested_at: Some(ingested_at),
        claude_md: claude_md.map(|s| s.to_owned()),
    }
}

fn short_name(repo: &str) -> String {
    repo.rsplit_once('/')
        .map(|(_, n)| n.to_owned())
        .unwrap_or_else(|| repo.to_owned())
}
