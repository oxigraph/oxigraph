//! Tests for SPARQL querying over N3 data including formulas.
//!
//! This module demonstrates how to:
//! - Parse N3 data with formulas
//! - Convert N3 data to RDF quads for storage
//! - Query formula contents using SPARQL via named graphs
//! - Extract and work with formulas from datasets

use oxrdf::{BlankNode, Dataset, Formula, GraphName, NamedNode, Quad};
use oxttl::n3::{N3Parser, N3Quad};
use spareval::{QueryEvaluator, QueryResults};
use spargebra::SparqlParser;

#[test]
fn test_query_n3_data_without_formulas() {
    // Parse simple N3 data (no formulas)
    let n3_data = r#"
        @prefix ex: <http://example.com/> .
        ex:alice ex:knows ex:bob .
        ex:bob ex:knows ex:charlie .
    "#;

    let mut dataset = Dataset::new();
    for result in N3Parser::new().for_reader(n3_data.as_bytes()) {
        let n3_quad = result.unwrap();
        if let Some(quad) = n3_quad.try_into_quad() {
            dataset.insert(quad);
        }
    }

    // Query the data
    let query = SparqlParser::new()
        .parse_query("PREFIX ex: <http://example.com/> SELECT ?x WHERE { ex:alice ex:knows ?x }")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset).unwrap();

    if let QueryResults::Solutions(mut solutions) = results {
        let solution = solutions.next().unwrap().unwrap();
        let x = solution.get("x").unwrap();
        assert_eq!(
            x.to_string(),
            "<http://example.com/bob>"
        );
    } else {
        panic!("Expected solutions");
    }
}

#[test]
fn test_query_formula_contents_via_graph() {
    // N3 data with a formula encoded as a named graph with blank node
    let formula_id = BlankNode::new("f1").unwrap();
    let ex_alice = NamedNode::new("http://example.com/alice").unwrap();
    let ex_knows = NamedNode::new("http://example.com/knows").unwrap();
    let ex_bob = NamedNode::new("http://example.com/bob").unwrap();

    let mut dataset = Dataset::new();

    // Add a triple inside the formula (represented as a named graph)
    dataset.insert(Quad::new(
        ex_alice.clone(),
        ex_knows.clone(),
        ex_bob.clone(),
        GraphName::BlankNode(formula_id.clone()),
    ));

    // Query the formula contents by querying the named graph
    let query = SparqlParser::new()
        .parse_query(&format!(
            "PREFIX ex: <http://example.com/> SELECT ?x ?y WHERE {{ GRAPH _:{} {{ ?x ex:knows ?y }} }}",
            formula_id.as_str()
        ))
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset).unwrap();

    if let QueryResults::Solutions(mut solutions) = results {
        let solution = solutions.next().unwrap().unwrap();
        assert_eq!(solution.get("x").unwrap().to_string(), "<http://example.com/alice>");
        assert_eq!(solution.get("y").unwrap().to_string(), "<http://example.com/bob>");
    } else {
        panic!("Expected solutions");
    }
}

#[test]
fn test_extract_formulas_from_dataset() {
    // Create a dataset with multiple formulas
    let f1 = BlankNode::new("f1").unwrap();
    let f2 = BlankNode::new("f2").unwrap();
    let ex = NamedNode::new("http://example.com/").unwrap();

    let mut dataset = Dataset::new();

    // Formula 1
    dataset.insert(Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f1.clone()),
    ));

    // Formula 2 with multiple triples
    dataset.insert(Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f2.clone()),
    ));
    dataset.insert(Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f2.clone()),
    ));

    // Extract formulas
    let formulas = Formula::from_dataset(&dataset);

    assert_eq!(formulas.len(), 2);

    // Find formulas by ID
    let formula1 = formulas.iter().find(|f| f.id() == &f1).unwrap();
    let formula2 = formulas.iter().find(|f| f.id() == &f2).unwrap();

    assert_eq!(formula1.triples().len(), 1);
    assert_eq!(formula2.triples().len(), 2);
}

#[test]
fn test_query_all_formula_graphs() {
    // Create a dataset with formulas
    let f1 = BlankNode::new("f1").unwrap();
    let f2 = BlankNode::new("f2").unwrap();
    let ex = NamedNode::new("http://example.com/").unwrap();

    let mut dataset = Dataset::new();

    // Add data to different formula graphs
    dataset.insert(Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f1.clone()),
    ));
    dataset.insert(Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f2.clone()),
    ));

    // Query across all graphs
    let query = SparqlParser::new()
        .parse_query("PREFIX ex: <http://example.com/> SELECT (COUNT(*) as ?count) WHERE { GRAPH ?g { ?s ?p ?o } }")
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset).unwrap();

    if let QueryResults::Solutions(mut solutions) = results {
        let solution = solutions.next().unwrap().unwrap();
        let count = solution.get("count").unwrap();
        // Should find 2 triples in named graphs
        assert_eq!(count.to_string(), "\"2\"^^<http://www.w3.org/2001/XMLSchema#integer>");
    } else {
        panic!("Expected solutions");
    }
}

