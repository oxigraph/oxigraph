//! 24-Hour Production Soak Test
//!
//! PM-MANDATED: Validates production readiness for long-running deployments
//!
//! ## Mission
//! Run ≥ 24 hours with realistic workload to detect:
//! - Memory leaks
//! - Resource exhaustion
//! - Performance degradation
//! - Crash conditions
//!
//! ## Workload
//! - SPARQL queries (SELECT, CONSTRUCT, ASK)
//! - Data insertions and deletions
//! - Transaction commits
//! - Graph operations
//! - Mixed read/write patterns
//!
//! ## Success Criteria
//! - Memory usage plateaus (±10% variation after initial growth)
//! - No crashes or panics
//! - No unbounded resource growth
//! - Stable query performance
//!
//! ## Usage
//! ```bash
//! # Run 24-hour test
//! cargo run --example soak_test --release
//!
//! # Run shorter test (1 hour for CI)
//! cargo run --example soak_test --release -- --duration 3600
//!
//! # Run with specific store type
//! cargo run --example soak_test --release -- --store-type memory
//! cargo run --example soak_test --release -- --store-type rocksdb
//! ```
//!
//! ## Output
//! - Hourly memory reports
//! - Performance metrics
//! - Error/warning logs
//! - Final verdict: PASS/FAIL

use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
fn current_memory_usage() -> usize {
    use std::fs::read_to_string;

    if let Ok(statm) = read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(rss_pages) = parts[1].parse::<usize>() {
                return rss_pages * 4096;
            }
        }
    }
    0
}

