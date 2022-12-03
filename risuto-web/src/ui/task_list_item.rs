use risuto_api::Task;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListItemProps {
    pub task: Task,
    pub on_done_change: Callback<()>,
}

#[function_component(TaskListItem)]
pub fn task_list(p: &TaskListItemProps) -> Html {
    html! {
        <li class="list-group-item d-flex align-items-center">
            <span class="drag-handle bi-btn bi-grip-vertical pe-3"></span>
            <span class="flex-fill">{ &p.task.current_title }</span>
            { button_done_change(&p.task, &p.on_done_change) }
        </li>
    }
}

fn button_done_change(t: &Task, on_done_change: &Callback<()>) -> Html {
    let icon_class = match t.is_done {
        true => "bi-arrow-counterclockwise",
        false => "bi-check-lg",
    };
    let aria_label = match t.is_done {
        true => "Mark undone",
        false => "Mark done",
    };
    html! {
        <button
            type="button"
            class={ classes!("btn", "bi-btn", icon_class) }
            aria-label={ aria_label }
            onclick={ on_done_change.reform(|_| ()) }
        >
        </button>
    }
}
