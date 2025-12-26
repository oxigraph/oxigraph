use std::time::Duration;

/// Resource limits for SPARQL query execution
///
/// These limits help prevent denial-of-service attacks from long-running or resource-intensive queries.
///
/// # Example
///
/// ```
/// use spareval::QueryExecutionLimits;
/// use std::time::Duration;
///
/// // Create strict limits for public endpoints
/// let limits = QueryExecutionLimits::strict();
///
/// // Or create custom limits
/// let custom = QueryExecutionLimits {
///     timeout: Some(Duration::from_secs(10)),
///     max_result_rows: Some(5_000),
///     ..QueryExecutionLimits::default()
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryExecutionLimits {
    /// Maximum query execution time
    ///
    /// If the query execution exceeds this duration, it will be cancelled.
    /// Default: 30 seconds
    pub timeout: Option<Duration>,

    /// Maximum number of result rows
    ///
    /// Applies to SELECT queries. If the result set exceeds this number,
    /// the query will be terminated.
    /// Default: 10,000 rows
    pub max_result_rows: Option<usize>,

    /// Maximum number of groups in GROUP BY
    ///
    /// Prevents memory exhaustion from queries that create excessive groupings.
    /// Default: 1,000 groups
    pub max_groups: Option<usize>,

    /// Maximum depth for property paths
    ///
    /// Limits recursive property path evaluation (e.g., `?s ex:parent+ ?o`).
    /// Default: 1,000 levels
    pub max_property_path_depth: Option<usize>,

    /// Maximum memory per query (in bytes)
    ///
    /// Note: This is a soft limit and may not be strictly enforced.
    /// Default: 1 GB
    pub max_memory_bytes: Option<usize>,
}

impl Default for QueryExecutionLimits {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            max_result_rows: Some(10_000),
            max_groups: Some(1_000),
            max_property_path_depth: Some(1_000),
            max_memory_bytes: Some(1024 * 1024 * 1024), // 1 GB
        }
    }
}

impl QueryExecutionLimits {
    /// Creates a new instance with default limits
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates strict limits suitable for public endpoints
    ///
    /// - Timeout: 5 seconds
    /// - Max result rows: 1,000
    /// - Max groups: 100
    /// - Max property path depth: 100
    /// - Max memory: 100 MB
    #[must_use]
    pub fn strict() -> Self {
        Self {
            timeout: Some(Duration::from_secs(5)),
            max_result_rows: Some(1_000),
            max_groups: Some(100),
            max_property_path_depth: Some(100),
            max_memory_bytes: Some(100 * 1024 * 1024), // 100 MB
        }
    }

    /// Creates permissive limits suitable for trusted internal queries
    ///
    /// - Timeout: 5 minutes
    /// - Max result rows: 100,000
    /// - Max groups: 10,000
    /// - Max property path depth: 10,000
    /// - Max memory: 10 GB
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            timeout: Some(Duration::from_secs(300)),
            max_result_rows: Some(100_000),
            max_groups: Some(10_000),
            max_property_path_depth: Some(10_000),
            max_memory_bytes: Some(10 * 1024 * 1024 * 1024), // 10 GB
        }
    }

    /// Disables all limits (no restrictions)
    ///
    /// Use with caution - only for trusted queries or local development.
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            timeout: None,
            max_result_rows: None,
            max_groups: None,
            max_property_path_depth: None,
            max_memory_bytes: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = QueryExecutionLimits::default();
        assert_eq!(limits.timeout, Some(Duration::from_secs(30)));
        assert_eq!(limits.max_result_rows, Some(10_000));
        assert_eq!(limits.max_groups, Some(1_000));
        assert_eq!(limits.max_property_path_depth, Some(1_000));
        assert_eq!(limits.max_memory_bytes, Some(1024 * 1024 * 1024));
    }

    #[test]
    fn test_strict_limits() {
        let limits = QueryExecutionLimits::strict();
        assert_eq!(limits.timeout, Some(Duration::from_secs(5)));
        assert_eq!(limits.max_result_rows, Some(1_000));
        assert_eq!(limits.max_groups, Some(100));
    }

    #[test]
    fn test_permissive_limits() {
        let limits = QueryExecutionLimits::permissive();
        assert_eq!(limits.timeout, Some(Duration::from_secs(300)));
        assert_eq!(limits.max_result_rows, Some(100_000));
    }

    #[test]
    fn test_unlimited() {
        let limits = QueryExecutionLimits::unlimited();
        assert_eq!(limits.timeout, None);
        assert_eq!(limits.max_result_rows, None);
        assert_eq!(limits.max_groups, None);
        assert_eq!(limits.max_property_path_depth, None);
        assert_eq!(limits.max_memory_bytes, None);
    }
}
