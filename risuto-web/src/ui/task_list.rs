use risuto_api::{Task, TaskId};
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub tasks: Vec<(TaskId, Task)>,
    pub on_done_change: Callback<(TaskId, bool)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    let list_items = p.tasks.iter().map(|(id, t)| {
        let on_done_change = {
            let id = *id;
            let is_done = t.is_done;
            p.on_done_change.reform(move |_| (id, !is_done))
        };
        let done_change_button = if t.is_done {
            html! {
                <button
                    type="button"
                    class="btn bi-btn bi-arrow-counterclockwise"
                    aria-label="Mark undone"
                    onclick={on_done_change}
                >
                </button>
            }
        } else {
            html! {
                <button
                    type="button"
                    class="btn bi-btn bi-check-lg"
                    aria-label="Mark done"
                    onclick={on_done_change}
                >
                </button>
            }
        };
        html! {
            <li class="list-group-item d-flex align-items-center">
                <span class="flex-grow-1">{ &t.current_title }</span>
                { done_change_button }
            </li>
        }
    });
    html! {
        <ul class="task-list list-group">
            { for list_items }
        </ul>
    }
}
