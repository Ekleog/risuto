use risuto_api::Task;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListItemProps {
    pub task: Task,
    pub on_done_change: Callback<()>,
}

#[function_component(TaskListItem)]
pub fn task_list(p: &TaskListItemProps) -> Html {
    html! { // align items vertically but also let them stretch
        <li class="list-group-item d-flex align-items-stretch">
            <div class="drag-handle d-flex align-items-center">
                <div class="bi-btn bi-grip-vertical pe-3"></div>
            </div>
            <div class="flex-fill d-flex align-items-center">
                { &p.task.current_title }
            </div>
            <div class="d-flex align-items-center">
                { button_done_change(&p.task, &p.on_done_change) }
            </div>
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
