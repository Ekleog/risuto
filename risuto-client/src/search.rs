use std::{cmp::Reverse, sync::Arc};

use risuto_api::{uuid, Uuid};

use crate::{
    api::{OrderId, Query, Tag, TagId, TimeQuery},
    Task,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchId(pub Uuid);

impl SearchId {
    pub fn stub() -> SearchId {
        SearchId(risuto_api::STUB_UUID)
    }

    pub fn today() -> SearchId {
        SearchId(uuid!("70DA1aaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")) // TODO: merge with risuto-api
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Search {
    pub id: SearchId,
    pub name: String,
    pub filter: Query,
    pub order: Order,
    pub priority: i64,
    // TODO: add "available offline" toggle for queries that also match archived tasks
}

impl Search {
    pub fn untagged() -> Search {
        Search {
            id: SearchId(uuid!("07A66EDa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")), // TODO: merge with risuto-api
            name: String::from("Untagged"),
            filter: Query::Untagged(true),
            order: Order::Custom(OrderId::untagged()),
            priority: 0,
        }
    }

    pub fn today(timezone: chrono_tz::Tz) -> Search {
        Search {
            id: SearchId::today(),
            name: String::from("Today"),
            filter: Query::ScheduledForBefore(TimeQuery::DayRelative {
                timezone,
                day_offset: 1,
            }),
            order: Order::Custom(OrderId::today()),
            priority: 0,
        }
    }

    pub fn for_tag(t: &Tag) -> Search {
        Search {
            id: SearchId(t.id.0),
            name: format!("#{}", t.name),
            filter: Query::tag(t.id),
            order: Order::Tag(t.id),
            priority: 0,
        }
    }

    pub fn for_tag_full(id: SearchId, tag: &Tag, backlog: bool) -> Search {
        Search {
            id,
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
            priority: 0,
        }
    }

    pub fn is_order_tag(&self) -> Option<TagId> {
        match self.order {
            Order::Tag(t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, bolero::generator::TypeGenerator)]
pub enum Order {
    Custom(OrderId),
    Tag(TagId),
    CreationDate(OrderType),
    LastEventDate(OrderType),
    ScheduledFor(OrderType),
    BlockedUntil(OrderType),
}

#[derive(Clone, Debug, Eq, PartialEq, bolero::generator::TypeGenerator)]
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
                    let prio = t.orders.get(o).copied().unwrap_or(i64::MIN);
                    (t.is_done, prio, Reverse(t.date), t.id)
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
