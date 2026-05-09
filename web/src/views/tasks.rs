use leptos::prelude::*;
use leptos::task::spawn_local;
use regie_shared::types::Task;

use crate::api;

#[component]
pub fn TasksPanel(repo_id: String) -> impl IntoView {
    let (tasks, set_tasks) = signal::<Vec<Task>>(Vec::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal::<Option<String>>(None);

    // Initial load
    let load_repo = repo_id.clone();
    let _initial = LocalResource::new(move || {
        let r = load_repo.clone();
        async move {
            match api::list_tasks(&r).await {
                Ok(list) => set_tasks.set(list),
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        }
    });

    let (title_value, set_title) = signal::<String>(String::new());
    let (prompt_value, set_prompt) = signal::<String>(String::new());

    let submit_repo = repo_id.clone();
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let title = title_value.get();
        let prompt = prompt_value.get();
        if title.is_empty() || prompt.is_empty() {
            set_error.set(Some("title and prompt are required".into()));
            return;
        }
        let r = submit_repo.clone();
        spawn_local(async move {
            match api::create_task(&r, &title, &prompt).await {
                Ok(task) => {
                    set_tasks.update(|list| list.insert(0, task));
                    set_title.set(String::new());
                    set_prompt.set(String::new());
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    view! {
        <div class="panel">
            <h2>"Tasks"</h2>
            <form on:submit=on_submit>
                <input
                    type="text"
                    placeholder="Title"
                    prop:value=move || title_value.get()
                    on:input=move |ev| set_title.set(event_target_value(&ev))
                />
                <textarea
                    placeholder="Prompt for Claude Code"
                    prop:value=move || prompt_value.get()
                    on:input=move |ev| set_prompt.set(event_target_value(&ev))
                />
                <button type="submit">"Create task"</button>
            </form>
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
            {move || if loading.get() {
                view! { <p class="loading">"Loading..."</p> }.into_any()
            } else if tasks.get().is_empty() {
                view! { <p class="empty">"No tasks yet."</p> }.into_any()
            } else {
                let rows = tasks.get().into_iter().map(|t| {
                    let status_label = format!("{:?}", t.status).to_lowercase();
                    view! {
                        <li>
                            <strong>{t.title}</strong>
                            " — "
                            <code>{status_label}</code>
                        </li>
                    }
                }).collect_view();
                view! { <ul>{rows}</ul> }.into_any()
            }}
        </div>
    }
}
