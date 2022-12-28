use std::{rc::Rc, str::FromStr, sync::Arc};

use chrono::{Datelike, Timelike};
use risuto_api::{DbDump, Event, EventData, TagId, Task, Time, Query};
use wasm_bindgen::prelude::*;
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
        <li class="list-group-item p-0">
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
            let changed_title = evts.iter().any(|e| matches!(e, Event { task: t, data: EventData::SetTitle(_), .. } if t == &task.id));
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
                let mut tasks = db.tasks_for_query(&Query::Tag { tag, backlog: Some(false) });
                db.sort_tasks_for_tag(&tag, &mut tasks);
                res.extend(util::compute_reordering_events(
                    db.owner,
                    tag,
                    task.id,
                    0,
                    false,
                    &tasks,
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

#[wasm_bindgen(inline_js = "
    export function show_picker(elt) {
        elt.showPicker();
    }
    export function get_timezone() {
        return Intl.DateTimeFormat().resolvedOptions().timeZone;
    }
")]
extern "C" {
    fn show_picker(elt: &web_sys::HtmlInputElement);
    fn get_timezone() -> String;
}

fn local_timezone() -> chrono_tz::Tz {
    chrono_tz::Tz::from_str(&get_timezone()).expect("host js timezone is not in chrono-tz database")
}

fn timeset_button(
    input_ref: NodeRef,
    current_date: &Option<Time>,
    label: &'static str,
    icon: &'static str,
    callback: &Callback<Option<Time>>,
) -> Html {
    let on_button_click = {
        let input_ref = input_ref.clone();
        Callback::from(move |_| {
            let input = input_ref
                .cast::<web_sys::HtmlInputElement>()
                .expect("input is not an html element");
            show_picker(&input);
        })
    };
    let on_input = callback.reform(|e: web_sys::InputEvent| {
        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
        let date = chrono::NaiveDateTime::parse_from_str(&input.value(), "%Y-%m-%dT%H:%M")
            .expect("datepicker value not in expected format");
        let timezone = local_timezone();
        while date.and_local_timezone(timezone) == chrono::LocalResult::None {
            date.checked_sub_signed(chrono::Duration::minutes(1))
                .expect("overflow while looking for a date that exists in local timezone");
        }
        let date = date.and_local_timezone(timezone).earliest().unwrap();
        let date = date.with_timezone(&chrono::Utc);
        Some(date)
    });
    let current_date = current_date.map(|t| t.with_timezone(&local_timezone()));
    let timeset_label = current_date
        .map(|d| {
            let remaining = d.signed_duration_since(chrono::Utc::now());
            match remaining {
                r if r > chrono::Duration::days(365) => format!("{}", d.year()),
                r if r > chrono::Duration::days(1) => format!("{}/{}", d.month(), d.day()),
                r if r > chrono::Duration::seconds(0) => format!("{}h{}", d.hour(), d.minute()),
                _ => "past".to_string(),
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
        <>
            <div class="size-zero overflow-hidden">
                <input
                    ref={ input_ref }
                    type="datetime-local"
                    value={ start_value }
                    aria-label={ label }
                    oninput={ on_input } // TODO: change value only on date picker closed
                />
            </div>
            <button
                type="button"
                class={classes!("timeset-button", "btn", "bi-btn", icon, "px-2")}
                title={ label }
                onclick={ on_button_click }
            >
                { for timeset_label }
            </button>
        </>
    }
}

#[function_component(ButtonScheduleFor)]
fn button_schedule_for(p: &TaskListItemProps) -> Html {
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
    let input_ref = use_node_ref();
    let db = p.db.clone();
    let current_tag = p.current_tag.clone();
    let task = p.task.clone(); // TODO: split risuto-api into api+api-helpers so not all changes make a server-rebuild
    let on_event = p.on_event.clone();
    timeset_button(
        input_ref,
        &p.task.blocked_until,
        "Blocked until",
        "bi-hourglass-split",
        &Callback::from(move |t| {
            on_event.emit(Event::now(db.owner, task.id, EventData::BlockedUntil(t)));
            // Move task to this task's backlog if it was not already
            if let Some(tag) = current_tag {
                if !task.current_tags.get(&tag).unwrap().backlog {
                    let mut tasks = db.tasks_for_query(&Query::Tag { tag, backlog: Some(true) });
                    db.sort_tasks_for_tag(&tag, &mut tasks);
                    let evts = util::compute_reordering_events(
                        db.owner,
                        tag,
                        task.id,
                        0,
                        true,
                        &tasks,
                    );
                    for e in evts {
                        on_event.emit(e);
                    }
                }
            }
        }),
    )
}
