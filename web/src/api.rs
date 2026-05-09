//! Browser-side API client for the regie-api server. Wraps `gloo-net`'s
//! fetch shim. Every call returns `Result<T, String>`; non-2xx responses
//! become an `Err` with the HTTP status; transport / decode errors become
//! an `Err` with the underlying message.

use gloo_net::http::Request;
use regie_shared::types::{
    CommitSummary, DeferredItem, IngestedRepo, Recommendation, Repo,
    ScorecardEntry, Task,
};

const BASE: &str = "http://localhost:3000";

/// `repo_id` arrives in the URL path; replace `/` with `:` once so the
/// segment doesn't get split. `repo_id_for_path("q04-oss/regie")` returns
/// `"q04-oss:regie"`. The API does the inverse on the way in.
fn repo_id_for_path(repo_id: &str) -> String {
    repo_id.replacen('/', ":", 1)
}

async fn get_json<T>(url: &str) -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), resp.status_text()));
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

async fn post_json<B, T>(url: &str, body: &B) -> Result<T, String>
where
    B: serde::Serialize,
    T: for<'de> serde::Deserialize<'de>,
{
    let resp = Request::post(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), resp.status_text()));
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

async fn post_empty<T>(url: &str) -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let resp = Request::post(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {} {}", resp.status(), resp.status_text()));
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

pub async fn list_repos() -> Result<Vec<Repo>, String> {
    get_json(&format!("{BASE}/api/repos")).await
}

pub async fn ingest_repo(repo_id: &str) -> Result<IngestedRepo, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/ingest")).await
}

pub async fn get_scorecard(repo_id: &str) -> Result<Vec<ScorecardEntry>, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/scorecard")).await
}

pub async fn get_deferred_items(repo_id: &str) -> Result<Vec<DeferredItem>, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/deferred")).await
}

pub async fn get_commits(repo_id: &str) -> Result<Vec<CommitSummary>, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/commits")).await
}

pub async fn get_recommendation(repo_id: &str) -> Result<Recommendation, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/recommendation")).await
}

/// `POST /recommendation` — force regenerate. Used by the "Regenerate"
/// button in the recommendation panel.
pub async fn force_recommendation(repo_id: &str) -> Result<Recommendation, String> {
    let path = repo_id_for_path(repo_id);
    post_empty(&format!("{BASE}/api/repos/{path}/recommendation")).await
}

#[derive(serde::Serialize)]
struct CreateTaskBody<'a> {
    title: &'a str,
    prompt: &'a str,
}

pub async fn create_task(
    repo_id: &str,
    title: &str,
    prompt: &str,
) -> Result<Task, String> {
    let path = repo_id_for_path(repo_id);
    post_json(
        &format!("{BASE}/api/repos/{path}/tasks"),
        &CreateTaskBody { title, prompt },
    )
    .await
}

pub async fn list_tasks(repo_id: &str) -> Result<Vec<Task>, String> {
    let path = repo_id_for_path(repo_id);
    get_json(&format!("{BASE}/api/repos/{path}/tasks")).await
}
