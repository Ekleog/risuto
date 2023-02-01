use std::str::FromStr;

use crate::{
    api::{Query, Time, TimeQuery},
    Comment, DbDump, Task,
};

use pest::{iterators::Pairs, pratt_parser::PrattParser, Parser as PestParser};
use risuto_api::{midnight_on, Error};

pub trait QueryExt {
    fn from_search(db: &DbDump, tz: &chrono_tz::Tz, search: &str) -> Query;
    fn validate_now(&self) -> Result<(), Error>;
    fn matches(&self, task: &Task) -> Result<bool, Error>;
}

impl QueryExt for Query {
    fn from_search(db: &DbDump, tz: &chrono_tz::Tz, search: &str) -> Query {
        tracing::trace!(?search, "parsing query");
        let res = match Parser::parse(Rule::everything, search) {
            Ok(mut pairs) => {
                // ignore the Pair generated by EOI
                let search_res = pairs
                    .next()
                    .expect("Rule::everything result without search result");
                parse_search(db, tz, search_res.into_inner())
            }
            e => todo!("should have proper error handling here: {:?}", e),
        };
        tracing::trace!(?search, ?res, "parsed query");
        res
    }

    fn validate_now(&self) -> Result<(), Error> {
        self.validate()?;
        match self {
            Query::Any(q) => q
                .iter()
                .map(QueryExt::validate_now)
                .collect::<Result<(), Error>>(),
            Query::All(q) => q
                .iter()
                .map(QueryExt::validate_now)
                .collect::<Result<(), Error>>(),
            Query::Not(q) => q.validate_now(),
            Query::Archived(_) => Ok(()),
            Query::Done(_) => Ok(()),
            Query::Tag { tag: _, backlog: _ } => Ok(()),
            Query::Untagged(_) => Ok(()),
            Query::ScheduledForBefore(q) => timeq_validate_now(q),
            Query::ScheduledForAfter(q) => timeq_validate_now(q),
            Query::BlockedUntilAtMost(q) => timeq_validate_now(q),
            Query::BlockedUntilAtLeast(q) => timeq_validate_now(q),
            Query::Phrase(_) => Ok(()),
        }
    }

    fn matches(&self, task: &Task) -> Result<bool, Error> {
        let tokenized = has_fts(self).then(|| tokenize_task(task));
        matches_impl(self, task, &tokenized)
    }
}

fn has_fts(q: &Query) -> bool {
    match q {
        Query::Any(q) => q.iter().any(|q| has_fts(q)),
        Query::All(q) => q.iter().any(|q| has_fts(q)),
        Query::Not(q) => has_fts(q),
        Query::Archived(_) => false,
        Query::Done(_) => false,
        Query::Tag { .. } => false,
        Query::Untagged(_) => false,
        Query::ScheduledForAfter(_) => false,
        Query::ScheduledForBefore(_) => false,
        Query::BlockedUntilAtLeast(_) => false,
        Query::BlockedUntilAtMost(_) => false,
        Query::Phrase(_) => true,
    }
}

fn timeq_validate_now(q: &TimeQuery) -> Result<(), Error> {
    q.eval_now().map(|_| ())
}

fn matches_impl(
    q: &Query,
    task: &Task,
    tokenized: &Option<Vec<Vec<String>>>,
) -> Result<bool, Error> {
    Ok(match q {
        Query::Any(queries) => queries
            .iter()
            .any(|q| matches_impl(q, task, tokenized) == Ok(true)),
        Query::All(queries) => queries
            .iter()
            .all(|q| matches_impl(q, task, tokenized) == Ok(true)),
        Query::Not(q) => matches_impl(q, task, tokenized) == Ok(false),
        Query::Archived(a) => task.is_archived == *a,
        Query::Done(d) => task.is_done == *d,
        Query::Tag { tag, backlog } => match task.current_tags.get(tag) {
            None => false,
            Some(info) => match backlog {
                None => true,
                Some(b) => *b == info.backlog,
            },
        },
        Query::Untagged(u) => task.current_tags.is_empty() == *u,
        Query::ScheduledForAfter(d) => timeq_matches(d, &task.scheduled_for, |q, t| t >= q)?,
        Query::ScheduledForBefore(d) => timeq_matches(d, &task.scheduled_for, |q, t| t <= q)?,
        Query::BlockedUntilAtLeast(d) => timeq_matches(d, &task.blocked_until, |q, t| t >= q)?,
        Query::BlockedUntilAtMost(d) => timeq_matches(d, &task.blocked_until, |q, t| t <= q)?,
        Query::Phrase(p) => {
            let q = tokenize(p);
            if q.is_empty() {
                return Ok(true); // query consisting of nothing but stop-words
            }
            let tokenized = tokenized.as_ref().expect(
                "called matched_impl on query that has fts without providing tokenized text",
            );
            for text in tokenized {
                if text.windows(q.len()).any(|w| w == q) {
                    return Ok(true);
                }
            }
            false
        }
    })
}

