use std::{cmp::Reverse, sync::Arc};

use crate::{
    api::{OrderId, Query, TagId},
    Task,
};

pub struct Search {
    pub filter: Query,
    pub order: Order,
}

pub enum Order {
    Custom(OrderId),
    Tag(TagId),
    CreationDate(OrderType),
    LastEventDate(OrderType),
    ScheduledFor(OrderType),
    BlockedUntil(OrderType),
}

pub enum OrderType {
    Asc,
    Desc,
}

impl Order {
    /// Panics if any task is not actually in this tag
    pub fn sort(&self, tasks: &mut [Arc<Task>]) {
        match self {
            Order::Custom(o) => {
                tasks.sort_unstable_by_key(|t| t.orders.get(o).copied().unwrap_or(0))
            }
            Order::Tag(tag) => tasks.sort_unstable_by_key(|t| {
                let tag_data = t
                    .current_tags
                    .get(tag)
                    .expect("task passed to Order::Tag(t)::sort is not actually in the tag");
                (tag_data.priority, t.id)
            }),
            Order::CreationDate(OrderType::Asc) => tasks.sort_unstable_by_key(|t| t.date),
            Order::CreationDate(OrderType::Desc) => tasks.sort_unstable_by_key(|t| Reverse(t.date)),
            Order::LastEventDate(OrderType::Asc) => tasks.sort_unstable_by_key(|t| {
                t.events
                    .last_key_value()
                    .map(|(d, _)| d.clone())
                    .unwrap_or(t.date)
            }),
            Order::LastEventDate(OrderType::Desc) => tasks.sort_unstable_by_key(|t| {
                Reverse(
                    t.events
                        .last_key_value()
                        .map(|(d, _)| d.clone())
                        .unwrap_or(t.date),
                )
            }),
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
