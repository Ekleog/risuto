use risuto_api::{Query, Uuid};

pub enum Bind {
    Bool(bool),
    Uuid(Uuid),
    String(String),
}

#[derive(Default)]
pub struct Sql {
    pub where_clause: String,
    pub binds: Vec<Bind>,
    // TODO: order_clause: Option<String>,
}

impl Sql {
    /// Adds a Bind, returning the index that should be used to refer to it assuming the first bind is at index first_bind_idx
    fn add_bind(&mut self, first_bind_idx: usize, b: Bind) -> usize {
        let res = first_bind_idx + self.binds.len();
        self.binds.push(b);
        res
    }
}

/// Assumes tables vta (v_tasks_archived), vtd(v_tasks_done), vtt (v_tasks_tags)
/// and vtx (v_tasks_text) are available
pub fn to_postgres(q: &Query, first_bind_idx: usize) -> Sql {
    let mut res = Default::default();
    add_to_postgres(q, first_bind_idx, &mut res);
    res
}

fn add_to_postgres(q: &Query, first_bind_idx: usize, res: &mut Sql) {
    match q {
        Query::Any(queries) => {
            res.where_clause.push_str("(false");
            for q in queries {
                res.where_clause.push_str(" OR ");
                add_to_postgres(q, first_bind_idx, &mut *res);
            }
            res.where_clause.push(')');
        }
        Query::All(queries) => {
            res.where_clause.push_str("(true");
            for q in queries {
                res.where_clause.push_str(" AND ");
                add_to_postgres(q, first_bind_idx, &mut *res);
            }
            res.where_clause.push(')');
        }
        Query::Not(q) => {
            res.where_clause.push_str("NOT ");
            add_to_postgres(q, first_bind_idx, &mut *res);
        }
        Query::Archived(b) => {
            let idx = res.add_bind(first_bind_idx, Bind::Bool(*b));
            res.where_clause
                .push_str(&format!("(vta.archived = ${idx})"));
        }
        Query::Done(b) => {
            let idx = res.add_bind(first_bind_idx, Bind::Bool(*b));
            res.where_clause.push_str(&format!("(vtd.done = ${idx})"));
        }
        Query::Tag { tag, backlog } => {
            let idx = res.add_bind(first_bind_idx, Bind::Uuid(tag.0));
            res.where_clause
                .push_str(&format!("(vtt.is_in = true AND vtt.tag_id = ${idx}"));
            if let Some(backlog) = backlog {
                let idx = res.add_bind(first_bind_idx, Bind::Bool(*backlog));
                res.where_clause
                    .push_str(&format!(" AND vtt.backlog = ${idx}"));
            }
            res.where_clause.push_str(")");
        }
        Query::Phrase(t) => {
            let idx = res.add_bind(first_bind_idx, Bind::String(t.clone()));
            res.where_clause
                .push_str(&format!("(vtx.text @@ phraseto_tsquery(${idx}))"));
        }
    }
}
