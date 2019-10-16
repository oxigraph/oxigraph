use rudf::model::*;
use rudf::{GraphSyntax, Repository, RepositoryConnection, MemoryRepository, Result};
use rudf::sparql::{BindingsIterator, GraphPattern, PreparedQuery, QueryOptions, QueryResult, ServiceHandler};
use failure::format_err;
use std::io::BufRead;



#[test]
fn simple_service_test() {

  struct TestServiceHandler;
  impl ServiceHandler for TestServiceHandler {
    fn handle<'a>(&'a self, _named_node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
      fn pattern_handler<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
        let triples = b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .".as_ref();
        do_pattern(triples, graph_pattern, QueryOptions::default())
      };
      Some(pattern_handler)
    }
  }

  let query = r#"
  SELECT ?s ?p ?o
  WHERE
    { 
      SERVICE <http://service1.org>
      { ?s ?p ?o
      }
   }
  "#.to_string();


  let options = QueryOptions::default().with_service_handler(Box::new(TestServiceHandler));
  let results = do_query(b"".as_ref(), query, options).unwrap();
  let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
  let solution = vec![
      vec![ Some(ex(String::from("s"))), Some(ex(String::from("p"))), Some(ex(String::from("o"))) ],
  ];
  assert_eq!(collected, solution);
  
}


#[test]
fn two_service_test() {

  #[derive(Clone,Copy)]
  struct TwoServiceTest;
  impl ServiceHandler for TwoServiceTest {
      fn handle<'a>(&'a self, named_node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
          let service1 = NamedNode::parse("http://service1.org").unwrap();
          let service2 = NamedNode::parse("http://service2.org").unwrap();
          if named_node == service1 { Some(TwoServiceTest::handle_service1) }
          else if named_node == service2 { Some(TwoServiceTest::handle_service2) }
          else { None } 
      }
  }


  impl TwoServiceTest {

    fn handle_service1<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
      let triples = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/name> "Bob" .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
        "#.as_ref();
      do_pattern(triples, graph_pattern, QueryOptions::default())
    }

    fn handle_service2<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
      let triples = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/mbox> <mailto:bob@example.com> .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/mbox> <mailto:alice@example.com> .
        "#.as_ref();
      do_pattern(triples, graph_pattern, QueryOptions::default())
    }

  }

  let query = r#"
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name ?mbox 
  WHERE
    { 
      SERVICE <http://service1.org>
      { ?s foaf:name ?name
      }

      SERVICE <http://service2.org>
      { ?s foaf:mbox ?mbox
      }
    }
  ORDER BY ?name
  "#.to_string();

  let options = QueryOptions::default().with_service_handler(Box::new(TwoServiceTest));
  let results = do_query(b"".as_ref(), query, options).unwrap();
  let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
  let solution = vec![
      vec![ Some(literal("Alice".to_string())), Some(mailto("alice@example.com".to_string())) ],
      vec![ Some(literal("Bob".to_string())), Some(mailto("bob@example.com".to_string())) ],
    ];
  assert_eq!(collected, solution);
}

fn ex(id: String) -> Term {
  Term::NamedNode(NamedNode::parse(format!("http://example.com/{}", &id)).unwrap())
}

fn mailto(id: String) -> Term {
  Term::NamedNode(NamedNode::parse(format!("mailto:{}", &id)).unwrap())
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
  let prepared_query = connection.prepare_query(&query, &options)?;
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

//fn pattern_repository<'a>(repository: MemoryRepository, pattern: GraphPattern, options: QueryOptions<'a>) -> Result<Vec<Vec<Option<Term>>>> {
fn pattern_repository<'a>(repository: MemoryRepository, pattern: GraphPattern, options: QueryOptions<'a>) -> Result<BindingsIterator<'a>> {
  let connection = repository.connection()?;
  let prepared_query = connection.prepare_query_from_pattern(&pattern, &options)?;
  let result = prepared_query.exec(&options)?;
  match result {
    QueryResult::Bindings(iterator) => {
      let (varaibles, iter) = iterator.destruct();
      let collected = iter.collect::<Vec<_>>();
      Ok(BindingsIterator::new(varaibles, Box::new(collected.into_iter())))
    }
    _ => Err(format_err!("Excpected bindings but got another QueryResult"))
  }
}

//fn do_query<'a>(reader: impl BufRead, query: String, options: QueryOptions<'a>) -> Result<Vec<Vec<Option<Term>>>> {
fn do_query<'a>(reader: impl BufRead, query: String, options: QueryOptions<'a>) -> Result<BindingsIterator<'a>> {
  let repository = make_repository(reader)?;
  query_repository(repository, query, options)
}

//fn do_pattern<'a>(reader: impl BufRead, pattern: GraphPattern, options: QueryOptions<'a>) -> Result<Vec<Vec<Option<Term>>>> {
fn do_pattern<'a>(reader: impl BufRead, pattern: GraphPattern, options: QueryOptions<'a>) -> Result<BindingsIterator<'a>> {
  let repository = make_repository(reader)?;
  pattern_repository(repository, pattern, options)
}
