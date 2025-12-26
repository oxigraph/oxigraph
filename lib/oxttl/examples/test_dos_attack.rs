use oxttl::TurtleParser;

fn generate_nested_collections(depth: usize) -> String {
    let mut turtle = String::from("@prefix : <http://example.org/> .\n:s :p ");
    for _ in 0..depth {
        turtle.push_str("( ");
    }
    turtle.push_str(":value ");
    for _ in 0..depth {
        turtle.push_str(") ");
    }
    turtle.push_str(".");
    turtle
}

fn main() {
    println!("=== Parser DoS Vulnerability Test ===\n");
    println!("Testing parser with increasing nesting depth...");

    for depth in [100, 500, 1000, 2000, 5000].iter() {
        println!("\nTesting depth: {}", depth);
        let nested = generate_nested_collections(*depth);
        println!("  Input size: {} bytes", nested.len());

        let mut count = 0;
        for result in TurtleParser::new().for_slice(&nested) {
            match result {
                Ok(_) => count += 1,
                Err(e) => {
                    println!("  ❌ Error at depth {}: {}", depth, e);
                    break;
                }
            }
        }
        if count > 0 {
            println!("  ✅ Successfully parsed {} triples at depth {}", count, depth);
        }
    }

    println!("\n=== Vulnerability Status ===");
    println!("If all depths succeeded: VULNERABLE (no limits enforced)");
    println!("If high depths failed: PROTECTED (limits working)");
}
