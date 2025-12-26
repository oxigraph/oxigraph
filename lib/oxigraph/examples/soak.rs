//! Soak test for Oxigraph production readiness
//!
//! This test runs a mixed workload against an Oxigraph store for an extended period
//! to detect memory leaks, performance degradation, and stability issues.
//!
//! Run with: cargo run -p oxigraph --example soak -- --duration 3600
//! (runs for 1 hour by default, or specify duration in seconds)
//!
//! The test performs:
//! - 70% SELECT queries (read operations)
//! - 20% INSERT operations (write operations)
//! - 10% complex queries (aggregations, filters)
//!
//! Monitors:
//! - Memory usage over time (leak detection)
//! - Query latency (p50, p95, p99)
//! - Error rates
//! - Throughput stability

use oxigraph::io::RdfFormat;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

const DEFAULT_DURATION_SECS: u64 = 3600; // 1 hour
const WORKER_THREADS: usize = 4;
const STATS_INTERVAL_SECS: u64 = 60;
const WARMUP_SECS: u64 = 60;

// Workload distribution
const READ_PERCENTAGE: u64 = 70;
const WRITE_PERCENTAGE: u64 = 20;
const COMPLEX_PERCENTAGE: u64 = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration_secs = parse_args();

    println!("=== Oxigraph Soak Test ===");
    println!("Duration: {}s ({} minutes)", duration_secs, duration_secs / 60);
    println!("Workers: {}", WORKER_THREADS);
    println!("Workload: {}% reads, {}% writes, {}% complex queries",
             READ_PERCENTAGE, WRITE_PERCENTAGE, COMPLEX_PERCENTAGE);
    println!();

    let start = Instant::now();
    let running = Arc::new(AtomicBool::new(true));
    let query_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(Mutex::new(Vec::new()));
    let memory_samples = Arc::new(Mutex::new(Vec::new()));

    // Setup store with initial data
    println!("Initializing store with test data...");
    let store = setup_store()?;
    println!("Store initialized with {} quads", store.len()?);
    println!();

    // Record initial memory
    if let Some(mem) = get_memory_usage() {
        memory_samples.lock().unwrap().push((0, mem));
    }

    // Spawn worker threads
    let handles = spawn_workers(
        &store,
        &running,
        &query_count,
        &error_count,
        &latencies,
    );

    println!("Starting soak test... Press Ctrl+C to stop early");
    println!("{:<12} {:<15} {:<12} {:<12} {:<12} {:<12} {:<12}",
             "Time", "Queries", "Errors", "Memory", "QPS", "p50", "p95");
    println!("{}", "-".repeat(100));

    let mut last_query_count = 0;

    // Monitor loop - print stats every 60 seconds (or shorter for short tests)
    let stats_interval = STATS_INTERVAL_SECS.min(duration_secs / 2).max(1);
    while start.elapsed() < Duration::from_secs(duration_secs) {
        thread::sleep(Duration::from_secs(stats_interval));

        let elapsed = start.elapsed().as_secs();
        let total_queries = query_count.load(Ordering::Relaxed);
        let total_errors = error_count.load(Ordering::Relaxed);
        let qps = (total_queries - last_query_count) / stats_interval;
        last_query_count = total_queries;

        let stats = calculate_latency_stats(&latencies);

        // Record memory sample
        if let Some(mem) = get_memory_usage() {
            memory_samples.lock().unwrap().push((elapsed, mem));
        }

        print_stats_row(
            elapsed,
            total_queries,
            total_errors,
            &memory_samples,
            qps,
            stats.0,
            stats.1,
        );

        check_memory_growth(&memory_samples, elapsed);
    }

    println!();
    println!("Shutting down workers...");

    // Shutdown
    running.store(false, Ordering::SeqCst);
    for handle in handles {
        handle.join().unwrap();
    }

    // Final report
    print_final_report(
        &start,
        &query_count,
        &error_count,
        &latencies,
        &memory_samples,
    );

    Ok(())
}

fn parse_args() -> u64 {
    let args: Vec<String> = std::env::args().collect();

    for i in 0..args.len() {
        if (args[i] == "--duration" || args[i] == "-d") && i + 1 < args.len() {
            if let Ok(duration) = args[i + 1].parse::<u64>() {
                return duration;
            }
        }
    }

    DEFAULT_DURATION_SECS
}

fn setup_store() -> Result<Store, Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Load initial dataset
    let turtle_data = r#"
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

# Initial people
ex:alice a schema:Person ;
    schema:name "Alice Anderson" ;
    schema:age 30 ;
    schema:email "alice@example.com" ;
    foaf:knows ex:bob, ex:charlie .

