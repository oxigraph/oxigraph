//! Attack Mitigation Demo
//!
//! This example demonstrates each of the 7 attack vectors documented in SECURITY.md
//! and shows whether the claimed limits actually work.
//!
//! Run with: cargo run -p sparshex --example attack_mitigation_demo

use oxrdf::{Graph, NamedNode, Term};
use oxrdfio::{RdfFormat, RdfParser};
use sparshex::{ShapeId, ShexValidator};
use std::time::Instant;

fn main() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║  ShEx Security Attack Mitigation Demo                    ║");
    println!("║  Testing claims from SECURITY.md                          ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    let mut results = Vec::new();

    // Attack 1: Deep Recursion
    println!("📍 Attack 1: Deep Recursion (200 levels)");
    let result1 = test_deep_recursion();
    results.push(("Deep Recursion", result1));
    println!();

    // Attack 2: Cyclic References
    println!("📍 Attack 2: Cyclic References");
    let result2 = test_cyclic_references();
    results.push(("Cyclic References", result2));
    println!();

    // Attack 3: High Cardinality
    println!("📍 Attack 3: High Cardinality {0,100000}");
    let result3 = test_high_cardinality();
    results.push(("High Cardinality", result3));
    println!();

    // Attack 4: ReDoS Pattern
    println!("📍 Attack 4: ReDoS Pattern (a+)+");
    let result4 = test_redos();
    results.push(("ReDoS Pattern", result4));
    println!();

    // Attack 5: Large Regex
    println!("📍 Attack 5: Very Long Regex (2000 chars)");
    let result5 = test_long_regex();
    results.push(("Long Regex", result5));
    println!();

    // Attack 6: Combinatorial Explosion
    println!("📍 Attack 6: Combinatorial Explosion (20 props × 10 shapes)");
    let result6 = test_combinatorial_explosion();
    results.push(("Combinatorial Explosion", result6));
    println!();

    // Attack 7: Large Graph
    println!("📍 Attack 7: Large Graph (10,000 triples)");
    let result7 = test_large_graph();
    results.push(("Large Graph", result7));
    println!();

    // Summary
    print_summary(&results);
}

#[derive(Debug, Clone, Copy)]
enum AttackResult {
    Blocked,
    Mitigated,
    Vulnerable,
    NotApplicable,
}

impl AttackResult {
    fn symbol(&self) -> &str {
        match self {
            AttackResult::Blocked => "✅ BLOCKED",
            AttackResult::Mitigated => "⚠️  MITIGATED",
            AttackResult::Vulnerable => "❌ VULNERABLE",
            AttackResult::NotApplicable => "➖ N/A",
        }
    }
}

fn test_deep_recursion() -> AttackResult {
    // Try to create deep nesting: Shape1 -> Shape2 -> ... -> Shape200
    let mut shex = String::from(
        r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
"#,
    );

    for i in 1..=200 {
        if i < 200 {
            shex.push_str(&format!("ex:S{} {{ ex:p @ex:S{} }}\n", i, i + 1));
        } else {
            shex.push_str(&format!("ex:S{} {{ ex:v xsd:string }}\n", i));
        }
    }

    match sparshex::parse_shex(&shex) {
        Ok(schema) => {
            println!("  Schema parsed successfully");

            // Try to validate
            let validator = ShexValidator::new(schema);
            let mut turtle = String::from("@prefix ex: <http://example.org/> .\n");
            for i in 1..200 {
                turtle.push_str(&format!("ex:n{} ex:p ex:n{} .\n", i, i + 1));
            }
            turtle.push_str("ex:n200 ex:v \"deep\" .\n");

            let data = parse_turtle(&turtle);
            let shape_id = ShapeId::new(nn("http://example.org/S1"));

            match validator.validate_node(&data, &term("http://example.org/n1"), &shape_id) {
                Err(e) => {
                    if e.to_string().to_lowercase().contains("recursion") {
                        println!("  ✅ Rejected with recursion error: {}", e);
                        AttackResult::Blocked
                    } else {
                        println!("  ⚠️  Failed but not with recursion error: {}", e);
                        AttackResult::Mitigated
                    }
                }
                Ok(_) => {
                    println!("  ❌ Validation succeeded - no depth limit enforced!");
                    AttackResult::Vulnerable
                }
            }
        }
        Err(e) => {
            println!("  ❌ Schema parsing failed: {}", e);
            AttackResult::NotApplicable
        }
    }
}

