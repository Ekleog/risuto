use std::collections::{BTreeMap, HashMap};

use crate::{
    api::{self, Event, EventData, OrderId, TagId, TaskId, Time, UserId},
    Comment,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskInTag {
    // higher is lower in the tag list
    pub priority: i64,

    /// if true, this task is in this tag's backlog
    pub backlog: bool,
}

// TODO: consider switching to the im crate for cheaply-clonable stuff here
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: TaskId,
    pub owner_id: UserId,
    pub date: Time,

    pub initial_title: String,
    pub current_title: String,

    pub top_comment: Comment,

    pub is_done: bool,
    pub is_archived: bool,
    pub blocked_until: Option<Time>,
    pub scheduled_for: Option<Time>,
    pub current_tags: HashMap<TagId, TaskInTag>,
    pub orders: HashMap<OrderId, i64>,

    /// List of comments in chronological order
    pub current_comments: im::OrdMap<Time, im::Vector<Comment>>,

    pub events: BTreeMap<Time, Vec<Event>>,
}

impl From<api::Task> for Task {
    fn from(t: api::Task) -> Task {
        Task {
            id: t.id,
            owner_id: t.owner_id,
            date: t.date,
            initial_title: t.initial_title.clone(),
            current_title: t.initial_title,
            top_comment: Comment {
                creation_id: t.top_comment_id,
                edits: im::OrdMap::new(),
                read: im::HashSet::new(),
                children: im::OrdMap::new(),
            },
            is_done: false,
            is_archived: false,
            blocked_until: None,
            scheduled_for: None,
            current_tags: HashMap::new(),
            orders: HashMap::new(),
            current_comments: im::OrdMap::new(),
            events: BTreeMap::new(),
        }
    }
}

impl Task {
    pub fn prio_tag(&self, tag: &TagId) -> Option<i64> {
        self.current_tags.get(tag).map(|t| t.priority)
    }

    pub fn prio_order(&self, order: &OrderId) -> Option<i64> {
        self.orders.get(order).copied()
    }

    pub fn last_event_time(&self) -> Time {
        self.events
            .last_key_value()
            .map(|(d, _)| d.clone())
            .unwrap_or(self.date)
    }

    pub fn add_event(&mut self, e: Event) {
        let insert_into = self.events.entry(e.date).or_insert(Vec::new());
        if insert_into.iter().find(|evt| **evt == e).is_none() {
            insert_into.push(e);
        }
    }

    pub fn refresh_metadata(&mut self, for_user: &UserId) {
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
                    EventData::ScheduleFor(time) => {
                        if e.owner_id == *for_user {
                            self.scheduled_for = *time;
                        }
                    }
                    EventData::SetOrder { order, prio } => {
                        if e.owner_id == *for_user {
                            self.orders.insert(order.clone(), *prio);
                        }
                    }
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
                    EventData::AddComment { text, parent_id }
                        if e.id == self.top_comment.creation_id =>
                    {
                        assert!(
                            parent_id.is_none(),
                            "parent_id must be None for a task's top-comment"
                        );
                        let mut edit = im::Vector::new();
                        edit.push_back(text.clone());
                        self.top_comment.edits.insert(e.date, edit);
                        self.top_comment.read.insert(e.owner_id);
                    }
                    EventData::AddComment { text, parent_id } => {
                        let mut edit = im::Vector::new();
                        edit.push_back(text.clone());
                        let mut edits = im::OrdMap::new();
                        edits.insert(e.date, edit);
                        let mut read = im::HashSet::new();
                        read.insert(e.owner_id);
                        let children = im::OrdMap::new();
                        let creation_id = e.id;
                        if let Some(parent) =
                            parent_id.and_then(|p| Comment::find_in(&mut self.current_comments, &p))
                        {
                            parent
                                .children
                                .entry(e.date)
                                .or_insert(im::Vector::new())
                                .push_back(Comment {
                                    creation_id,
                                    edits,
                                    read,
                                    children,
                                });
                        } else {
                            // Also add as a top-level comment if the parent could not be found (TODO: log a warning)
                            self.current_comments
                                .entry(e.date)
                                .or_insert(im::Vector::new())
                                .push_back(Comment {
                                    creation_id,
                                    edits,
                                    read,
                                    children,
                                });
                        }
                    }
                    EventData::EditComment { comment_id, text }
                        if *comment_id == self.top_comment.creation_id =>
                    {
                        self.top_comment
                            .edits
                            .entry(e.date)
                            .or_insert(im::Vector::new())
                            .push_back(text.clone());
                        self.top_comment.read = im::HashSet::new();
                        self.top_comment.read.insert(e.owner_id);
                    }
                    EventData::EditComment { comment_id, text } => {
                        if let Some(comment) =
                            Comment::find_in(&mut self.current_comments, comment_id)
                        {
                            comment
                                .edits
                                .entry(e.date)
                                .or_insert(im::Vector::new())
                                .push_back(text.clone());
                            comment.read = im::HashSet::new();
                            comment.read.insert(e.owner_id);
                        }
                    }
                    EventData::SetEventRead { event_id, now_read } => {
                        if let Some(comment) =
                            Comment::find_in(&mut self.current_comments, event_id)
                        {
                            if *now_read {
                                comment.read.insert(e.owner_id);
                            } else {
                                comment.read.remove(&e.owner_id);
                            }
                        } // ignore non-comment events
                    }
                }
            }
        }
        assert!(
            !self.top_comment.edits.is_empty(),
            "task {self:?} has no top comment"
        );
    }
}