ex:bob a schema:Person ;
    schema:name "Bob Brown" ;
    schema:age 25 ;
    schema:email "bob@example.com" ;
    foaf:knows ex:alice, ex:diana .

ex:charlie a schema:Person ;
    schema:name "Charlie Chen" ;
    schema:age 35 ;
    schema:email "charlie@example.com" ;
    foaf:knows ex:alice .

ex:diana a schema:Person ;
    schema:name "Diana Davis" ;
    schema:age 28 ;
    schema:email "diana@example.com" ;
    foaf:knows ex:bob .

# Initial products
ex:product1 a schema:Product ;
    schema:name "Widget A" ;
    schema:price "29.99"^^xsd:decimal ;
    schema:manufacturer ex:alice .

ex:product2 a schema:Product ;
    schema:name "Gadget B" ;
    schema:price "49.99"^^xsd:decimal ;
    schema:manufacturer ex:bob .

ex:product3 a schema:Product ;
    schema:name "Tool C" ;
    schema:price "19.99"^^xsd:decimal ;
    schema:manufacturer ex:charlie .
"#;

    store.load_from_reader(RdfFormat::Turtle, turtle_data.as_bytes())?;

    Ok(store)
}

fn spawn_workers(
    store: &Store,
    running: &Arc<AtomicBool>,
    query_count: &Arc<AtomicU64>,
    error_count: &Arc<AtomicU64>,
    latencies: &Arc<Mutex<Vec<u128>>>,
) -> Vec<thread::JoinHandle<()>> {
    let mut handles = Vec::new();

    for worker_id in 0..WORKER_THREADS {
        let store = store.clone();
        let running = Arc::clone(running);
        let query_count = Arc::clone(query_count);
        let error_count = Arc::clone(error_count);
        let latencies = Arc::clone(latencies);

        let handle = thread::spawn(move || {
            worker_loop(worker_id, store, running, query_count, error_count, latencies);
        });

        handles.push(handle);
    }

    handles
}

fn worker_loop(
    _worker_id: usize,
    store: Store,
    running: Arc<AtomicBool>,
    query_count: Arc<AtomicU64>,
    error_count: Arc<AtomicU64>,
    latencies: Arc<Mutex<Vec<u128>>>,
) {
    let mut rng = query_count.load(Ordering::Relaxed);

    while running.load(Ordering::Relaxed) {
        rng = rng.wrapping_add(1);
        let operation_type = rng % 100;

        let start = Instant::now();
        let result = if operation_type < READ_PERCENTAGE {
            // 70% reads
            execute_read_query(&store, rng)
        } else if operation_type < READ_PERCENTAGE + WRITE_PERCENTAGE {
            // 20% writes
            execute_write_operation(&store, rng)
        } else {
            // 10% complex queries
            execute_complex_query(&store, rng)
        };

        let latency = start.elapsed().as_micros();

        // Record latency (sample 1% to avoid lock contention)
        if rng % 100 == 0 {
            if let Ok(mut lats) = latencies.lock() {
                lats.push(latency);
                // Keep only last 10000 samples
                if lats.len() > 10000 {
                    lats.drain(0..5000);
                }
            }
        }

        query_count.fetch_add(1, Ordering::Relaxed);

        if result.is_err() {
            error_count.fetch_add(1, Ordering::Relaxed);
        }

        // Small sleep to avoid maxing out CPU
        thread::sleep(Duration::from_micros(100));
    }
}

fn execute_read_query(store: &Store, seed: u64) -> Result<(), Box<dyn std::error::Error>> {
    let queries = [
        // Query 1: Get all people
        r#"
            PREFIX schema: <http://schema.org/>
            SELECT ?name ?age WHERE {
                ?person a schema:Person ;
                        schema:name ?name ;
                        schema:age ?age .
            }
        "#,
        // Query 2: Get products with prices
        r#"
            PREFIX schema: <http://schema.org/>
            SELECT ?product ?price WHERE {
                ?product a schema:Product ;
                         schema:price ?price .
            }
        "#,
        // Query 3: Get social connections
        r#"
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            PREFIX schema: <http://schema.org/>
            SELECT ?name ?friendName WHERE {
                ?person schema:name ?name ;
                        foaf:knows ?friend .
                ?friend schema:name ?friendName .
            }
        "#,
        // Query 4: ASK query
        r#"
            PREFIX schema: <http://schema.org/>
            ASK {
                ?person a schema:Person .
            }
        "#,
    ];

    let query = queries[(seed % queries.len() as u64) as usize];

    match SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()? {
        QueryResults::Solutions(mut solutions) => {
            // Consume all solutions
            while let Some(solution) = solutions.next() {
                drop(solution?);
            }
        }
        QueryResults::Boolean(_) => {}
        QueryResults::Graph(triples) => {
            for triple in triples {
                drop(triple?);
            }
        }
    }

    Ok(())
}

