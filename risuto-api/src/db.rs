use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;

use crate::{AuthInfo, EventId, Tag, TagId, Task, TaskId, Time, User, UserId};

#[async_trait]
pub trait Db {
    async fn auth_info_for(&mut self, t: TaskId) -> anyhow::Result<AuthInfo>;
    async fn list_tags_for(&mut self, t: TaskId) -> anyhow::Result<Vec<TagId>>;
    async fn get_event_info(&mut self, e: EventId) -> anyhow::Result<(UserId, Time, TaskId)>;
    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool>;
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

    /// Returns a list of all the tasks currently in this tag, ordered by increasing priority
    pub fn tasks_in_tag(&self, tag: &TagId) -> Vec<Arc<Task>> {
        let mut res = self
            .tasks
            .values()
            .filter_map(|t| t.current_tags.get(tag).map(|p| (p.priority, t.clone())))
            .collect::<Vec<_>>();
        res.sort_unstable_by_key(|(prio, t)| (*prio, t.id));
        res.into_iter().map(|(_, t)| t).collect()
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
