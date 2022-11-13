use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};

pub use uuid::Uuid;
pub type Time = chrono::DateTime<Utc>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UserId(pub Uuid);

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TagId(pub Uuid);

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Tag {
    pub owner: UserId,
    pub name: String,
    pub archived: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TaskId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct EventId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Event {
    pub id: EventId,
    pub owner: UserId,
    pub date: Time,

    pub contents: EventType,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DbDump {
    pub owner: UserId,
    pub users: HashMap<UserId, User>,
    pub tags: HashMap<TagId, Tag>,
    pub tasks: HashMap<TaskId, Task>,
}

impl Task {
    pub fn refresh_metadata(&mut self) {
        self.current_title = self.initial_title.clone();
        for evts in self.events.values() {
            if evts.len() > 1 {
                tracing::warn!(
                    num_evts = evts.len(),
                    "multiple events for task at same timestamp"
                )
            }
            for e in evts {
                match &e.contents {
                    EventType::SetTitle(title) => self.current_title = title.clone(),
                    EventType::Complete => self.is_done = true,
                    EventType::Reopen => self.is_done = false,
                    EventType::Archive => self.is_archived = true,
                    EventType::Unarchive => self.is_archived = false,
                    EventType::Schedule(time) => self.scheduled_for = *time,
                    EventType::AddDepBeforeSelf(task) => {
                        self.deps_before_self.insert(*task);
                    }
                    EventType::AddDepAfterSelf(task) => {
                        self.deps_after_self.insert(*task);
                    }
                    EventType::RmDepBeforeSelf(task) => {
                        self.deps_before_self.remove(task);
                    }
                    EventType::RmDepAfterSelf(task) => {
                        self.deps_after_self.remove(task);
                    }
                    EventType::AddTag { tag, prio } => {
                        self.current_tags.insert(*tag, *prio);
                    }
                    EventType::RmTag(tag) => {
                        self.current_tags.remove(tag);
                    }
                    EventType::AddComment(txt) => {
                        let mut edits = BTreeMap::new();
                        edits.insert(e.date, vec![txt.clone()]);
                        self.current_comments
                            .entry(e.date)
                            .or_insert(Vec::new())
                            .push(edits);
                    }
                    EventType::EditComment(evt, txt) => {
                        if let Some((id, evt)) = self
                            .events
                            .values()
                            .flat_map(|v| {
                                v.iter()
                                    .filter(|e| matches!(e.contents, EventType::AddComment(_)))
                                    .enumerate()
                            })
                            .find(|(_, e)| &e.id == evt)
                        {
                            self.current_comments.get_mut(&evt.date).unwrap()[id]
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(txt.clone());
                        }
                    }
                }
            }
        }
    }
}
