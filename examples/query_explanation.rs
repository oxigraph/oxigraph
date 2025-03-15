use oxigraph::model::*;
use oxigraph::sparql::{QueryOptions, QueryResults};
use oxigraph::store::Store;
use serde_json::{from_slice, to_string_pretty};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Oxigraph Query Execution Explanation ===\n");

    // Step 1: Create an in-memory RDF store
    println!("Step 1: Creating an in-memory RDF store");
    let store = Store::new()?;

    // Step 2: Insert sample data about books and authors
    println!("Step 2: Inserting sample data");

    // Helper function to create URIs for our entities
    let ex_uri = |suffix: &str| -> Result<NamedNode, Box<dyn Error>> {
        Ok(NamedNode::new(format!("http://example.org/{}", suffix))?)
    };

    let rdf_uri = |suffix: &str| -> Result<NamedNode, Box<dyn Error>> {
        Ok(NamedNode::new(format!(
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#{}",
            suffix
        ))?)
    };

    let rdfs_uri = |suffix: &str| -> Result<NamedNode, Box<dyn Error>> {
        Ok(NamedNode::new(format!(
            "http://www.w3.org/2000/01/rdf-schema#{}",
            suffix
        ))?)
    };

    // Add class definitions
    store.insert(&Quad::new(
        ex_uri("Book")?,
        rdf_uri("type")?,
        rdfs_uri("Class")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("Author")?,
        rdf_uri("type")?,
        rdfs_uri("Class")?,
        GraphName::DefaultGraph,
    ))?;

    // Add some books and authors
    store.insert(&Quad::new(
        ex_uri("book1")?,
        rdf_uri("type")?,
        ex_uri("Book")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("book1")?,
        ex_uri("title")?,
        Literal::new_simple_literal("The Lord of the Rings"),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("book1")?,
        ex_uri("author")?,
        ex_uri("author1")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("author1")?,
        rdf_uri("type")?,
        ex_uri("Author")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("author1")?,
        ex_uri("name")?,
        Literal::new_simple_literal("J.R.R. Tolkien"),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("author1")?,
        ex_uri("born")?,
        Literal::new_typed_literal(
            "1892-01-03",
            NamedNode::new("http://www.w3.org/2001/XMLSchema#date")?,
        ),
        GraphName::DefaultGraph,
    ))?;

    // Add more books and authors
    store.insert(&Quad::new(
        ex_uri("book2")?,
        rdf_uri("type")?,
        ex_uri("Book")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("book2")?,
        ex_uri("title")?,
        Literal::new_simple_literal("Dune"),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("book2")?,
        ex_uri("author")?,
        ex_uri("author2")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("author2")?,
        rdf_uri("type")?,
        ex_uri("Author")?,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        ex_uri("author2")?,
        ex_uri("name")?,
        Literal::new_simple_literal("Frank Herbert"),
        GraphName::DefaultGraph,
    ))?;

    println!("Data loaded: {} triples in the store", store.len()?);

    // Step 3: Define a SPARQL query
    println!("\nStep 3: Defining a SPARQL query");
    let query = r#"
    PREFIX ex: <http://example.org/>
    PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
    
    SELECT ?book ?title ?author_name
    WHERE {
        ?book rdf:type ex:Book .       # Find all books
        ?book ex:title ?title .        # Get their titles
        ?book ex:author ?author .      # Find the author relationship
        ?author ex:name ?author_name . # Get author names
        
        # Only include books with "The" in the title
        FILTER(CONTAINS(?title, "The"))
    }
    "#;

    println!("Query to execute:");
    println!("{}", query);

    // Step 4: Execute the query and get the explanation
    println!("\nStep 4: Query Execution Process\n");

    // Use explain_query_opt to get both results and explanation
    // Set with_stats to true to get statistics about the execution
    let (results, explanation) = store.explain_query_opt(query, QueryOptions::default(), true)?;

    // Print the explanation in debug format
    println!("{:#?}", explanation);

    // Step 5: Actually show the query results
    println!("\nStep 5: Actual Query Results\n");

    match results? {
        QueryResults::Solutions(solutions) => {
            println!("| Book | Title | Author |");
            println!("| ---- | ----- | ------ |");
            for solution in solutions {
                let solution = solution?;
                let book = solution.get("book").unwrap().to_string();
                let title = solution.get("title").unwrap().to_string();
                let author = solution.get("author_name").unwrap().to_string();
                println!("| {} | {} | {} |", book, title, author);
            }
        }
        _ => println!("Unexpected query result type"),
    }

    // Optionally, we could also output the explanation in JSON format:
    println!("\nExplanation in JSON format:");
    let mut buffer = Vec::new();
    explanation.write_in_json(&mut buffer)?;

    // Parse the JSON and then pretty print it
    let json_value: serde_json::Value = from_slice(&buffer)?;
    let pretty_json = to_string_pretty(&json_value)?;
    println!("{}", pretty_json);

    Ok(())
}