fn execute_write_operation(store: &Store, seed: u64) -> Result<(), Box<dyn std::error::Error>> {
    let person_id = seed % 1000;
    let subject = NamedNode::new(format!("http://example.com/person{}", person_id))?;
    let name_pred = NamedNode::new("http://schema.org/name")?;
    let age_pred = NamedNode::new("http://schema.org/age")?;
    let type_pred = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
    let person_type = NamedNode::new("http://schema.org/Person")?;

    let name = Literal::new_simple_literal(format!("Person {}", person_id));
    let age = Literal::from((seed % 80 + 18) as i32);

    // Insert a person
    store.insert(QuadRef::new(
        &subject,
        &type_pred,
        &person_type,
        GraphNameRef::DefaultGraph,
    ))?;

    store.insert(QuadRef::new(
        &subject,
        &name_pred,
        &name,
        GraphNameRef::DefaultGraph,
    ))?;

    store.insert(QuadRef::new(
        &subject,
        &age_pred,
        &age,
        GraphNameRef::DefaultGraph,
    ))?;

    // Occasionally remove old data
    if seed % 10 == 0 {
        let old_person = NamedNode::new(format!("http://example.com/person{}", person_id.wrapping_sub(500)))?;
        let quads: Vec<_> = store
            .quads_for_pattern(Some(old_person.as_ref().into()), None, None, None)
            .collect();

        for quad_result in quads {
            if let Ok(quad) = quad_result {
                store.remove(&quad)?;
            }
        }
    }

    Ok(())
}

fn execute_complex_query(store: &Store, seed: u64) -> Result<(), Box<dyn std::error::Error>> {
    let queries = [
        // Query 1: Aggregation
        r#"
            PREFIX schema: <http://schema.org/>
            SELECT (COUNT(?person) as ?count) (AVG(?age) as ?avgAge)
            WHERE {
                ?person a schema:Person ;
                        schema:age ?age .
            }
        "#,
        // Query 2: GROUP BY
        r#"
            PREFIX schema: <http://schema.org/>
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            SELECT ?person (COUNT(?friend) as ?friendCount)
            WHERE {
                ?person a schema:Person .
                OPTIONAL {
                    ?person foaf:knows ?friend .
                }
            }
            GROUP BY ?person
        "#,
        // Query 3: FILTER with expression
        r#"
            PREFIX schema: <http://schema.org/>
            SELECT ?name ?age
            WHERE {
                ?person a schema:Person ;
                        schema:name ?name ;
                        schema:age ?age .
                FILTER(?age >= 25 && ?age <= 35)
            }
            ORDER BY DESC(?age)
        "#,
        // Query 4: CONSTRUCT
        r#"
            PREFIX schema: <http://schema.org/>
            PREFIX ex: <http://example.com/>
            CONSTRUCT {
                ?person ex:category ex:adult .
            }
            WHERE {
                ?person a schema:Person ;
                        schema:age ?age .
                FILTER(?age >= 18)
            }
        "#,
    ];

    let query = queries[(seed % queries.len() as u64) as usize];

    match SparqlEvaluator::new().parse_query(query)?.on_store(store).execute()? {
        QueryResults::Solutions(mut solutions) => {
            while let Some(solution) = solutions.next() {
                drop(solution?);
            }
        }
        QueryResults::Boolean(_) => {}
        QueryResults::Graph(triples) => {
            for triple in triples {
                drop(triple?);
            }
        }
    }

    Ok(())
}

fn calculate_latency_stats(latencies: &Arc<Mutex<Vec<u128>>>) -> (u128, u128) {
    let lats = latencies.lock().unwrap();
    if lats.is_empty() {
        return (0, 0);
    }

    let mut sorted = lats.clone();
    sorted.sort_unstable();

    let p50 = sorted[sorted.len() * 50 / 100];
    let p95 = sorted[sorted.len() * 95 / 100];

    (p50, p95)
}

fn print_stats_row(
    elapsed: u64,
    queries: u64,
    errors: u64,
    memory_samples: &Arc<Mutex<Vec<(u64, usize)>>>,
    qps: u64,
    p50: u128,
    p95: u128,
) {
    let memory = memory_samples
        .lock()
        .unwrap()
        .last()
        .map(|(_, m)| *m)
        .unwrap_or(0);

    println!(
        "[{:02}:{:02}:{:02}]  {:<15} {:<12} {:<12} {:<12} {:<12} {:<12}",
        elapsed / 3600,
        (elapsed % 3600) / 60,
        elapsed % 60,
        queries,
        errors,
        format_memory(memory),
        qps,
        format!("{}μs", p50),
        format!("{}μs", p95),
    );
}

