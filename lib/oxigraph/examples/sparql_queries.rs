//! SPARQL Queries Example
//!
//! This example demonstrates SPARQL query operations with Oxigraph:
//! - SPARQL SELECT queries
//! - SPARQL ASK queries
//! - SPARQL CONSTRUCT queries
//! - SPARQL DESCRIBE queries
//! - Using the SparqlEvaluator with custom options
//! - Iterating over query results
//!
//! Run with: cargo run -p oxigraph --example sparql_queries

use oxigraph::io::RdfFormat;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Oxigraph SPARQL Queries Example ===\n");

    // Create and populate a store with sample data
    let store = setup_sample_data()?;

    // Example 1: SELECT queries
    select_query_example(&store)?;

    // Example 2: ASK queries
    ask_query_example(&store)?;

    // Example 3: CONSTRUCT queries
    construct_query_example(&store)?;

    // Example 4: DESCRIBE queries
    describe_query_example(&store)?;

    // Example 5: Complex queries with filters and aggregations
    complex_query_example(&store)?;

    // Example 6: Using SparqlEvaluator options
    evaluator_options_example(&store)?;

    println!("\n=== All SPARQL examples completed successfully! ===");
    Ok(())
}

/// Set up sample RDF data for the examples
fn setup_sample_data() -> Result<Store, Box<dyn std::error::Error>> {
    println!("--- Setting up sample data ---");

    let store = Store::new()?;

    // Load sample Turtle data
    let turtle_data = r#"
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

ex:alice a schema:Person ;
    schema:name "Alice Anderson" ;
    schema:age 30 ;
    schema:email "alice@example.com" ;
    schema:knows ex:bob, ex:charlie .

ex:bob a schema:Person ;
    schema:name "Bob Brown" ;
    schema:age 25 ;
    schema:email "bob@example.com" ;
    schema:knows ex:alice .

ex:charlie a schema:Person ;
    schema:name "Charlie Chen" ;
    schema:age 35 ;
    schema:email "charlie@example.com" ;
    schema:knows ex:alice .

ex:diana a schema:Person ;
    schema:name "Diana Davis" ;
    schema:age 28 ;
    schema:email "diana@example.com" .

ex:book1 a schema:Book ;
    schema:name "Introduction to RDF" ;
    schema:author ex:alice ;
    schema:datePublished "2020-01-15"^^xsd:date ;
    schema:numberOfPages 250 .

ex:book2 a schema:Book ;
    schema:name "SPARQL Queries Explained" ;
    schema:author ex:charlie ;
    schema:datePublished "2021-06-20"^^xsd:date ;
    schema:numberOfPages 300 .

ex:book3 a schema:Book ;
    schema:name "Semantic Web Basics" ;
    schema:author ex:alice ;
    schema:datePublished "2022-03-10"^^xsd:date ;
    schema:numberOfPages 200 .
"#;

    store.load_from_reader(RdfFormat::Turtle, turtle_data.as_bytes())?;

    let count = store.len()?;
    println!("✓ Loaded {} triples into store\n", count);

    Ok(store)
}

/// Example 1: SELECT queries
fn select_query_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: SELECT Queries ---");

    // Simple SELECT query
    let query = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?name ?age WHERE {
            ?person a schema:Person ;
                    schema:name ?name ;
                    schema:age ?age .
        }
        ORDER BY ?age
    "#;

    println!("Query: Get all people with their names and ages");

    if let QueryResults::Solutions(mut solutions) =
        SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()?
    {
        // Get variable names
        let variables = solutions.variables();
        println!("Variables: {:?}", variables);

        // Iterate over solutions
        let mut count = 0;
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            count += 1;

            let name = solution.get("name");
            let age = solution.get("age");

            println!(
                "  Solution {}: name={:?}, age={:?}",
                count,
                name.map(|t| t.to_string()),
                age.map(|t| t.to_string())
            );
        }
        println!("✓ Found {} solutions", count);
    }

    // SELECT with FILTER
    let query_with_filter = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?name WHERE {
            ?person a schema:Person ;
                    schema:name ?name ;
                    schema:age ?age .
            FILTER(?age >= 30)
        }
    "#;

    println!("\nQuery: People aged 30 or older");

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query_with_filter)?
        .on_store(store)
        .execute()?
    {
        let names: Vec<_> = solutions
            .filter_map(|s| s.ok())
            .filter_map(|s| s.get("name").map(|t| t.to_string()))
            .collect();

        for name in &names {
            println!("  - {}", name);
        }
        println!("✓ Found {} people aged 30+", names.len());
    }

    println!();
    Ok(())
}

