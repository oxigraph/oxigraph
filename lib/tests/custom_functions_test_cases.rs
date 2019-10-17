use rudf::model::*;
use rudf::sparql::{CustomFunctionsHandler, QueryOptions};

mod support;
use support::*;

#[test]
fn simple_custom_function_test() {

  struct TestHandler;

  impl CustomFunctionsHandler for TestHandler {
    fn handle(&self, node: &NamedNode, parameters: &Vec<Option<Term>>) -> Option<Term> {
      let reverse = NamedNode::parse("http://example.com#REVERSE").ok()?;
      if *node == reverse {
        let param = &parameters[0];
        if let Some(Term::Literal(literal)) = param {
          let value = literal.value();
          let reversed = value.chars().rev().collect::<String>();
          let literal = Literal::new_simple_literal(reversed);
          Some(Term::Literal(literal))
        } else {
          None 
        }
      } else {
        None
      }
    }
  }

  let query = r#"
  PREFIX ex: <http://example.com#>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name ?reverse
  WHERE
    { 
      ?s foaf:name ?name .
      BIND(ex:REVERSE(?name) as ?reverse)
    }
  ORDER BY ?name
  "#.to_string();


  let options = QueryOptions::default().with_custom_functions_handler(Box::new(TestHandler));
  let triples = br#"
    <http://example.com/bob> <http://xmlns.com/foaf/0.1/name> "Bob" .
    <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
    "#.as_ref();
  let results = do_query(triples, query, options).unwrap();
  let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
  let solution = vec![
      vec![ Some(literal(String::from("Alice"))), Some(literal(String::from("ecilA"))) ],
      vec![ Some(literal(String::from("Bob"))), Some(literal(String::from("boB"))) ],
  ];
  assert_eq!(collected, solution);
  
}

#[test]
fn simple_default_custom_function_test() {

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
    "#.as_ref();
  let results = do_query(triples, query, options).unwrap();
  let collected = results.into_values_iter().map(move |b| b.unwrap()).collect::<Vec<_>>();
  let solution = vec![
      vec![ Some(literal(String::from("Alice"))), None ],
      vec![ Some(literal(String::from("Bob"))), None ],
  ];
  assert_eq!(collected, solution);
  
}
