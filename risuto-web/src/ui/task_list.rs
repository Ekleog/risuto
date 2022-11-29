use risuto_api::{Task, TaskId};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub tasks_normal: Rc<Vec<(TaskId, Task)>>,
    pub tasks_backlog: Rc<Vec<(TaskId, Task)>>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_backlog_change: Callback<(TaskId, bool)>,
    pub on_order_change: Callback<(usize, usize)>,
}

fn task_list_for<'a>(p: &'a TaskListProps, tasks: &'a Vec<(TaskId, Task)>, is_backlog: bool) -> impl 'a + Iterator<Item = Html> {
    tasks.iter().map(move |(id, t)| {
        // (Un)backlog button
        // TODO: make disappear on untagged list
        let on_backlog_change = {
            let id = *id;
            p.on_backlog_change.reform(move |_| (id, !is_backlog))
        };
        let backlog_change_button = if is_backlog {
            html! {
                <button
                    type="button"
                    class="btn bi-btn bi-arrow-up"
                    aria-label="Get out of backlog"
                    onclick={on_backlog_change}
                >
                </button>
            }
        } else {
            html! {
                <button
                    type="button"
                    class="btn bi-btn bi-arrow-down"
                    aria-label="Put in backlog"
                    onclick={on_backlog_change}
                >
                </button>
            }
        };

        // Done button
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

        // Put everything together
        html! {
            <li class="list-group-item d-flex align-items-center">
                <span class="flex-grow-1">{ &t.current_title }</span>
                { backlog_change_button }
                { done_change_button }
            </li>
        }
    })
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let normal_list_items = task_list_for(&p, &p.tasks_normal, false);
    let backlog_list_items = task_list_for(&p, &p.tasks_backlog, true);

    // Then, make list sortable
    let normal_list_ref = use_node_ref();
    let backlog_list_ref = use_node_ref();
    let on_order_change = p.on_order_change.clone();
    use_effect_with_deps(
        move |(normal_list_ref, backlog_list_ref)| {
            let normal_list = normal_list_ref
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let backlog_list = backlog_list_ref
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let mut options = sortable_js::Options::new();
            options.animation_ms(150.)
                .on_update(move |e| {
                    let old = e.old_index.expect("got update event without old index");
                    let new = e.new_index.expect("got update event without old index");
                    on_order_change.emit((old, new));
                });
            let keepalive = (options.apply(&normal_list), options.apply(&backlog_list));
            move || {
                std::mem::drop(keepalive);
            }
        },
        (normal_list_ref.clone(), backlog_list_ref.clone()),
    );

    // Finally, put everything together
    html! {
        <>
            <ul ref={normal_list_ref} class="task-list list-group">
                { for normal_list_items }
            </ul>
            <h2>{ "Backlog" }</h2>
            <ul ref={backlog_list_ref} class="task-list list-group">
                { for backlog_list_items }
            </ul>
        </>
    }
}
