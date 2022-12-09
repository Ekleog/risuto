use risuto_api::Task;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListItemProps {
    pub task: Task,
    pub on_title_change: Callback<String>,
    pub on_done_change: Callback<()>,
}

#[function_component(TaskListItem)]
pub fn task_list(p: &TaskListItemProps) -> Html {
    let title_edit = use_state(|| None);

    html! { // align items vertically but also let them stretch
        <li class="list-group-item d-flex align-items-stretch">
            <div class="drag-handle d-flex align-items-center">
                <div class="bi-btn bi-grip-vertical pe-3"></div>
            </div>
            { title_div(&p, title_edit) }
            <div class="d-flex align-items-center">
                { button_done_change(&p.task, &p.on_done_change) }
            </div>
        </li>
    }
}

fn title_div(p: &TaskListItemProps, edit: UseStateHandle<Option<String>>) -> Html {
    let current_title = p.task.current_title.clone();
    let on_validate = {
        let edit = edit.clone();
        p.on_title_change.reform(move |t| {
            edit.set(None);
            t
        })
    };
    match (*edit).clone() {
        None => html! {
            <div
                class="flex-fill d-flex align-items-center"
                ondblclick={ Callback::from(move |_| {
                    edit.set(Some(current_title.clone()))
                }) }
            >
                { &p.task.current_title }
            </div>
        },
        Some(t) => html! {
            <div class="flex-fill d-flex align-items-center">
                <input
                    type="text"
                    value={ t.clone() }
                    onchange={ Callback::from(move |e: web_sys::Event| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        edit.set(Some(input.value()))
                    }) }
                    onfocusout={ let t = t.clone(); on_validate.reform(move |_| t.clone()) }
                    onkeyup={ Callback::from(move |e: web_sys::KeyboardEvent| {
                        if e.key() == "Enter" {
                            on_validate.emit(t.clone())
                        }
                    }) }
                />
            </div>
        },
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
