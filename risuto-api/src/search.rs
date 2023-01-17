use crate::{uuid, OrderId, Query, Tag, TagId, TimeQuery, Uuid, STUB_UUID};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct SearchId(pub Uuid);

impl SearchId {
    pub fn stub() -> SearchId {
        SearchId(STUB_UUID)
    }

    pub fn today() -> SearchId {
        SearchId(uuid!("70DA1aaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")) // TODO: merge with risuto-api
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum Order {
    Custom(OrderId),
    Tag(TagId),
    CreationDate(OrderType),
    LastEventDate(OrderType),
    ScheduledFor(OrderType),
    BlockedUntil(OrderType),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum OrderType {
    Asc,
    Desc,
}
