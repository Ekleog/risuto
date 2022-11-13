use chrono::Utc;
use risuto_api::*;
use yew::prelude::*;

fn main() {
    yew::start_app::<App>();
}

#[function_component(App)]
fn app() -> Html {
    let mut tasks = Vec::new();
    tasks.push((TaskId(Uuid::new_v4()), Task {
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
    }));
    tasks.push((TaskId(Uuid::new_v4()), Task {
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
    }));
    html! {
        <>
            <h1>{ "Tasks" }</h1>
            <ul class="list-group">
                <TaskList tasks={tasks} />
            </ul>
        </>
    }
}

#[derive(Clone, PartialEq, Properties)]
struct TaskListProps {
    tasks: Vec<(TaskId, Task)>,
}

#[function_component(TaskList)]
fn task_list(TaskListProps { tasks }: &TaskListProps) -> Html {
    tasks.iter().map(|(_, t)| html! {
        <li class="list-group-item">{ format!("{} (owned by {})", t.current_title, t.owner.0)}</li>
    }).collect()
}
