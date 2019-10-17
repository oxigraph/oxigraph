use failure::format_err;
use rudf::model::*;
use rudf::sparql::{
    BindingsIterator, GraphPattern, QueryOptions, ServiceHandler,
};
use rudf::Result;

mod support;
use support::*;

#[test]
fn simple_service_test() {
    struct TestServiceHandler;
    impl ServiceHandler for TestServiceHandler {
        fn handle<'a>(
            &'a self,
            _named_node: &NamedNode,
        ) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
            fn pattern_handler<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
                let triples =
                    b"<http://example.com/s> <http://example.com/p> <http://example.com/o> ."
                        .as_ref();
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
  "#
    .to_string();

    let options = QueryOptions::default().with_service_handler(Box::new(TestServiceHandler));
    let results = do_query(b"".as_ref(), query, options).unwrap();
    let collected = results
        .into_values_iter()
        .map(move |b| b.unwrap())
        .collect::<Vec<_>>();
    let solution = vec![vec![
        Some(ex(String::from("s"))),
        Some(ex(String::from("p"))),
        Some(ex(String::from("o"))),
    ]];
    assert_eq!(collected, solution);
}

#[test]
fn two_service_test() {
    #[derive(Clone, Copy)]
    struct TwoServiceTest;
    impl ServiceHandler for TwoServiceTest {
        fn handle<'a>(
            &'a self,
            named_node: &NamedNode,
        ) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
            let service1 = NamedNode::parse("http://service1.org").unwrap();
            let service2 = NamedNode::parse("http://service2.org").unwrap();
            if named_node == &service1 {
                Some(TwoServiceTest::handle_service1)
            } else if named_node == &service2 {
                Some(TwoServiceTest::handle_service2)
            } else {
                None
            }
        }
    }

    impl TwoServiceTest {
        fn handle_service1<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
            let triples = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/name> "Bob" .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
        "#
            .as_ref();
            do_pattern(triples, graph_pattern, QueryOptions::default())
        }

        fn handle_service2<'a>(graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
            let triples = br#"
        <http://example.com/bob> <http://xmlns.com/foaf/0.1/mbox> <mailto:bob@example.com> .
        <http://example.com/alice> <http://xmlns.com/foaf/0.1/mbox> <mailto:alice@example.com> .
        "#
            .as_ref();
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
  "#
    .to_string();

    let options = QueryOptions::default().with_service_handler(Box::new(TwoServiceTest));
    let results = do_query(b"".as_ref(), query, options).unwrap();
    let collected = results
        .into_values_iter()
        .map(move |b| b.unwrap())
        .collect::<Vec<_>>();
    let solution = vec![
        vec![
            Some(literal("Alice".to_string())),
            Some(mailto("alice@example.com".to_string())),
        ],
        vec![
            Some(literal("Bob".to_string())),
            Some(mailto("bob@example.com".to_string())),
        ],
    ];
    assert_eq!(collected, solution);
}

#[test]
fn silent_service_empty_set_test() {
    #[derive(Clone, Copy)]
    struct ServiceTest;
    impl ServiceHandler for ServiceTest {
        fn handle<'a>(
            &'a self,
            _named_node: &NamedNode,
        ) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
            Some(ServiceTest::handle_service)
        }
    }

    impl ServiceTest {
        fn handle_service<'a>(_graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
            Err(format_err!("This is supposed to fail"))
        }
    }

    let query = r#"
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name ?mbox 
  WHERE
    { 
      SERVICE SILENT <http://service1.org>
      { ?s foaf:name ?name
      }
      
    }
  ORDER BY ?name
  "#
    .to_string();

    let triples = b"".as_ref();
    let options = QueryOptions::default().with_service_handler(Box::new(ServiceTest));
    let results = do_query(triples, query, options).unwrap();
    let collected = results
        .into_values_iter()
        .map(move |b| b.unwrap())
        .collect::<Vec<_>>();
    println!("Collected: {:?}", collected);
    assert_eq!(collected.len(), 0);
}

#[test]
fn non_silent_service_test() {
    #[derive(Clone, Copy)]
    struct ServiceTest;
    impl ServiceHandler for ServiceTest {
        fn handle<'a>(
            &'a self,
            _named_node: &NamedNode,
        ) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)> {
            Some(ServiceTest::handle_service)
        }
    }

    impl ServiceTest {
        fn handle_service<'a>(_graph_pattern: GraphPattern) -> Result<BindingsIterator<'a>> {
            Err(format_err!("This is supposed to fail"))
        }
    }

    let query = r#"
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name
  WHERE
    { 
      SERVICE <http://service1.org>
      { ?s foaf:name ?name
      }
      
    }
  ORDER BY ?name
  "#
    .to_string();

    let triples = b"".as_ref();
    let options = QueryOptions::default().with_service_handler(Box::new(ServiceTest));
    let results = do_query(triples, query, options).unwrap();
    let result = results.into_values_iter().next();
    match result {
        Some(Err(_)) => assert_eq!(true, true),
        _ => assert_eq!(
            true, false,
            "This should have been an error since the service fails"
        ),
    }
}