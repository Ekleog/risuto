use crate::{api::Query, DbDump};

use pest::iterators::Pairs;
use pest::pratt_parser::PrattParser;

#[derive(pest_derive::Parser)]
#[grammar = "search.pest"]
pub struct Parser;

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

pub fn parse_search(db: &DbDump, pairs: Pairs<Rule>) -> Query {
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
            Rule::search => parse_search(db, p.into_inner()),
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
