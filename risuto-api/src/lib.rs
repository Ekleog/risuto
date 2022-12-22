use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::Utc;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::BitOr,
};

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
pub struct Comment {
    /// EventId of this comment's creation
    creation_id: EventId,

    /// List of edits in chronological order
    edits: BTreeMap<Time, Vec<String>>,

    /// Set of users who already read this comment
    read: HashSet<UserId>,

    /// Child comments
    children: BTreeMap<Time, Vec<Comment>>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Task {
    pub owner: UserId,
    pub date: Time,

    pub initial_title: String,
    pub current_title: String,

    pub is_done: bool,
    pub is_archived: bool,
    pub blocked_until: Option<Time>,
    pub scheduled_for: Option<Time>,
    pub current_tags: HashMap<TagId, TaskInTag>,

    pub deps_before_self: HashSet<TaskId>,
    pub deps_after_self: HashSet<TaskId>,

    /// List of comments in chronological order
    pub current_comments: BTreeMap<Time, Vec<Comment>>,

    pub events: BTreeMap<Time, Vec<Event>>,
}

fn find_comment<'a>(comments: &'a mut BTreeMap<Time, Vec<Comment>>, creation_id: &EventId) -> Option<&'a mut Comment> {
    for c in comments.values_mut().flat_map(|v| v.iter_mut()) {
        if c.creation_id == *creation_id {
            return Some(c);
        }
        if let Some(res) = find_comment(&mut c.children, &creation_id) {
            return Some(res);
        }
    }
    None
}

impl Task {
    pub fn prio(&self, tag: &TagId) -> Option<i64> {
        self.current_tags.get(tag).map(|t| t.priority)
    }

