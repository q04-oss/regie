use leptos::prelude::*;

pub mod api;
pub mod views;

#[component]
pub fn App() -> impl IntoView {
    let (selected_repo, set_selected_repo) = signal::<Option<String>>(None);

    view! {
        <div class="regie">
            <header>
                <h1>"Régie"</h1>
                <span class="subtitle">"engineering intelligence"</span>
            </header>
            <main>
                {move || match selected_repo.get() {
                    None => view! {
                        <views::repo_list::RepoList on_select=set_selected_repo />
                    }.into_any(),
                    Some(repo_id) => view! {
                        <views::repo_dashboard::RepoDashboard
                            repo_id=repo_id
                            on_back=move || set_selected_repo.set(None)
                        />
                    }.into_any(),
                }}
            </main>
        </div>
    }
}
