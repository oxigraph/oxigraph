//! Health check and metrics endpoints for observability

use oxigraph::metrics::StoreMetrics;
use oxigraph::store::Store;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

/// Application start time for uptime calculation
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Initialize the start time
pub fn init_start_time() {
    START_TIME.get_or_init(Instant::now);
}

/// Get uptime in seconds
pub fn uptime_seconds() -> u64 {
    START_TIME
        .get()
        .map(|start| start.elapsed().as_secs())
        .unwrap_or(0)
}

/// Health status response
#[derive(Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triple_count: Option<usize>,
}

impl HealthStatus {
    /// Create health status from store
    pub fn from_store(store: &Store) -> Self {
        Self {
            status: "healthy",
            version: env!("CARGO_PKG_VERSION"),
            uptime_seconds: uptime_seconds(),
            triple_count: store.len().ok(),
        }
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Global metrics instance
static METRICS: std::sync::OnceLock<Arc<StoreMetrics>> = std::sync::OnceLock::new();

/// Initialize global metrics
pub fn init_metrics() -> Arc<StoreMetrics> {
    METRICS
        .get_or_init(|| Arc::new(StoreMetrics::new()))
        .clone()
}

/// Get global metrics instance
pub fn get_metrics() -> Option<Arc<StoreMetrics>> {
    METRICS.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serialization() {
        let health = HealthStatus {
            status: "healthy",
            version: "0.4.0",
            uptime_seconds: 123,
            triple_count: Some(1000),
        };

        let json = health.to_json().unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"0.4.0\""));
        assert!(json.contains("\"uptime_seconds\":123"));
        assert!(json.contains("\"triple_count\":1000"));
    }

    #[test]
    fn test_uptime() {
        init_start_time();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let uptime = uptime_seconds();
        assert!(uptime == 0); // Less than 1 second
    }
}
