use crate::api::{EventId, Time, UserId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    /// EventId of this comment's creation
    pub creation_id: EventId,

    /// List of edits in chronological order
    pub edits: im::OrdMap<Time, im::Vector<String>>,

    /// Set of users who already read this comment
    // TODO: this should be per-edit
    // TODO: this should just be a bool flag, and handled in refresh_metadata's for_user
    pub read: im::HashSet<UserId>,

    /// Child comments
    pub children: im::OrdMap<Time, im::Vector<Comment>>,
}

impl Comment {
    fn find_path(
        comments: &im::OrdMap<Time, im::Vector<Comment>>,
        creation_id: &EventId,
    ) -> Option<Vec<(Time, usize)>> {
        for (k, v) in comments.iter() {
            for (i, c) in v.iter().enumerate() {
                if c.creation_id == *creation_id {
                    return Some(vec![(k.clone(), i)]);
                }
                if let Some(mut path) = Comment::find_path(&c.children, &creation_id) {
                    path.push((k.clone(), i));
                    return Some(path);
                }
            }
        }
        None
    }

    /// Assumes path is of len at least 1, panics otherwise
    fn follow_path_mut<'a>(
        comments: &'a mut im::OrdMap<Time, im::Vector<Comment>>,
        mut path: Vec<(Time, usize)>,
    ) -> Option<&'a mut Comment> {
        if path.len() > 1 {
            let last = path.pop().unwrap();
            Comment::follow_path_mut(&mut comments.get_mut(&last.0)?[last.1].children, path)
        } else {
            let last = path.pop().unwrap();
            Some(&mut comments.get_mut(&last.0)?[last.1])
        }
    }

    pub fn find_in<'a>(
        comments: &'a mut im::OrdMap<Time, im::Vector<Comment>>,
        creation_id: &EventId,
    ) -> Option<&'a mut Comment> {
        // TODO: replace with .iter_mut().flat_map() like in find_path, and directly
        // returning instead of re-indexing again mutably later on.
        // (see commit adding this line for example code)
        // Unfortunately OrdMap::iter_mut() currently does not exist.
        let path = Comment::find_path(&comments, creation_id)?;
        Comment::follow_path_mut(comments, path)
    }
}
