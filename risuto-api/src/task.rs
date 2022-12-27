use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use crate::{Comment, Event, EventData, TagId, Time, UserId};

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
    pub id: TaskId,
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
                match &e.data {
                    EventData::SetTitle(title) => self.current_title = title.clone(),
                    EventData::SetDone(now_done) => self.is_done = *now_done,
                    EventData::SetArchived(now_archived) => self.is_archived = *now_archived,
                    EventData::BlockedUntil(time) => self.blocked_until = *time,
                    EventData::ScheduleFor(time) => self.scheduled_for = *time,
                    EventData::AddTag { tag, prio, backlog } => {
                        self.current_tags.insert(
                            *tag,
                            TaskInTag {
                                priority: *prio,
                                backlog: *backlog,
                            },
                        );
                    }
                    EventData::RmTag(tag) => {
                        self.current_tags.remove(tag);
                    }
                    EventData::AddComment { text, parent_id } => {
                        let mut edits = BTreeMap::new();
                        edits.insert(e.date, vec![text.clone()]);
                        let mut read = HashSet::new();
                        read.insert(e.owner);
                        let children = BTreeMap::new();
                        let creation_id = e.id;
                        if let Some(parent) =
                            parent_id.and_then(|p| Comment::find_in(&mut self.current_comments, &p))
                        {
                            parent
                                .children
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(Comment {
                                    creation_id,
                                    edits,
                                    read,
                                    children,
                                });
                        } else {
                            // Also add as a top-level comment if the parent could not be found (TODO: log a warning)
                            self.current_comments
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(Comment {
                                    creation_id,
                                    edits,
                                    read,
                                    children,
                                });
                        }
                    }
                    EventData::EditComment { comment_id, text } => {
                        if let Some(comment) =
                            Comment::find_in(&mut self.current_comments, comment_id)
                        {
                            comment
                                .edits
                                .entry(e.date)
                                .or_insert(Vec::new())
                                .push(text.clone());
                            comment.read = HashSet::new();
                            comment.read.insert(e.owner);
                        }
                    }
                    EventData::SetEventRead { event_id, now_read } => {
                        if let Some(comment) =
                            Comment::find_in(&mut self.current_comments, event_id)
                        {
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
