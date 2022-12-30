use crate::{TagId, Time};

#[derive(Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Query {
    Any(Vec<Query>),
    All(Vec<Query>),
    Not(Box<Query>),
    Archived(bool),
    Done(bool),
    Tag { tag: TagId, backlog: Option<bool> },
    Untagged(bool),
    ScheduledForBefore(Time),
    ScheduledForAfter(Time),
    BlockedUntilAtMost(Time),
    BlockedUntilAtLeast(Time),
    Phrase(String), // full-text search of one contiguous word vec
}

impl Query {
    pub fn tag(tag: TagId) -> Query {
        Query::Tag { tag, backlog: None }
    }
}
