use crate::{search, Comment, DbDump, TagId, Task};

use pest::Parser;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Query {
    Any(Vec<Query>),
    All(Vec<Query>),
    Not(Box<Query>),
    Archived(bool),
    Tag {
        tag: TagId,
        backlog: Option<bool>,
    },
    // TODO: be able to search for untagged tasks only
    Phrase(String), // full-text search of one contiguous word vec
}

impl Query {
    pub fn tag(tag: TagId) -> Query {
        Query::Tag { tag, backlog: None }
    }

    pub fn from_search(db: &DbDump, search: &str) -> Query {
        match search::Parser::parse(search::Rule::everything, search) {
            Ok(mut pairs) => {
                // ignore the Pair generated by EOI
                let search_res = pairs
                    .next()
                    .expect("Rule::everything result without search result");
                match search::parse_search(db, search_res.into_inner()) {
                    Some(res) => res,
                    None => todo!("failed to pratt-parse search"),
                }
            }
            e => todo!("should have proper error handling here: {:?}", e),
        }
    }
}

impl Query {
    pub fn matches(&self, task: &Task) -> bool {
        let tokenized = self.has_fts().then(|| Query::tokenize_task(task));
        self.matches_impl(task, &tokenized)
    }

    fn has_fts(&self) -> bool {
        match self {
            Query::Any(q) => q.iter().any(|q| q.has_fts()),
            Query::All(q) => q.iter().any(|q| q.has_fts()),
            Query::Not(q) => q.has_fts(),
            Query::Archived(_) => false,
            Query::Tag { .. } => false,
            Query::Phrase(_) => true,
        }
    }

