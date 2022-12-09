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
    html! { // align items vertically but also let them stretch
        <li class="list-group-item d-flex align-items-stretch">
            <div class="drag-handle d-flex align-items-center">
                <div class="bi-btn bi-grip-vertical pe-3"></div>
            </div>
            <TitleDiv ..p.clone() />
            <div class="d-flex align-items-center">
                { button_done_change(&p.task, &p.on_done_change) }
            </div>
        </li>
    }
}

#[function_component(TitleDiv)]
fn title_div(p: &TaskListItemProps) -> Html {
    let div_ref = use_node_ref();

    let on_validate = {
        let div_ref = div_ref.clone();
        let initial_title = p.task.current_title.clone();
        let on_title_change = p.on_title_change.clone();
        Callback::from(move |()| {
            let text = div_ref
                .get()
                .expect("validated while div_ref is not attached to an html element")
                .text_content()
                .expect("div_ref has no text_content");
            if text != initial_title {
                on_title_change.emit(text);
            }
        })
    };

    html! {
        <div
            ref={div_ref}
            class="flex-fill d-flex align-items-center"
            contenteditable="true"
            onfocusout={ on_validate.reform(|_| ()) }
            onkeydown={ Callback::from(move |e: web_sys::KeyboardEvent| {
                match &e.key() as &str {
                    "Enter" => on_validate.emit(()),
                    "Escape" => {
                        let elt: web_sys::HtmlElement = e.target_unchecked_into();
                        let _ = elt.blur();
                    }
                    _ => (),
                }
            }) }
        >
            { &p.task.current_title }
        </div>
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
