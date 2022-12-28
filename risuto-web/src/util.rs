use std::sync::Arc;

use risuto_api::{AuthInfo, Event, EventData, Tag, TagId, Task, TaskId, UserId};

use crate::TODAY_TAG;

pub fn sort_tags(current_user: &UserId, tags: &mut Vec<(&TagId, &(Tag, AuthInfo))>) {
    tags.sort_unstable_by_key(|(id, t)| {
        // TODO: extract into a freestanding fn and reuse for sorting the tag list below tasks
        let is_tag_today = t.0.name == TODAY_TAG;
        let is_owner_me = t.0.owner == *current_user;
        let name = t.0.name.clone();
        let id = (*id).clone();
        (!is_tag_today, !is_owner_me, name, id)
    });
}

pub fn compute_reordering_events(
    owner: UserId,
    tag: TagId,
    task: TaskId,
    index: usize,
    into_backlog: bool,
    into: &Vec<Arc<Task>>,
) -> Vec<Event> {
    macro_rules! evt {
        ( $task:expr, $prio:expr ) => {
            Event::now(
                owner,
                $task,
                EventData::AddTag {
                    tag,
                    prio: $prio,
                    backlog: into_backlog,
                },
            )
        };
    }
    macro_rules! prio {
        ($task:expr) => {
            $task
                .prio(&tag)
                .expect("computing events reordering with task not in tag")
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