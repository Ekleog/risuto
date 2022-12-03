use crate::ui;
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
    pub offline: bool,
    pub tasks_open: Rc<Vec<(TaskId, Task)>>,
    pub tasks_backlog: Rc<Vec<(TaskId, Task)>>,
    pub on_logout: Callback<()>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_order_change: Callback<TaskOrderChangeEvent>,
}

#[function_component(MainView)]
pub fn main_view(p: &MainViewProps) -> Html {
    // The lists must be sortable
    let ref_open = use_node_ref();
    let ref_backlog = use_node_ref();
    use_effect_with_deps(
        |(ref_open, ref_backlog, on_order_change)| {
            let ref_open = ref_open
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let ref_backlog = ref_backlog
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
                let ref_open = ref_open.clone();
                let ref_backlog = ref_backlog.clone();
                let on_order_change = on_order_change.clone();
                options.on_end(move |e| {
                    let before = TaskPosition {
                        index: e.old_index.expect("got update event without old index"),
                        in_backlog: *e.from == ref_backlog,
                    };
                    let after = TaskPosition {
                        index: e.new_index.expect("got update event without old index"),
                        in_backlog: *e.to == ref_backlog,
                    };
                    assert!(
                        before.in_backlog || *e.from == ref_open,
                        "got event that is from neither normal nor backlog list"
                    );
                    assert!(
                        after.in_backlog || *e.to == ref_open,
                        "got event that is to neither normal nor backlog list"
                    );
                    if before != after {
                        on_order_change.emit(TaskOrderChangeEvent { before, after });
                    }
                });
            }
            let keepalive = (options.apply(&ref_open), options.apply(&ref_backlog));
            move || {
                std::mem::drop(keepalive);
            }
        },
        (
            ref_open.clone(),
            ref_backlog.clone(),
            p.on_order_change.clone(),
        ),
    );

    // Prepare the offline banner
    let offline_banner = p.offline.then(|| html! {
        <div class="offline-banner d-flex align-items-center">
            <div class="spinner-border m-4" role="status"></div>
            <div class="fs-5">{ "Currently offline, trying to reconnect..." }</div>
        </div>
    });

    // Put everything together
    html! {
        <div class="h-100 d-flex flex-column">
            <button
                class="position-absolute top-0 end-0 float-above"
                onclick={p.on_logout.reform(|_| ())}
            >
                { "Logout" }
            </button>
            { for offline_banner }
            <div class="flex-fill overflow-auto p-lg-5">
                <ui::TaskList
                    ref_this={ref_open}
                    tasks={p.tasks_open.clone()}
                    on_done_change={p.on_done_change.clone()}
                />
            </div>
            <div class="overflow-auto p-lg-5" style="min-height: 50%; max-height: 50%;">
                <h2>{ "Backlog" }</h2>
                <ui::TaskList
                    ref_this={ref_backlog}
                    tasks={p.tasks_backlog.clone()}
                    on_done_change={p.on_done_change.clone()}
                />
            </div>
        </div>
    }
}
