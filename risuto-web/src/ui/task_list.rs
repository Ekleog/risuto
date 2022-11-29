use risuto_api::{Task, TaskId};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub tasks: Rc<Vec<(TaskId, Task)>>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_order_change: Callback<(usize, usize)>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
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

    // Then, make list sortable
    let list_ref = use_node_ref();
    let on_order_change = p.on_order_change.clone();
    use_effect_with_deps(
        move |list_ref| {
            let list = list_ref
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let sortable = sortable_js::Options::new()
                .animation_ms(150.)
                .on_update(move |e| {
                    let old = e.old_index.expect("got update event without old index");
                    let new = e.new_index.expect("got update event without old index");
                    on_order_change.emit((old, new));
                })
                .apply(&list);
            move || {
                std::mem::drop(sortable);
            }
        },
        list_ref.clone(),
    );

    // Finally, put everything together
    html! {
        <ul ref={list_ref} class="task-list list-group">
            { for list_items }
        </ul>
    }
}
