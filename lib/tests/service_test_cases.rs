use rudf::model::*;
use rudf::{GraphSyntax, Repository, RepositoryConnection, MemoryRepository, Result};
use rudf::sparql::{BindingsIterator, GraphPattern, PreparedQuery, QueryOptions, QueryResult, ServiceHandler};
use failure::format_err;

fn ex(id: String) -> Term {
  Term::NamedNode(NamedNode::parse(format!("http://example.com/{}", &id)).unwrap())
}

fn foaf(id: String) -> Term {
  Term::NamedNode(NamedNode::parse(format!("http://xmlns.com/foaf/0.1/{}", &id)).unwrap())
}

fn mailto(id: String) -> Term {
  Term::NamedNode(NamedNode::parse(format!("mailto:{}", &id)).unwrap())
}

fn literal(str: String) -> Term {
  Term::Literal(Literal::new_simple_literal(str))
}

/*
#[derive(Clone,Copy)]
struct SimpleServiceTest;
impl ServiceHandler for SimpleServiceTest {
    fn handle<'a>(&'a self, named_node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
      Some(SimpleServiceTest::handle_service) 
    }
}

impl SimpleServiceTest {
  fn handle_service<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
    let repository = MemoryRepository::default();
    let mut connection = repository.connection().unwrap();
    let file = b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
    connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None).unwrap();
    let prepared_query = connection.prepare_query_from_pattern(&graph_pattern, None).unwrap();
    let result = prepared_query.exec(&Some(SimpleServiceTest)).unwrap();
    match result {
      QueryResult::Bindings(iterator) => {
        let (variables, iter) = iterator.destruct();
        let cloned_iter = iter.collect::<Vec<_>>().into_iter();
        let new_iter = BindingsIterator::new(variables, Box::new(cloned_iter));
        Ok(new_iter)
      },
      _ => Err(format_err!("Excpected bindings but got another QueryResult"))
    }
  }
}
*/

#[test]
fn simple_service_test() {

  struct TestServiceHandler;
  impl ServiceHandler for TestServiceHandler {
    fn handle<'a>(&'a self, named_node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
      fn pattern_handler<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
        let repository = MemoryRepository::default();
        let mut connection = repository.connection().unwrap();
        let file = b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
        connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None).unwrap();
        let query_options = QueryOptions::default();
        let prepared_query = connection.prepare_query_from_pattern(&graph_pattern, &query_options).unwrap();
        let result = prepared_query.exec(&query_options).unwrap();
        match result {
          QueryResult::Bindings(iterator) => {
            let (variables, iter) = iterator.destruct();
            let cloned_iter = iter.collect::<Vec<_>>().into_iter();
            let new_iter = BindingsIterator::new(variables, Box::new(cloned_iter));
            Ok(new_iter)
          },
          _ => Err(format_err!("Excpected bindings but got another QueryResult"))
        }
      };
      Some(pattern_handler)
    }
  }

  
  

  let repository = MemoryRepository::default();
  let connection = repository.connection().unwrap();

  let query = r#"
  SELECT ?s ?p ?o
  WHERE
    { 
      SERVICE <http://service1.org>
      { ?s ?p ?o
      }
   }
  "#;


  let query_options = QueryOptions::default().with_service_handler(Box::new(TestServiceHandler));
  let prepared_query = connection.prepare_query(query, &query_options).unwrap();
  let results = prepared_query.exec(&query_options).unwrap();
  if let QueryResult::Bindings(results) = results {
    let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
    let solution = vec![
      vec![ Some(ex(String::from("s"))), Some(ex(String::from("p"))), Some(ex(String::from("o"))) ],
    ];
    assert_eq!(collected, solution);
  } else {
    assert_eq!(true, false);
  }
}




#[test]
fn two_service_test() {

  #[derive(Clone,Copy)]
  struct TwoServiceTest;
  impl ServiceHandler for TwoServiceTest {
      fn handle<'a>(&'a self, named_node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
          println!("Handler called for {:?}", named_node);   
          let service1 = NamedNode::parse("http://service1.org").unwrap();
          let service2 = NamedNode::parse("http://service2.org").unwrap();
          if named_node == service1 { Some(TwoServiceTest::handle_service1) }
          else if named_node == service2 { Some(TwoServiceTest::handle_service2) }
          else { None} 
      }
  }


  impl TwoServiceTest {

    fn handle_service1<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
      let repository = MemoryRepository::default();
      let mut connection = repository.connection().unwrap();
      let file = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/name> "Bob" .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
        "#;
      connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None).unwrap();
      let query_options = QueryOptions::default().with_service_handler(Box::new(TwoServiceTest));
      let prepared_query = connection.prepare_query_from_pattern(&graph_pattern, &query_options).unwrap();
      let result = prepared_query.exec(&query_options).unwrap();
      match result {
        QueryResult::Bindings(iterator) => {
          let (variables, iter) = iterator.destruct();
          let cloned_iter = iter.collect::<Vec<_>>().into_iter();
          let new_iter = BindingsIterator::new(variables, Box::new(cloned_iter));
          Ok(new_iter)
        },
        _ => Err(format_err!("Excpected bindings but got another QueryResult"))
      }
    }




    fn handle_service2<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
      let repository = MemoryRepository::default();
      let mut connection = repository.connection().unwrap();
      let file = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/mbox> <mailto:bob@example.com> .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/mbox> <mailto:alice@example.com> .
        "#;
      connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None).unwrap();
      let query_options = QueryOptions::default().with_service_handler(Box::new(TwoServiceTest));
      let prepared_query = connection.prepare_query_from_pattern(&graph_pattern, &query_options).unwrap();
      let result = prepared_query.exec(&query_options).unwrap();
      match result {
        QueryResult::Bindings(iterator) => {
          let (variables, iter) = iterator.destruct();
          let cloned_iter = iter.collect::<Vec<_>>().into_iter();
          let new_iter = BindingsIterator::new(variables, Box::new(cloned_iter));
          Ok(new_iter)
        },
        _ => Err(format_err!("Excpected bindings but got another QueryResult"))
      }
    }

  }

  let repository = MemoryRepository::default();
  let connection = repository.connection().unwrap();

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
  "#;

  let query_options = QueryOptions::default().with_service_handler(Box::new(TwoServiceTest));
  let prepared_query = connection.prepare_query(query, &query_options).unwrap();
  let results = prepared_query.exec(&query_options).unwrap();
  if let QueryResult::Bindings(results) = results {
    let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
    for c in collected.clone() {
      println!("{:?}", c);
    }
    println!("\n\n\n");
    let solution = vec![
      vec![ Some(literal("Alice".to_string())), Some(mailto("alice@example.com".to_string())) ],
      vec![ Some(literal("Bob".to_string())), Some(mailto("bob@example.com".to_string())) ],
    ];
    println!("Results: {:?}", collected);
    assert_eq!(collected, solution);
  } else {
    assert_eq!(true, false);
  }
}


