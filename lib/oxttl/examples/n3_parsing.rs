//! N3 Parsing Example with Formulas
//!
//! This example demonstrates how to parse N3 (Notation3) files with formulas.
//! N3 extends Turtle with additional features including:
//! - Formulas: {...} blocks that quote/reify statements
//! - Variables: ?x or $x syntax for universal quantification
//! - Path expressions: property chains using ! and ^
//!
//! Formulas in N3 allow statements to be used as terms (subjects/objects),
//! enabling reasoning about statements themselves (meta-level reasoning).
//!
//! Run with: cargo run -p oxttl --example n3_parsing

use oxrdf::GraphName;
use oxttl::n3::{N3Parser, N3Quad, N3Term};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== N3 Parsing Example with Formulas ===\n");

    // Example 1: Parse simple N3 with formulas
    parse_basic_formula_example()?;

    // Example 2: Parse nested formulas
    parse_nested_formulas_example()?;

    // Example 3: Parse formulas with variables
    parse_formula_with_variables_example()?;

    // Example 4: Extract and analyze formula contents
    analyze_formula_structure_example()?;

    // Example 5: Error handling with malformed N3
    error_handling_example()?;

    // Example 6: Parse from file (simulated)
    parse_complex_n3_example()?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Example 1: Parse basic N3 with formulas
///
/// Formulas in N3 are enclosed in curly braces {...} and represent quoted statements.
/// The graph_name field in N3Quad encodes which formula a statement belongs to.
fn parse_basic_formula_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: Basic Formula Parsing ---");

    let n3_data = r#"
        @prefix ex: <http://example.com/> .
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .

        # Alice believes a statement (the statement is quoted in a formula)
        ex:alice ex:believes {
            ex:bob foaf:name "Bob" .
        } .
    "#;

    println!("Parsing N3 data:");
    println!("{}", n3_data);

    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_data.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads", quads.len());

    // Separate quads by context: default graph vs. formulas
    let (default_graph_quads, formula_quads): (Vec<_>, Vec<_>) = quads
        .iter()
        .partition(|q| q.graph_name == GraphName::DefaultGraph);

    println!("  - {} quads in default graph", default_graph_quads.len());
    println!("  - {} quads inside formulas", formula_quads.len());

    // Display the outer statement
    for quad in &default_graph_quads {
        println!("\n  Outer statement:");
        println!("    Subject:   {}", quad.subject);
        println!("    Predicate: {}", quad.predicate);
        println!("    Object:    {} (type: {})",
            quad.object,
            match &quad.object {
                N3Term::BlankNode(_) => "Formula (represented as blank node)",
                N3Term::NamedNode(_) => "Named Node",
                N3Term::Literal(_) => "Literal",
                N3Term::Variable(_) => "Variable",
                #[cfg(feature = "rdf-12")]
                N3Term::Triple(_) => "Triple (RDF-star)",
            }
        );
    }

    // Display statements inside formulas
    println!("\n  Statements inside formula:");
    for quad in &formula_quads {
        println!("    {} {} {}", quad.subject, quad.predicate, quad.object);
        println!("      (in formula: {:?})", quad.graph_name);
    }

    println!();
    Ok(())
}

/// Example 2: Parse nested formulas
///
/// N3 supports arbitrary nesting of formulas, allowing complex meta-level reasoning.
fn parse_nested_formulas_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Nested Formulas ---");

    let n3_data = r#"
        @prefix ex: <http://example.com/> .

        # Alice believes that Bob thinks something
        ex:alice ex:believes {
            ex:bob ex:thinks {
                ex:charlie ex:age 25 .
                ex:charlie ex:city "Boston" .
            } .
        } .
    "#;

    println!("Parsing nested formulas:");
    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_data.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads total", quads.len());

    // Group quads by their formula context (graph_name)
    let mut formulas: HashMap<String, Vec<&N3Quad>> = HashMap::new();

    for quad in &quads {
        let key = match &quad.graph_name {
            GraphName::DefaultGraph => "default".to_string(),
            GraphName::NamedNode(n) => format!("named:{}", n),
            GraphName::BlankNode(b) => format!("formula:{}", b),
        };
        formulas.entry(key).or_default().push(quad);
    }

    println!("  Found {} different contexts (formulas)", formulas.len());

    for (context, quads) in &formulas {
        println!("\n  Context: {}", context);
        println!("    Quads: {}", quads.len());
        for quad in quads {
            println!("      {} {} {}", quad.subject, quad.predicate, quad.object);
        }
    }

    println!();
    Ok(())
}

