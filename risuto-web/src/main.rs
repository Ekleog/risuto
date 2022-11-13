use chrono::Utc;
use risuto_api::*;
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    let mut tasks = std::collections::HashMap::new();
    tasks.insert("000", Task {
        owner: UserId(Uuid::new_v4()),
        date: Utc::now(),
        initial_title: "Task 1".to_string(),
        current_title: "Task 1".to_string(),
        is_done: false,
        is_archived: false,
        scheduled_for: None,
        current_tags: std::collections::HashMap::new(),
        deps_before_self: std::collections::HashSet::new(),
        deps_after_self: std::collections::HashSet::new(),
        current_comments: std::collections::BTreeMap::new(),
        events: std::collections::BTreeMap::new(),
    });
    tasks.insert("001", Task {
        owner: UserId(Uuid::new_v4()),
        date: Utc::now(),
        initial_title: "Task 2".to_string(),
        current_title: "Task 2 new title".to_string(),
        is_done: false,
        is_archived: false,
        scheduled_for: None,
        current_tags: std::collections::HashMap::new(),
        deps_before_self: std::collections::HashSet::new(),
        deps_after_self: std::collections::HashSet::new(),
        current_comments: std::collections::BTreeMap::new(),
        events: std::collections::BTreeMap::new(),
    });
    let task_items = tasks.values().map(|t| html! {
        <li class="list-group-item">{ format!("{} (owned by {})", t.current_title, t.owner.0)}</li>
    }).collect::<Html>();
    html! {
        <>
            <h1>{ "Tasks" }</h1>
            <ul class="list-group">
                { task_items }
            </ul>
        </>
    }
}

fn main() {
    yew::start_app::<App>();
}