/// Example 2: ASK queries
fn ask_query_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: ASK Queries ---");

    // ASK query to check if data exists
    let query = r#"
        PREFIX schema: <http://schema.org/>
        PREFIX ex: <http://example.com/>
        ASK {
            ex:alice schema:knows ex:bob .
        }
    "#;

    println!("Query: Does Alice know Bob?");

    if let QueryResults::Boolean(result) =
        SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()?
    {
        println!("✓ Result: {}", if result { "YES" } else { "NO" });
    }

    // Another ASK query
    let query2 = r#"
        PREFIX schema: <http://schema.org/>
        ASK {
            ?person a schema:Person ;
                    schema:age ?age .
            FILTER(?age > 100)
        }
    "#;

    println!("\nQuery: Is there anyone over 100 years old?");

    if let QueryResults::Boolean(result) =
        SparqlEvaluator::new().parse_query(query2)?.on_store(store).execute()?
    {
        println!("✓ Result: {}", if result { "YES" } else { "NO" });
    }

    println!();
    Ok(())
}

/// Example 3: CONSTRUCT queries
fn construct_query_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: CONSTRUCT Queries ---");

    // CONSTRUCT a new graph from existing data
    let query = r#"
        PREFIX schema: <http://schema.org/>
        PREFIX ex: <http://example.com/>
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        CONSTRUCT {
            ?person foaf:name ?name ;
                    foaf:mbox ?email .
        }
        WHERE {
            ?person a schema:Person ;
                    schema:name ?name ;
                    schema:email ?email .
        }
    "#;

    println!("Query: Transform schema.org vocabulary to FOAF vocabulary");

    if let QueryResults::Graph(triples) =
        SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()?
    {
        let mut count = 0;
        for triple in triples {
            let triple = triple?;
            count += 1;
            if count <= 5 {
                // Only print first 5
                println!("  {}", triple);
            }
        }
        println!("✓ Constructed {} triples", count);
    }

    // CONSTRUCT with pattern matching
    let query2 = r#"
        PREFIX schema: <http://schema.org/>
        PREFIX ex: <http://example.com/>

        CONSTRUCT {
            ?author ex:hasWritten ?book .
            ?book ex:writtenBy ?author .
        }
        WHERE {
            ?book a schema:Book ;
                  schema:author ?author .
        }
    "#;

    println!("\nQuery: Create bidirectional author-book relationships");

    if let QueryResults::Graph(triples) =
        SparqlEvaluator::new().parse_query(query2)?.on_store(store).execute()?
    {
        let count = triples.count();
        println!("✓ Constructed {} relationship triples", count);
    }

    println!();
    Ok(())
}

/// Example 4: DESCRIBE queries
fn describe_query_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 4: DESCRIBE Queries ---");

    // DESCRIBE a specific resource
    let query = r#"
        PREFIX ex: <http://example.com/>
        DESCRIBE ex:alice
    "#;

    println!("Query: Describe everything about ex:alice");

    if let QueryResults::Graph(triples) =
        SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()?
    {
        let mut count = 0;
        for triple in triples {
            let triple = triple?;
            count += 1;
            println!("  {}", triple);
        }
        println!("✓ Found {} triples describing Alice", count);
    }

    println!();
    Ok(())
}