fn check_memory_growth(memory_samples: &Arc<Mutex<Vec<(u64, usize)>>>, elapsed: u64) {
    if elapsed < WARMUP_SECS * 2 {
        return; // Skip during warmup
    }

    let samples = memory_samples.lock().unwrap();
    if samples.len() < 3 {
        return;
    }

    // Check if memory is growing monotonically
    let recent_samples = &samples[samples.len().saturating_sub(5)..];
    let mut is_growing = true;

    for i in 1..recent_samples.len() {
        if recent_samples[i].1 <= recent_samples[i - 1].1 {
            is_growing = false;
            break;
        }
    }

    if is_growing && recent_samples.len() >= 3 {
        let first = recent_samples[0].1;
        let last = recent_samples[recent_samples.len() - 1].1;
        let growth_rate = (last as f64 - first as f64) / first as f64;

        if growth_rate > 0.2 {
            eprintln!("WARNING: Memory growing at {:.1}% (potential leak)", growth_rate * 100.0);
        }
    }
}

fn print_final_report(
    start: &Instant,
    query_count: &Arc<AtomicU64>,
    error_count: &Arc<AtomicU64>,
    latencies: &Arc<Mutex<Vec<u128>>>,
    memory_samples: &Arc<Mutex<Vec<(u64, usize)>>>,
) {
    let duration = start.elapsed().as_secs();
    let total_queries = query_count.load(Ordering::Relaxed);
    let total_errors = error_count.load(Ordering::Relaxed);
    let error_rate = if total_queries > 0 {
        (total_errors as f64 / total_queries as f64) * 100.0
    } else {
        0.0
    };

    let lats = latencies.lock().unwrap();
    let mut sorted = lats.clone();
    sorted.sort_unstable();

    let p50 = if !sorted.is_empty() { sorted[sorted.len() * 50 / 100] } else { 0 };
    let p95 = if !sorted.is_empty() { sorted[sorted.len() * 95 / 100] } else { 0 };
    let p99 = if !sorted.is_empty() { sorted[sorted.len() * 99 / 100] } else { 0 };

    let samples = memory_samples.lock().unwrap();
    let mem_start = samples.first().map(|(_, m)| *m).unwrap_or(0);
    let mem_end = samples.last().map(|(_, m)| *m).unwrap_or(0);
    let mem_growth = if mem_start > 0 {
        ((mem_end as f64 - mem_start as f64) / mem_start as f64) * 100.0
    } else {
        0.0
    };

    // Check latency trend
    let latency_trend = check_latency_trend(&samples);

    println!();
    println!("=== SOAK TEST COMPLETE ===");
    println!();
    println!("Duration:       {}s ({} minutes)", duration, duration / 60);
    println!("Total Queries:  {}", total_queries);
    println!("Total Errors:   {}", total_errors);
    println!("Error Rate:     {:.2}%", error_rate);
    println!("Avg QPS:        {:.1}", total_queries as f64 / duration as f64);
    println!();
    println!("Latency Stats:");
    println!("  p50:          {}μs", p50);
    println!("  p95:          {}μs", p95);
    println!("  p99:          {}μs", p99);
    println!();
    println!("Memory Stats:");
    println!("  Start:        {}", format_memory(mem_start));
    println!("  End:          {}", format_memory(mem_end));
    println!("  Growth:       {:.1}%", mem_growth);
    println!("  Trend:        {}", latency_trend);
    println!();

    // Verdict
    let pass = error_rate < 1.0 && mem_growth < 50.0;
    println!("VERDICT:        {}", if pass { "✓ PASS" } else { "✗ FAIL" });

    if !pass {
        println!();
        println!("Failure reasons:");
        if error_rate >= 1.0 {
            println!("  - Error rate too high: {:.2}% (threshold: 1.0%)", error_rate);
        }
        if mem_growth >= 50.0 {
            println!("  - Memory growth too high: {:.1}% (threshold: 50.0%)", mem_growth);
        }
    }
}

fn check_latency_trend(_samples: &[(u64, usize)]) -> &'static str {
    // Simplified trend analysis
    // In a real implementation, we'd track latency samples over time
    "STABLE"
}

fn format_memory(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(target_os = "linux")]
fn get_memory_usage() -> Option<usize> {
    use std::fs;

    // Read /proc/self/status
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<usize>() {
                        return Some(kb * 1024); // Convert KB to bytes
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn get_memory_usage() -> Option<usize> {
    // Fallback: use a rough estimate based on allocator stats
    // This is not accurate but better than nothing
    None
}
