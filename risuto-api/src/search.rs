use crate::{OrderId, Query, Tag, TagId, TimeQuery, Uuid, STUB_UUID, UUID_UNTAGGED, UUID_TODAY};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct SearchId(pub Uuid);

impl SearchId {
    pub fn stub() -> SearchId {
        SearchId(STUB_UUID)
    }

    pub fn today() -> SearchId {
        SearchId(UUID_TODAY)
    }

    pub fn untagged() -> SearchId {
        SearchId(UUID_UNTAGGED)
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
            id: SearchId::untagged(),
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

    pub fn stub_for_query(filter: Query) -> Search {
        Search {
            id: SearchId::stub(),
            name: String::from("stub"),
            filter,
            order: Order::Custom(OrderId::stub()),
            priority: 0,
        }
    }

    pub fn stub_for_query_order(filter: Query, order: Order) -> Search {
        Search {
            id: SearchId::stub(),
            name: String::from("stub"),
            filter,
            order,
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