    pub fn add_event(&mut self, e: Event) {
        let insert_into = self.events.entry(e.date).or_insert(Vec::new());
        if insert_into.iter().find(|evt| **evt == e).is_none() {
            insert_into.push(e);
        }
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
                    EventType::BlockedUntil(time) => self.blocked_until = *time,
                    EventType::ScheduleFor(time) => self.scheduled_for = *time,
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
                    EventType::AddComment { text, parent_id } => {
                        let mut edits = BTreeMap::new();
                        edits.insert(e.date, vec![text.clone()]);
                        let mut read = HashSet::new();
                        read.insert(e.owner);
                        let children = BTreeMap::new();
                        let creation_id = e.id;
                        if let Some(parent) = parent_id.and_then(|p| find_comment(&mut self.current_comments, &p)) {
                            parent.children
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(Comment { creation_id, edits, read, children });
                        } else { // Also add as a top-level comment if the parent could not be found (TODO: log a warning)
                            self.current_comments
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(Comment { creation_id, edits, read, children });
                        }
                    }
                    EventType::EditComment { comment_id, text } => {
                        if let Some(comment) = find_comment(&mut self.current_comments, comment_id) {
                            comment
                                .edits
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(text.clone());
                            comment.read = HashSet::new();
                            comment.read.insert(e.owner);
                        }
                    }
                    EventType::SetEventRead { event_id, now_read } => {
                        if let Some(comment) = find_comment(&mut self.current_comments, event_id) {
                            if *now_read {
                                comment.read.insert(e.owner);
                            } else {
                                comment.read.remove(&e.owner);
                            }
                        } // ignore non-comment events
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
    pub task: TaskId,

    pub contents: EventType,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum EventType {
    SetTitle(String),
    SetDone(bool),
    SetArchived(bool),
    BlockedUntil(Option<Time>),
    ScheduleFor(Option<Time>),
    AddTag {
        tag: TagId,
        prio: i64,
        backlog: bool,
    },
    RmTag(TagId),
    AddComment {
        text: String,
        parent_id: Option<EventId>,
    },
    EditComment {
        text: String,
        comment_id: EventId,
    },
    SetEventRead {
        event_id: EventId,
        now_read: bool,
    },
}

impl Event {
    pub fn now(owner: UserId, task: TaskId, contents: EventType) -> Event {
        Event {
            id: EventId(Uuid::new_v4()),
            owner,
            date: Utc::now(),
            task,
            contents,
        }
    }

    /// Takes AuthInfo as the authorization status for the user for self.task
    pub async fn is_authorized<D: Db>(&self, db: &mut D) -> anyhow::Result<bool> {
        macro_rules! auth {
            ($t:expr) => {{
                let t = $t;
                db.auth_info_for(t)
                    .await
                    .with_context(|| format!("fetching auth info for task {:?}", t))?
            }};
        }
        macro_rules! check_parent_event {
            ($c:expr) => {{
                let (par_owner, par_date, par_task) = db
                    .get_event_info($c)
                    .await
                    .with_context(|| format!("getting info of comment {:?}", $c))?;
                if par_date >= self.date { // TODO: remove this requirement by fixing event insertion into tasks
                    return Ok(false);
                }
                (par_owner, par_date, par_task)
            }};
        }
        Ok(match self.contents {
            EventType::SetTitle { .. } => auth!(self.task).can_edit,
            EventType::SetDone { .. }
            | EventType::SetArchived { .. }
            | EventType::BlockedUntil { .. }
            | EventType::ScheduleFor { .. } => auth!(self.task).can_triage,
            EventType::AddTag { tag, .. } => {
                let auth = auth!(self.task);
                auth.can_relabel_to_any
                    || (auth.can_triage
                        && db
                            .list_tags_for(self.task)
                            .await
                            .with_context(|| format!("listing tags for task {:?}", self.task))?
                            .contains(&tag))
            }
            EventType::RmTag { .. } => auth!(self.task).can_relabel_to_any,
            EventType::AddComment { parent_id, .. } => {
                if let Some(parent_id) = parent_id {
                    check_parent_event!(parent_id);
                }
                auth!(self.task).can_comment
            }
            EventType::EditComment { comment_id, .. } => {
                let (comm_owner, _, comm_task) = check_parent_event!(comment_id);
                let is_comment_owner = self.owner == comm_owner;
                let is_first_comment =
                    db.is_first_comment(comm_task, comment_id).await.with_context(|| {
                        format!("checking if comment {:?} is first comment", comment_id)
                    })?;
                is_comment_owner || (auth!(comm_task).can_edit && is_first_comment)
            }
            EventType::SetEventRead { event_id, .. } => {
                let (_, _, par_task) = check_parent_event!(event_id);
                auth!(par_task).can_read
            }
        })
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DbDump {
    pub owner: UserId,
    pub users: HashMap<UserId, User>,
    pub tags: HashMap<TagId, (Tag, AuthInfo)>,
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

impl DbDump {
    fn get_task_for_event(&mut self, event: EventId) -> anyhow::Result<TaskId> {
        for (task, t) in self.tasks.iter() {
            for evts in t.events.values() {
                for e in evts.iter() {
                    if e.id == event {
                        return Ok(*task);
                    }
                }
            }
        }
        Err(anyhow!(
            "requested task for event {:?} that is not in db",
            event
        ))
    }
}

#[async_trait]
impl Db for DbDump {
    async fn auth_info_for(&mut self, t: TaskId) -> anyhow::Result<AuthInfo> {
        let t = match self.tasks.get(&t) {
            None => {
                return Err(anyhow!(
                    "requested auth info for task {:?} that is not in db",
                    t
                ))
            }
            Some(t) => t,
        };
        let for_task = AuthInfo::all_or_nothing(t.owner == self.owner);
        let mut for_tags = AuthInfo::none();
        for tag in t.current_tags.keys() {
            if let Some((_, auth)) = self.tags.get(&tag) {
                for_tags = for_tags | *auth;
            }
        }
        Ok(for_task | for_tags)
    }

    async fn list_tags_for(&mut self, t: TaskId) -> anyhow::Result<Vec<TagId>> {
        Ok(self
            .tasks
            .get(&t)
            .ok_or_else(|| anyhow!("requested tag listing for task {:?} that is not in db", t))?
            .current_tags
            .keys()
            .copied()
            .collect())
    }

    async fn get_event_info(&mut self, e: EventId) -> anyhow::Result<(UserId, Time, TaskId)> {
        let task_id = self.get_task_for_event(e)?;
        let t = self.tasks.get(&task_id).ok_or_else(|| {
            anyhow!("requested comment owner for event {e:?} for which task {task_id:?} is not in db",)
        })?;
        Ok((t.owner, t.date, task_id))
    }

    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool> {
        Ok(comment
            == self
                .tasks
                .get(&task)
                .ok_or_else(|| {
                    anyhow!(
                        "requested is_first_comment for task {:?} that is not in db",
                        task
                    )
                })?
                .current_comments
                .values()
                .flat_map(|comms| comms.iter())
                .next()
                .ok_or_else(|| {
                    anyhow!(
                        "requested is_first_comment for task {:?} that has no comment",
                        task
                    )
                })?
                .creation_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AuthInfo {
    pub can_read: bool,
    pub can_edit: bool,
    pub can_triage: bool,
    pub can_relabel_to_any: bool, // TODO: rename into can_admin?
    pub can_comment: bool,
}

impl AuthInfo {
    pub fn owner() -> AuthInfo {
        Self::all_or_nothing(true)
    }

    pub fn none() -> AuthInfo {
        Self::all_or_nothing(false)
    }

    pub fn all_or_nothing(all: bool) -> AuthInfo {
        AuthInfo {
            can_read: all,
            can_edit: all,
            can_triage: all,
            can_relabel_to_any: all,
            can_comment: all,
        }
    }
}

impl BitOr for AuthInfo {
    type Output = Self;

    fn bitor(self, rhs: AuthInfo) -> AuthInfo {
        // TODO: use some bitset crate?
        AuthInfo {
            can_read: self.can_read || rhs.can_read,
            can_edit: self.can_edit || rhs.can_edit,
            can_triage: self.can_triage || rhs.can_triage,
            can_relabel_to_any: self.can_relabel_to_any || rhs.can_relabel_to_any,
            can_comment: self.can_comment || rhs.can_comment,
        }
    }
}

#[async_trait]
pub trait Db {
    async fn auth_info_for(&mut self, t: TaskId) -> anyhow::Result<AuthInfo>;
    async fn list_tags_for(&mut self, t: TaskId) -> anyhow::Result<Vec<TagId>>;
    async fn get_event_info(&mut self, e: EventId) -> anyhow::Result<(UserId, Time, TaskId)>;
    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool>;
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FeedMessage {
    Pong,
    NewEvent(Event),
}
