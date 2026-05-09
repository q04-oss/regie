use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};

pub mod github;
pub mod ingestion;
pub mod parsers;

use github::GitHubClient;
use ingestion::IngestionService;

#[derive(Clone)]
pub struct AppState {
    pub ingestion: Arc<IngestionService>,
}

pub async fn run() {
    dotenvy::dotenv().ok();
    let github_token =
        std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

    let github = GitHubClient::new(github_token);
    let state = AppState {
        ingestion: Arc::new(IngestionService::new(github)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/repos/{repo_id}/ingest", get(ingest_repo))
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

/// `repo_id` is colon-separated in the URL (`q04-oss:box-fraise-platform`)
/// because `/` would split the path. Normalise back to the GitHub form
/// before calling the ingestion service.
async fn ingest_repo(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let repo = repo_id.replacen(':', "/", 1);
    state
        .ingestion
        .ingest_repo(&repo)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))
}
