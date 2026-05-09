use leptos::prelude::*;
use regie_shared::types::{DeferredItem, DeferredItemPriority};

use crate::api;

#[component]
pub fn DeferredPanel(repo_id: String) -> impl IntoView {
    let resource = LocalResource::new(move || {
        let r = repo_id.clone();
        async move { api::get_deferred_items(&r).await }
    });

    view! {
        <div class="panel">
            <h2>"Deferred items"</h2>
            <Suspense fallback=move || view! { <p class="loading">"Loading..."</p> }>
                {move || resource.get().map(|res| match res.as_ref() {
                    Ok(items) if items.is_empty() => view! {
                        <p class="empty">"No deferred items."</p>
                    }.into_any(),
                    Ok(items) => {
                        let high   = render_group("High",   filter(items, DeferredItemPriority::High),   "priority-high");
                        let medium = render_group("Medium", filter(items, DeferredItemPriority::Medium), "priority-medium");
                        let low    = render_group("Low",    filter(items, DeferredItemPriority::Low),    "priority-low");
                        view! {
                            <div>{high}{medium}{low}</div>
                        }.into_any()
                    }
                    Err(e) => view! { <p class="error">{e.clone()}</p> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

fn filter(items: &[DeferredItem], p: DeferredItemPriority) -> Vec<DeferredItem> {
    items
        .iter()
        .filter(|i| matches!((i.priority, p),
            (DeferredItemPriority::High,   DeferredItemPriority::High)   |
            (DeferredItemPriority::Medium, DeferredItemPriority::Medium) |
            (DeferredItemPriority::Low,    DeferredItemPriority::Low)
        ))
        .cloned()
        .collect()
}

fn render_group(label: &'static str, items: Vec<DeferredItem>, class: &'static str) -> AnyView {
    if items.is_empty() {
        return view! { <></> }.into_any();
    }
    let rendered = items.into_iter().map(|item| {
        let file_ref = item.file_ref.clone();
        view! {
            <li>
                <span>{item.description}</span>
                {file_ref.map(|f| view! { " " <code class="commit-sha">{f}</code> })}
            </li>
        }
    }).collect_view();
    view! {
        <div>
            <h3 class=class>{label}</h3>
            <ul>{rendered}</ul>
        </div>
    }
    .into_any()
}
