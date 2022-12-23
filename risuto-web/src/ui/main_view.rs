use crate::ui;
use risuto_api::{Event, Task, TaskId};
use std::{collections::VecDeque, rc::Rc};
use yew::prelude::*;

#[derive(Debug, Eq, PartialEq)]
pub enum ListType {
    Open,
    Done,
    Backlog,
}

impl ListType {
    pub fn is_backlog(&self) -> bool {
        use ListType::*;
        match self {
            Open => false,
            Done => false,
            Backlog => true,
        }
    }

    pub fn is_done(&self) -> bool {
        use ListType::*;
        match self {
            Open => false,
            Done => true,
            Backlog => false,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TaskPosition {
    pub index: usize,
    pub list: ListType,
}

#[derive(Debug)]
pub struct TaskOrderChangeEvent {
    pub before: TaskPosition,
    pub after: TaskPosition,
}

#[derive(Clone, PartialEq, Properties)]
pub struct MainViewProps {
    pub connection_state: ui::ConnState,
    pub events_pending_submission: VecDeque<Event>,
    pub tasks_open: Rc<Vec<(TaskId, Task)>>,
    pub tasks_done: Rc<Vec<(TaskId, Task)>>,
    pub tasks_backlog: Rc<Vec<(TaskId, Task)>>,
    pub on_logout: Callback<()>,
    pub on_title_change: Callback<(TaskId, String)>,
    pub on_done_change: Callback<(TaskId, bool)>,
    pub on_order_change: Callback<TaskOrderChangeEvent>,
}

#[function_component(MainView)]
pub fn main_view(p: &MainViewProps) -> Html {
    // The lists must be sortable
    let ref_open = use_node_ref();
    let ref_done = use_node_ref();
    let ref_backlog = use_node_ref();
    use_effect_with_deps(
        |(ref_open, ref_done, ref_backlog, on_order_change)| {
            let ref_open = ref_open
                .cast::<web_sys::Element>()
                .expect("list_ref is not attached to an element");
            let ref_done = ref_done
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
                let ref_done = ref_done.clone();
                let ref_backlog = ref_backlog.clone();
                let on_order_change = on_order_change.clone();
                options.on_end(move |e| {
                    let as_task_list = |elt: &web_sys::HtmlElement| match elt {
                        e if **e == ref_open => ListType::Open,
                        e if **e == ref_done => ListType::Done,
                        e if **e == ref_backlog => ListType::Backlog,
                        _ => panic!("got event that is from neither open, done nor backlog list"),
                    };
                    let before = TaskPosition {
                        index: e.old_index.expect("got update event without old index"),
                        list: as_task_list(&e.from),
                    };
                    let after = TaskPosition {
                        index: e.new_index.expect("got update event without old index"),
                        list: as_task_list(&e.to),
                    };
                    if before != after {
                        on_order_change.emit(TaskOrderChangeEvent { before, after });
                    }
                });
            }
            let keepalive = (
                options.apply(&ref_open),
                options.apply(&ref_done),
                options.apply(&ref_backlog),
            );
            move || {
                std::mem::drop(keepalive);
            }
        },
        (
            ref_open.clone(),
            ref_done.clone(),
            ref_backlog.clone(),
            p.on_order_change.clone(),
        ),
    );

    // Put everything together
    html! {
        <div class="h-100 d-flex flex-column">
            <ui::OfflineBanner connection_state={p.connection_state.clone()} />

            // Top-right corner
            <div class="position-absolute top-0 end-0 float-above d-flex">
                <ui::EventSubmissionSpinner events_pending_submission={p.events_pending_submission.clone()} />
                <div>
                    <button onclick={p.on_logout.reform(|_| ())}>
                        { "Logout" }
                    </button>
                </div>
            </div>

            // Main task list
            <div class="flex-fill overflow-auto p-lg-5">
                <ui::TaskList
                    ref_this={ref_open}
                    tasks={p.tasks_open.clone()}
                    on_title_change={p.on_title_change.clone()}
                    on_done_change={p.on_done_change.clone()}
                />

                <div class="mt-4 done-task-list">
                    <ui::TaskList
                        ref_this={ref_done}
                        tasks={p.tasks_done.clone()}
                        on_title_change={p.on_title_change.clone()}
                        on_done_change={p.on_done_change.clone()}
                    />
                </div>
            </div>

            // Backlog task list
            <div class="overflow-auto p-lg-5" style="min-height: 50%; max-height: 50%;">
                <h2>{ "Backlog" }</h2>
                <ui::TaskList
                    ref_this={ref_backlog}
                    tasks={p.tasks_backlog.clone()}
                    on_title_change={p.on_title_change.clone()}
                    on_done_change={p.on_done_change.clone()}
                />
            </div>
        </div>
    }
}
