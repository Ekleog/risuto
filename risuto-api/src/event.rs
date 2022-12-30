use anyhow::Context;
use chrono::Utc;
use uuid::{uuid, Uuid};

use crate::{Db, TagId, TaskId, Time, UserId};

#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn today() -> OrderId {
        // picked with a totally fair dice roll
        OrderId(uuid!("70DA1aaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))
    }

    pub fn untagged() -> OrderId {
        // picked with a totally fair dice roll
        OrderId(uuid!("07A66EDa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct EventId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Event {
    pub id: EventId,
    pub owner_id: UserId,
    pub date: Time,
    pub task_id: TaskId,

    pub data: EventData,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum EventData {
    SetTitle(String),
    SetDone(bool),
    SetArchived(bool),
    BlockedUntil(Option<Time>),
    ScheduleFor(Option<Time>),
    SetOrder {
        order: OrderId,
        prio: i64,
    },
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
    pub fn now(owner_id: UserId, task_id: TaskId, data: EventData) -> Event {
        Event {
            id: EventId(Uuid::new_v4()),
            owner_id,
            date: Utc::now(),
            task_id,
            data,
        }
    }

    /// Takes AuthInfo as the authorization status for the user for self.task_id
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
            EventData::SetTitle { .. } => auth!(self.task_id).can_edit,
            EventData::SetDone { .. }
            | EventData::SetArchived { .. }
            | EventData::BlockedUntil { .. }
            | EventData::ScheduleFor { .. } => auth!(self.task_id).can_triage,
            EventData::SetOrder { .. } => auth!(self.task_id).can_read,
            EventData::AddTag { tag, .. } => {
                let auth = auth!(self.task_id);
                auth.can_relabel_to_any
                    || (auth.can_triage
                        && db
                            .list_tags_for(self.task_id)
                            .await
                            .with_context(|| format!("listing tags for task {:?}", self.task_id))?
                            .contains(&tag))
            }
            EventData::RmTag { .. } => auth!(self.task_id).can_relabel_to_any,
            EventData::AddComment { parent_id, .. } => {
                if let Some(parent_id) = parent_id {
                    check_parent_event!(parent_id);
                }
                auth!(self.task_id).can_comment
            }
            EventData::EditComment { comment_id, .. } => {
                let (comm_owner, _, comm_task) = check_parent_event!(comment_id);
                let is_comment_owner = self.owner_id == comm_owner;
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
