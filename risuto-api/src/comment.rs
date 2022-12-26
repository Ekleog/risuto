use std::collections::{BTreeMap, HashSet};

use crate::{EventId, Time, UserId};

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
