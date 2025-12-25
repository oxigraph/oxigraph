//! Example: Querying N3 data with formulas using SPARQL
//!
//! This example demonstrates how to:
//! 1. Parse N3 data that contains formulas (quoted graphs)
//! 2. Convert N3 quads to regular RDF quads for storage
//! 3. Query formula contents using SPARQL via named graphs
//! 4. Extract and manipulate formulas from datasets
//!
//! Formulas in N3 are represented as RDF quads where the graph name is a blank node.
//! This allows formulas to be queried like any other named graph in SPARQL.

use oxrdf::{BlankNode, Dataset, Formula, GraphName, NamedNode};
use oxttl::n3::N3Parser;
use spareval::{QueryEvaluator, QueryResults};
use spargebra::SparqlParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== N3 Formula Querying with SPARQL ===\n");

    // Example 1: Parse and query simple N3 data (no formulas)
    example_1_simple_n3()?;

    // Example 2: Query formula contents via GRAPH pattern
    example_2_query_formulas()?;

    // Example 3: Complex scenario with beliefs/quotations
    example_3_beliefs()?;

    // Example 4: Extract formulas from dataset
    example_4_extract_formulas()?;

    Ok(())
}

fn example_1_simple_n3() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 1: Simple N3 data without formulas");
    println!("-------------------------------------------");

    let n3_data = r#"
        @prefix ex: <http://example.com/> .

        ex:alice ex:knows ex:bob .
        ex:bob ex:knows ex:charlie .
        ex:charlie ex:knows ex:alice .
    "#;

    // Parse N3 and convert to Dataset
    let mut dataset = Dataset::new();
    for result in N3Parser::new().for_reader(n3_data.as_bytes()) {
        let n3_quad = result?;
        // Only add quads that don't contain variables
        if let Some(quad) = n3_quad.try_into_quad() {
            dataset.insert(quad);
        }
    }

    println!("Loaded {} triples", dataset.len());

    // Query: Who does Alice know?
    let query = SparqlParser::new().parse_query(
        r#"PREFIX ex: <http://example.com/>
           SELECT ?person WHERE {
               ex:alice ex:knows ?person
           }"#,
    )?;

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        println!("\nQuery: Who does Alice know?");
        for solution in solutions {
            let solution = solution?;
            println!("  - {}", solution.get("person").unwrap());
        }
    }

    println!();
    Ok(())
}

fn example_2_query_formulas() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 2: Query formula contents via GRAPH pattern");
    println!("----------------------------------------------------");

    // Manually create a dataset with formulas
    // In N3, formulas are like { ?x ex:knows ?y } and are represented
    // as named graphs with blank node identifiers
    let formula_id = BlankNode::new("belief1")?;
    let alice = NamedNode::new("http://example.com/alice")?;
    let knows = NamedNode::new("http://example.com/knows")?;
    let bob = NamedNode::new("http://example.com/bob")?;

    let mut dataset = Dataset::new();

    // Add triple inside the formula (as a named graph)
    dataset.insert(oxrdf::Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::BlankNode(formula_id.clone()),
    ));

    println!("Created formula {} with 1 triple", formula_id);

    // Query the formula contents using GRAPH pattern
    let query = SparqlParser::new().parse_query(&format!(
        r#"PREFIX ex: <http://example.com/>
           SELECT ?s ?p ?o WHERE {{
               GRAPH _:{} {{ ?s ?p ?o }}
           }}"#,
        formula_id.as_str()
    ))?;

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        println!("\nQuery: What's in the formula?");
        for solution in solutions {
            let solution = solution?;
            println!(
                "  {} {} {}",
                solution.get("s").unwrap(),
                solution.get("p").unwrap(),
                solution.get("o").unwrap()
            );
        }
    }

    println!();
    Ok(())
}

fn example_3_beliefs() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 3: Beliefs and quotations using formulas");
    println!("------------------------------------------------");

    // Model a scenario where Alice believes that Bob knows Charlie
    // The belief is a formula, and we have metadata about who believes it

    let belief1 = BlankNode::new("belief1")?;
    let alice = NamedNode::new("http://example.com/alice")?;
    let bob = NamedNode::new("http://example.com/bob")?;
    let charlie = NamedNode::new("http://example.com/charlie")?;
    let believes = NamedNode::new("http://example.com/believes")?;
    let knows = NamedNode::new("http://example.com/knows")?;

    let mut dataset = Dataset::new();

    // Metadata: Alice believes belief1
    dataset.insert(oxrdf::Quad::new(
        alice.clone(),
        believes.clone(),
        belief1.clone(),
        GraphName::DefaultGraph,
    ));

    // Content of belief1 (a formula): Bob knows Charlie
    dataset.insert(oxrdf::Quad::new(
        bob.clone(),
        knows.clone(),
        charlie.clone(),
        GraphName::BlankNode(belief1.clone()),
    ));

    println!("Alice believes that Bob knows Charlie");
    println!("Belief ID: {}", belief1);

    // Query: What does Alice believe, and what are the contents?
    let query = SparqlParser::new().parse_query(
        r#"PREFIX ex: <http://example.com/>
           SELECT ?believer ?subject ?predicate ?object WHERE {
               ?believer ex:believes ?belief .
               GRAPH ?belief { ?subject ?predicate ?object }
           }"#,
    )?;

    let evaluator = QueryEvaluator::new();
    let results = evaluator.prepare(&query).execute(&dataset)?;

    if let QueryResults::Solutions(solutions) = results {
        println!("\nQuery: Who believes what?");
        for solution in solutions {
            let solution = solution?;
            println!(
                "  {} believes that {} {} {}",
                solution.get("believer").unwrap(),
                solution.get("subject").unwrap(),
                solution.get("predicate").unwrap(),
                solution.get("object").unwrap()
            );
        }
    }

    println!();
    Ok(())
}

fn example_4_extract_formulas() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 4: Extract formulas from dataset");
    println!("-----------------------------------------");

    // Create a dataset with multiple formulas
    let f1 = BlankNode::new("formula1")?;
    let f2 = BlankNode::new("formula2")?;
    let ex = NamedNode::new("http://example.com/")?;

    let mut dataset = Dataset::new();

    // Formula 1 with one triple
    dataset.insert(oxrdf::Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f1.clone()),
    ));

    // Formula 2 with two triples
    dataset.insert(oxrdf::Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f2.clone()),
    ));
    dataset.insert(oxrdf::Quad::new(
        ex.clone(),
        ex.clone(),
        ex.clone(),
        GraphName::BlankNode(f2.clone()),
    ));

    println!("Created dataset with 2 formulas");

    // Extract all formulas from the dataset
    let formulas = Formula::from_dataset(&dataset);

    println!("\nExtracted {} formulas:", formulas.len());
    for formula in &formulas {
        println!("  - Formula {}: {} triples", formula.id(), formula.triples().len());
    }

    // Access individual formulas
    let formula1 = formulas.iter().find(|f| f.id() == &f1).unwrap();
    println!("\nFormula 1 details:");
    println!("  ID: {}", formula1.id());
    println!("  Triples: {}", formula1.triples().len());
    for triple in formula1.triples() {
        println!("    {}", triple);
    }

    println!();
    Ok(())
}
