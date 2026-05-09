//! Repository layer — every SQL statement Régie issues lives here.
//! Functions take `&mut PgConnection` so callers can choose between a pool
//! connection and a transaction; nothing here opens its own.

use regie_shared::types::{
    CommitSummary, DeferredItem, Recommendation, Repo, ScorecardEntry, Task,
};
use sqlx::PgConnection;

// ── repos ────────────────────────────────────────────────────────────────────

pub async fn upsert_repo(
    conn: &mut PgConnection,
    repo: &Repo,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO repos \
         (id, name, grade, test_count, last_commit_sha, last_commit_at, last_ingested_at, claude_md, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now()) \
         ON CONFLICT (id) DO UPDATE SET \
             name             = EXCLUDED.name, \
             grade            = EXCLUDED.grade, \
             test_count       = EXCLUDED.test_count, \
             last_commit_sha  = EXCLUDED.last_commit_sha, \
             last_commit_at   = EXCLUDED.last_commit_at, \
             last_ingested_at = EXCLUDED.last_ingested_at, \
             claude_md        = EXCLUDED.claude_md, \
             updated_at       = now()",
    )
    .bind(&repo.id)
    .bind(&repo.name)
    .bind(&repo.grade)
    .bind(repo.test_count)
    .bind(&repo.last_commit_sha)
    .bind(repo.last_commit_at)
    .bind(repo.last_ingested_at)
    .bind(&repo.claude_md)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn get_repo(
    conn: &mut PgConnection,
    repo_id: &str,
) -> Result<Option<Repo>, sqlx::Error> {
    sqlx::query_as::<_, Repo>(
        "SELECT id, name, grade, test_count, last_commit_sha, last_commit_at, last_ingested_at, claude_md \
         FROM repos WHERE id = $1",
    )
    .bind(repo_id)
    .fetch_optional(conn)
    .await
}

pub async fn list_repos(conn: &mut PgConnection) -> Result<Vec<Repo>, sqlx::Error> {
    sqlx::query_as::<_, Repo>(
        "SELECT id, name, grade, test_count, last_commit_sha, last_commit_at, last_ingested_at, claude_md \
         FROM repos ORDER BY id",
    )
    .fetch_all(conn)
    .await
}

// ── scorecard_entries ────────────────────────────────────────────────────────