    fn matches_impl(&self, task: &Task, tokenized: &Option<Vec<Vec<String>>>) -> bool {
        match self {
            Query::Any(q) => q.iter().any(|q| q.matches_impl(task, tokenized)),
            Query::All(q) => q.iter().all(|q| q.matches_impl(task, tokenized)),
            Query::Not(q) => !q.matches_impl(task, tokenized),
            Query::Archived(a) => task.is_archived == *a,
            Query::Tag { tag, backlog } => {
                match task.current_tags.get(tag) {
                    None => false,
                    Some(info) => match backlog {
                        None => true,
                        Some(b) => *b == info.backlog,
                    }
                }
            }
            Query::Phrase(p) => {
                let q = Query::tokenize(p);
                let tokenized = tokenized.as_ref().expect(
                    "called matched_impl on query that has fts without providing tokenized text",
                );
                for text in tokenized {
                    if text.windows(q.len()).any(|w| w == q) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Returns a Vec<String> for the title and one per comment, where each String is a token
    // TODO: this should be cached in-memory at the time of db dump receiving maybe?
    fn tokenize_task(task: &Task) -> Vec<Vec<String>> {
        let mut res = Vec::with_capacity(1 + task.current_comments.len());
        res.push(Query::tokenize(&task.current_title));
        fn also_tokenize_comment(c: &Comment, res: &mut Vec<Vec<String>>) {
            res.push(Query::tokenize(
                &c.edits
                    .last_key_value()
                    .expect("comment with no edits")
                    .1
                    .last()
                    .expect("comment-edit btreemap entry with no edit"),
            ));
            for child in c.children.values().flat_map(|c| c.iter()) {
                also_tokenize_comment(&child, &mut *res);
            }
        }
        for c in task.current_comments.values().flat_map(|c| c.iter()) {
            also_tokenize_comment(&c, &mut res);
        }
        res
    }

    fn tokenize(s: &str) -> Vec<String> {
        use tantivy::tokenizer::*;
        let tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser)
            .filter(AsciiFoldingFilter)
            .filter(Stemmer::new(Language::English)) // TODO: make this configurable
            .filter(StopWordFilter::new(Language::English).unwrap());
        let mut stream = tokenizer.token_stream(s);
        let mut res = Vec::new();
        while stream.advance() {
            let token = stream.token_mut();
            res.push(std::mem::replace(&mut token.text, String::new()));
        }
        res
    }
}

impl Query {
    /// Assumes tables vta (v_tasks_archived), vtt (v_tasks_tags)
    /// and vtx (v_tasks_text) are available
    pub fn to_postgres(&self, first_bind_idx: usize) -> SqlQuery {
        let mut res = Default::default();
        self.add_to_postgres(first_bind_idx, &mut res);
        res
    }

    fn add_to_postgres(&self, first_bind_idx: usize, res: &mut SqlQuery) {
        match self {
            Query::Any(queries) => {
                res.where_clause.push_str("(false");
                for q in queries {
                    res.where_clause.push_str(" OR ");
                    q.add_to_postgres(first_bind_idx, &mut *res);
                }
                res.where_clause.push(')');
            }
            Query::All(queries) => {
                res.where_clause.push_str("(true");
                for q in queries {
                    res.where_clause.push_str(" AND ");
                    q.add_to_postgres(first_bind_idx, &mut *res);
                }
                res.where_clause.push(')');
            }
            Query::Not(q) => {
                res.where_clause.push_str("NOT ");
                q.add_to_postgres(first_bind_idx, &mut *res);
            }
            Query::Archived(b) => {
                let idx = res.add_bind(first_bind_idx, QueryBind::Bool(*b));
                res.where_clause.push_str(&format!("(vta.archived = ${idx})"));
            }
            Query::Tag { tag, backlog } => {
                let idx = res.add_bind(first_bind_idx, QueryBind::Uuid(tag.0));
                res.where_clause.push_str(&format!("(vtt.is_in = true AND vtt.tag_id = ${idx}"));
                if let Some(backlog) = backlog {
                    let idx = res.add_bind(first_bind_idx, QueryBind::Bool(*backlog));
                    res.where_clause.push_str(&format!(" AND vtt.backlog = ${idx}"));

                }
                res.where_clause.push_str(")");
            }
            Query::Phrase(t) => {
                let idx = res.add_bind(first_bind_idx, QueryBind::String(t.clone()));
                res.where_clause
                    .push_str(&format!("(to_tsvector(vtx.text) @@ phraseto_tsquery(${idx}))"));
            }
        }
    }
}

pub enum QueryBind {
    Bool(bool),
    Uuid(Uuid),
    String(String),
}

#[derive(Default)]
pub struct SqlQuery {
    pub where_clause: String,
    pub binds: Vec<QueryBind>,
    // TODO: order_clause: Option<String>,
}

impl SqlQuery {
    /// Adds a QueryBind, returning the index that should be used to refer to it assuming the first bind is at index first_bind_idx
    fn add_bind(&mut self, first_bind_idx: usize, b: QueryBind) -> usize {
        let res = first_bind_idx + self.binds.len();
        self.binds.push(b);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthInfo, Tag, UserId};
    use std::collections::HashMap;

    fn example_db() -> DbDump {
        let mut tags = HashMap::new();
        tags.insert(
            TagId(Uuid::new_v4()),
            (
                Tag {
                    owner: UserId::stub(),
                    name: String::from("foo"),
                    archived: false,
                },
                AuthInfo::all(),
            ),
        );
        tags.insert(
            TagId(Uuid::new_v4()),
            (
                Tag {
                    owner: UserId::stub(),
                    name: String::from("bar"),
                    archived: false,
                },
                AuthInfo::all(),
            ),
        );
        tags.insert(
            TagId(Uuid::new_v4()),
            (
                Tag {
                    owner: UserId::stub(),
                    name: String::from("baz"),
                    archived: false,
                },
                AuthInfo::all(),
            ),
        );
        DbDump {
            owner: UserId::stub(),
            users: HashMap::new(),
            tags,
            tasks: HashMap::new(),
        }
    }

    fn phrase(s: &str) -> Query {
        Query::Phrase(s.to_string())
    }

    #[test]
    fn primary_archived() {
        let db = example_db();
        assert_eq!(
            Query::from_search(&db, "archived:true"),
            Query::Archived(true),
        );
        assert_eq!(
            Query::from_search(&db, "archived:false"),
            Query::Archived(false),
        );
    }

    #[test]
    fn primary_tag() {
        let db = example_db();
        assert_eq!(
            Query::from_search(&db, "tag:foo"),
            Query::Tag(db.tag_id("foo").unwrap()),
        );
        assert_eq!(
            Query::from_search(&db, "tag:bar"),
            Query::Tag(db.tag_id("bar").unwrap()),
        );
        // TODO: also test behavior for unknown tag
    }

    #[test]
    fn primary_word() {
        let db = example_db();

        // Basic words (including tag name)
        assert_eq!(Query::from_search(&db, "test"), phrase("test"),);
        assert_eq!(Query::from_search(&db, "foo"), phrase("foo"),);

        // Words matching special query parameters
        assert_eq!(Query::from_search(&db, "archived"), phrase("archived"),);
        assert_eq!(Query::from_search(&db, "tag"), phrase("tag"),);
    }

    #[test]
    fn primary_phrase() {
        let db = example_db();

        // Basic usage
        assert_eq!(Query::from_search(&db, r#""test""#), phrase("test"),);
        assert_eq!(Query::from_search(&db, r#""foo bar""#), phrase("foo bar"),);

        // Things that look like queries
        assert_eq!(
            Query::from_search(&db, r#""(foo bar OR archived:false)""#),
            phrase("(foo bar OR archived:false)"),
        );
        assert_eq!(Query::from_search(&db, r#""(test""#), phrase("(test"),);

        // Escapes
        assert_eq!(
            Query::from_search(&db, r#""foo\" bar""#),
            phrase(r#"foo" bar"#),
        );
        assert_eq!(
            Query::from_search(&db, r#""foo\\ bar""#),
            phrase(r#"foo\ bar"#),
        );
        assert_eq!(
            Query::from_search(&db, r#""foo\\\" bar""#),
            phrase(r#"foo\" bar"#),
        );
    }

    #[test]
    fn infixes() {
        let db = example_db();

        // Nothing is and
        assert_eq!(
            Query::from_search(&db, "foo bar"),
            Query::All(vec![phrase("foo"), phrase("bar")]),
        );
        assert_eq!(
            Query::from_search(&db, r#""foo bar" "baz""#),
            Query::All(vec![phrase("foo bar"), phrase("baz")]),
        );

        // Explicit and
        assert_eq!(
            Query::from_search(&db, "foo AND archived:false"),
            Query::All(vec![phrase("foo"), Query::Archived(false)]),
        );

        // Explicit or
        assert_eq!(
            Query::from_search(&db, "foo or archived:false"),
            Query::Any(vec![phrase("foo"), Query::Archived(false)]),
        );
    }

    #[test]
    fn complex() {
        let db = example_db();
        assert_eq!(
            Query::from_search(&db, "foo bar baz"),
            Query::All(vec![phrase("foo"), phrase("bar"), phrase("baz")]),
        );
        assert_eq!(
            Query::from_search(&db, "foo bar or baz"),
            Query::All(vec![
                phrase("foo"),
                Query::Any(vec![phrase("bar"), phrase("baz")])
            ]),
        );
        assert_eq!(
            Query::from_search(&db, "(foo bar) or baz"),
            Query::Any(vec![
                Query::All(vec![phrase("foo"), phrase("bar")]),
                phrase("baz")
            ]),
        );
        assert_eq!(
            Query::from_search(&db, "(archived:true bar) or baz"),
            Query::Any(vec![
                Query::All(vec![Query::Archived(true), phrase("bar")]),
                phrase("baz")
            ]),
        );
    }
}
