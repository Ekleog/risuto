use risuto_api::{Task, TaskId};
use std::rc::Rc;
use yew::prelude::*;

#[derive(Debug, Eq, PartialEq)]
pub struct TaskPosition {
    pub index: usize,
    pub in_backlog: bool,
}

#[derive(Debug)]
pub struct TaskOrderChangeEvent {
    pub before: TaskPosition,
    pub after: TaskPosition,
}

#[derive(Clone, PartialEq, Properties)]
pub struct MainViewProps {
    pub tasks_normal: Rc<Vec<(TaskId, Task)>>,
    pub tasks_backlog: Rc<Vec<(TaskId, Task)>>,
    pub on_logout: Callback<()>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_order_change: Callback<TaskOrderChangeEvent>,
}

fn task_list_for<'a>(
    p: &'a MainViewProps,
    tasks: &'a Vec<(TaskId, Task)>,
) -> impl 'a + Iterator<Item = Html> {
    tasks.iter().map(move |(id, t)| {
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
                <span class="drag-handle bi-btn bi-grip-vertical pe-3"></span>
                <span class="flex-grow-1">{ &t.current_title }</span>
                { done_change_button }
            </li>
        }
    })
}

#[function_component(MainView)]
pub fn main_view(p: &MainViewProps) -> Html {
    // First, build the list items
    let normal_list_items = task_list_for(&p, &p.tasks_normal);
    let backlog_list_items = task_list_for(&p, &p.tasks_backlog);

    // Then, make list sortable
    let normal_list_ref = use_node_ref();
    let backlog_list_ref = use_node_ref();
    use_effect_with_deps(
        |(normal_list_ref, backlog_list_ref, on_order_change)| {
            let normal_list = normal_list_ref
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let backlog_list = backlog_list_ref
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let mut options = sortable_js::Options::new();
            options
                .animation_ms(150.)
                .group("task-lists")
                .handle(".drag-handle")
                .revert_on_spill(true)
                .scroll(true)
                .bubble_scroll(true)
                .revert_dom(true);
            {
                let normal_list = normal_list.clone();
                let backlog_list = backlog_list.clone();
                let on_order_change = on_order_change.clone();
                options.on_end(move |e| {
                    let before = TaskPosition {
                        index: e.old_index.expect("got update event without old index"),
                        in_backlog: *e.from == backlog_list,
                    };
                    let after = TaskPosition {
                        index: e.new_index.expect("got update event without old index"),
                        in_backlog: *e.to == backlog_list,
                    };
                    assert!(
                        before.in_backlog || *e.from == normal_list,
                        "got event that is from neither normal nor backlog list"
                    );
                    assert!(
                        after.in_backlog || *e.to == normal_list,
                        "got event that is to neither normal nor backlog list"
                    );
                    if before != after {
                        on_order_change.emit(TaskOrderChangeEvent { before, after });
                    }
                });
            }
            let keepalive = (options.apply(&normal_list), options.apply(&backlog_list));
            move || {
                std::mem::drop(keepalive);
            }
        },
        (
            normal_list_ref.clone(),
            backlog_list_ref.clone(),
            p.on_order_change.clone(),
        ),
    );

    // Finally, put everything together
    html! {
        <div class="h-100">
            <button
                class="position-absolute top-0 end-0 float-above"
                onclick={p.on_logout.reform(|_| ())}
            >
                { "Logout" }
            </button>
            <div class="h-50 overflow-auto p-lg-5">
                <ul ref={normal_list_ref} class="task-list list-group">
                    { for normal_list_items }
                </ul>
            </div>
            <div class="h-50 overflow-auto p-lg-5">
                <h2>{ "Backlog" }</h2>
                <ul ref={backlog_list_ref} class="task-list list-group">
                    { for backlog_list_items }
                </ul>
            </div>
        </div>
    }
}
