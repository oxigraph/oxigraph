//! N3 Rules and Implications Example
//!
//! This example demonstrates how to work with N3 rules (implications) which are
//! a key feature of N3 reasoning. While Oxigraph focuses on parsing and serializing
//! N3, this example shows the structure of N3 rules and how to extract them.
//!
//! N3 rules typically use the log:implies predicate to connect premise (antecedent)
//! formulas to conclusion (consequent) formulas:
//!
//! { ?x :parent ?y } => { ?y :hasParent ?x } .
//!
//! This is represented in N3 as:
//! { ?x :parent ?y } log:implies { ?y :hasParent ?x } .
//!
//! Run with: cargo run -p oxttl --example n3_rules

use oxrdf::{BlankNode, GraphName, NamedNode, Variable};
use oxttl::n3::{N3Parser, N3Quad, N3Serializer, N3Term};
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== N3 Rules and Implications Example ===\n");

    // Example 1: Parse basic implication rule
    parse_basic_implication()?;

    // Example 2: Parse multiple rules in a knowledge base
    parse_rule_knowledge_base()?;

    // Example 3: Extract rule structure (premise and conclusion)
    extract_rule_structure()?;

    // Example 4: Parse bi-directional rules
    parse_bidirectional_rules()?;

    // Example 5: Serialize rules to N3
    serialize_rules_example()?;

    // Example 6: Complex reasoning rules
    parse_complex_reasoning_rules()?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Example 1: Parse basic N3 implication rule
///
/// In N3, implications are represented using formulas connected by log:implies.
/// The premise (if-part) and conclusion (then-part) are both formulas.
fn parse_basic_implication() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: Basic Implication Rule ---");

    let n3_rule = r#"
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .
        @prefix ex: <http://example.com/> .

        # If someone is a parent, then they have a child
        {
            ?person ex:parent ?child .
        } log:implies {
            ?child ex:hasParent ?person .
        } .
    "#;

    println!("Parsing N3 rule:");
    println!("{}", n3_rule);

    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_rule.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads", quads.len());

    // Find the implication triple
    let implication = quads.iter().find(|q| {
        if let N3Term::NamedNode(pred) = &q.predicate {
            pred.as_str() == "http://www.w3.org/2000/10/swap/log#implies"
        } else {
            false
        }
    });

    if let Some(impl_quad) = implication {
        println!("\nFound implication:");
        println!("  Subject (premise):   {} (formula)", impl_quad.subject);
        println!("  Predicate:           {}", impl_quad.predicate);
        println!("  Object (conclusion): {} (formula)", impl_quad.object);

        // Extract the blank nodes representing the premise and conclusion formulas
        let premise_bn = match &impl_quad.subject {
            N3Term::BlankNode(bn) => Some(bn.clone()),
            _ => None,
        };

        let conclusion_bn = match &impl_quad.object {
            N3Term::BlankNode(bn) => Some(bn.clone()),
            _ => None,
        };

        if let Some(prem_bn) = premise_bn {
            let premise_quads: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == &prem_bn))
                .collect();

            println!("\n  Premise contains {} statements:", premise_quads.len());
            for quad in premise_quads {
                println!("    {} {} {}", quad.subject, quad.predicate, quad.object);
            }
        }

        if let Some(concl_bn) = conclusion_bn {
            let conclusion_quads: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == &concl_bn))
                .collect();

            println!("\n  Conclusion contains {} statements:", conclusion_quads.len());
            for quad in conclusion_quads {
                println!("    {} {} {}", quad.subject, quad.predicate, quad.object);
            }
        }
    }

    println!();
    Ok(())
}

