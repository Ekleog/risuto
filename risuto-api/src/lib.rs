mod comment;
mod query;
mod search;
mod tag;
mod task;
mod user;

pub use comment::Comment;
pub use query::{Query, QueryBind, SqlQuery};
pub use tag::{Tag, TagId};
pub use task::{Task, TaskId, TaskInTag};
pub use user::{AuthToken, NewSession, User, UserId};

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use chrono::Utc;
use std::{collections::HashMap, ops::BitOr, sync::Arc};

pub use uuid::{uuid, Uuid};
pub type Time = chrono::DateTime<Utc>;

pub const STUB_UUID: Uuid = uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff");

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct EventId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Event {
    pub id: EventId,
    pub owner: UserId,
    pub date: Time,
    pub task: TaskId,

    pub data: EventData,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum EventData {
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
    pub fn now(owner: UserId, task: TaskId, data: EventData) -> Event {
        Event {
            id: EventId(Uuid::new_v4()),
            owner,
            date: Utc::now(),
            task,
            data,
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
                if par_date >= self.date {
                    // TODO: remove this requirement by fixing event insertion into tasks
                    return Ok(false);
                }
                (par_owner, par_date, par_task)
            }};
        }
        Ok(match self.data {
            EventData::SetTitle { .. } => auth!(self.task).can_edit,
            EventData::SetDone { .. }
            | EventData::SetArchived { .. }
            | EventData::BlockedUntil { .. }
            | EventData::ScheduleFor { .. } => auth!(self.task).can_triage,
            EventData::AddTag { tag, .. } => {
                let auth = auth!(self.task);
                auth.can_relabel_to_any
                    || (auth.can_triage
                        && db
                            .list_tags_for(self.task)
                            .await
                            .with_context(|| format!("listing tags for task {:?}", self.task))?
                            .contains(&tag))
            }
            EventData::RmTag { .. } => auth!(self.task).can_relabel_to_any,
            EventData::AddComment { parent_id, .. } => {
                if let Some(parent_id) = parent_id {
                    check_parent_event!(parent_id);
                }
                auth!(self.task).can_comment
            }
            EventData::EditComment { comment_id, .. } => {
                let (comm_owner, _, comm_task) = check_parent_event!(comment_id);
                let is_comment_owner = self.owner == comm_owner;
                let is_first_comment = db
                    .is_first_comment(comm_task, comment_id)
                    .await
                    .with_context(|| {
                        format!("checking if comment {:?} is first comment", comment_id)
                    })?;
                is_comment_owner || (auth!(comm_task).can_edit && is_first_comment)
            }
            EventData::SetEventRead { event_id, .. } => {
                let (_, _, par_task) = check_parent_event!(event_id);
                auth!(par_task).can_read
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DbDump {
    pub owner: UserId,
    pub users: HashMap<UserId, User>,
    pub tags: HashMap<TagId, (Tag, AuthInfo)>,
    pub tasks: HashMap<TaskId, Arc<Task>>,
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

    pub fn tag_id(&self, tagname: &str) -> Option<TagId> {
        self.tags
            .iter()
            .find(|(_, (t, _))| t.name == tagname)
            .map(|(id, _)| *id)
    }

    pub fn tag_name(&self, id: &TagId) -> Option<&str> {
        self.tags.get(id).map(|(t, _)| &t.name as &str)
    }
}

impl DbDump {
    fn get_task_for_event(&self, event: EventId) -> anyhow::Result<TaskId> {
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
impl Db for &DbDump {
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
            anyhow!(
                "requested comment owner for event {e:?} for which task {task_id:?} is not in db",
            )
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

    pub fn all() -> AuthInfo {
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