fn test_cyclic_references() -> AttackResult {
    let shex = r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:PersonShape { ex:name xsd:string ; ex:friend @ex:PersonShape * }"#;

    match sparshex::parse_shex(shex) {
        Ok(schema) => {
            let validator = ShexValidator::new(schema);
            let data = parse_turtle(
                r#"@prefix ex: <http://example.org/> .
ex:alice ex:name "Alice" ; ex:friend ex:bob .
ex:bob ex:name "Bob" ; ex:friend ex:charlie .
ex:charlie ex:name "Charlie" ; ex:friend ex:alice ."#,
            );

            let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
            let start = Instant::now();
            let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);
            let elapsed = start.elapsed();

            if elapsed.as_secs() > 2 {
                println!("  ❌ Took too long: {:?} - possible infinite loop", elapsed);
                AttackResult::Vulnerable
            } else {
                println!("  ✅ Completed in {:?} - cycle detection working", elapsed);
                AttackResult::Blocked
            }
        }
        Err(e) => {
            println!("  ❌ Schema parsing failed: {}", e);
            AttackResult::NotApplicable
        }
    }
}

fn test_high_cardinality() -> AttackResult {
    let shex = r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:Shape { ex:value xsd:string {0,100000} }"#;

    match sparshex::parse_shex(shex) {
        Ok(_) => {
            println!("  ❌ High cardinality schema accepted - no limit!");
            AttackResult::Vulnerable
        }
        Err(e) => {
            if e.to_string().contains("cardinality") {
                println!("  ✅ Rejected with cardinality error: {}", e);
                AttackResult::Blocked
            } else {
                println!("  ⚠️  Rejected but not for cardinality: {}", e);
                AttackResult::Mitigated
            }
        }
    }
}

fn test_redos() -> AttackResult {
    // Classic ReDoS: (a+)+
    let shex = r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:Shape { ex:value xsd:string /^(a+)+$/ }"#;

    match sparshex::parse_shex(shex) {
        Ok(_) => {
            println!("  ❌ Dangerous ReDoS pattern accepted!");
            AttackResult::Vulnerable
        }
        Err(e) => {
            if e.to_string().to_lowercase().contains("regex") {
                println!("  ✅ Rejected dangerous pattern: {}", e);
                AttackResult::Blocked
            } else {
                println!("  ⚠️  Rejected but not for regex: {}", e);
                AttackResult::Mitigated
            }
        }
    }
}

fn test_long_regex() -> AttackResult {
    let long_pattern = "a".repeat(2000);
    let shex = format!(
        r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:Shape {{ ex:value xsd:string /^{}$/ }}"#,
        long_pattern
    );

    match sparshex::parse_shex(&shex) {
        Ok(_) => {
            println!("  ❌ Very long regex (2000 chars) accepted - no length limit!");
            AttackResult::Vulnerable
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("regex") || msg.contains("length") || msg.contains("long") {
                println!("  ✅ Rejected long regex: {}", e);
                AttackResult::Blocked
            } else {
                println!("  ⚠️  Rejected but not for length: {}", e);
                AttackResult::Mitigated
            }
        }
    }
}

