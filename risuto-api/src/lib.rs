use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};

pub use uuid::Uuid;
pub type Time = chrono::DateTime<Utc>;

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
pub struct UserId(pub Uuid);

#[derive(serde::Serialize)]
pub struct User {
    pub name: String,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
pub struct TagId(pub Uuid);

#[derive(serde::Serialize)]
pub struct Tag {
    pub owner: UserId,
    pub name: String,
    pub archived: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
pub struct TaskId(pub Uuid);

#[derive(serde::Serialize)]
pub struct Task {
    pub owner: UserId,
    pub date: Time,

    pub initial_title: String,
    pub current_title: String,

    pub is_done: bool,
    pub is_archived: bool,
    pub scheduled_for: Option<Time>,
    pub current_tags: HashMap<TagId, i64>,

    pub deps_before_self: HashSet<TaskId>,
    pub deps_after_self: HashSet<TaskId>,

    /// List of comments in chronological order, with for each comment each edit in chronological order
    pub current_comments: BTreeMap<Time, Vec<BTreeMap<Time, Vec<String>>>>,

    pub events: BTreeMap<Time, Vec<Event>>,
}

#[derive(Clone, Copy, Eq, PartialEq, serde::Serialize)]
pub struct EventId(pub Uuid);

#[derive(serde::Serialize)]
pub struct Event {
    pub id: EventId,
    pub owner: UserId,
    pub date: Time,

    pub contents: EventType,
}

#[derive(serde::Serialize)]
pub enum EventType {
    SetTitle(String),
    Complete,
    Reopen,
    Archive,
    Unarchive,
    Schedule(Option<Time>),
    AddDepBeforeSelf(TaskId),
    AddDepAfterSelf(TaskId),
    RmDepBeforeSelf(TaskId),
    RmDepAfterSelf(TaskId),
    AddTag { tag: TagId, prio: i64 },
    RmTag(TagId),
    AddComment(String),
    EditComment(EventId, String),
}

#[derive(serde::Serialize)]
pub struct DbDump {
    pub users: HashMap<UserId, User>,
    pub tags: HashMap<TagId, Tag>,
    pub tasks: HashMap<TaskId, Task>,
}
