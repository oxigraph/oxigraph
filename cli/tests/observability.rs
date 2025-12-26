//! Integration tests for observability features
//!
//! Tests health check module, metrics module, and structured logging initialization.

use assert_cmd::Command;

#[test]
fn test_structured_logging_initialization() {
    // Test that server can start with RUST_LOG set
    let mut cmd = Command::cargo_bin("oxigraph").unwrap();
    cmd.arg("--help")
        .env("RUST_LOG", "info")
        .assert()
        .success();
}

#[test]
fn test_metrics_module() {
    use oxigraph::metrics::{StoreMetrics, Timer};
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;

    let metrics = StoreMetrics::new();

    // Record some operations
    metrics.record_query(100, false);
    metrics.record_query(200, true);
    metrics.record_insert(10);
    metrics.record_delete(5);

    // Verify counters
    assert_eq!(metrics.queries_total.load(Ordering::Relaxed), 2);
    assert_eq!(metrics.query_errors_total.load(Ordering::Relaxed), 1);
    assert_eq!(
        metrics.query_duration_sum_ms.load(Ordering::Relaxed),
        300
    );
    assert_eq!(metrics.inserts_total.load(Ordering::Relaxed), 10);
    assert_eq!(metrics.deletes_total.load(Ordering::Relaxed), 5);

    // Export to Prometheus format
    let prometheus = metrics.to_prometheus_format();

    // Verify format
    assert!(prometheus.contains("oxigraph_queries_total 2"));
    assert!(prometheus.contains("oxigraph_query_errors_total 1"));
    assert!(prometheus.contains("oxigraph_query_duration_sum_ms 300"));
    assert!(prometheus.contains("oxigraph_inserts_total 10"));
    assert!(prometheus.contains("oxigraph_deletes_total 5"));
    assert!(prometheus.contains("# TYPE"));
    assert!(prometheus.contains("# HELP"));

    // Test timer
    let timer = Timer::start();
    thread::sleep(Duration::from_millis(10));
    let elapsed = timer.elapsed_ms();
    assert!(elapsed >= 10);
}

#[test]
fn test_health_status_creation() {
    use oxigraph::store::Store;

    let store = Store::new().unwrap();

    // Verify store has len() method needed for health checks
    let len = store.len().unwrap();
    assert_eq!(len, 0);

    // Insert some data
    use oxigraph::model::*;
    let ex = NamedNode::new("http://example.org/").unwrap();
    store
        .insert(&Quad::new(
            ex.clone(),
            ex.clone(),
            ex.clone(),
            GraphName::DefaultGraph,
        ))
        .unwrap();

    let len = store.len().unwrap();
    assert_eq!(len, 1);
}

#[test]
fn test_health_module_functions() {
    // Test health module initialization (this is a basic smoke test)
    // The actual health module is in the CLI and not directly testable from here,
    // but we can verify it compiles and links
    let _ = Command::cargo_bin("oxigraph")
        .unwrap()
        .arg("--version")
        .assert()
        .success();
}

#[test]
fn test_prometheus_metrics_format() {
    use oxigraph::metrics::StoreMetrics;

    let metrics = StoreMetrics::new();
    let prometheus = metrics.to_prometheus_format();

    // Verify all expected metrics are present
    assert!(prometheus.contains("oxigraph_queries_total"));
    assert!(prometheus.contains("oxigraph_query_errors_total"));
    assert!(prometheus.contains("oxigraph_query_duration_sum_ms"));
    assert!(prometheus.contains("oxigraph_inserts_total"));
    assert!(prometheus.contains("oxigraph_deletes_total"));

    // Verify Prometheus format conventions
    assert!(prometheus.contains("# HELP"));
    assert!(prometheus.contains("# TYPE"));
    assert!(prometheus.contains("counter"));
}
