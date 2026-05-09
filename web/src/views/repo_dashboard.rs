use leptos::prelude::*;

use crate::views::{
    commits::CommitsPanel, deferred::DeferredPanel, recommendation::RecommendationPanel,
    scorecard::ScorecardPanel, tasks::TasksPanel,
};

#[component]
pub fn RepoDashboard(
    repo_id: String,
    on_back: impl Fn() + 'static,
) -> impl IntoView {
    // Each panel needs its own owned copy so it can be clone-captured into
    // a long-lived async fetcher.
    let title = repo_id.clone();
    let scorecard_repo = repo_id.clone();
    let deferred_repo = repo_id.clone();
    let commits_repo = repo_id.clone();
    let rec_repo = repo_id.clone();
    let tasks_repo = repo_id.clone();

    view! {
        <div class="dashboard">
            <div class="dashboard-header">
                <button on:click=move |_| on_back()>"← Back"</button>
                <h2 class="dashboard-title">{title}</h2>
            </div>
            <div class="dashboard-grid">
                <ScorecardPanel repo_id=scorecard_repo />
                <DeferredPanel repo_id=deferred_repo />
                <CommitsPanel repo_id=commits_repo />
                <RecommendationPanel repo_id=rec_repo />
            </div>
            <TasksPanel repo_id=tasks_repo />
        </div>
    }
}
