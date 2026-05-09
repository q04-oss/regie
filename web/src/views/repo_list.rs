use leptos::prelude::*;

use crate::api;

#[component]
pub fn RepoList(on_select: WriteSignal<Option<String>>) -> impl IntoView {
    let repos = LocalResource::new(|| async move { api::list_repos().await });

    view! {
        <div class="panel">
            <h2>"Repositories"</h2>
            <Suspense fallback=move || view! { <p class="loading">"Loading..."</p> }>
                {move || repos.get().map(|res| match res.as_ref() {
                    Ok(list) if list.is_empty() => view! {
                        <p class="empty">
                            "No repositories tracked. POST /api/repos with { repo_id }."
                        </p>
                    }.into_any(),
                    Ok(list) => {
                        let rows = list.iter().cloned().map(|repo| {
                            let id_for_click = repo.id.clone();
                            let grade = repo.grade.clone().unwrap_or_else(|| "-".into());
                            let grade_class = grade_css_class(&grade);
                            let last = repo
                                .last_ingested_at
                                .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                                .unwrap_or_else(|| "never".into());
                            view! {
                                <tr>
                                    <td>
                                        <a href="#" on:click=move |ev| {
                                            ev.prevent_default();
                                            on_select.set(Some(id_for_click.clone()));
                                        }>
                                            {repo.name}
                                        </a>
                                    </td>
                                    <td class=grade_class>{grade}</td>
                                    <td>{last}</td>
                                </tr>
                            }
                        }).collect_view();
                        view! {
                            <table>
                                <thead>
                                    <tr>
                                        <th>"Repository"</th>
                                        <th>"Grade"</th>
                                        <th>"Last ingested"</th>
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

fn grade_css_class(grade: &str) -> &'static str {
    match grade.chars().next() {
        Some('A') => "grade-a",
        Some('B') => "grade-b",
        Some('C') | Some('D') | Some('F') => "grade-c",
        _ => "",
    }
}
