//! Observability metrics for Oxigraph Store
//!
//! Provides lightweight metrics collection for production monitoring.
//! Uses atomic counters for lock-free metric recording.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Metrics collected by the Store for observability
#[derive(Debug, Default)]
pub struct StoreMetrics {
    /// Total number of queries executed
    pub queries_total: AtomicU64,
    /// Total number of query errors
    pub query_errors_total: AtomicU64,
    /// Sum of query durations in milliseconds
    pub query_duration_sum_ms: AtomicU64,
    /// Total number of triples/quads inserted
    pub inserts_total: AtomicU64,
    /// Total number of triples/quads deleted
    pub deletes_total: AtomicU64,
}

impl StoreMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a query execution
    pub fn record_query(&self, duration_ms: u64, error: bool) {
        self.queries_total.fetch_add(1, Ordering::Relaxed);
        self.query_duration_sum_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        if error {
            self.query_errors_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record triple/quad insertion
    pub fn record_insert(&self, count: u64) {
        self.inserts_total.fetch_add(count, Ordering::Relaxed);
    }

    /// Record triple/quad deletion
    pub fn record_delete(&self, count: u64) {
        self.deletes_total.fetch_add(count, Ordering::Relaxed);
    }

    /// Export metrics in Prometheus text format
    pub fn to_prometheus_format(&self) -> String {
        let queries = self.queries_total.load(Ordering::Relaxed);
        let errors = self.query_errors_total.load(Ordering::Relaxed);
        let duration = self.query_duration_sum_ms.load(Ordering::Relaxed);
        let inserts = self.inserts_total.load(Ordering::Relaxed);
        let deletes = self.deletes_total.load(Ordering::Relaxed);

        format!(
            "# HELP oxigraph_queries_total Total number of queries executed\n\
             # TYPE oxigraph_queries_total counter\n\
             oxigraph_queries_total {queries}\n\
             # HELP oxigraph_query_errors_total Total number of query errors\n\
             # TYPE oxigraph_query_errors_total counter\n\
             oxigraph_query_errors_total {errors}\n\
             # HELP oxigraph_query_duration_sum_ms Total query execution time in milliseconds\n\
             # TYPE oxigraph_query_duration_sum_ms counter\n\
             oxigraph_query_duration_sum_ms {duration}\n\
             # HELP oxigraph_inserts_total Total number of triples/quads inserted\n\
             # TYPE oxigraph_inserts_total counter\n\
             oxigraph_inserts_total {inserts}\n\
             # HELP oxigraph_deletes_total Total number of triples/quads deleted\n\
             # TYPE oxigraph_deletes_total counter\n\
             oxigraph_deletes_total {deletes}\n"
        )
    }
}

/// Timer for measuring operation duration
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = StoreMetrics::new();

        // Record successful query
        metrics.record_query(100, false);
        assert_eq!(metrics.queries_total.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.query_errors_total.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.query_duration_sum_ms.load(Ordering::Relaxed), 100);

        // Record failed query
        metrics.record_query(50, true);
        assert_eq!(metrics.queries_total.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.query_errors_total.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.query_duration_sum_ms.load(Ordering::Relaxed), 150);
    }

    #[test]
    fn test_prometheus_format() {
        let metrics = StoreMetrics::new();
        metrics.record_query(100, false);
        metrics.record_insert(10);

        let output = metrics.to_prometheus_format();
        assert!(output.contains("oxigraph_queries_total 1"));
        assert!(output.contains("oxigraph_inserts_total 10"));
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 10);
    }
}