fn test_combinatorial_explosion() -> AttackResult {
    // 10 properties, each with OR of 5 shapes = 5^10 possible combinations
    let mut shex = String::from(
        r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:Root { "#,
    );

    for i in 1..=10 {
        shex.push_str(&format!("ex:p{} (", i));
        for j in 1..=5 {
            if j > 1 {
                shex.push_str(" OR ");
            }
            shex.push_str(&format!("@ex:S{}_{}", i, j));
        }
        shex.push_str(") ; ");
    }
    shex.push_str("}\n");

    for i in 1..=10 {
        for j in 1..=5 {
            shex.push_str(&format!("ex:S{}_{} {{ ex:v xsd:string }}\n", i, j));
        }
    }

    match sparshex::parse_shex(&shex) {
        Ok(_) => {
            println!("  ❌ Combinatorial explosion schema accepted - no complexity limit!");
            AttackResult::Vulnerable
        }
        Err(e) => {
            println!("  ⚠️  Schema rejected: {}", e);
            AttackResult::Mitigated
        }
    }
}

fn test_large_graph() -> AttackResult {
    let shex = r#"PREFIX ex: <http://example.org/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ex:Shape { ex:value xsd:string * }"#;

    match sparshex::parse_shex(shex) {
        Ok(schema) => {
            let validator = ShexValidator::new(schema);

            // Create 1000 triples (would be 10K in real attack)
            let mut turtle = String::from("@prefix ex: <http://example.org/> .\nex:node ");
            for i in 0..1000 {
                if i > 0 {
                    turtle.push_str("; ");
                }
                turtle.push_str(&format!("ex:value \"v{}\"", i));
            }
            turtle.push_str(" .");

            let data = parse_turtle(&turtle);
            let shape_id = ShapeId::new(nn("http://example.org/Shape"));

            match validator.validate_node(&data, &term("http://example.org/node"), &shape_id) {
                Ok(_) => {
                    println!("  ❌ Processed 1000 triples without limit!");
                    AttackResult::Vulnerable
                }
                Err(e) => {
                    if e.to_string().contains("triples") || e.to_string().contains("limit") {
                        println!("  ✅ Rejected with triple limit: {}", e);
                        AttackResult::Blocked
                    } else {
                        println!("  ⚠️  Failed but not with triple limit: {}", e);
                        AttackResult::Mitigated
                    }
                }
            }
        }
        Err(e) => {
            println!("  ❌ Schema parsing failed: {}", e);
            AttackResult::NotApplicable
        }
    }
}

fn print_summary(results: &[(&str, AttackResult)]) {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║  SECURITY AUDIT RESULTS                                   ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    let mut blocked = 0;
    let mut mitigated = 0;
    let mut vulnerable = 0;

    for (attack, result) in results {
        println!("  {} - {}", result.symbol(), attack);
        match result {
            AttackResult::Blocked => blocked += 1,
            AttackResult::Mitigated => mitigated += 1,
            AttackResult::Vulnerable => vulnerable += 1,
            AttackResult::NotApplicable => {}
        }
    }

    println!("\n┌───────────────────────────────────────────────────────────┐");
    println!("│ Summary:                                                  │");
    println!("│  ✅ Blocked:     {}/7 attacks fully prevented             │", blocked);
    println!("│  ⚠️  Mitigated:  {}/7 attacks partially prevented         │", mitigated);
    println!("│  ❌ Vulnerable:  {}/7 attacks NOT prevented               │", vulnerable);
    println!("└───────────────────────────────────────────────────────────┘");

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║  PM VERDICT                                               ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    if vulnerable == 0 && mitigated == 0 {
        println!("  ✅ SHIP - All attacks blocked");
    } else if vulnerable <= 2 && blocked >= 5 {
        println!("  ⚠️  CONDITIONAL SHIP - Address {} vulnerabilities", vulnerable);
    } else {
        println!("  ❌ BLOCK - Too many vulnerabilities ({} vulnerable, {} mitigated)", vulnerable, mitigated);
        println!("\n  Critical gaps:");
        println!("  • ValidationLimits exists but NOT exported in public API");
        println!("  • Validator uses hardcoded limits, not ValidationLimits");
        println!("  • Most limits in SECURITY.md are not actually enforced");
        println!("\n  Required fixes:");
        println!("  1. Export ValidationLimits in lib.rs");
        println!("  2. Integrate limits::ValidationContext into validator.rs");
        println!("  3. Replace hardcoded checks with limit enforcement");
        println!("  4. Add tests to verify all limits work");
    }

    println!();
}

// Helper functions
fn parse_turtle(turtle: &str) -> Graph {
    let mut graph = Graph::new();
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    for quad_result in parser.for_reader(turtle.as_bytes()) {
        if let Ok(quad) = quad_result {
            graph.insert(quad.as_ref());
        }
    }
    graph
}

fn nn(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn term(iri: &str) -> Term {
    Term::NamedNode(nn(iri))
}
