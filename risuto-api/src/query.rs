use crate::{Error, TagId, Time};

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    arbitrary::Arbitrary,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum TimeQuery {
    Absolute(#[generator(bolero::gen_arbitrary())] Time),

    /// Offset today().and_hms(0, 0, 0) by day_offset days
    DayRelative {
        #[generator(bolero::gen_arbitrary())]
        timezone: chrono_tz::Tz,
        day_offset: i64,
    },
}

impl TimeQuery {
    pub fn validate(&self) -> Result<(), Error> {
        crate::validate_time(&self.eval_now()?)
    }

    pub fn eval_now(&self) -> Result<Time, Error> {
        match self {
            TimeQuery::Absolute(t) => Ok(*t),
            TimeQuery::DayRelative {
                timezone,
                day_offset,
            } => {
                // TODO: for safety, see (currently open) https://github.com/chronotope/chrono/pull/927
                let date = chrono::Utc::now().date_naive();
                let date = match *day_offset >= 0 {
                    true => date.checked_add_days(chrono::naive::Days::new(*day_offset as u64)),
                    false => day_offset
                        .checked_neg()
                        .map(|d| chrono::naive::Days::new(d as u64))
                        .and_then(|offset| date.checked_sub_days(offset)),
                };
                date.map(|d| crate::midnight_on(d, timezone))
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .ok_or(Error::IntegerOutOfRange(*day_offset))
            }
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    arbitrary::Arbitrary,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum Query {
    // TODO: use TypeGenerator after fixing bolero's handling of recursive structs
    // (right now the fuzzer rightfully finds a stack overflow in... bolero's TypeGenerator)
    Any(#[generator(bolero::gen_arbitrary())] Vec<Query>),
    All(#[generator(bolero::gen_arbitrary())] Vec<Query>),
    // TODO: the attr below should not be necessary, but see https://github.com/rust-lang/rust/issues/48214#issuecomment-1374372954
    Not(#[generator(bolero::gen_arbitrary())] Box<Query>),
    Archived(bool),
    Done(bool),
    Tag { tag: TagId, backlog: Option<bool> },
    Untagged(bool),
    ScheduledForBefore(TimeQuery),
    ScheduledForAfter(TimeQuery),
    BlockedUntilAtMost(TimeQuery),
    BlockedUntilAtLeast(TimeQuery),
    Phrase(#[generator(bolero::gen_with::<String>().len(0..15usize))] String), // full-text search of one contiguous word vec
}

impl Query {
    pub fn tag(tag: TagId) -> Query {
        Query::Tag { tag, backlog: None }
    }

    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Query::Any(queries) => {
                for q in queries {
                    q.validate()?;
                }
                Ok(())
            }
            Query::All(queries) => {
                for q in queries {
                    q.validate()?;
                }
                Ok(())
            }
            Query::Not(q) => q.validate(),
            Query::Archived(_) => Ok(()),
            Query::Done(_) => Ok(()),
            Query::Tag { tag: _, backlog: _ } => Ok(()),
            Query::Untagged(_) => Ok(()),
            Query::ScheduledForBefore(t) => t.validate(),
            Query::ScheduledForAfter(t) => t.validate(),
            Query::BlockedUntilAtMost(t) => t.validate(),
            Query::BlockedUntilAtLeast(t) => t.validate(),
            Query::Phrase(s) => crate::validate_string(s),
        }
    }
}