/// Example 3: Parse formulas with variables
///
/// Variables in N3 use ?var or $var syntax and can appear anywhere,
/// including inside formulas for representing rules and queries.
fn parse_formula_with_variables_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: Formulas with Variables ---");

    let n3_data = r#"
        @prefix ex: <http://example.com/> .
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .

        # A rule using variables and formulas
        ex:grandparentRule ex:states {
            ?person ex:parent ?child .
            ?child ex:parent ?grandchild .
        } .

        # Another formula with mixed variables and constants
        ex:example ex:pattern {
            ?x ex:knows ex:bob .
            ex:bob ex:knows ?y .
        } .
    "#;

    println!("Parsing formulas with variables:");
    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_data.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads", quads.len());

    // Find quads inside formulas that contain variables
    let formula_quads: Vec<_> = quads
        .iter()
        .filter(|q| q.graph_name != GraphName::DefaultGraph)
        .collect();

    println!("  {} quads inside formulas\n", formula_quads.len());

    // Analyze variable usage
    for (i, quad) in formula_quads.iter().enumerate() {
        println!("  Formula statement {}:", i + 1);

        // Check subject
        match &quad.subject {
            N3Term::Variable(v) => println!("    Subject:   ?{} (variable)", v.as_str()),
            other => println!("    Subject:   {} (constant)", other),
        }

        // Predicate
        println!("    Predicate: {}", quad.predicate);

        // Check object
        match &quad.object {
            N3Term::Variable(v) => println!("    Object:    ?{} (variable)", v.as_str()),
            other => println!("    Object:    {} (constant)", other),
        }
    }

    println!();
    Ok(())
}

/// Example 4: Extract and analyze formula structure
///
/// This example shows how to programmatically analyze the structure of
/// formulas, extracting variables and building a representation.
fn analyze_formula_structure_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 4: Analyzing Formula Structure ---");

    let n3_data = r#"
        @prefix ex: <http://example.com/> .

        ex:rule1 ex:defines {
            ?person a ex:Person .
            ?person ex:name ?name .
            ?person ex:email ?email .
        } .
    "#;

    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_data.as_bytes())
        .collect::<Result<_, _>>()?;

    // Extract variables from formula quads
    let mut variables = Vec::new();

    for quad in &quads {
        if quad.graph_name != GraphName::DefaultGraph {
            // Check subject
            if let N3Term::Variable(v) = &quad.subject {
                if !variables.contains(&v.as_str().to_string()) {
                    variables.push(v.as_str().to_string());
                }
            }

            // Check object
            if let N3Term::Variable(v) = &quad.object {
                if !variables.contains(&v.as_str().to_string()) {
                    variables.push(v.as_str().to_string());
                }
            }
        }
    }

    println!("Extracted variables from formula: {:?}", variables);

    // Count statements in formula
    let formula_statement_count = quads
        .iter()
        .filter(|q| q.graph_name != GraphName::DefaultGraph)
        .count();

    println!("Formula contains {} statements", formula_statement_count);

    // Identify the formula's blank node identifier
    for quad in &quads {
        if quad.graph_name == GraphName::DefaultGraph {
            if let N3Term::BlankNode(bn) = &quad.object {
                println!("Formula is identified by blank node: {}", bn);
            }
        }
    }

    println!();
    Ok(())
}

