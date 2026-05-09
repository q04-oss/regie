use leptos::prelude::*;

use crate::api;

#[component]
pub fn CommitsPanel(repo_id: String) -> impl IntoView {
    let resource = LocalResource::new(move || {
        let r = repo_id.clone();
        async move { api::get_commits(&r).await }
    });

    view! {
        <div class="panel">
            <h2>"Recent commits"</h2>
            <Suspense fallback=move || view! { <p class="loading">"Loading..."</p> }>
                {move || resource.get().map(|res| match res.as_ref() {
                    Ok(commits) if commits.is_empty() => view! {
                        <p class="empty">"No commits ingested."</p>
                    }.into_any(),
                    Ok(commits) => {
                        let rows = commits.iter().take(5).cloned().map(|c| {
                            let short_sha: String = c.sha.chars().take(7).collect();
                            let body = c
                                .semantic_summary
                                .clone()
                                .unwrap_or_else(|| {
                                    c.message.lines().next().unwrap_or("").to_string()
                                });
                            let date = c.committed_at.format("%Y-%m-%d").to_string();
                            view! {
                                <li>
                                    <span class="commit-sha">{short_sha}</span>
                                    <span>{body}</span>
                                    <div class="commit-meta">
                                        <small>{c.author} " · " {date}</small>
                                    </div>
                                </li>
                            }
                        }).collect_view();
                        view! { <ul>{rows}</ul> }.into_any()
                    }
                    Err(e) => view! { <p class="error">{e.clone()}</p> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}
