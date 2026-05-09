use leptos::prelude::*;
use leptos::task::spawn_local;
use regie_shared::types::Recommendation;

use crate::api;

#[component]
pub fn RecommendationPanel(repo_id: String) -> impl IntoView {
    // Held-state signal is the source of truth; the resource bootstraps it
    // and the "Regenerate" button replaces it with a fresh value.
    let (state, set_state) =
        signal::<Option<Result<Recommendation, String>>>(None);

    let bootstrap_repo = repo_id.clone();
    let _bootstrap = LocalResource::new(move || {
        let r = bootstrap_repo.clone();
        async move {
            let res = api::get_recommendation(&r).await;
            set_state.set(Some(res));
        }
    });

    let regen_repo = repo_id.clone();
    let on_regen = move |_ev| {
        let r = regen_repo.clone();
        set_state.set(None);
        spawn_local(async move {
            let res = api::force_recommendation(&r).await;
            set_state.set(Some(res));
        });
    };

    view! {
        <div class="panel">
            <h2>"Recommendation"</h2>
            {move || match state.get() {
                None => view! { <p class="loading">"Loading..."</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(rec)) => {
                    let pills = rec.related_deferred_items.iter().cloned().map(|id| {
                        view! { <code class="commit-sha">{id}</code> " " }
                    }).collect_view();
                    view! {
                        <div>
                            <p class="rec-action"><strong>{rec.top_action}</strong></p>
                            <p>{rec.justification}</p>
                            <p class="rec-impact">{rec.estimated_impact}</p>
                            <div>{pills}</div>
                        </div>
                    }.into_any()
                }
            }}
            <button on:click=on_regen>"Regenerate"</button>
        </div>
    }
}
