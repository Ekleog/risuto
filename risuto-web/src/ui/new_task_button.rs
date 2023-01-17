use std::rc::Rc;

use risuto_client::{
    api::{self, Action, EventId, TaskId, Uuid},
    DbDump,
};
use yew::prelude::*;

use crate::util;

#[derive(Clone, PartialEq, Properties)]
pub struct NewTaskButtonProps {
    pub db: Rc<DbDump>,
    pub on_action: Callback<Action>,
}

// TODO: default to adding the tag of the current view / ScheduledFor(now) for today view
#[function_component(NewTaskButton)]
pub fn new_task_button(p: &NewTaskButtonProps) -> Html {
    let popup_shown = use_state(|| false);
    let title_ref = use_node_ref();
    let popup_class = popup_shown.then(|| "shown");
    let on_submit = {
        let db = p.db.clone();
        let on_action = p.on_action.clone();
        Callback::from(move |title| {
            let task_id = TaskId(Uuid::new_v4());
            let (title, evts) = util::parse_tag_changes(&*db, task_id, title);
            on_action.emit(Action::NewTask(
                api::Task {
                    id: task_id,
                    owner_id: db.owner,
                    date: chrono::Utc::now(),
                    initial_title: title,
                    top_comment_id: EventId(Uuid::new_v4()),
                },
                String::from(""), // TODO: allow setting initial top comment value
            ));
            for e in evts {
                on_action.emit(Action::NewEvent(e));
            }
        })
    };
    html! {
        <div class="float-above-20">
            <button
                type="button"
                class="btn btn-light btn-circle mt-3 ms-3 bi-btn bi-plus fs-6"
                title="New Task"
                onclick={
                    let title_ref = title_ref.clone();
                    let popup_shown = popup_shown.clone();
                    Callback::from(move |_| {
                        popup_shown.set(true);
                        title_ref.cast::<web_sys::HtmlInputElement>()
                            .expect("title is not an html input element")
                            .focus()
                            .expect("failed focusing title input");
                    })
                }
            >
            </button>
            <div class={ classes!(popup_class, "new-task-popup", "p-4") }>
                <div class="new-task-form p-3">
                    <input
                        ref={ title_ref }
                        type="text"
                        placeholder="Task Title"
                        aria-label="Task Title"
                        onkeydown={ Callback::from(move |e: web_sys::KeyboardEvent| {
                            match &e.key() as &str {
                                "Enter" => {
                                    let elt: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    on_submit.emit(elt.value());
                                    elt.set_value("");
                                    let _ = elt.blur();
                                    popup_shown.set(false);
                                }
                                "Escape" => {
                                    let elt: web_sys::HtmlElement = e.target_unchecked_into();
                                    let _ = elt.blur();
                                    popup_shown.set(false);
                                }
                                _ => (),
                            }
                        }) }
                    />
                </div>
                // TODO: add textarea to allow setting the top-comment right there (tab to it)
            </div>
            // TODO: add inline search to help dedup tasks
        </div>
    }
}
