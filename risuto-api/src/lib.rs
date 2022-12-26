mod auth;
mod comment;
mod db;
mod event;
mod query;
mod search;
mod tag;
mod task;
mod user;

pub use auth::{AuthInfo, AuthToken, NewSession};
pub use comment::Comment;
pub use db::{Db, DbDump};
pub use event::{Event, EventData, EventId};
pub use query::{Query, QueryBind, SqlQuery};
pub use tag::{Tag, TagId};
pub use task::{Task, TaskId, TaskInTag};
pub use user::{User, UserId};

pub use uuid::{uuid, Uuid};
pub type Time = chrono::DateTime<chrono::Utc>;

pub const STUB_UUID: Uuid = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FeedMessage {
    Pong,
    NewEvent(Event),
}
