use anyhow::Context;
use arrayvec::ArrayVec;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};

pub use uuid::{uuid, Uuid};
pub type Time = chrono::DateTime<Utc>;

pub const STUB_UUID: Uuid = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");

#[derive(serde::Deserialize, serde::Serialize)]
pub struct NewSession {
    pub user: String,
    pub password: String,
    pub device: String,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AuthToken(pub Uuid);

impl AuthToken {
    pub fn stub() -> AuthToken {
        AuthToken(STUB_UUID)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn stub() -> UserId {
        UserId(STUB_UUID)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub name: String,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct TagId(pub Uuid);

impl TagId {
    pub fn stub() -> TagId {
        TagId(STUB_UUID)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Tag {
    pub owner: UserId,
    pub name: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TaskInTag {
    // higher is lower in the tag list
    pub priority: i64,

    /// if true, this task is in this tag's backlog
    pub backlog: bool,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
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
    pub current_tags: HashMap<TagId, TaskInTag>,

    pub deps_before_self: HashSet<TaskId>,
    pub deps_after_self: HashSet<TaskId>,

    /// List of comments in chronological order, with for each comment each edit in chronological order
    pub current_comments: BTreeMap<Time, Vec<BTreeMap<Time, Vec<String>>>>,

    pub events: BTreeMap<Time, Vec<Event>>,
}

impl Task {
    pub fn prio(&self, tag: &TagId) -> Option<i64> {
        self.current_tags
            .get(tag)
            .map(|t| t.priority)
    }

    pub fn add_event(&mut self, e: Event) {
        self.events.entry(e.date).or_insert(Vec::new()).push(e);
    }

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
                    EventType::SetDone(now_done) => self.is_done = *now_done,
                    EventType::SetArchived(now_archived) => self.is_archived = *now_archived,
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
                    EventType::AddTag { tag, prio, backlog } => {
                        self.current_tags.insert(
                            *tag,
                            TaskInTag {
                                priority: *prio,
                                backlog: *backlog,
                            },
                        );
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
    SetDone(bool),
    SetArchived(bool),
    Schedule(Option<Time>),
    AddDepBeforeSelf(TaskId),
    AddDepAfterSelf(TaskId),
    RmDepBeforeSelf(TaskId),
    RmDepAfterSelf(TaskId),
    AddTag {
        tag: TagId,
        prio: i64,
        backlog: bool,
    },
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

impl DbDump {
    pub fn stub() -> DbDump {
        DbDump {
            owner: UserId::stub(),
            users: HashMap::new(),
            tags: HashMap::new(),
            tasks: HashMap::new(),
        }
    }
}

pub struct AuthInfo {
    pub can_read: bool,
    pub can_edit: bool,
    pub can_triage: bool,
    pub can_relabel_to_any: bool,
    pub can_comment: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum NewEventContents {
    SetTitle {
        task: TaskId,
        title: String,
    },
    SetDone {
        task: TaskId,
        now_done: bool,
    },
    SetArchived {
        task: TaskId,
        now_archived: bool,
    },
    Schedule {
        task: TaskId,
        scheduled_date: Option<Time>,
    },
    AddDep {
        first: TaskId,
        then: TaskId,
    },
    RmDep {
        first: TaskId,
        then: TaskId,
    },
    AddTag {
        task: TaskId,
        tag: TagId,
        prio: i64,
        backlog: bool,
    },
    RmTag {
        task: TaskId,
        tag: TagId,
    },
    AddComment {
        task: TaskId,
        text: String,
    },
    EditComment {
        untrusted_task: TaskId,
        comment: EventId,
        text: String,
    },
}

#[async_trait]
pub trait Db {
    async fn auth_info_for(&mut self, t: TaskId) -> anyhow::Result<AuthInfo>;
    async fn list_tags_for(&mut self, t: TaskId) -> anyhow::Result<Vec<TagId>>;
    async fn get_comment_owner(&mut self, e: EventId) -> anyhow::Result<UserId>;
    async fn get_task_for_comment(&mut self, comment: EventId) -> anyhow::Result<TaskId>;
    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool>;
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct NewEvent {
    pub id: EventId,
    pub owner: UserId,
    pub date: Time,
    pub contents: NewEventContents,
}

impl NewEvent {
    pub fn now(owner: UserId, contents: NewEventContents) -> NewEvent {
        NewEvent {
            id: EventId(Uuid::new_v4()),
            owner,
            date: Utc::now(),
            contents,
        }
    }

    pub async fn make_untrusted_trusted<D: Db>(&mut self, db: &mut D) -> anyhow::Result<()> {
        match self.contents {
            NewEventContents::EditComment {
                ref mut untrusted_task,
                comment,
                ..
            } => {
                let real_task = db
                    .get_task_for_comment(comment)
                    .await
                    .with_context(|| format!("getting task for comment {:?}", comment))?;
                if *untrusted_task != real_task {
                    *untrusted_task = real_task;
                    tracing::warn!(event=?self, ?real_task, "got event that lied on its attached task");
                }
            }
            _ => (),
        }
        Ok(())
    }

    pub fn untrusted_task_event_list(&self) -> ArrayVec<(TaskId, Event), 2> {
        let mut res = ArrayVec::new();
        macro_rules! event {
            ($task_id:expr, $type:expr) => {
                res.push((
                    $task_id,
                    Event {
                        id: self.id,
                        owner: self.owner,
                        date: self.date,
                        contents: $type,
                    },
                ))
            };
        }
        match &self.contents {
            NewEventContents::SetTitle { task, title } => {
                event!(*task, EventType::SetTitle(title.clone()))
            }
            NewEventContents::SetDone { task, now_done } => {
                event!(*task, EventType::SetDone(*now_done))
            }
            NewEventContents::SetArchived { task, now_archived } => {
                event!(*task, EventType::SetArchived(*now_archived))
            }
            NewEventContents::Schedule {
                task,
                scheduled_date,
            } => event!(*task, EventType::Schedule(*scheduled_date)),
            NewEventContents::AddDep { first, then } => {
                event!(*first, EventType::AddDepAfterSelf(*then));
                event!(*then, EventType::AddDepBeforeSelf(*then));
            }
            NewEventContents::RmDep { first, then } => {
                event!(*first, EventType::RmDepAfterSelf(*then));
                event!(*then, EventType::RmDepBeforeSelf(*then));
            }
            NewEventContents::AddTag {
                task,
                tag,
                prio,
                backlog,
            } => event!(
                *task,
                EventType::AddTag {
                    tag: *tag,
                    prio: *prio,
                    backlog: *backlog
                }
            ),
            NewEventContents::RmTag { task, tag } => event!(*task, EventType::RmTag(*tag)),
            NewEventContents::AddComment { task, text } => {
                event!(*task, EventType::AddComment(text.clone()))
            }
            NewEventContents::EditComment {
                untrusted_task,
                comment,
                text,
            } => event!(
                *untrusted_task,
                EventType::EditComment(*comment, text.clone())
            ),
        }
        res
    }

    /// Takes AuthInfo as the authorization status for the user for self.task
    // TODO: refactor as an async fn that takes in 4 async callbacks
    pub async fn is_authorized<D: Db>(&self, db: &mut D) -> anyhow::Result<bool> {
        macro_rules! auth {
            ($t:expr) => {{
                let t = $t;
                db.auth_info_for(t)
                    .await
                    .with_context(|| format!("fetching auth info for task {:?}", t))?
            }};
        }
        Ok(match self.contents {
            NewEventContents::SetTitle { task, .. } => auth!(task).can_edit,
            NewEventContents::SetDone { task, .. }
            | NewEventContents::SetArchived { task, .. }
            | NewEventContents::Schedule { task, .. } => auth!(task).can_triage,
            NewEventContents::AddDep { first, then } | NewEventContents::RmDep { first, then } => {
                auth!(first).can_triage && auth!(then).can_triage
            }
            NewEventContents::AddTag { task, tag, .. } => {
                let auth = auth!(task);
                auth.can_relabel_to_any
                    || (auth.can_triage
                        && db
                            .list_tags_for(task)
                            .await
                            .with_context(|| format!("listing tags for task {:?}", task))?
                            .contains(&tag))
            }
            NewEventContents::RmTag { task, .. } => auth!(task).can_relabel_to_any,
            NewEventContents::AddComment { task, .. } => auth!(task).can_comment,
            NewEventContents::EditComment { comment, .. } => {
                let is_comment_owner = self.owner
                    == db
                        .get_comment_owner(comment)
                        .await
                        .with_context(|| format!("getting owner of comment {:?}", comment))?;
                let task = db
                    .get_task_for_comment(comment)
                    .await
                    .with_context(|| format!("getting task for comment {:?}", comment))?;
                let is_first_comment =
                    db.is_first_comment(task, comment).await.with_context(|| {
                        format!("checking if comment {:?} is first comment", comment)
                    })?;
                is_comment_owner || (auth!(task).can_edit && is_first_comment)
            }
        })
    }
}
