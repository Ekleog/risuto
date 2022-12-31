use crate::{TagId, Time};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TimeQuery {
    Absolute(Time),

    /// Offset today().and_hms(0, 0, 0) by day_offset days
    DayRelative {
        timezone: chrono_tz::Tz,
        day_offset: i64,
    },
}

impl TimeQuery {
    pub fn eval_now(&self) -> Option<Time> {
        match self {
            TimeQuery::Absolute(t) => Some(*t),
            TimeQuery::DayRelative {
                timezone,
                day_offset,
            } => {
                // TODO: for safety, see (currently open) https://github.com/chronotope/chrono/pull/927
                let date = chrono::Utc::now().date_naive();
                let date = match *day_offset >= 0 {
                    true => date.checked_add_days(chrono::naive::Days::new(*day_offset as u64)),
                    false => date.checked_sub_days(chrono::naive::Days::new(-day_offset as u64)),
                };
                date.and_then(|d| d.and_hms_opt(0, 0, 0))
                    .and_then(|d| d.and_local_timezone(*timezone).single())
                    .map(|d| d.with_timezone(&chrono::Utc))
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Query {
    Any(Vec<Query>),
    All(Vec<Query>),
    Not(Box<Query>),
    Archived(bool),
    Done(bool),
    Tag { tag: TagId, backlog: Option<bool> },
    Untagged(bool),
    ScheduledForBefore(TimeQuery),
    ScheduledForAfter(TimeQuery),
    BlockedUntilAtMost(TimeQuery),
    BlockedUntilAtLeast(TimeQuery),
    Phrase(String), // full-text search of one contiguous word vec
}

impl Query {
    pub fn tag(tag: TagId) -> Query {
        Query::Tag { tag, backlog: None }
    }
}
