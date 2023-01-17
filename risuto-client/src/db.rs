use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;

use crate::{
    api::{self, AuthInfo, Db, EventId, Search, SearchId, Tag, TagId, TaskId, Time, User, UserId},
    OrderExt, QueryExt, Task,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DbDump {
    pub owner: UserId,
    pub users: Arc<HashMap<UserId, User>>,
    pub tags: Arc<HashMap<TagId, Tag>>,
    pub searches: Arc<HashMap<SearchId, Search>>,
    pub perms: Arc<HashMap<TagId, AuthInfo>>,
    pub tasks: Arc<HashMap<TaskId, Arc<Task>>>,
}

impl DbDump {
    pub fn stub() -> DbDump {
        DbDump {
            owner: UserId::stub(),
            users: Arc::new(HashMap::new()),
            tags: Arc::new(HashMap::new()),
            searches: Arc::new(HashMap::new()),
            perms: Arc::new(HashMap::new()),
            tasks: Arc::new(HashMap::new()),
        }
    }

    pub fn add_users(&mut self, users: Vec<api::User>) {
        Arc::make_mut(&mut self.users).extend(users.into_iter().map(|u| (u.id, u)));
    }

    pub fn add_tags(&mut self, new_tags: Vec<(api::Tag, api::AuthInfo)>) {
        let tags = Arc::make_mut(&mut self.tags);
        let perms = Arc::make_mut(&mut self.perms);
        tags.reserve(tags.len());
        perms.reserve(tags.len());
        for (tag, perm) in new_tags.into_iter() {
            perms.insert(tag.id, perm);
            tags.insert(tag.id, tag);
        }
    }

    pub fn add_searches(&mut self, new_searches: Vec<Search>) {
        Arc::make_mut(&mut self.searches).extend(new_searches.into_iter().map(|s| (s.id, s)));
    }

    pub fn add_tasks(&mut self, tasks: Vec<api::Task>) {
        Arc::make_mut(&mut self.tasks)
            .extend(tasks.into_iter().map(|t| (t.id, Arc::new(Task::from(t)))))
    }

    pub fn add_events_and_refresh_all(&mut self, events: Vec<api::Event>) {
        let tasks = Arc::make_mut(&mut self.tasks);
        for e in events {
            if let Some(t) = tasks.get_mut(&e.task_id) {
                let t = Arc::make_mut(t);
                t.add_event(e);
            }
        }
        for t in tasks.values_mut() {
            let t = Arc::make_mut(t);
            t.refresh_metadata(&self.owner);
        }
    }

    pub fn tag_id(&self, tagname: &str) -> Option<TagId> {
        self.tags.values().find(|t| t.name == tagname).map(|t| t.id)
    }

    pub fn tag_name(&self, id: &TagId) -> Option<&str> {
        self.tags.get(id).map(|t| &t.name as &str)
    }

    pub fn tag(&self, tagname: &str) -> Option<Tag> {
        self.tags
            .values()
            .find(|t| t.name == tagname)
            .map(|t| t.clone())
    }

    /// Returns a list of all the tasks currently in this tag, ordered by increasing
    /// priority and partitioned according to the sorting order
    pub fn search(&self, s: &Search) -> Vec<Arc<Task>> {
        let mut res = self
            .tasks
            .values()
            .filter(|t| s.filter.matches(t))
            .cloned()
            .collect::<Vec<_>>();
        s.order.sort(&mut res);
        res
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
    fn current_user(&self) -> UserId {
        self.owner
    }

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
        let for_task = AuthInfo::all_or_nothing(t.owner_id == self.owner);
        let mut for_tags = AuthInfo::none();
        for tag in t.current_tags.keys() {
            if let Some(auth) = self.perms.get(&tag) {
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
        Ok((t.owner_id, t.date, task_id))
    }

    async fn is_top_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool> {
        Ok(comment
            == self
                .tasks
                .get(&task)
                .ok_or_else(|| {
                    anyhow!(
                        "requested is_top_comment for task {:?} that is not in db",
                        task
                    )
                })?
                .top_comment
                .creation_id)
    }
}
