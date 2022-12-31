mod auth;
mod db;
mod event;
mod query;
mod tag;
mod task;
mod user;

pub use auth::{AuthInfo, AuthToken, NewSession};
pub use db::Db;
pub use event::{Event, EventData, EventId, OrderId};
pub use query::{Query, TimeQuery};
pub use tag::{Tag, TagId};
pub use task::{Task, TaskId};
pub use user::{User, UserId};

pub use uuid::{uuid, Uuid};
pub type Time = chrono::DateTime<chrono::Utc>;

pub const STUB_UUID: Uuid = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FeedMessage {
    Pong,
    NewEvent(Event),
}
