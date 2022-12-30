use std::collections::{BTreeMap, HashSet};

use crate::api::{EventId, Time, UserId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    /// EventId of this comment's creation
    pub creation_id: EventId,

    /// List of edits in chronological order
    pub edits: BTreeMap<Time, Vec<String>>,

    /// Set of users who already read this comment
    // TODO: this should be per-edit
    // TODO: this should just be a bool flag, and handled in refresh_metadata's for_user
    pub read: HashSet<UserId>,

    /// Child comments
    pub children: BTreeMap<Time, Vec<Comment>>,
}

impl Comment {
    pub fn find_in<'a>(
        comments: &'a mut BTreeMap<Time, Vec<Comment>>,
        creation_id: &EventId,
    ) -> Option<&'a mut Comment> {
        for c in comments.values_mut().flat_map(|v| v.iter_mut()) {
            if c.creation_id == *creation_id {
                return Some(c);
            }
            if let Some(res) = Comment::find_in(&mut c.children, &creation_id) {
                return Some(res);
            }
        }
        None
    }
}