/// Example 2: Parse knowledge base with multiple rules
///
/// Real-world N3 files often contain multiple rules for different inferences.
fn parse_rule_knowledge_base() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Knowledge Base with Multiple Rules ---");

    let n3_kb = r#"
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .
        @prefix ex: <http://example.com/> .
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .

        # Rule 1: Parent-child relationship symmetry
        {
            ?person ex:parent ?child .
        } log:implies {
            ?child ex:hasParent ?person .
        } .

        # Rule 2: Transitivity of knowledge
        {
            ?a foaf:knows ?b .
            ?b foaf:knows ?c .
        } log:implies {
            ?a ex:indirectlyKnows ?c .
        } .

        # Rule 3: Age-based classification
        {
            ?person a foaf:Person .
            ?person ex:age ?age .
        } log:implies {
            ?person ex:hasAge ?age .
        } .
    "#;

    println!("Parsing knowledge base with multiple rules...");
    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_kb.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads total", quads.len());

    // Count implications
    let implications: Vec<_> = quads
        .iter()
        .filter(|q| {
            if let N3Term::NamedNode(pred) = &q.predicate {
                pred.as_str() == "http://www.w3.org/2000/10/swap/log#implies"
            } else {
                false
            }
        })
        .collect();

    println!("  Found {} implication rules", implications.len());

    for (i, impl_quad) in implications.iter().enumerate() {
        println!("\n  Rule {}:", i + 1);
        println!("    Premise:    {}", impl_quad.subject);
        println!("    =>          {}", impl_quad.predicate);
        println!("    Conclusion: {}", impl_quad.object);
    }

    println!();
    Ok(())
}

/// Example 3: Extract and analyze rule structure
///
/// This example shows how to programmatically extract the structure of rules,
/// including variables, predicates, and patterns used.
fn extract_rule_structure() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: Extract Rule Structure ---");

    let n3_rule = r#"
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .
        @prefix ex: <http://example.com/> .

        # Grandparent inference rule
        {
            ?person ex:parent ?child .
            ?child ex:parent ?grandchild .
        } log:implies {
            ?person ex:grandparent ?grandchild .
        } .
    "#;

    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_rule.as_bytes())
        .collect::<Result<_, _>>()?;

    // Find the implication
    if let Some(impl_quad) = quads.iter().find(|q| {
        matches!(&q.predicate, N3Term::NamedNode(pred)
            if pred.as_str() == "http://www.w3.org/2000/10/swap/log#implies")
    }) {
        println!("Analyzing rule structure:\n");

        // Extract premise and conclusion blank nodes
        let premise_bn = if let N3Term::BlankNode(bn) = &impl_quad.subject {
            Some(bn)
        } else {
            None
        };

        let conclusion_bn = if let N3Term::BlankNode(bn) = &impl_quad.object {
            Some(bn)
        } else {
            None
        };

        // Analyze premise
        if let Some(prem_bn) = premise_bn {
            let premise_statements: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == prem_bn))
                .collect();

            println!("Premise Analysis:");
            println!("  Statements: {}", premise_statements.len());

            // Extract variables
            let mut premise_vars = HashSet::new();
            for stmt in &premise_statements {
                if let N3Term::Variable(v) = &stmt.subject {
                    premise_vars.insert(v.as_str());
                }
                if let N3Term::Variable(v) = &stmt.object {
                    premise_vars.insert(v.as_str());
                }
            }
            println!("  Variables: {:?}", premise_vars);

            // Extract predicates
            let predicates: Vec<_> = premise_statements
                .iter()
                .map(|s| s.predicate.to_string())
                .collect();
            println!("  Predicates: {:?}", predicates);
        }

        // Analyze conclusion
        if let Some(concl_bn) = conclusion_bn {
            let conclusion_statements: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == concl_bn))
                .collect();

            println!("\nConclusion Analysis:");
            println!("  Statements: {}", conclusion_statements.len());

            // Extract variables
            let mut conclusion_vars = HashSet::new();
            for stmt in &conclusion_statements {
                if let N3Term::Variable(v) = &stmt.subject {
                    conclusion_vars.insert(v.as_str());
                }
                if let N3Term::Variable(v) = &stmt.object {
                    conclusion_vars.insert(v.as_str());
                }
            }
            println!("  Variables: {:?}", conclusion_vars);

            // Extract predicates
            let predicates: Vec<_> = conclusion_statements
                .iter()
                .map(|s| s.predicate.to_string())
                .collect();
            println!("  Predicates: {:?}", predicates);
        }
    }

    println!();
    Ok(())
}