/// Example 5: Complex queries with aggregations and grouping
fn complex_query_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 5: Complex Queries ---");

    // Query with aggregation
    let query = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?author (COUNT(?book) as ?bookCount) (AVG(?pages) as ?avgPages)
        WHERE {
            ?book a schema:Book ;
                  schema:author ?author ;
                  schema:numberOfPages ?pages .
        }
        GROUP BY ?author
        HAVING (COUNT(?book) > 1)
        ORDER BY DESC(?bookCount)
    "#;

    println!("Query: Authors with multiple books and their average page count");

    if let QueryResults::Solutions(mut solutions) =
        SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            let author = solution.get("author").map(|t| t.to_string());
            let book_count = solution.get("bookCount").map(|t| t.to_string());
            let avg_pages = solution.get("avgPages").map(|t| t.to_string());

            println!(
                "  Author: {:?}, Books: {:?}, Avg Pages: {:?}",
                author, book_count, avg_pages
            );
        }
        println!("✓ Aggregation query completed");
    }

    // Query with OPTIONAL and FILTER
    let query2 = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?name (COUNT(?friend) as ?friendCount)
        WHERE {
            ?person a schema:Person ;
                    schema:name ?name .
            OPTIONAL {
                ?person schema:knows ?friend .
            }
        }
        GROUP BY ?name
        ORDER BY DESC(?friendCount)
    "#;

    println!("\nQuery: People ordered by number of friends (including those with 0 friends)");

    if let QueryResults::Solutions(mut solutions) =
        SparqlEvaluator::new().parse_query(query2)?.on_store(store).execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            let name = solution.get("name").map(|t| t.to_string());
            let friend_count = solution.get("friendCount").map(|t| t.to_string());
            println!("  {:?}: {:?} friends", name, friend_count);
        }
        println!("✓ Optional pattern query completed");
    }

    // Subquery example
    let query3 = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?name ?age
        WHERE {
            {
                SELECT (MAX(?age) as ?maxAge)
                WHERE {
                    ?person a schema:Person ;
                            schema:age ?age .
                }
            }
            ?person a schema:Person ;
                    schema:name ?name ;
                    schema:age ?age .
            FILTER(?age = ?maxAge)
        }
    "#;

    println!("\nQuery: Find the oldest person");

    if let QueryResults::Solutions(mut solutions) =
        SparqlEvaluator::new().parse_query(query3)?.on_store(store).execute()?
    {
        if let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "✓ Oldest person: {:?}, age: {:?}",
                solution.get("name").map(|t| t.to_string()),
                solution.get("age").map(|t| t.to_string())
            );
        }
    }

    println!();
    Ok(())
}

/// Example 6: Using SparqlEvaluator with custom options
fn evaluator_options_example(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 6: SparqlEvaluator Options ---");

    // Custom base IRI
    let query = r#"
        SELECT ?s WHERE { ?s ?p ?o }
        LIMIT 1
    "#;

    println!("Query with custom base IRI:");

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .with_base_iri("http://example.com/base")?
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        if let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  First subject: {:?}", solution.get("s"));
        }
    }
    println!("✓ Query with base IRI executed");

    // Custom prefix
    let query2 = r#"
        SELECT ?name WHERE {
            ?person a schema:Person ;
                    schema:name ?name .
        }
        LIMIT 1
    "#;

    println!("\nQuery with custom prefix:");

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .with_prefix("schema", "http://schema.org/")?
        .parse_query(query2)?
        .on_store(store)
        .execute()?
    {
        if let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  First name: {:?}", solution.get("name"));
        }
    }
    println!("✓ Query with custom prefix executed");

    // Custom function
    let query3 = r#"
        PREFIX ex: <http://example.com/>
        PREFIX custom: <http://example.com/custom/>
        SELECT (custom:double(5) AS ?result) WHERE {}
    "#;

    println!("\nQuery with custom function (double):");

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .with_custom_function(
            NamedNode::new("http://example.com/custom/double")?,
            |args| {
                args.get(0).and_then(|term| {
                    if let Term::Literal(lit) = term {
                        if let Ok(value) = lit.value().parse::<i64>() {
                            return Some(Literal::from(value * 2).into());
                        }
                    }
                    None
                })
            },
        )
        .parse_query(query3)?
        .on_store(store)
        .execute()?
    {
        if let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  Result: {:?}", solution.get("result"));
        }
    }
    println!("✓ Query with custom function executed");

    // Query with VALUES clause
    let query4 = r#"
        PREFIX schema: <http://schema.org/>
        SELECT ?name ?age WHERE {
            VALUES ?name { "Alice Anderson" "Bob Brown" }
            ?person schema:name ?name ;
                    schema:age ?age .
        }
    "#;

    println!("\nQuery with VALUES clause:");

    if let QueryResults::Solutions(mut solutions) =
        SparqlEvaluator::new().parse_query(query4)?.on_store(store).execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {:?}: {:?} years old",
                solution.get("name"),
                solution.get("age")
            );
        }
    }
    println!("✓ VALUES query executed");

    println!();
    Ok(())
}
