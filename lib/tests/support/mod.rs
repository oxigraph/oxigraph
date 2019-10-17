#![cfg_attr(test, allow(dead_code))]

use failure::format_err;
use rudf::model::*;
use rudf::sparql::{BindingsIterator, GraphPattern, PreparedQuery, QueryOptions, QueryResult};
use rudf::{GraphSyntax, MemoryRepository, Repository, RepositoryConnection, Result};
use std::io::BufRead;


pub(crate) fn ex(id: String) -> Term {
    Term::NamedNode(NamedNode::parse(format!("http://example.com/{}", &id)).unwrap())
}

pub(crate) fn mailto(id: String) -> Term {
    Term::NamedNode(NamedNode::parse(format!("mailto:{}", &id)).unwrap())
}

pub(crate) fn literal(str: String) -> Term {
    Term::Literal(Literal::new_simple_literal(str))
}

pub(crate) fn make_repository(reader: impl BufRead) -> Result<MemoryRepository> {
    let repository = MemoryRepository::default();
    let mut connection = repository.connection()?;
    connection
        .load_graph(reader, GraphSyntax::NTriples, None, None)
        .unwrap();
    Ok(repository)
}

pub(crate) fn query_repository<'a>(
    repository: MemoryRepository,
    query: String,
    options: QueryOptions<'a>,
) -> Result<BindingsIterator<'a>> {
    match repository
        .connection()?
        .prepare_query(&query, options)?
        .exec()?
    {
        QueryResult::Bindings(iterator) => {
            let (varaibles, iter) = iterator.destruct();
            let collected = iter.collect::<Vec<_>>();
            Ok(BindingsIterator::new(
                varaibles,
                Box::new(collected.into_iter()),
            ))
        }
        _ => Err(format_err!(
            "Excpected bindings but got another QueryResult"
        )),
    }
}

pub(crate) fn pattern_repository<'a>(
    repository: MemoryRepository,
    pattern: GraphPattern,
    options: QueryOptions<'a>,
) -> Result<BindingsIterator<'a>> {
    match repository
        .connection()?
        .prepare_query_from_pattern(&pattern, options)?
        .exec()?
    {
        QueryResult::Bindings(iterator) => {
            let (varaibles, iter) = iterator.destruct();
            let collected = iter.collect::<Vec<_>>();
            Ok(BindingsIterator::new(
                varaibles,
                Box::new(collected.into_iter()),
            ))
        }
        _ => Err(format_err!("Expected bindings but got another QueryResult")),
    }
}

pub(crate) fn do_query<'a>(
    reader: impl BufRead,
    query: String,
    options: QueryOptions<'a>,
) -> Result<BindingsIterator<'a>> {
    let repository = make_repository(reader)?;
    query_repository(repository, query, options)
}

pub(crate) fn do_pattern<'a>(
    reader: impl BufRead,
    pattern: GraphPattern,
    options: QueryOptions<'a>,
) -> Result<BindingsIterator<'a>> {
    let repository = make_repository(reader)?;
    pattern_repository(repository, pattern, options)
}