/// Example 4: Parse bi-directional rules (equivalence)
///
/// Some reasoning systems use bi-directional rules to express equivalence.
fn parse_bidirectional_rules() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 4: Bi-directional Rules ---");

    let n3_rules = r#"
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .
        @prefix ex: <http://example.com/> .

        # Forward rule: parent implies hasChild
        {
            ?person ex:parent ?child .
        } log:implies {
            ?person ex:hasChild ?child .
        } .

        # Backward rule: hasChild implies parent
        {
            ?person ex:hasChild ?child .
        } log:implies {
            ?person ex:parent ?child .
        } .
    "#;

    println!("Parsing bi-directional rules for equivalence...");
    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(n3_rules.as_bytes())
        .collect::<Result<_, _>>()?;

    let implications: Vec<_> = quads
        .iter()
        .filter(|q| {
            matches!(&q.predicate, N3Term::NamedNode(pred)
                if pred.as_str() == "http://www.w3.org/2000/10/swap/log#implies")
        })
        .collect();

    println!("✓ Found {} implications (forward and backward)", implications.len());

    for (i, impl_quad) in implications.iter().enumerate() {
        println!("\n  Rule {}:", i + 1);

        // Get premise statements
        if let N3Term::BlankNode(prem_bn) = &impl_quad.subject {
            let prem_stmts: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == prem_bn))
                .collect();

            if let Some(stmt) = prem_stmts.first() {
                println!("    If:   {} {} {}", stmt.subject, stmt.predicate, stmt.object);
            }
        }

        // Get conclusion statements
        if let N3Term::BlankNode(concl_bn) = &impl_quad.object {
            let concl_stmts: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == concl_bn))
                .collect();

            if let Some(stmt) = concl_stmts.first() {
                println!("    Then: {} {} {}", stmt.subject, stmt.predicate, stmt.object);
            }
        }
    }

    println!();
    Ok(())
}

/// Example 5: Serialize rules to N3 format
///
/// Shows how to create and serialize N3 rules programmatically.
fn serialize_rules_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 5: Serialize Rules to N3 ---");

    let mut serializer = N3Serializer::new()
        .with_prefix("log", "http://www.w3.org/2000/10/swap/log#")?
        .with_prefix("ex", "http://example.com/")?
        .for_writer(Vec::new());

    // Create blank nodes for premise and conclusion formulas
    let premise_formula = BlankNode::default();
    let conclusion_formula = BlankNode::default();
    let log_implies = NamedNode::new("http://www.w3.org/2000/10/swap/log#implies")?;

    // Serialize the implication statement in default graph
    serializer.serialize_quad(&N3Quad {
        subject: N3Term::BlankNode(premise_formula.clone()),
        predicate: N3Term::NamedNode(log_implies),
        object: N3Term::BlankNode(conclusion_formula.clone()),
        graph_name: GraphName::DefaultGraph,
    })?;

    // Serialize premise statements (in the premise formula context)
    let person_var = Variable::new("person")?;
    let friend_var = Variable::new("friend")?;
    let knows_pred = NamedNode::new("http://example.com/knows")?;

    serializer.serialize_quad(&N3Quad {
        subject: N3Term::Variable(person_var.clone()),
        predicate: N3Term::NamedNode(knows_pred.clone()),
        object: N3Term::Variable(friend_var.clone()),
        graph_name: GraphName::BlankNode(premise_formula.clone()),
    })?;

    // Serialize conclusion statements (in the conclusion formula context)
    let friend_of_pred = NamedNode::new("http://example.com/friendOf")?;

    serializer.serialize_quad(&N3Quad {
        subject: N3Term::Variable(friend_var.clone()),
        predicate: N3Term::NamedNode(friend_of_pred),
        object: N3Term::Variable(person_var.clone()),
        graph_name: GraphName::BlankNode(conclusion_formula),
    })?;

    let n3_output = String::from_utf8(serializer.finish()?)?;

    println!("Serialized N3 rule:");
    println!("{}", n3_output);
    println!("✓ Successfully serialized rule with formulas");

    println!();
    Ok(())
}

