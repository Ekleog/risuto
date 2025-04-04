use std::{str::FromStr, sync::Arc};

use risuto_client::{
    api::{Event, EventData, Order, Query, Search, Tag, TaskId, UserId},
    DbDump, Task,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "
    export function show_picker(elt) {
        elt.showPicker();
    }
    export function get_timezone() {
        return Intl.DateTimeFormat().resolvedOptions().timeZone;
    }
")]
extern "C" {
    // TODO: remove once https://github.com/rustwasm/wasm-bindgen/pull/3215 gets released
    pub fn show_picker(elt: &web_sys::HtmlInputElement);
    fn get_timezone() -> String;
}

pub fn local_tz() -> chrono_tz::Tz {
    chrono_tz::Tz::from_str(&get_timezone()).expect("host js timezone is not in chrono-tz database")
}

pub fn sort_tags<'a, T, F>(current_user: &UserId, tags: &mut [T], get_tag: F)
where
    F: for<'b> Fn(&'b T) -> &'a Tag,
{
    tags.sort_unstable_by_key(|t| {
        let t = get_tag(t);
        let is_owner_me = t.owner_id == *current_user;
        let name = t.name.clone();
        let id = t.id;
        (!is_owner_me, name, id)
    });
}

pub fn compute_reordering_events(
    owner: UserId,
    search: &Search,
    task: TaskId,
    index: usize,
    into_backlog: bool,
    into: &Vec<Arc<Task>>,
) -> Vec<Event> {
    macro_rules! evt {
        ( $task:expr, $prio:expr ) => {
            match &search.order {
                Order::Tag(tag) => Event::now(
                    owner,
                    $task,
                    EventData::AddTag {
                        tag: tag.clone(),
                        prio: $prio,
                        backlog: into_backlog,
                    },
                ),
                Order::Custom(order) => Event::now(
                    owner,
                    $task,
                    EventData::SetOrder {
                        order: order.clone(),
                        prio: $prio,
                    },
                ),
                _ => panic!("attempted reordering in a non-reorderable search"),
            }
        };
    }
    macro_rules! prio {
        ($task:expr) => {
            match &search.order {
                Order::Tag(tag) => $task
                    .prio_tag(tag)
                    .expect("computing events reordering with task not in tag"),
                Order::Custom(order) => $task
                    .prio_order(order)
                    .expect("computing events reordering with task not in search"),
                _ => panic!("attempted reordering in a non-reorderable search"),
            }
        };
    }
    // this value was taken after intense finger-based wind-speed-taking
    // basically we can add 2^(64-40) items at the beginning or end this way, and intersperse 40 items in-between other items, all without a redistribution
    const SPACING: i64 = 1 << 40;

    if into.len() == 0 {
        // Easy case: inserting into an empty list
        return vec![evt!(task, 0)];
    }

    if index == 0 {
        // Inserting in the first position
        let first_prio = prio!(into[0]);
        let subtract = match first_prio > i64::MIN + SPACING {
            true => SPACING,
            false => (first_prio - i64::MIN) / 2,
        };
        if subtract > 0 {
            return vec![evt!(task, first_prio - subtract)];
        }
    } else if index == into.len() {
        // Inserting in the last position
        let last_prio = prio!(into[index - 1]);
        let add = match last_prio < i64::MAX - SPACING {
            true => SPACING,
            false => (i64::MAX - last_prio) / 2,
        };
        if add > 0 {
            return vec![evt!(task, last_prio + add)];
        }
    } else {
        // Inserting in-between two elements
        use num::integer::Average;
        let prio_before = prio!(into[index - 1]);
        let prio_after = prio!(into[index]);
        let new_prio = prio_before.average_floor(&prio_after); // no overflow here
        if new_prio != prio_before {
            return vec![evt!(task, new_prio)];
        }
    }

    // Do a full redistribute
    // TODO: maybe we could only partially redistribute? not sure whether that'd actually be better...
    into[..index]
        .iter()
        .enumerate()
        .map(|(i, t)| evt!(t.id, (i as i64).checked_mul(SPACING).unwrap()))
        .chain(std::iter::once(evt!(
            task,
            (index as i64).checked_mul(SPACING).unwrap()
        )))
        .chain(into[index..].iter().enumerate().map(|(i, t)| {
            evt!(
                t.id,
                (index as i64 + 1 + i as i64).checked_mul(SPACING).unwrap()
            )
        }))
        .collect()
}

pub fn parse_tag_changes(db: &DbDump, task_id: TaskId, mut title: String) -> (String, Vec<Event>) {
    let mut res = Vec::new();
    loop {
        title.truncate(title.trim_end().len());

        if let Some(i) = title.rfind(" -") {
            let tag_start = i + " -".len();
            if let Some(t) = title.get(tag_start..).and_then(|t| db.tag_id(t)) {
                res.push(Event::now(db.owner, task_id, EventData::RmTag(t)));
                title.truncate(i);
                continue;
            }
        }

        if let Some(i) = title.rfind(" +") {
            let tag_start = i + " +".len();
            if let Some(tag) = title.get(tag_start..).and_then(|t| db.tag(t)) {
                let search = Search::stub_for_query_order(
                    Query::Tag {
                        tag: tag.id,
                        backlog: Some(false),
                    },
                    Order::Tag(tag.id),
                );
                let tasks = db.search(&search).expect("Infallible search failed");
                res.extend(compute_reordering_events(
                    db.owner, &search, task_id, 0, false, &tasks,
                ));
                title.truncate(i);
                continue;
            }
        }

        return (title, res);
    }
}
