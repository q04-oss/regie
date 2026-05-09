use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// A connected repository tracked by Régie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct Repo {
    pub id: String,           // e.g. "q04-oss/box-fraise-platform"
    pub name: String,
    pub grade: Option<String>,
    pub test_count: Option<i32>,
    pub last_commit_sha: Option<String>,
    pub last_commit_at: Option<DateTime<Utc>>,
    pub last_ingested_at: Option<DateTime<Utc>>,
}

/// A single scorecard entry parsed from SCORECARD.md.
///
/// The schema column is `entry_date`; the field stays `date` for ergonomic
/// access. Queries that read this struct must alias the column
/// (`SELECT entry_date AS date, ...`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct ScorecardEntry {
    pub repo_id: String,
    pub date: chrono::NaiveDate,
    pub grade: String,
    pub weighted_score: f64,
    pub security: f64,
    pub architecture: f64,
    pub engineer_usability: f64,
    pub protocol_conformance: f64,
    pub operational_readiness: f64,
    pub product_completeness: f64,
}

/// A deferred item parsed from HARDENING.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct DeferredItem {
    pub repo_id: String,
    pub id: String,
    pub description: String,
    pub file_ref: Option<String>,
    pub section: Option<String>,
    pub priority: DeferredItemPriority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::Type))]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "db", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
pub enum DeferredItemPriority {
    High,
    Medium,
    Low,
}

/// A commit with a Claude-generated semantic summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct CommitSummary {
    pub repo_id: String,
    pub sha: String,
    pub message: String,
    pub author: String,
    pub committed_at: DateTime<Utc>,
    pub semantic_summary: Option<String>,
    pub files_changed: Option<i32>,
}

/// A task created in Régie for Claude Code to pick up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct Task {
    pub id: uuid::Uuid,
    pub repo_id: String,
    pub title: String,
    pub prompt: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::Type))]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "db", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
pub enum TaskStatus {
    Pending,
    InProgress,
    Complete,
    Cancelled,
}

/// Claude's recommendation for what to work on next.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct Recommendation {
    pub repo_id: String,
    pub generated_at: DateTime<Utc>,
    pub top_action: String,
    pub justification: String,
    pub estimated_impact: String,
    pub related_deferred_items: Vec<String>,
}