#[test]
fn test_n3_quad_conversion() {
    let ex = NamedNode::new("http://example.com/").unwrap();

    // Create an N3Quad without variables
    let n3_quad = N3Quad::new(
        ex.clone().into(),
        ex.clone().into(),
        ex.clone().into(),
        GraphName::DefaultGraph,
    );

    // Should convert successfully
    let quad = n3_quad.try_into_quad();
    assert!(quad.is_some());

    let quad = quad.unwrap();
    assert_eq!(quad.subject.to_string(), "<http://example.com/>");
    assert_eq!(quad.predicate.to_string(), "<http://example.com/>");
    assert_eq!(quad.object.to_string(), "<http://example.com/>");
}

#[test]
fn test_n3_quad_with_variable_no_conversion() {
    use oxrdf::Variable;
    use oxttl::n3::N3Term;

    let ex = NamedNode::new("http://example.com/").unwrap();
    let var = Variable::new("x").unwrap();

    // Create an N3Quad with a variable
    let n3_quad = N3Quad::new(
        N3Term::Variable(var),
        ex.clone().into(),
        ex.clone().into(),
        GraphName::DefaultGraph,
    );

    // Should NOT convert because it has a variable
    let quad = n3_quad.try_into_quad();
    assert!(quad.is_none());
}

#[test]
fn test_formula_to_quads_round_trip() {
    let id = BlankNode::new("f1").unwrap();
    let ex = NamedNode::new("http://example.com/").unwrap();

    // Create a formula
    let triple = oxrdf::Triple::new(ex.clone(), ex.clone(), ex.clone());
    let formula = Formula::new(id.clone(), vec![triple]);

    // Convert to quads
    let quads = formula.to_quads();
    assert_eq!(quads.len(), 1);

    // Add to dataset
    let mut dataset = Dataset::new();
    for quad in quads {
        dataset.insert(quad);
    }

    // Extract formulas back
    let formulas = Formula::from_dataset(&dataset);
    assert_eq!(formulas.len(), 1);
    assert_eq!(formulas[0].id(), &id);
}

#[test]
fn test_query_formula_metadata() {
    // Create a dataset where formulas themselves are subjects
    let f1 = BlankNode::new("f1").unwrap();
    let ex_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type").unwrap();
    let ex_formula = NamedNode::new("http://example.com/Formula").unwrap();
    let ex_says = NamedNode::new("http://example.com/says").unwrap();
    let ex_alice = NamedNode::new("http://example.com/alice").unwrap();

    let mut dataset = Dataset::new();

    // Metadata about the formula in the default graph
    dataset.insert(Quad::new(
        f1.clone(),
        ex_type,
        ex_formula,
        GraphName::DefaultGraph,
    ));
    dataset.insert(Quad::new(
        ex_alice.clone(),
        ex_says.clone(),
        f1.clone(),
        GraphName::DefaultGraph,
    ));

    // Content inside the formula
    dataset.insert(Quad::new(
        ex_alice.clone(),
        ex_says.clone(),
        ex_alice.clone(),
        GraphName::BlankNode(f1.clone()),
    ));

    // Query for who says what
    let query = SparqlParser::new()
        .parse_query(&format!(
            "PREFIX ex: <http://example.com/> SELECT ?who ?formula WHERE {{ ?who ex:says ?formula }}"
        ))
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset).unwrap();

    if let QueryResults::Solutions(mut solutions) = results {
        let solution = solutions.next().unwrap().unwrap();
        assert_eq!(solution.get("who").unwrap().to_string(), "<http://example.com/alice>");
        assert_eq!(solution.get("formula").unwrap().to_string(), format!("_:{}", f1.as_str()));
    } else {
        panic!("Expected solutions");
    }
}

#[test]
fn test_complex_n3_formula_scenario() {
    // Simulate a scenario where we have:
    // - Regular triples in the default graph
    // - Formula contents in named graphs (blank nodes)
    // - Metadata linking to formulas

    let f1 = BlankNode::new("belief1").unwrap();
    let alice = NamedNode::new("http://example.com/alice").unwrap();
    let bob = NamedNode::new("http://example.com/bob").unwrap();
    let believes = NamedNode::new("http://example.com/believes").unwrap();
    let knows = NamedNode::new("http://example.com/knows").unwrap();

    let mut dataset = Dataset::new();

    // Alice believes something (formula)
    dataset.insert(Quad::new(
        alice.clone(),
        believes.clone(),
        f1.clone(),
        GraphName::DefaultGraph,
    ));

    // The content of what Alice believes (in the formula)
    dataset.insert(Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::BlankNode(f1.clone()),
    ));

    // Query: What does Alice believe?
    let query = SparqlParser::new()
        .parse_query(&format!(
            r#"PREFIX ex: <http://example.com/>
               SELECT ?belief ?subject ?predicate ?object WHERE {{
                 ex:alice ex:believes ?belief .
                 GRAPH ?belief {{ ?subject ?predicate ?object }}
               }}"#
        ))
        .unwrap();

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset).unwrap();

    if let QueryResults::Solutions(mut solutions) = results {
        let solution = solutions.next().unwrap().unwrap();
        assert_eq!(solution.get("subject").unwrap().to_string(), "<http://example.com/alice>");
        assert_eq!(solution.get("predicate").unwrap().to_string(), "<http://example.com/knows>");
        assert_eq!(solution.get("object").unwrap().to_string(), "<http://example.com/bob>");
    } else {
        panic!("Expected solutions");
    }
}
