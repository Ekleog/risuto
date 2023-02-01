use std::{cmp::Reverse, sync::Arc};

use crate::{
    api::{Order, OrderType},
    Task,
};

pub trait OrderExt {
    fn sort(&self, tasks: &mut [Arc<Task>]);
}

impl OrderExt for Order {
    /// Panics if any task is not actually in this tag
    fn sort(&self, tasks: &mut [Arc<Task>]) {
        match self {
            Order::Custom(o) => {
                // Put any unordered task at the top of the list
                tasks.sort_unstable_by_key(|t| {
                    let prio = t.orders.get(o).copied().unwrap_or(i64::MIN);
                    (t.is_done, prio, Reverse(t.date), t.id)
                })
            }
            Order::Tag(tag) => tasks.sort_unstable_by_key(|t| {
                // Tasks not actually in the tag get pushed to the bottom of the list
                let tag_data = match t.current_tags.get(tag) {
                    Some(tag_data) => tag_data,
                    None => return (3, 0, Reverse(t.date), t.id),
                };
                let category = match (tag_data.backlog, t.is_done) {
                    (false, false) => 0,
                    (false, true) => 1,
                    (true, _) => 2,
                    // 3 is used above for tasks not actually in this tag
                };
                (category, tag_data.priority, Reverse(t.date), t.id)
            }),
            Order::CreationDate(OrderType::Asc) => tasks.sort_unstable_by_key(|t| t.date),
            Order::CreationDate(OrderType::Desc) => tasks.sort_unstable_by_key(|t| Reverse(t.date)),
            Order::LastEventDate(OrderType::Asc) => {
                tasks.sort_unstable_by_key(|t| t.last_event_time())
            }
            Order::LastEventDate(OrderType::Desc) => {
                tasks.sort_unstable_by_key(|t| Reverse(t.last_event_time()))
            }
            Order::ScheduledFor(OrderType::Asc) => tasks.sort_unstable_by_key(|t| t.scheduled_for),
            Order::ScheduledFor(OrderType::Desc) => {
                tasks.sort_unstable_by_key(|t| Reverse(t.scheduled_for))
            }
            Order::BlockedUntil(OrderType::Asc) => tasks.sort_unstable_by_key(|t| t.blocked_until),
            Order::BlockedUntil(OrderType::Desc) => {
                tasks.sort_unstable_by_key(|t| Reverse(t.blocked_until))
            }
        }
    }
}
