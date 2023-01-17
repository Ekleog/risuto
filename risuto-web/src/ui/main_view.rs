use crate::ui;
use risuto_client::{
    api::{Action, TagId},
    DbDump, Task,
};
use std::{collections::VecDeque, rc::Rc, sync::Arc};
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
    pub actions_pending_submission: VecDeque<Action>,
    pub db: Rc<DbDump>,
    pub current_tag: Option<TagId>,
    pub tasks_open: Rc<Vec<Arc<Task>>>,
    pub tasks_done: Rc<Vec<Arc<Task>>>,
    pub tasks_backlog: Rc<Vec<Arc<Task>>>,
    pub on_logout: Callback<()>,
    pub on_action: Callback<Action>,
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

    let backlog_list_ref = use_node_ref();
    let on_backlog_handle_drag = {
        let backlog_list_ref = backlog_list_ref.clone();
        Callback::from(move |e: web_sys::DragEvent| {
            let mouse_y = e.client_y();
            if mouse_y == 0 {
                return; // out of window
            }
            let window_height = web_sys::window()
                .expect("no web_sys window")
                .inner_height()
                .expect("failed retrieving inner_height of window")
                .as_f64()
                .expect("inner_height was not a float");
            let backlog_height = format!("{}px", window_height as i32 - mouse_y);
            let backlog_list = backlog_list_ref
                .cast::<web_sys::HtmlElement>()
                .expect("no main task list html element");
            let style = backlog_list.style();
            style
                .set_property("min-height", &backlog_height)
                .expect("failed setting min-height property");
            style
                .set_property("max-height", &backlog_height)
                .expect("failed setting max-height property");
        })
    };

    let empty_ref = use_node_ref();
    let hide_drag_image = {
        let empty_ref = empty_ref.clone();
        Callback::from(move |e: web_sys::DragEvent| {
            if let Some(t) = e.data_transfer() {
                t.set_drag_image(&empty_ref.cast().expect("no empty element"), 0, 0);
            }
        })
    };

    // Put everything together
    html! {
        <div class="h-100 d-flex flex-column overflow-hidden position-relative">
            <div ref={empty_ref}></div>
            <ui::OfflineBanner connection_state={p.connection_state.clone()} />

            // Top float-above bar corner
            <div class="float-above-container">
                <ui::SearchBar db={ p.db.clone() } />
                <ui::ActionSubmissionSpinner actions_pending_submission={ p.actions_pending_submission.clone() } />
                <ui::NewTaskButton user_id={ p.db.owner } on_action={ p.on_action.clone() }/>
                <ui::SettingsMenu on_logout={ p.on_logout.clone() } />
            </div>

            // Main task list
            <div class="flex-fill overflow-auto p-0">
                <div class="m-lg-5">
                    <ui::TaskList
                        ref_this={ ref_open }
                        db={ p.db.clone() }
                        current_tag={ p.current_tag.clone() }
                        tasks={ p.tasks_open.clone() }
                        on_event={ p.on_action.reform(Action::NewEvent) }
                    />
                </div>

                <div class="m-lg-5">
                    <ui::TaskList
                        ref_this={ ref_done }
                        db={ p.db.clone() }
                        current_tag={ p.current_tag.clone() }
                        tasks={ p.tasks_done.clone() }
                        on_event={ p.on_action.reform(Action::NewEvent) }
                    />
                </div>
            </div>

            // Backlog task list
            <div
                ref={backlog_list_ref}
                class="backlog-task-list p-0"
                style="min-height: 0px; max-height: 0px"
            >
                <button
                    class="backlog-drag-handle translate-middle btn btn-primary btn-circle"
                    type="button"
                    draggable="true"
                    ondragstart={hide_drag_image}
                    ondrag={on_backlog_handle_drag}
                >
                    <span class="bi-journal-text" aria-hidden="true"></span>
                    <span class="visually-hidden">{ "Backlog" }</span>
                </button>
                <div class="overflow-auto p-0 mh-100">
                    <div class="m-lg-5">
                        <ui::TaskList
                            ref_this={ ref_backlog }
                            db={ p.db.clone() }
                            current_tag={ p.current_tag.clone() }
                            tasks={ p.tasks_backlog.clone() }
                            on_event={ p.on_action.reform(Action::NewEvent) }
                        />
                    </div>
                </div>
            </div>
        </div>
    }
}
