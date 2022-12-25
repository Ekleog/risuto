use std::{sync::Arc, str::FromStr};

use chrono::{Datelike, Timelike};
use risuto_api::{Task, Time, EventData};
use wasm_bindgen::prelude::*;
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListItemProps {
    pub task: Arc<Task>,
    pub on_event: Callback<EventData>,
}

#[function_component(TaskListItem)]
pub fn task_list(p: &TaskListItemProps) -> Html {
    html! { // align items vertically but also let them stretch
        <li class="list-group-item d-flex align-items-stretch p-1">
            <div class="drag-handle d-flex align-items-center">
                <div class="bi-btn bi-grip-vertical p-2"></div>
            </div>
            <TitleDiv ..p.clone() />
            <div class="d-flex align-items-center">
                <ButtonBlockedUntil ..p.clone() />
                <ButtonScheduleFor ..p.clone() />
                { button_done_change(&p.task, &p.on_event) }
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
        let on_event = p.on_event.clone();
        Callback::from(move |()| {
            let text = div_ref
                .get()
                .expect("validated while div_ref is not attached to an html element")
                .text_content()
                .expect("div_ref has no text_content");
            if text != initial_title {
                on_event.emit(EventData::SetTitle(text));
            }
        })
    };

    html! {
        <div
            ref={div_ref}
            class="flex-fill d-flex align-items-center"
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

fn button_done_change(t: &Task, on_event: &Callback<EventData>) -> Html {
    let icon_class = match t.is_done {
        true => "bi-arrow-counterclockwise",
        false => "bi-check-lg",
    };
    let aria_label = match t.is_done {
        true => "Mark undone",
        false => "Mark done",
    };
    let currently_done = t.is_done;
    html! {
        <button
            type="button"
            class={ classes!("btn", "bi-btn", icon_class, "ps-2") }
            title={ aria_label }
            onclick={ on_event.reform(move |_| EventData::SetDone(!currently_done)) }
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

fn timeset_button(input_ref: NodeRef, current_date: &Option<Time>, label: &'static str, icon: &'static str, callback: &Callback<Option<Time>>) -> Html {
    let on_button_click = {
        let input_ref = input_ref.clone();
        Callback::from(move |_| {
            let input = input_ref.cast::<web_sys::HtmlInputElement>().expect("input is not an html element");
            show_picker(&input);
        })
    };
    let on_input = callback.reform(|e: web_sys::InputEvent| {
        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
        let date = chrono::NaiveDateTime::parse_from_str(&input.value(), "%Y-%m-%dT%H:%M").expect("datepicker value not in expected format");
        let timezone = local_timezone();
        while date.and_local_timezone(timezone) == chrono::LocalResult::None {
            date.checked_sub_signed(chrono::Duration::minutes(1)).expect("overflow while looking for a date that exists in local timezone");
        }
        let date = date.and_local_timezone(timezone).earliest().unwrap();
        let date = date.with_timezone(&chrono::Utc);
        Some(date)
    });
    let current_date = current_date
        .map(|t| t.with_timezone(&local_timezone()));
    let timeset_label = current_date.map(|d| {
        let remaining = d.signed_duration_since(chrono::Utc::now());
        match remaining {
            r if r > chrono::Duration::days(365) => format!("{}", d.year()),
            r if r > chrono::Duration::days(1) => format!("{}/{}", d.month(), d.day()),
            r if r > chrono::Duration::seconds(0) => format!("{}h{}", d.hour(), d.minute()),
            _ => "past".to_string(),
        }
    }).map(|l| html! {
        <span class="timeset-label rounded-pill">
            { l }
        </span>
    });
    let start_value = current_date.map(|d| format!("{:04}-{:02}-{:02}T{:02}:{:02}", d.year(), d.month(), d.day(), d.hour(), d.minute()));
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
    timeset_button(input_ref, &p.task.scheduled_for, "Schedule for", "bi-alarm", &p.on_event.reform(EventData::ScheduleFor))
}

#[function_component(ButtonBlockedUntil)]
fn button_blocked_until(p: &TaskListItemProps) -> Html {
    let input_ref = use_node_ref();
    timeset_button(input_ref, &p.task.blocked_until, "Blocked until", "bi-hourglass-split", &p.on_event.reform(EventData::BlockedUntil))
}