fn timeq_matches(
    q: &TimeQuery,
    t: &Option<Time>,
    check: impl FnOnce(&Time, &Time) -> bool,
) -> Result<bool, Error> {
    let q = q.eval_now()?;
    match t {
        None => Ok(false),
        Some(t) => Ok(check(&q, t)),
    }
}

/// Returns a Vec<String> for the title and one per comment, where each String is a token
// TODO: this should be cached in-memory at the time of db dump receiving maybe?
fn tokenize_task(task: &Task) -> Vec<Vec<String>> {
    let mut res = Vec::with_capacity(1 + task.current_comments.len());
    res.push(tokenize(&task.current_title));
    fn also_tokenize_comment(c: &Comment, res: &mut Vec<Vec<String>>) {
        res.push(tokenize(
            &c.edits
                .iter()
                .next_back()
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

#[derive(pest_derive::Parser)]
#[grammar = "query.pest"]
struct Parser;

lazy_static::lazy_static! {
    static ref SEARCH_PARSER: PrattParser<Rule> = {
        use pest::pratt_parser::{Assoc::*, Op};
        use Rule::*;
        PrattParser::new()
            .op(Op::infix(and, Left))
            .op(Op::infix(or, Left))
            .op(Op::prefix(not))
    };
}

// Unescape a quoted-string
fn unescape(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut s = s.chars();
    assert_eq!(s.next(), Some('"'), "first char is not a double quote");
    while let Some(c) = s.next() {
        if c == '\\' {
            res.push(s.next().expect("got terminal backslash"));
        } else {
            res.push(c);
        }
    }
    assert_eq!(res.pop(), Some('"'), "last char is not a double quote");
    res
}

fn parse_search(db: &DbDump, tz: &chrono_tz::Tz, pairs: Pairs<Rule>) -> Query {
    SEARCH_PARSER
        .map_primary(|p| match p.as_rule() {
            Rule::archived => Query::Archived(match p.into_inner().next().map(|p| p.as_rule()) {
                Some(Rule::r#true) => true,
                Some(Rule::r#false) => false,
                r => unreachable!("Rule::archived unexpected atom: {:?}", r),
            }),
            Rule::done => Query::Done(match p.into_inner().next().map(|p| p.as_rule()) {
                Some(Rule::r#true) => true,
                Some(Rule::r#false) => false,
                r => unreachable!("Rule::done unexpected atom: {:?}", r),
            }),
            Rule::untagged => Query::Untagged(match p.into_inner().next().map(|p| p.as_rule()) {
                Some(Rule::r#true) => true,
                Some(Rule::r#false) => false,
                r => unreachable!("Rule::untagged unexpected atom: {:?}", r),
            }),
            Rule::tag => {
                let tagname = p.into_inner().next();
                let tagname = match tagname.as_ref().map(|p| p.as_rule()) {
                    Some(Rule::tagname) => tagname.unwrap().as_str(),
                    r => unreachable!("Rule::tag unexpected atom: {:?}", r),
                };
                // TODO: is there a need for querying only tasks in/out of backlog from text search?
                db.tag_id(tagname)
                    .map(|tag| Query::Tag { tag, backlog: None })
                    .unwrap_or_else(|| Query::Phrase(format!("tag:{tagname}")))
            }
            Rule::scheduled => parse_date_cmp(
                p.into_inner(),
                tz,
                Query::ScheduledForAfter,
                Query::ScheduledForBefore,
            ),
            Rule::blocked => parse_date_cmp(
                p.into_inner(),
                tz,
                Query::BlockedUntilAtLeast,
                Query::BlockedUntilAtMost,
            ),
            Rule::search => parse_search(db, tz, p.into_inner()),
            Rule::phrase => Query::Phrase(unescape(p.as_str())),
            Rule::word => Query::Phrase(p.as_str().to_string()),
            r => unreachable!("Search unexpected primary: {:?}", r),
        })
        .map_infix(|lhs, op, rhs| match op.as_rule() {
            Rule::and => match lhs {
                Query::All(mut v) => {
                    v.push(rhs);
                    Query::All(v)
                }
                _ => Query::All(vec![lhs, rhs]),
            },
            Rule::or => match lhs {
                Query::Any(mut v) => {
                    v.push(rhs);
                    Query::Any(v)
                }
                _ => Query::Any(vec![lhs, rhs]),
            },
            r => unreachable!("Search unexpected infix: {:?}", r),
        })
        .map_prefix(|op, rhs| match op.as_rule() {
            Rule::not => Query::Not(Box::new(rhs)),
            r => unreachable!("Search unexpected prefix: {:?}", r),
        })
        .parse(pairs)
}

fn parse_date_cmp(
    mut reader: Pairs<Rule>,
    tz: &chrono_tz::Tz,
    date_after: impl Fn(TimeQuery) -> Query,
    date_before: impl Fn(TimeQuery) -> Query,
) -> Query {
    let cmp = reader.next().expect("parsing date cmp without an operator");
    let timequery = reader.next().expect("parsing date cmp without a timequery");
    let timequery = match timequery.as_rule() {
        Rule::abstimeq => TimeQuery::Absolute(
            // TODO: for safety, see (currently open) https://github.com/chronotope/chrono/pull/927
            midnight_on(
                chrono::NaiveDate::parse_from_str(timequery.as_str(), "%Y-%m-%d")
                    .expect("parsing date cmp with ill-formed absolute date"),
                tz,
            )
            .with_timezone(&chrono::Utc),
        ),
        Rule::reltimeq => {
            let mut reader = timequery.into_inner();
            let op = reader.next();
            match op {
                None => TimeQuery::DayRelative {
                    timezone: tz.clone(),
                    day_offset: 0,
                },
                Some(op) => {
                    let offset = reader
                        .next()
                        .expect("parsing relative time query without offset");
                    let offset =
                        i64::from_str(offset.as_str()).expect("failed parsing i64 from str");
                    let day_offset = match op.as_str() {
                        "+" => offset,
                        "-" => -offset,
                        _ => unreachable!("got unexpected offset operator"),
                    };
                    TimeQuery::DayRelative {
                        timezone: tz.clone(),
                        day_offset,
                    }
                }
            }
        }
        _ => unreachable!("got unexpected timequery type"),
    };
    match cmp.as_str() {
        ">" => date_after(start_of_next_day(tz, timequery)),
        "<=" => date_before(start_of_next_day(tz, timequery)),
        "<" => date_before(timequery),
        ">=" => date_after(timequery),
        ":" => Query::All(vec![
            date_after(timequery.clone()),
            date_before(start_of_next_day(tz, timequery)),
        ]),
        _ => panic!("parsing date cmp with ill-formed cmp op"),
    }
}

fn start_of_next_day<Tz>(tz: &Tz, day: TimeQuery) -> TimeQuery
where
    Tz: Clone + std::fmt::Debug + chrono::TimeZone,
{
    match day {
        TimeQuery::DayRelative {
            timezone,
            day_offset,
        } => TimeQuery::DayRelative {
            timezone,
            day_offset: day_offset + 1,
        },
        TimeQuery::Absolute(t) => TimeQuery::Absolute(
            midnight_on(
                t.date_naive()
                    .succ_opt()
                    .expect("failed figuring out a date for day+1"),
                tz,
            )
            .with_timezone(&chrono::Utc),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::*;
    use std::{collections::HashMap, sync::Arc};

    fn example_db() -> DbDump {
        let mut tags = HashMap::new();
        let mut perms = HashMap::new();
        for t in ["foo", "bar", "baz"] {
            let id = TagId(Uuid::new_v4());
            tags.insert(
                id,
                Tag {
                    id,
                    owner_id: UserId::stub(),
                    name: String::from(t),
                    archived: false,
                },
            );
            perms.insert(id, AuthInfo::all());
        }
        DbDump {
            owner: UserId::stub(),
            users: Arc::new(HashMap::new()),
            tags: Arc::new(tags),
            perms: Arc::new(perms),
            searches: Arc::new(HashMap::new()),
            tasks: Arc::new(HashMap::new()),
        }
    }

    fn example_tz() -> chrono_tz::Tz {
        chrono_tz::Tz::Europe__Paris
    }

    fn phrase(s: &str) -> Query {
        Query::Phrase(s.to_string())
    }

    #[test]
    fn primary_archived() {
        let db = example_db();
        let tz = example_tz();
        assert_eq!(
            Query::from_search(&db, &tz, "archived:true"),
            Query::Archived(true),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "archived:false"),
            Query::Archived(false),
        );
    }

    #[test]
    fn primary_done() {
        let db = example_db();
        let tz = example_tz();
        assert_eq!(Query::from_search(&db, &tz, "done:true"), Query::Done(true),);
        assert_eq!(
            Query::from_search(&db, &tz, "done:false"),
            Query::Done(false),
        );
    }

    #[test]
    fn primary_tag() {
        let db = example_db();
        let tz = example_tz();
        assert_eq!(
            Query::from_search(&db, &tz, "tag:foo"),
            Query::tag(db.tag_id("foo").unwrap()),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "tag:bar"),
            Query::tag(db.tag_id("bar").unwrap()),
        );
    }

    #[test]
    fn primary_untagged() {
        let db = example_db();
        let tz = example_tz();
        assert_eq!(
            Query::from_search(&db, &tz, "untagged:true"),
            Query::Untagged(true),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "untagged:false"),
            Query::Untagged(false),
        );
    }

    #[test]
    fn primary_word() {
        let db = example_db();
        let tz = example_tz();

        // Basic words (including tag name)
        assert_eq!(Query::from_search(&db, &tz, "test"), phrase("test"),);
        assert_eq!(Query::from_search(&db, &tz, "foo"), phrase("foo"),);

        // Words matching special query parameters
        assert_eq!(Query::from_search(&db, &tz, "archived"), phrase("archived"),);
        assert_eq!(Query::from_search(&db, &tz, "tag"), phrase("tag"),);
    }

    #[test]
    fn primary_phrase() {
        let db = example_db();
        let tz = example_tz();

        // Basic usage
        assert_eq!(Query::from_search(&db, &tz, r#""test""#), phrase("test"),);
        assert_eq!(
            Query::from_search(&db, &tz, r#""foo bar""#),
            phrase("foo bar"),
        );

        // Things that look like queries
        assert_eq!(
            Query::from_search(&db, &tz, r#""(foo bar OR archived:false)""#),
            phrase("(foo bar OR archived:false)"),
        );
        assert_eq!(Query::from_search(&db, &tz, r#""(test""#), phrase("(test"),);

        // Escapes
        assert_eq!(
            Query::from_search(&db, &tz, r#""foo\" bar""#),
            phrase(r#"foo" bar"#),
        );
        assert_eq!(
            Query::from_search(&db, &tz, r#""foo\\ bar""#),
            phrase(r#"foo\ bar"#),
        );
        assert_eq!(
            Query::from_search(&db, &tz, r#""foo\\\" bar""#),
            phrase(r#"foo\" bar"#),
        );
    }

    #[test]
    fn infixes() {
        let db = example_db();
        let tz = example_tz();

        // Nothing is and
        assert_eq!(
            Query::from_search(&db, &tz, "foo bar"),
            Query::All(vec![phrase("foo"), phrase("bar")]),
        );
        assert_eq!(
            Query::from_search(&db, &tz, r#""foo bar" "baz""#),
            Query::All(vec![phrase("foo bar"), phrase("baz")]),
        );

        // Explicit and
        assert_eq!(
            Query::from_search(&db, &tz, "foo AND archived:false"),
            Query::All(vec![phrase("foo"), Query::Archived(false)]),
        );

        // Explicit or
        assert_eq!(
            Query::from_search(&db, &tz, "foo or archived:false"),
            Query::Any(vec![phrase("foo"), Query::Archived(false)]),
        );
    }

    #[test]
    fn complex() {
        let db = example_db();
        let tz = example_tz();
        assert_eq!(
            Query::from_search(&db, &tz, "foo bar baz"),
            Query::All(vec![phrase("foo"), phrase("bar"), phrase("baz")]),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "foo bar or baz"),
            Query::All(vec![
                phrase("foo"),
                Query::Any(vec![phrase("bar"), phrase("baz")])
            ]),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "(foo bar) or baz"),
            Query::Any(vec![
                Query::All(vec![phrase("foo"), phrase("bar")]),
                phrase("baz")
            ]),
        );
        assert_eq!(
            Query::from_search(&db, &tz, "(archived:true bar) or baz"),
            Query::Any(vec![
                Query::All(vec![Query::Archived(true), phrase("bar")]),
                phrase("baz")
            ]),
        );
    }
}