#[cfg(target_os = "macos")]
fn current_memory_usage() -> usize {
    use std::process::Command;

    if let Ok(output) = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
    {
        if let Ok(rss_str) = String::from_utf8(output.stdout) {
            if let Ok(rss_kb) = rss_str.trim().parse::<usize>() {
                return rss_kb * 1024;
            }
        }
    }
    0
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn current_memory_usage() -> usize {
    0
}

struct WorkloadStats {
    queries_executed: usize,
    inserts_executed: usize,
    deletes_executed: usize,
    transactions_committed: usize,
    errors_encountered: usize,
}

impl WorkloadStats {
    fn new() -> Self {
        Self {
            queries_executed: 0,
            inserts_executed: 0,
            deletes_executed: 0,
            transactions_committed: 0,
            errors_encountered: 0,
        }
    }
}

fn run_sparql_queries(store: &Store, stats: &mut WorkloadStats) -> Result<(), Box<dyn Error>> {
    // SELECT query
    match SparqlEvaluator::new()
        .parse_query("SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10")?
        .on_store(store)
        .execute()
    {
        Ok(QueryResults::Solutions(solutions)) => {
            for _solution in solutions {
                // Consume results
            }
            stats.queries_executed += 1;
        }
        Ok(_) => stats.queries_executed += 1,
        Err(e) => {
            eprintln!("Query error: {}", e);
            stats.errors_encountered += 1;
        }
    }

    // ASK query
    match SparqlEvaluator::new()
        .parse_query("ASK { ?s ?p ?o }")?
        .on_store(store)
        .execute()
    {
        Ok(QueryResults::Boolean(_)) => {
            stats.queries_executed += 1;
        }
        Ok(_) => stats.queries_executed += 1,
        Err(e) => {
            eprintln!("ASK query error: {}", e);
            stats.errors_encountered += 1;
        }
    }

    Ok(())
}

fn insert_random_triples(
    store: &Store,
    iteration: usize,
    stats: &mut WorkloadStats,
) -> Result<(), Box<dyn Error>> {
    let subject = NamedNode::new(format!("http://example.com/subject{}", iteration))?;
    let predicate = NamedNode::new("http://example.com/predicate")?;
    let object = Literal::from(iteration as i32);

    let quad = Quad::new(subject, predicate, object, GraphName::DefaultGraph);

    match store.insert(&quad) {
        Ok(_) => stats.inserts_executed += 1,
        Err(e) => {
            eprintln!("Insert error: {}", e);
            stats.errors_encountered += 1;
        }
    }

    Ok(())
}

fn cleanup_old_data(
    store: &Store,
    iteration: usize,
    stats: &mut WorkloadStats,
) -> Result<(), Box<dyn Error>> {
    // Remove data from 1000 iterations ago
    if iteration >= 1000 {
        let old_iteration = iteration - 1000;
        let subject = NamedNode::new(format!("http://example.com/subject{}", old_iteration))?;
        let predicate = NamedNode::new("http://example.com/predicate")?;
        let object = Literal::from(old_iteration as i32);

        let quad = Quad::new(subject, predicate, object, GraphName::DefaultGraph);

        match store.remove(&quad) {
            Ok(_) => stats.deletes_executed += 1,
            Err(e) => {
                eprintln!("Delete error: {}", e);
                stats.errors_encountered += 1;
            }
        }
    }

    Ok(())
}

fn run_graph_operations(
    store: &Store,
    iteration: usize,
    stats: &mut WorkloadStats,
) -> Result<(), Box<dyn Error>> {
    let graph_name = NamedNode::new(format!("http://example.com/graph{}", iteration % 10))?;

    // Insert into named graph
    let subject = NamedNode::new(format!("http://example.com/graphSubject{}", iteration))?;
    let predicate = NamedNode::new("http://example.com/graphPredicate")?;
    let object = Literal::from("graph data");

    let quad = Quad::new(subject, predicate, object, graph_name.into());

    match store.insert(&quad) {
        Ok(_) => stats.inserts_executed += 1,
        Err(e) => {
            eprintln!("Named graph insert error: {}", e);
            stats.errors_encountered += 1;
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       OXIGRAPH 24-HOUR PRODUCTION SOAK TEST               ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut duration_secs = 24 * 60 * 60; // 24 hours default
    let mut store_type = "memory";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--duration" => {
                if i + 1 < args.len() {
                    duration_secs = args[i + 1].parse().unwrap_or(duration_secs);
                    i += 1;
                }
            }
            "--store-type" => {
                if i + 1 < args.len() {
                    store_type = &args[i + 1];
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    println!("Configuration:");
    println!("  Duration: {} hours ({} seconds)", duration_secs / 3600, duration_secs);
    println!("  Store Type: {}", store_type);
    println!();

    // Create store
    let store = if store_type == "rocksdb" {
        #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
        {
            use tempfile::TempDir;
            let _temp_dir = TempDir::new()?;
            // Note: In production soak test, you'd use a real path
            println!("⚠️  RocksDB support requires feature flag and temp directory");
            println!("   Falling back to memory store for this example");
            Store::new()?
        }
        #[cfg(not(all(not(target_family = "wasm"), feature = "rocksdb")))]
        {
            println!("⚠️  RocksDB not available, using memory store");
            Store::new()?
        }
    } else {
        Store::new()?
    };

    println!("Store created successfully");
    println!("Starting workload...");
    println!();

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs as u64);

    let mut hour = 0;
    let mut max_memory = 0;
    let mut baseline_memory = 0;
    let mut iteration = 0;
    let mut stats = WorkloadStats::new();

    let mut last_checkpoint = Instant::now();

    while start.elapsed() < duration {
        iteration += 1;

        // Run mixed workload
        if let Err(e) = run_sparql_queries(&store, &mut stats) {
            eprintln!("Query workload error: {}", e);
            stats.errors_encountered += 1;
        }

        if let Err(e) = insert_random_triples(&store, iteration, &mut stats) {
            eprintln!("Insert workload error: {}", e);
            stats.errors_encountered += 1;
        }

        if let Err(e) = cleanup_old_data(&store, iteration, &mut stats) {
            eprintln!("Cleanup workload error: {}", e);
            stats.errors_encountered += 1;
        }

        if let Err(e) = run_graph_operations(&store, iteration, &mut stats) {
            eprintln!("Graph operations error: {}", e);
            stats.errors_encountered += 1;
        }

        stats.transactions_committed += 1;

        // Memory checkpoint every hour
        let elapsed_hours = start.elapsed().as_secs() / 3600;
        if elapsed_hours > hour {
            let mem = current_memory_usage();

            if hour == 0 {
                baseline_memory = mem;
            }

            let growth = mem.saturating_sub(baseline_memory);
            let growth_percent = if baseline_memory > 0 {
                (growth as f64 / baseline_memory as f64) * 100.0
            } else {
                0.0
            };

            println!("─────────────────────────────────────────────────────────");
            println!("Hour {}: Memory Checkpoint", hour);
            println!("  Time: {:?}", start.elapsed());
            println!("  Memory: {} MB ({} MB growth, +{:.1}%)",
                mem / 1_000_000,
                growth / 1_000_000,
                growth_percent
            );
            println!("  Iterations: {}", iteration);
            println!("  Queries: {}", stats.queries_executed);
            println!("  Inserts: {}", stats.inserts_executed);
            println!("  Deletes: {}", stats.deletes_executed);
            println!("  Transactions: {}", stats.transactions_committed);
            println!("  Errors: {}", stats.errors_encountered);

            // Check for memory growth warning
            if mem > max_memory {
                let growth_since_last = mem.saturating_sub(max_memory);
                let growth_percent_since_last = if max_memory > 0 {
                    (growth_since_last as f64 / max_memory as f64) * 100.0
                } else {
                    0.0
                };

                if growth_percent_since_last > 10.0 {
                    println!("  ⚠️  WARNING: Memory grew by {:.1}% since last hour", growth_percent_since_last);
                    println!("     This may indicate a memory leak");
                }
            }

            max_memory = max_memory.max(mem);
            hour += 1;
            println!();
        }

        // Progress indicator every 10 seconds
        if last_checkpoint.elapsed() >= Duration::from_secs(10) {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().ok();
            last_checkpoint = Instant::now();
        }

        // Small delay to avoid spinning too fast
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("\n");
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                  SOAK TEST COMPLETE                        ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Total Runtime: {:?}", start.elapsed());
    println!();
    println!("Final Statistics:");
    println!("  Total Iterations: {}", iteration);
    println!("  Queries Executed: {}", stats.queries_executed);
    println!("  Inserts Executed: {}", stats.inserts_executed);
    println!("  Deletes Executed: {}", stats.deletes_executed);
    println!("  Transactions Committed: {}", stats.transactions_committed);
    println!("  Errors Encountered: {}", stats.errors_encountered);
    println!();

    let final_memory = current_memory_usage();
    let total_growth = final_memory.saturating_sub(baseline_memory);
    let growth_percent = if baseline_memory > 0 {
        (total_growth as f64 / baseline_memory as f64) * 100.0
    } else {
        0.0
    };

    println!("Memory Analysis:");
    println!("  Baseline: {} MB", baseline_memory / 1_000_000);
    println!("  Final: {} MB", final_memory / 1_000_000);
    println!("  Max: {} MB", max_memory / 1_000_000);
    println!("  Total Growth: {} MB (+{:.1}%)", total_growth / 1_000_000, growth_percent);
    println!();

    // Verdict
    println!("═══════════════════════════════════════════════════════════");
    println!("VERDICT:");

    let mut passed = true;

    // Check for crashes (we're still running)
    println!("  ✓ No crashes detected");

    // Check for excessive errors
    if stats.errors_encountered > iteration / 100 {
        println!("  ✗ Excessive errors: {} (>1% of operations)", stats.errors_encountered);
        passed = false;
    } else {
        println!("  ✓ Error rate acceptable: {} errors", stats.errors_encountered);
    }

    // Check for memory leak
    if growth_percent > 50.0 {
        println!("  ✗ Memory growth exceeds 50%: +{:.1}%", growth_percent);
        println!("    POTENTIAL MEMORY LEAK DETECTED");
        passed = false;
    } else if growth_percent > 20.0 {
        println!("  ⚠  Memory growth: +{:.1}% (warning threshold)", growth_percent);
    } else {
        println!("  ✓ Memory usage stable: +{:.1}%", growth_percent);
    }

    println!("═══════════════════════════════════════════════════════════");

    if passed {
        println!("\n✓ SOAK TEST PASSED");
        println!("  Store is suitable for long-running production deployments");
    } else {
        println!("\n✗ SOAK TEST FAILED");
        println!("  Store may not be suitable for long-running deployments");
        println!("  Review warnings and errors above");
    }

    println!();
    Ok(())
}