pub async fn upsert_scorecard_entries(
    conn: &mut PgConnection,
    entries: &[ScorecardEntry],
) -> Result<(), sqlx::Error> {
    for e in entries {
        sqlx::query(
            "INSERT INTO scorecard_entries \
             (repo_id, entry_date, grade, weighted_score, security, architecture, \
              engineer_usability, protocol_conformance, operational_readiness, product_completeness) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT (repo_id, entry_date) DO UPDATE SET \
                 grade                 = EXCLUDED.grade, \
                 weighted_score        = EXCLUDED.weighted_score, \
                 security              = EXCLUDED.security, \
                 architecture          = EXCLUDED.architecture, \
                 engineer_usability    = EXCLUDED.engineer_usability, \
                 protocol_conformance  = EXCLUDED.protocol_conformance, \
                 operational_readiness = EXCLUDED.operational_readiness, \
                 product_completeness  = EXCLUDED.product_completeness",
        )
        .bind(&e.repo_id)
        .bind(e.date)
        .bind(&e.grade)
        .bind(e.weighted_score)
        .bind(e.security)
        .bind(e.architecture)
        .bind(e.engineer_usability)
        .bind(e.protocol_conformance)
        .bind(e.operational_readiness)
        .bind(e.product_completeness)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn get_scorecard_history(
    conn: &mut PgConnection,
    repo_id: &str,
) -> Result<Vec<ScorecardEntry>, sqlx::Error> {
    sqlx::query_as::<_, ScorecardEntry>(
        "SELECT repo_id, entry_date AS date, grade, weighted_score, security, \
                architecture, engineer_usability, protocol_conformance, \
                operational_readiness, product_completeness \
         FROM scorecard_entries WHERE repo_id = $1 ORDER BY entry_date ASC",
    )
    .bind(repo_id)
    .fetch_all(conn)
    .await
}

// ── deferred_items ───────────────────────────────────────────────────────────

pub async fn upsert_deferred_items(
    conn: &mut PgConnection,
    items: &[DeferredItem],
) -> Result<(), sqlx::Error> {
    for item in items {
        sqlx::query(
            "INSERT INTO deferred_items \
             (id, repo_id, description, file_ref, section, priority, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, now()) \
             ON CONFLICT (repo_id, id) DO UPDATE SET \
                 description = EXCLUDED.description, \
                 file_ref    = EXCLUDED.file_ref, \
                 section     = EXCLUDED.section, \
                 priority    = EXCLUDED.priority, \
                 updated_at  = now()",
        )
        .bind(&item.id)
        .bind(&item.repo_id)
        .bind(&item.description)
        .bind(&item.file_ref)
        .bind(&item.section)
        .bind(item.priority)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn get_deferred_items(
    conn: &mut PgConnection,
    repo_id: &str,
) -> Result<Vec<DeferredItem>, sqlx::Error> {
    // ORDER BY priority alphabetises high < low < medium — fine for v1; the
    // dashboard re-buckets by enum on the client, so we don't pay for a CASE.
    sqlx::query_as::<_, DeferredItem>(
        "SELECT repo_id, id, description, file_ref, section, priority \
         FROM deferred_items WHERE repo_id = $1 AND resolved = false \
         ORDER BY priority, id",
    )
    .bind(repo_id)
    .fetch_all(conn)
    .await
}

// ── commit_summaries ─────────────────────────────────────────────────────────

pub async fn upsert_commit_summaries(
    conn: &mut PgConnection,
    commits: &[CommitSummary],
) -> Result<(), sqlx::Error> {
    // COALESCE merge on conflict: the existing `semantic_summary` wins if
    // it's already populated (don't overwrite Claude's work on re-ingest);
    // an existing NULL gets filled in by the new value if the caller has
    // one. Other columns are immutable in git, so we don't update them.
    for c in commits {
        sqlx::query(
            "INSERT INTO commit_summaries \
             (sha, repo_id, message, author, committed_at, semantic_summary, files_changed) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (repo_id, sha) DO UPDATE SET \
                 semantic_summary = COALESCE(commit_summaries.semantic_summary, EXCLUDED.semantic_summary)",
        )
        .bind(&c.sha)
        .bind(&c.repo_id)
        .bind(&c.message)
        .bind(&c.author)
        .bind(c.committed_at)
        .bind(&c.semantic_summary)
        .bind(c.files_changed)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn get_recent_commits(
    conn: &mut PgConnection,
    repo_id: &str,
    limit: i64,
) -> Result<Vec<CommitSummary>, sqlx::Error> {
    sqlx::query_as::<_, CommitSummary>(
        "SELECT repo_id, sha, message, author, committed_at, semantic_summary, files_changed \
         FROM commit_summaries WHERE repo_id = $1 \
         ORDER BY committed_at DESC LIMIT $2",
    )
    .bind(repo_id)
    .bind(limit)
    .fetch_all(conn)
    .await
}

// ── tasks ────────────────────────────────────────────────────────────────────

pub async fn insert_task(
    conn: &mut PgConnection,
    task: &Task,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks (id, repo_id, title, prompt, status, created_at, completed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(task.id)
    .bind(&task.repo_id)
    .bind(&task.title)
    .bind(&task.prompt)
    .bind(task.status)
    .bind(task.created_at)
    .bind(task.completed_at)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn list_tasks(
    conn: &mut PgConnection,
    repo_id: &str,
) -> Result<Vec<Task>, sqlx::Error> {
    sqlx::query_as::<_, Task>(
        "SELECT id, repo_id, title, prompt, status, created_at, completed_at \
         FROM tasks WHERE repo_id = $1 ORDER BY created_at DESC",
    )
    .bind(repo_id)
    .fetch_all(conn)
    .await
}

// ── recommendations ──────────────────────────────────────────────────────────

pub async fn insert_recommendation(
    conn: &mut PgConnection,
    rec: &Recommendation,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO recommendations \
         (repo_id, generated_at, top_action, justification, estimated_impact, related_deferred_items) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&rec.repo_id)
    .bind(rec.generated_at)
    .bind(&rec.top_action)
    .bind(&rec.justification)
    .bind(&rec.estimated_impact)
    .bind(&rec.related_deferred_items)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn get_latest_recommendation(
    conn: &mut PgConnection,
    repo_id: &str,
) -> Result<Option<Recommendation>, sqlx::Error> {
    sqlx::query_as::<_, Recommendation>(
        "SELECT repo_id, generated_at, top_action, justification, estimated_impact, related_deferred_items \
         FROM recommendations WHERE repo_id = $1 \
         ORDER BY generated_at DESC LIMIT 1",
    )
    .bind(repo_id)
    .fetch_optional(conn)
    .await
}
