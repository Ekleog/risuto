mod action;
mod auth;
mod db;
mod event;
mod query;
mod search;
mod tag;
mod task;
mod user;

pub use action::Action;
pub use auth::{AuthInfo, AuthToken, NewSession};
pub use db::Db;
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
    Pong,
    Action(Action),
}
