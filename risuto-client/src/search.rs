use std::{cmp::Reverse, sync::Arc};

use crate::{
    api::{OrderId, Query, Tag, TagId},
    Task,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Search {
    pub name: String,
    pub filter: Query,
    pub order: Order,
}

impl Search {
    pub fn untagged() -> Search {
        Search {
            name: String::from("Untagged"),
            filter: Query::Untagged(true),
            order: Order::Custom(OrderId::untagged()),
        }
    }

    pub fn for_tag(t: &Tag) -> Search {
        Search {
            name: format!("#{}", t.name),
            filter: Query::tag(t.id),
            order: Order::Tag(t.id),
        }
    }

    pub fn for_tag_full(tag: &Tag, backlog: bool) -> Search {
        Search {
            name: format!(
                "#{} ({})",
                tag.name,
                if backlog { "backlog" } else { "not-backlog " }
            ),
            filter: Query::Tag {
                tag: tag.id,
                backlog: Some(backlog),
            },
            order: Order::Tag(tag.id),
        }
    }

    pub fn is_order_tag(&self) -> Option<TagId> {
        match self.order {
            Order::Tag(t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Order {
    Custom(OrderId),
    Tag(TagId),
    CreationDate(OrderType),
    LastEventDate(OrderType),
    ScheduledFor(OrderType),
    BlockedUntil(OrderType),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderType {
    Asc,
    Desc,
}

impl Order {
    /// Panics if any task is not actually in this tag
    pub fn sort(&self, tasks: &mut [Arc<Task>]) {
        match self {
            Order::Custom(o) => {
                // Put any unordered task at the top of the list
                tasks.sort_unstable_by_key(|t| {
                    (
                        t.orders.get(o).copied().unwrap_or(i64::MIN),
                        Reverse(t.date),
                    )
                })
            }
            Order::Tag(tag) => tasks.sort_unstable_by_key(|t| {
                let tag_data = t
                    .current_tags
                    .get(tag)
                    .expect("task passed to Order::Tag(t)::sort is not actually in the tag");
                let category = match (tag_data.backlog, t.is_done) {
                    (false, false) => 0,
                    (false, true) => 1,
                    (true, _) => 2,
                };
                (category, tag_data.priority, t.id)
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
