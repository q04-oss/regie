use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use regie_shared::types::{
    CommitSummary, DeferredItem, Recommendation, Repo, ScorecardEntry, Task, TaskStatus,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub mod anthropic;
pub mod db;
pub mod github;
pub mod ingestion;
pub mod parsers;

use anthropic::AnthropicClient;
use github::GitHubClient;
use ingestion::{IngestedRepo, IngestionService};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub ingestion: Arc<IngestionService>,
}

pub async fn run() {
    dotenvy::dotenv().ok();
    let github_token =
        std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let mut skip_ai = std::env::var("SKIP_AI").unwrap_or_default() == "true";

    if anthropic_api_key.is_empty() {
        tracing::warn!(
            "ANTHROPIC_API_KEY not set — disabling AI features (commit summaries + recommendations)",
        );
        skip_ai = true;
    }

    let pool = PgPool::connect(&database_url)
        .await
        .expect("connect to DATABASE_URL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("apply migrations");

    let github = GitHubClient::new(github_token);
    let anthropic = AnthropicClient::new(anthropic_api_key);
    let state = AppState {
        pool: pool.clone(),
        ingestion: Arc::new(IngestionService::new(github, pool, anthropic, skip_ai)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/repos", get(list_repos).post(create_repo))
        .route("/api/repos/{repo_id}", get(get_repo))
        .route("/api/repos/{repo_id}/ingest", get(reingest_repo))
        .route("/api/repos/{repo_id}/scorecard", get(get_scorecard))
        .route("/api/repos/{repo_id}/deferred", get(get_deferred))
        .route("/api/repos/{repo_id}/commits", get(get_commits))
        .route("/api/repos/{repo_id}/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/repos/{repo_id}/recommendation",
            get(get_recommendation).post(force_recommendation),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    tracing::info!("Regie API listening on :3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// `:` is the path-safe separator used in URLs (the path segment can't contain
/// `/`); the canonical GitHub form is `owner/name`. Rewrite once, on entry.
fn normalize_repo_id(repo_id: &str) -> String {
    repo_id.replacen(':', "/", 1)
}

type ApiResult<T> = Result<T, (StatusCode, String)>;

fn db_err(e: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn upstream_err(e: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::BAD_GATEWAY, e.to_string())
}

// ── list / create ────────────────────────────────────────────────────────────

async fn list_repos(State(state): State<AppState>) -> ApiResult<Json<Vec<Repo>>> {
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let repos = db::list_repos(&mut conn).await.map_err(db_err)?;
    Ok(Json(repos))
}

#[derive(Deserialize)]
pub struct CreateRepoRequest {
    pub repo_id: String,
}

async fn create_repo(
    State(state): State<AppState>,
    Json(body): Json<CreateRepoRequest>,
) -> ApiResult<Json<IngestedRepo>> {
    let bundle = state
        .ingestion
        .ingest_repo(&body.repo_id)
        .await
        .map_err(upstream_err)?;
    Ok(Json(bundle))
}

// ── per-repo reads ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RepoDetail {
    pub repo: Repo,
    pub scorecard_entries: Vec<ScorecardEntry>,
    pub deferred_items: Vec<DeferredItem>,
}

async fn get_repo(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<RepoDetail>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let row = db::get_repo(&mut conn, &repo)
        .await
        .map_err(db_err)?
        .ok_or((StatusCode::NOT_FOUND, format!("repo {repo} not tracked")))?;
    let scorecard_entries = db::get_scorecard_history(&mut conn, &repo)
        .await
        .map_err(db_err)?;
    let deferred_items = db::get_deferred_items(&mut conn, &repo)
        .await
        .map_err(db_err)?;
    Ok(Json(RepoDetail {
        repo: row,
        scorecard_entries,
        deferred_items,
    }))
}

async fn reingest_repo(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<IngestedRepo>> {
    let repo = normalize_repo_id(&repo_id);
    let bundle = state
        .ingestion
        .ingest_repo(&repo)
        .await
        .map_err(upstream_err)?;
    Ok(Json(bundle))
}

async fn get_scorecard(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Vec<ScorecardEntry>>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let entries = db::get_scorecard_history(&mut conn, &repo)
        .await
        .map_err(db_err)?;
    Ok(Json(entries))
}

async fn get_deferred(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Vec<DeferredItem>>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let items = db::get_deferred_items(&mut conn, &repo)
        .await
        .map_err(db_err)?;
    Ok(Json(items))
}

async fn get_commits(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Vec<CommitSummary>>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let commits = db::get_recent_commits(&mut conn, &repo, 10)
        .await
        .map_err(db_err)?;
    Ok(Json(commits))
}

// ── recommendation ───────────────────────────────────────────────────────────

async fn get_recommendation(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Recommendation>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    if let Some(rec) = db::get_latest_recommendation(&mut conn, &repo)
        .await
        .map_err(db_err)?
    {
        return Ok(Json(rec));
    }
    drop(conn);
    if state.ingestion.skip_ai {
        return Err((
            StatusCode::NOT_FOUND,
            "no recommendation yet (AI disabled)".into(),
        ));
    }
    let rec = state
        .ingestion
        .regenerate_recommendation(&repo)
        .await
        .map_err(upstream_err)?;
    Ok(Json(rec))
}

async fn force_recommendation(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Recommendation>> {
    let repo = normalize_repo_id(&repo_id);
    let rec = state
        .ingestion
        .regenerate_recommendation(&repo)
        .await
        .map_err(upstream_err)?;
    Ok(Json(rec))
}

// ── tasks ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub prompt: String,
}

async fn create_task(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Json(body): Json<CreateTaskRequest>,
) -> ApiResult<(StatusCode, Json<Task>)> {
    let repo = normalize_repo_id(&repo_id);
    let task = Task {
        id: Uuid::new_v4(),
        repo_id: repo,
        title: body.title,
        prompt: body.prompt,
        status: TaskStatus::Pending,
        created_at: Utc::now(),
        completed_at: None,
    };
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    db::insert_task(&mut conn, &task).await.map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn list_tasks(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> ApiResult<Json<Vec<Task>>> {
    let repo = normalize_repo_id(&repo_id);
    let mut conn = state.pool.acquire().await.map_err(db_err)?;
    let tasks = db::list_tasks(&mut conn, &repo).await.map_err(db_err)?;
    Ok(Json(tasks))
}