/// Example 6: Complex reasoning rules with multiple conditions
///
/// Demonstrates more sophisticated rules with multiple premises.
fn parse_complex_reasoning_rules() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 6: Complex Reasoning Rules ---");

    let complex_rules = r#"
        @prefix log: <http://www.w3.org/2000/10/swap/log#> .
        @prefix ex: <http://example.com/> .
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

        # Complex rule: Employee benefits eligibility
        # If someone works at a company, is over 18, and has worked for 90+ days,
        # then they are eligible for benefits
        {
            ?person ex:worksAt ?company .
            ?person ex:age ?age .
            ?person ex:daysEmployed ?days .
            # Note: In real N3 reasoners, you'd use built-ins for comparisons
        } log:implies {
            ?person ex:eligibleForBenefits ?company .
        } .

        # Transitive property rule: manager hierarchy
        {
            ?employee ex:reportsTo ?manager .
            ?manager ex:reportsTo ?seniorManager .
        } log:implies {
            ?employee ex:indirectlyReportsTo ?seniorManager .
        } .

        # Social network inference: friend of a friend
        {
            ?person1 foaf:knows ?person2 .
            ?person2 foaf:knows ?person3 .
        } log:implies {
            ?person1 ex:mightKnow ?person3 .
        } .

        # Data validation rule: ensure consistency
        {
            ?person a foaf:Person .
            ?person foaf:name ?name .
        } log:implies {
            ?person ex:hasValidName ?name .
        } .
    "#;

    println!("Parsing complex reasoning rules...");
    let quads: Vec<N3Quad> = N3Parser::new()
        .for_slice(complex_rules.as_bytes())
        .collect::<Result<_, _>>()?;

    println!("✓ Parsed {} quads total", quads.len());

    // Find all implications
    let implications: Vec<_> = quads
        .iter()
        .filter(|q| {
            matches!(&q.predicate, N3Term::NamedNode(pred)
                if pred.as_str() == "http://www.w3.org/2000/10/swap/log#implies")
        })
        .collect();

    println!("  Found {} rules\n", implications.len());

    // Analyze complexity of each rule
    for (i, impl_quad) in implications.iter().enumerate() {
        println!("Rule {}:", i + 1);

        // Count premise conditions
        if let N3Term::BlankNode(prem_bn) = &impl_quad.subject {
            let conditions: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == prem_bn))
                .collect();

            println!("  Premise conditions: {}", conditions.len());

            // Count unique variables in premise
            let mut vars = HashSet::new();
            for cond in &conditions {
                if let N3Term::Variable(v) = &cond.subject {
                    vars.insert(v.as_str());
                }
                if let N3Term::Variable(v) = &cond.object {
                    vars.insert(v.as_str());
                }
            }
            println!("  Variables: {}", vars.len());

            // Show first condition as example
            if let Some(first) = conditions.first() {
                println!("  Example: {} {} {}", first.subject, first.predicate, first.object);
            }
        }

        // Count conclusion statements
        if let N3Term::BlankNode(concl_bn) = &impl_quad.object {
            let conclusions: Vec<_> = quads
                .iter()
                .filter(|q| matches!(&q.graph_name, GraphName::BlankNode(bn) if bn == concl_bn))
                .collect();

            println!("  Conclusion statements: {}", conclusions.len());

            // Show conclusion
            if let Some(first) = conclusions.first() {
                println!("  Infers: {} {} {}", first.subject, first.predicate, first.object);
            }
        }

        println!();
    }

    println!();
    Ok(())
}
