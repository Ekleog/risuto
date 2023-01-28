mod action;
mod auth;
mod db;
mod error;
mod event;
mod query;
mod search;
mod tag;
mod task;
mod user;

pub use action::Action;
pub use auth::{AuthInfo, AuthToken, NewSession};
use chrono::Datelike;
pub use db::Db;
pub use error::Error;
pub use event::{Event, EventData, EventId, OrderId};
pub use query::{Query, TimeQuery};
pub use search::{Order, OrderType, Search, SearchId};
pub use tag::{Tag, TagId};
pub use task::{Task, TaskId};
pub use user::{NewUser, User, UserId};

pub use uuid::{uuid, Uuid};
pub type Time = chrono::DateTime<chrono::Utc>;

pub const STUB_UUID: Uuid = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");

// picked with a totally fair dice roll
const UUID_TODAY: Uuid = uuid!("70DA1aaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
const UUID_UNTAGGED: Uuid = uuid!("07A66EDa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FeedMessage {
    Pong, // TODO: this should be replaced with axum::extract::ws::Message::{Ping,Pong}
    Action(Action),
}

/// Helper function to easily know whether a string is valid to send to the API
pub fn validate_string(s: &str) -> Result<(), Error> {
    if s.chars().any(|c| c == '\0') {
        Err(Error::NullByteInString(String::from(s)))
    } else {
        Ok(())
    }
}

/// Helper function to easily know whether a timestamp is valid to send to the API
pub fn validate_time(s: &Time) -> Result<(), Error> {
    let year = s.year();
    if year < -4000 || year > 200000 {
        Err(Error::InvalidTime(*s))
    } else {
        Ok(())
    }
}

// See https://github.com/chronotope/chrono/issues/948
pub fn midnight_on<Tz>(date: chrono::NaiveDate, tz: &Tz) -> chrono::DateTime<Tz>
where
    Tz: Clone + std::fmt::Debug + chrono::TimeZone,
{
    let base = chrono::NaiveTime::MIN;
    for multiple in 0..=24 {
        let start_time = base + chrono::Duration::minutes(multiple * 15);
        match date.and_time(start_time).and_local_timezone(tz.clone()) {
            chrono::LocalResult::None => continue,
            chrono::LocalResult::Single(dt) => return dt,
            chrono::LocalResult::Ambiguous(dt1, dt2) => {
                if dt1.naive_utc() < dt2.naive_utc() {
                    return dt1;
                } else {
                    return dt2;
                }
            }
        }
    }

    panic!(
        "Unable to calculate start time for date {} and time zone {:?}",
        date, tz
    )
}