/// Example 5: Error handling with malformed N3
///
/// Demonstrates how to handle parsing errors gracefully.
fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 5: Error Handling ---");

    // Valid N3
    let valid_n3 = r#"
        @prefix ex: <http://example.com/> .
        ex:alice ex:knows ex:bob .
    "#;

    println!("Parsing valid N3:");
    match N3Parser::new().for_slice(valid_n3.as_bytes()).collect::<Result<Vec<_>, _>>() {
        Ok(quads) => println!("✓ Successfully parsed {} quads", quads.len()),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Malformed N3 - unclosed formula
    let invalid_n3 = r#"
        @prefix ex: <http://example.com/> .
        ex:alice ex:believes {
            ex:bob ex:knows ex:charlie .
        # Missing closing brace
    "#;

    println!("\nParsing malformed N3 (unclosed formula):");
    match N3Parser::new().for_slice(invalid_n3.as_bytes()).collect::<Result<Vec<_>, _>>() {
        Ok(quads) => println!("✓ Parsed {} quads (unexpected)", quads.len()),
        Err(e) => println!("✓ Correctly detected error: {}", e),
    }

    // Lenient mode - more forgiving parsing
    println!("\nTrying lenient mode:");
    let lenient_quads: Result<Vec<_>, _> = N3Parser::new()
        .lenient()
        .for_slice(valid_n3.as_bytes())
        .collect();

    match lenient_quads {
        Ok(quads) => println!("✓ Lenient parsing succeeded with {} quads", quads.len()),
        Err(e) => println!("✗ Error: {}", e),
    }

    println!();
    Ok(())
}

/// Example 6: Parse complex N3 with multiple features
///
/// Combines formulas, variables, prefixes, and base IRIs.
fn parse_complex_n3_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 6: Complex N3 Document ---");

    let complex_n3 = r#"
        @base <http://example.org/> .
        @prefix : <http://example.org/> .
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

        # Real-world use case: A knowledge base with beliefs and rules

        :alice foaf:name "Alice Smith" ;
               foaf:age "30"^^xsd:integer ;
               :believes {
                   :bob foaf:knows :charlie .
                   :charlie :worksAt :ACME .
               } .

        :bob foaf:name "Bob Jones" ;
             :claims {
                 ?company :hasEmployee ?person .
                 ?person :worksAt ?company .
             } .

        # Formula representing a policy
        :policy1 :states {
            ?employee :worksAt ?company .
            ?company :location ?city .
        } ;
        :validFrom "2024-01-01"^^xsd:date .
    "#;

    println!("Parsing complex N3 document with multiple features...");

    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(complex_n3.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Successfully parsed {} quads\n", quads.len());

    // Analyze the document structure
    let default_count = quads.iter().filter(|q| q.graph_name == GraphName::DefaultGraph).count();
    let formula_count = quads.iter().filter(|q| q.graph_name != GraphName::DefaultGraph).count();

    println!("Document structure:");
    println!("  - {} quads in default graph", default_count);
    println!("  - {} quads in formulas", formula_count);

    // Count unique formulas
    let unique_formulas: std::collections::HashSet<_> = quads
        .iter()
        .filter_map(|q| match &q.graph_name {
            GraphName::BlankNode(bn) => Some(bn.clone()),
            _ => None,
        })
        .collect();

    println!("  - {} unique formulas", unique_formulas.len());

    // Count variables
    let mut variable_names = std::collections::HashSet::new();
    for quad in &quads {
        if let N3Term::Variable(v) = &quad.subject {
            variable_names.insert(v.as_str());
        }
        if let N3Term::Variable(v) = &quad.object {
            variable_names.insert(v.as_str());
        }
    }

    println!("  - {} unique variables: {:?}", variable_names.len(), variable_names);

    // Display a sample of the parsed data
    println!("\nSample parsed quads:");
    for (i, quad) in quads.iter().take(5).enumerate() {
        let context = match &quad.graph_name {
            GraphName::DefaultGraph => "default graph",
            _ => "formula",
        };
        println!("  {}. [{}] {} {} {}",
            i + 1, context, quad.subject, quad.predicate, quad.object);
    }

    println!();
    Ok(())
}
