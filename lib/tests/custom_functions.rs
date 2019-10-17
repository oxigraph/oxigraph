use rudf::model::*;
use rudf::{GraphSyntax, Repository, RepositoryConnection, MemoryRepository, Result};
use rudf::sparql::{BindingsIterator, PreparedQuery, QueryOptions, QueryResult};
use failure::format_err;
use std::io::BufRead;



#[test]
fn simple_custom_function_test() {

  let query = r#"
  PREFIX ex: <http://example.com#>
  SELECT ?name ?reverse
  WHERE
    { 
      ?s <http://xmlns.com/foaf/0.1/name> ?name .
      BIND(ex:REVERSE(?name) as ?reverse)
    }
  ORDER BY ?name
  "#.to_string();


  let options = QueryOptions::default();
  let triples = br#"
    <http://example.com/bob> <http://xmlns.com/foaf/0.1/name> "Bob" .
    <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
    <http://example.com/bob> <http://xmlns.com/foaf/0.1/mbox> <mailto:bob@example.com> .
    <http://example.com/alice> <http://xmlns.com/foaf/0.1/mbox> <mailto:alice@example.com> .
    "#.as_ref();
  let results = do_query(triples, query, options).unwrap();
  let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
  let solution = vec![
      vec![ Some(literal(String::from("Alice"))), Some(literal(String::from("ecilA"))) ],
      vec![ Some(literal(String::from("Bob"))), Some(literal(String::from("boB"))) ],
  ];
  assert_eq!(collected, solution);
  
}

fn literal(str: String) -> Term {
  Term::Literal(Literal::new_simple_literal(str))
}

fn make_repository(reader: impl BufRead) -> Result<MemoryRepository> {
  let repository = MemoryRepository::default();
  let mut connection = repository.connection()?;
  connection.load_graph(reader, GraphSyntax::NTriples, None, None).unwrap();
  Ok(repository)
}

fn query_repository<'a>(repository: MemoryRepository, query: String, options: QueryOptions<'a>) -> Result<BindingsIterator<'a>> {
  let connection = repository.connection()?;
  let prepared_query = connection.prepare_query(&query, None)?;
  let result = prepared_query.exec(&options)?;
  match result {
    QueryResult::Bindings(iterator) => {
      let (varaibles, iter) = iterator.destruct();
      let collected = iter.collect::<Vec<_>>();
      Ok(BindingsIterator::new(varaibles, Box::new(collected.into_iter())))
    },
    _ => Err(format_err!("Excpected bindings but got another QueryResult"))
  }
} 

fn do_query<'a>(reader: impl BufRead, query: String, options: QueryOptions<'a>) -> Result<BindingsIterator<'a>> {
  let repository = make_repository(reader)?;
  query_repository(repository, query, options)
}
