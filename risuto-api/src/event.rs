use anyhow::Context;
use chrono::Utc;
use uuid::Uuid;

use crate::{Db, Error, TagId, TaskId, Time, UserId, STUB_UUID, UUID_TODAY, UUID_UNTAGGED};

#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct OrderId(#[generator(bolero::generator::gen_arbitrary())] pub Uuid);

impl OrderId {
    pub fn stub() -> OrderId {
        OrderId(STUB_UUID)
    }

    pub fn today() -> OrderId {
        OrderId(UUID_TODAY)
    }

    pub fn untagged() -> OrderId {
        OrderId(UUID_UNTAGGED)
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct EventId(#[generator(bolero::generator::gen_arbitrary())] pub Uuid);

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct Event {
    pub id: EventId,
    pub owner_id: UserId,
    #[generator(bolero::generator::gen_arbitrary())]
    pub date: Time,
    pub task_id: TaskId,

    pub data: EventData,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    bolero::generator::TypeGenerator,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum EventData {
    SetTitle(#[generator(bolero::generator::gen_with::<String>().len(0..100usize))] String),
    SetDone(bool),
    SetArchived(bool),
    BlockedUntil(#[generator(bolero::generator::gen_arbitrary())] Option<Time>),
    ScheduleFor(#[generator(bolero::generator::gen_arbitrary())] Option<Time>),
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
        #[generator(bolero::generator::gen_with::<String>().len(0..100usize))]
        text: String,
        parent_id: Option<EventId>,
    },
    EditComment {
        #[generator(bolero::generator::gen_with::<String>().len(0..100usize))]
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

    pub async fn is_authorized<D: Db>(&self, db: &mut D) -> anyhow::Result<bool> {
        if self.owner_id != db.current_user() {
            return Ok(false);
        }
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
            EventData::SetDone { .. } | EventData::BlockedUntil { .. } => {
                auth!(self.task_id).can_triage
            }
            EventData::SetArchived { .. } => auth!(self.task_id).can_archive,
            EventData::ScheduleFor { .. } | EventData::SetOrder { .. } => {
                auth!(self.task_id).can_read
            }
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
                let is_top_comment = db
                    .is_top_comment(comm_task, comment_id)
                    .await
                    .with_context(|| {
                        format!("checking if comment {:?} is first comment", comment_id)
                    })?;
                is_comment_owner || (auth!(comm_task).can_edit && is_top_comment)
            }
            EventData::SetEventRead { event_id, .. } => {
                let (_, _, par_task) = check_parent_event!(event_id);
                auth!(par_task).can_read
            }
        })
    }

    // See comments on other `validate` functions throughout risuto-api
    pub fn validate(&self) -> Result<(), Error> {
        crate::validate_time(&self.date)?;
        self.data.validate()
    }
}

impl EventData {
    // See comments on other `validate` functions throughout risuto-api
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            EventData::SetTitle(t) => crate::validate_string(t),
            EventData::SetDone(_) => Ok(()),
            EventData::SetArchived(_) => Ok(()),
            EventData::BlockedUntil(None) => Ok(()),
            EventData::BlockedUntil(Some(t)) => crate::validate_time(t),
            EventData::ScheduleFor(None) => Ok(()),
            EventData::ScheduleFor(Some(t)) => crate::validate_time(t),
            EventData::SetOrder { order: _, prio: _ } => Ok(()),
            EventData::AddTag {
                tag: _,
                prio: _,
                backlog: _,
            } => Ok(()),
            EventData::RmTag(_) => Ok(()),
            EventData::AddComment { text, parent_id: _ } => crate::validate_string(text),
            EventData::EditComment {
                text,
                comment_id: _,
            } => crate::validate_string(text),
            EventData::SetEventRead {
                event_id: _,
                now_read: _,
            } => Ok(()),
        }
    }
}
