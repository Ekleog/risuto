use std::{rc::Rc, sync::Arc};

use chrono::{Datelike, Timelike};
use risuto_client::{
    api::{Event, EventData, Query, TagId, Time},
    DbDump, Task,
};
use yew::prelude::*;

use crate::{util, TODAY_TAG};

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListItemProps {
    pub db: Rc<DbDump>,
    pub current_tag: Option<TagId>,
    pub task: Arc<Task>,
    pub on_event: Callback<Event>,
}

#[function_component(TaskListItem)]
pub fn task_list(p: &TaskListItemProps) -> Html {
    let mut tags = p
        .task
        .current_tags
        .keys()
        .filter(|t| p.current_tag.as_ref().map(|c| c != *t).unwrap_or(true))
        .filter_map(|t| p.db.tags.get(t).map(|tag| (t, tag)))
        .collect::<Vec<_>>();
    util::sort_tags(&p.db.owner, &mut tags);
    let no_tags = tags.is_empty();
    let tags = tags.into_iter().map(|(_, (t, _))| {
        html! {
            <span class="badge rounded-pill tag-pill me-1">{ &t.name }</span>
        }
    });
    html! { // align items vertically but also let them stretch
        <li class={classes!(p.task.is_done.then(|| "task-item-done"), "list-group-item", "p-0")}>
            <div class="d-flex align-items-stretch p-1">
                <div class="drag-handle d-flex align-items-center">
                    <div class="bi-btn bi-grip-vertical p-2"></div>
                </div>
                <div class="flex-fill d-flex flex-column align-items-stretch">
                    <TitleDiv
                        db={p.db.clone()}
                        task={p.task.clone()}
                        center_vertically={no_tags}
                        on_event={p.on_event.clone()}
                    />
                    <div class="px-3">{ for tags }</div>
                </div>
                <div class="d-flex align-items-center">
                    <ButtonBlockedUntil ..p.clone() />
                    <ButtonScheduleFor ..p.clone() />
                    <ButtonDoneChange ..p.clone() />
                </div>
            </div>
        </li>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct TitleDivProps {
    pub db: Rc<DbDump>,
    pub task: Arc<Task>,
    pub center_vertically: bool,
    pub on_event: Callback<Event>,
}

#[function_component(TitleDiv)]
fn title_div(p: &TitleDivProps) -> Html {
    let div_ref = use_node_ref();

    let on_validate = {
        let div_ref = div_ref.clone();
        let db = p.db.clone();
        let task = p.task.clone();
        let on_event = p.on_event.clone();
        Callback::from(move |()| {
            let div = div_ref
                .cast::<web_sys::HtmlElement>()
                .expect("validated while div_ref is not attached to an html element");
            let text = div.text_content().expect("div_ref has no text_content");
            let evts = parse_new_title(&db, text, &task);
            let changed_title = evts.iter().any(|e| matches!(e, Event { task_id, data: EventData::SetTitle(_), .. } if task_id == &task.id));
            for e in evts {
                on_event.emit(e);
            }
            div.blur().expect("failed blurring div_ref");
            if !changed_title {
                // TODO: find a way to force yew to resync html dom with its vdom even if the vdom doesn't change
                div.set_text_content(Some(&task.current_title));
            }
        })
    };

    let align = match p.center_vertically {
        true => "align-items-center",
        false => "align-items-end",
    };

    html! {
        <div
            ref={div_ref}
            class={classes!("flex-fill", "d-flex", align, "p-1")}
            contenteditable="true"
            spellcheck="false"
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

fn parse_new_title(db: &DbDump, mut title: String, task: &Task) -> Vec<Event> {
    let mut res = Vec::new();
    loop {
        title.truncate(title.trim_end().len());

        if let Some(i) = title.rfind(" -") {
            let tag_start = i + " -".len();
            if let Some(t) = title.get(tag_start..).and_then(|t| db.tag_id(t)) {
                res.push(Event::now(db.owner, task.id, EventData::RmTag(t)));
                title.truncate(i);
                continue;
            }
        }

        if let Some(i) = title.rfind(" +") {
            let tag_start = i + " +".len();
            if let Some(tag) = title.get(tag_start..).and_then(|t| db.tag_id(t)) {
                let mut tasks = db.tasks_for_query(&Query::Tag {
                    tag,
                    backlog: Some(false),
                });
                db.sort_tasks_for_tag(&tag, &mut tasks);
                res.extend(util::compute_reordering_events(
                    db.owner, tag, task.id, 0, false, &tasks,
                ));
                title.truncate(i);
                continue;
            }
        }

        if title != task.current_title {
            res.push(Event::now(db.owner, task.id, EventData::SetTitle(title)));
        }

        return res;
    }
}

#[function_component(ButtonDoneChange)]
fn button_done_change(p: &TaskListItemProps) -> Html {
    let icon_class = match p.task.is_done {
        true => "bi-arrow-counterclockwise",
        false => "bi-check-lg",
    };
    let aria_label = match p.task.is_done {
        true => "Mark undone",
        false => "Mark done",
    };
    let onclick = {
        let owner = p.db.owner;
        let task = p.task.id;
        let currently_done = p.task.is_done;
        p.on_event
            .reform(move |_| Event::now(owner, task, EventData::SetDone(!currently_done)))
    };
    html! {
        <button
            type="button"
            class={ classes!("btn", "bi-btn", icon_class, "ps-2") }
            title={ aria_label }
            { onclick }
        >
        </button>
    }
}

fn timeset_button(
    input_ref: NodeRef,
    is_shown: UseStateHandle<bool>,
    current_date: &Option<Time>,
    label: &'static str,
    icon: &'static str,
    callback: &Callback<Option<Time>>,
) -> Html {
    let close_input = {
        let input_ref = input_ref.clone();
        let is_shown = is_shown.clone();
        let callback = callback.clone();
        Callback::from(move |_| {
            is_shown.set(false);
            let input = input_ref
                .cast::<web_sys::HtmlInputElement>()
                .expect("input is not an html element")
                .value();
            let date = match input.is_empty() {
                true => None,
                false => Some({
                    let date = chrono::NaiveDateTime::parse_from_str(&input, "%Y-%m-%dT%H:%M")
                        .expect("datepicker value not in expected format");
                    let timezone = util::local_tz();
                    while date.and_local_timezone(timezone) == chrono::LocalResult::None {
                        date.checked_sub_signed(chrono::Duration::minutes(1))
                            .expect(
                                "overflow while looking for a date that exists in local timezone",
                            );
                    }
                    let date = date.and_local_timezone(timezone).earliest().unwrap();
                    date.with_timezone(&chrono::Utc)
                }),
            };
            callback.emit(date);
        })
    };
    let on_button_click = {
        let input_ref = input_ref.clone();
        let is_shown = is_shown.clone();
        let close_input = close_input.clone();
        Callback::from(move |_| {
            if *is_shown {
                close_input.emit(());
            } else {
                is_shown.set(true);
                let input = input_ref
                    .cast::<web_sys::HtmlInputElement>()
                    .expect("input is not an html element");
                input.focus().expect("failed focusing date picker");
                util::show_picker(&input);
            }
        })
    };
    let current_date = current_date.map(|t| t.with_timezone(&util::local_tz()));
    let timeset_label = current_date
        .and_then(|d| {
            let remaining = d.signed_duration_since(chrono::Utc::now());
            match remaining {
                r if r > chrono::Duration::days(365) => Some(format!("{}", d.year())),
                r if r > chrono::Duration::days(1) => Some(format!("{}/{}", d.month(), d.day())),
                r if r > chrono::Duration::seconds(0) => {
                    Some(format!("{}h{}", d.hour(), d.minute()))
                }
                _ => None, // task blocked or scheduled for the past is just not blocked/scheduled
            }
        })
        .map(|l| {
            html! {
                <span class="timeset-label rounded-pill">
                    { l }
                </span>
            }
        });
    let start_value = current_date.map(|d| {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            d.year(),
            d.month(),
            d.day(),
            d.hour(),
            d.minute()
        )
    });
    html! {
        <div class={ classes!("timeset-container", "d-flex", "align-items-center", is_shown.then(|| "shown")) }>
            <button
                type="button"
                class={ classes!("timeset-button", "btn", "bi-btn", icon, "px-2") }
                title={ label }
                onclick={ on_button_click }
            >
                { for timeset_label }
            </button>
            <div class={ classes!("timeset-input", is_shown.then(|| "shown")) }>
                <input
                    ref={ input_ref }
                    class="mx-2"
                    type="datetime-local"
                    value={ start_value }
                    onfocusout={ close_input.reform(|_| ()) }
                    aria-label={ label }
                />
            </div>
        </div>
    }
}

#[function_component(ButtonScheduleFor)]
fn button_schedule_for(p: &TaskListItemProps) -> Html {
    let is_shown = use_state(|| false);
    let input_ref = use_node_ref();
    let db = p.db.clone();
    let task = p.task.clone();
    let on_event = p.on_event.clone();
    // TODO: re-add today tag when hitting the time
    // This will require keeping one scheduled-date per owner actually!
    // as each user has their own today tag and it wouldn't make sense to
    // add stuff to other people's today tag
    timeset_button(
        input_ref,
        is_shown,
        &p.task.scheduled_for,
        "Schedule for",
        "bi-alarm",
        &Callback::from(move |t| {
            // First reschedule, then remove tag
            // Otherwise, if the tag was the one giving us the perms to edit the event, it'll crash
            on_event.emit(Event::now(db.owner, task.id, EventData::ScheduleFor(t)));
            let today_id = db.tag_id(TODAY_TAG).expect("no today tag");
            if task.current_tags.contains_key(&today_id) {
                on_event.emit(Event::now(db.owner, task.id, EventData::RmTag(today_id)));
            }
        }),
    )
}

#[function_component(ButtonBlockedUntil)]
fn button_blocked_until(p: &TaskListItemProps) -> Html {
    let is_shown = use_state(|| false);
    let input_ref = use_node_ref();
    let db = p.db.clone();
    let task = p.task.clone();
    let on_event = p.on_event.clone();
    timeset_button(
        input_ref,
        is_shown,
        &p.task.blocked_until,
        "Blocked until",
        "bi-hourglass-split",
        &Callback::from(move |t| {
            on_event.emit(Event::now(db.owner, task.id, EventData::BlockedUntil(t)));
        }),
    )
}
