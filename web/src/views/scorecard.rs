use leptos::prelude::*;

use crate::api;

#[component]
pub fn ScorecardPanel(repo_id: String) -> impl IntoView {
    let resource = LocalResource::new(move || {
        let r = repo_id.clone();
        async move { api::get_scorecard(&r).await }
    });

    view! {
        <div class="panel">
            <h2>"Scorecard"</h2>
            <Suspense fallback=move || view! { <p class="loading">"Loading..."</p> }>
                {move || resource.get().map(|res| match res.as_ref() {
                    Ok(entries) if entries.is_empty() => view! {
                        <p class="empty">"No scorecard history yet."</p>
                    }.into_any(),
                    Ok(entries) => {
                        // Last 5 entries, newest first.
                        let rows = entries.iter().rev().take(5).cloned().map(|e| {
                            view! {
                                <tr>
                                    <td>{e.date.to_string()}</td>
                                    <td>{e.grade}</td>
                                    <td>{format_score(e.security)}</td>
                                    <td>{format_score(e.architecture)}</td>
                                    <td>{format_score(e.engineer_usability)}</td>
                                    <td>{format_score(e.protocol_conformance)}</td>
                                    <td>{format_score(e.operational_readiness)}</td>
                                    <td>{format_score(e.product_completeness)}</td>
                                    <td>{format_score(e.weighted_score)}</td>
                                </tr>
                            }
                        }).collect_view();
                        view! {
                            <table>
                                <thead>
                                    <tr>
                                        <th>"Date"</th>
                                        <th>"Grade"</th>
                                        <th>"Sec"</th>
                                        <th>"Arch"</th>
                                        <th>"Usab"</th>
                                        <th>"Proto"</th>
                                        <th>"Ops"</th>
                                        <th>"Prod"</th>
                                        <th>"Weighted"</th>
                                    </tr>
                                </thead>
                                <tbody>{rows}</tbody>
                            </table>
                        }.into_any()
                    }
                    Err(e) => view! { <p class="error">{e.clone()}</p> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

fn format_score(s: f64) -> String {
    format!("{s:.1}")
}
