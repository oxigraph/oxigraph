//! Resource limits for ShEx validation.
//!
//! This module provides configurable limits to protect against resource exhaustion attacks
//! during ShEx validation. These limits prevent denial-of-service (DoS) scenarios caused by
//! deeply nested shapes, infinite recursion, regex bombs, and excessive data consumption.

use std::time::{Duration, Instant};

/// Default maximum recursion depth for shape validation.
///
/// This prevents stack overflow from deeply nested or cyclic shape references.
/// Set conservatively to handle legitimate complex schemas while blocking attacks.
pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 100;

/// Default maximum number of shape references per validation session.
///
/// This limits the total number of shape evaluations to prevent combinatorial explosion
/// in shapes with many nested references or complex logical operators (AND, OR, NOT).
pub const DEFAULT_MAX_SHAPE_REFERENCES: usize = 1000;

/// Default maximum number of RDF triples to examine per validation.
///
/// This prevents validation from consuming unbounded memory when processing
/// very large graphs or when malicious shapes force examination of entire datasets.
pub const DEFAULT_MAX_TRIPLES_EXAMINED: usize = 100_000;

/// Default timeout for a single validation operation.
///
/// This prevents long-running validations from tying up resources indefinitely.
/// Can be disabled by setting to None for trusted environments.
pub const DEFAULT_TIMEOUT: Option<Duration> = Some(Duration::from_secs(30));

/// Default maximum length for regex patterns in PATTERN constraints.
///
/// Prevents regex DoS (ReDoS) attacks via extremely long or complex patterns.
pub const DEFAULT_MAX_REGEX_LENGTH: usize = 1000;

/// Default maximum length for value constraint lists (IN, LanguageIn, etc.).
///
/// Prevents memory exhaustion from shapes with massive value lists.
pub const DEFAULT_MAX_LIST_LENGTH: usize = 10_000;

/// Configurable resource limits for ShEx validation.
///
/// These limits protect against various DoS attack vectors:
/// - **Recursion depth**: Prevents stack overflow from cyclic/nested shapes
/// - **Shape references**: Prevents combinatorial explosion in complex schemas
/// - **Triples examined**: Prevents unbounded memory consumption
/// - **Timeout**: Prevents indefinite processing
/// - **Regex length**: Prevents ReDoS attacks
/// - **List length**: Prevents memory exhaustion
///
/// # Examples
///
/// ```
/// use sparshex::ValidationLimits;
/// use std::time::Duration;
///
/// // Production settings with strict limits
/// let limits = ValidationLimits::default()
///     .with_max_recursion_depth(50)
///     .with_timeout(Duration::from_secs(10));
///
/// // Development settings with relaxed limits
/// let dev_limits = ValidationLimits::default()
///     .with_max_recursion_depth(200)
///     .with_max_shape_references(5000)
///     .without_timeout();
///
/// // Trusted environment with minimal limits
/// let trusted_limits = ValidationLimits::permissive();
/// ```
#[derive(Debug, Clone)]
pub struct ValidationLimits {
    /// Maximum recursion depth for nested shape validation.
    pub max_recursion_depth: usize,

    /// Maximum total number of shape references during validation.
    pub max_shape_references: usize,

    /// Maximum number of RDF triples to examine.
    pub max_triples_examined: usize,

    /// Optional timeout for the entire validation operation.
    pub timeout: Option<Duration>,

    /// Maximum length for regex patterns.
    pub max_regex_length: usize,

    /// Maximum length for value constraint lists.
    pub max_list_length: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
            max_shape_references: DEFAULT_MAX_SHAPE_REFERENCES,
            max_triples_examined: DEFAULT_MAX_TRIPLES_EXAMINED,
            timeout: DEFAULT_TIMEOUT,
            max_regex_length: DEFAULT_MAX_REGEX_LENGTH,
            max_list_length: DEFAULT_MAX_LIST_LENGTH,
        }
    }
}

impl ValidationLimits {
    /// Creates a new ValidationLimits with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates permissive limits suitable for trusted environments.
    ///
    /// Warning: Only use in fully trusted environments where all schemas
    /// and data are controlled and validated externally.
    pub fn permissive() -> Self {
        Self {
            max_recursion_depth: 500,
            max_shape_references: 100_000,
            max_triples_examined: 10_000_000,
            timeout: None,
            max_regex_length: 10_000,
            max_list_length: 1_000_000,
        }
    }

    /// Creates strict limits suitable for public-facing services.
    ///
    /// Recommended for production environments handling untrusted input.
    pub fn strict() -> Self {
        Self {
            max_recursion_depth: 50,
            max_shape_references: 500,
            max_triples_examined: 10_000,
            timeout: Some(Duration::from_secs(5)),
            max_regex_length: 500,
            max_list_length: 1_000,
        }
    }

    /// Sets the maximum recursion depth.
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Sets the maximum number of shape references.
    pub fn with_max_shape_references(mut self, count: usize) -> Self {
        self.max_shape_references = count;
        self
    }

    /// Sets the maximum number of triples to examine.
    pub fn with_max_triples_examined(mut self, count: usize) -> Self {
        self.max_triples_examined = count;
        self
    }

    /// Sets the validation timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Disables the validation timeout.
    ///
    /// Warning: Only use in trusted environments. This allows validations
    /// to run indefinitely, potentially tying up resources.
    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Sets the maximum regex pattern length.
    pub fn with_max_regex_length(mut self, length: usize) -> Self {
        self.max_regex_length = length;
        self
    }

    /// Sets the maximum list length for value constraints.
    pub fn with_max_list_length(mut self, length: usize) -> Self {
        self.max_list_length = length;
        self
    }
}

/// Validation context that tracks resource consumption and enforces limits.
///
/// This struct is used internally during validation to monitor resource usage
/// and terminate early if limits are exceeded.
#[derive(Debug)]
pub struct ValidationContext {
    /// The configured limits.
    limits: ValidationLimits,

    /// Current recursion depth.
    current_depth: usize,

    /// Total number of shape references evaluated.
    shape_reference_count: usize,

    /// Total number of triples examined.
    triples_examined: usize,

    /// Start time of validation (for timeout enforcement).
    start_time: Instant,
}

impl ValidationContext {
    /// Creates a new validation context with the given limits.
    pub fn new(limits: ValidationLimits) -> Self {
        Self {
            limits,
            current_depth: 0,
            shape_reference_count: 0,
            triples_examined: 0,
            start_time: Instant::now(),
        }
    }

    /// Enters a deeper recursion level.
    ///
    /// Returns an error if the recursion depth limit is exceeded.
    pub fn enter_recursion(&mut self) -> Result<(), ValidationLimitError> {
        self.current_depth += 1;
        if self.current_depth > self.limits.max_recursion_depth {
            return Err(ValidationLimitError::MaxRecursionDepthExceeded {
                depth: self.current_depth,
                limit: self.limits.max_recursion_depth,
            });
        }
        Ok(())
    }

    /// Exits a recursion level.
    pub fn exit_recursion(&mut self) {
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    /// Records a shape reference evaluation.
    ///
    /// Returns an error if the shape reference limit is exceeded.
    pub fn record_shape_reference(&mut self) -> Result<(), ValidationLimitError> {
        self.shape_reference_count += 1;
        if self.shape_reference_count > self.limits.max_shape_references {
            return Err(ValidationLimitError::MaxShapeReferencesExceeded {
                count: self.shape_reference_count,
                limit: self.limits.max_shape_references,
            });
        }
        Ok(())
    }

    /// Records examination of triples.
    ///
    /// Returns an error if the triple examination limit is exceeded.
    pub fn record_triples_examined(&mut self, count: usize) -> Result<(), ValidationLimitError> {
        self.triples_examined += count;
        if self.triples_examined > self.limits.max_triples_examined {
            return Err(ValidationLimitError::MaxTriplesExaminedExceeded {
                count: self.triples_examined,
                limit: self.limits.max_triples_examined,
            });
        }
        Ok(())
    }

    /// Checks if the timeout has been exceeded.
    ///
    /// Returns an error if validation has exceeded the configured timeout.
    pub fn check_timeout(&self) -> Result<(), ValidationLimitError> {
        if let Some(timeout) = self.limits.timeout {
            let elapsed = self.start_time.elapsed();
            if elapsed > timeout {
                return Err(ValidationLimitError::TimeoutExceeded {
                    elapsed,
                    limit: timeout,
                });
            }
        }
        Ok(())
    }

    /// Validates that a regex pattern length is within limits.
    pub fn validate_regex_length(&self, pattern: &str) -> Result<(), ValidationLimitError> {
        if pattern.len() > self.limits.max_regex_length {
            return Err(ValidationLimitError::RegexTooLong {
                length: pattern.len(),
                limit: self.limits.max_regex_length,
            });
        }
        Ok(())
    }

    /// Validates that a list length is within limits.
    pub fn validate_list_length(&self, length: usize) -> Result<(), ValidationLimitError> {
        if length > self.limits.max_list_length {
            return Err(ValidationLimitError::ListTooLong {
                length,
                limit: self.limits.max_list_length,
            });
        }
        Ok(())
    }

    /// Returns the current recursion depth.
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    /// Returns the total number of shape references evaluated.
    pub fn shape_reference_count(&self) -> usize {
        self.shape_reference_count
    }

    /// Returns the total number of triples examined.
    pub fn triples_examined(&self) -> usize {
        self.triples_examined
    }

    /// Returns the elapsed time since validation started.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Returns a reference to the validation limits.
    pub fn limits(&self) -> &ValidationLimits {
        &self.limits
    }
}

/// Errors that occur when validation limits are exceeded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ValidationLimitError {
    /// Maximum recursion depth exceeded.
    #[error(
        "Maximum recursion depth exceeded: reached {depth}, limit is {limit}. \
        This may indicate cyclic shape references or excessively nested shapes. \
        Consider simplifying the schema or increasing the limit."
    )]
    MaxRecursionDepthExceeded { depth: usize, limit: usize },

    /// Maximum shape references exceeded.
    #[error(
        "Maximum shape references exceeded: {count} references, limit is {limit}. \
        This may indicate a combinatorial explosion in shape evaluations. \
        Consider simplifying the schema or increasing the limit."
    )]
    MaxShapeReferencesExceeded { count: usize, limit: usize },

    /// Maximum triples examined exceeded.
    #[error(
        "Maximum triples examined exceeded: {count} triples, limit is {limit}. \
        This may indicate validation against very large graphs or inefficient shape constraints. \
        Consider validating smaller subsets or increasing the limit."
    )]
    MaxTriplesExaminedExceeded { count: usize, limit: usize },

    /// Validation timeout exceeded.
    #[error(
        "Validation timeout exceeded: elapsed {elapsed:?}, limit is {limit:?}. \
        This may indicate complex shapes, large data, or inefficient constraints. \
        Consider simplifying the validation or increasing the timeout."
    )]
    TimeoutExceeded { elapsed: Duration, limit: Duration },

    /// Regex pattern too long.
    #[error(
        "Regex pattern too long: {length} characters, limit is {limit}. \
        This may be a ReDoS (regex denial-of-service) attack attempt. \
        Consider simplifying the pattern or increasing the limit."
    )]
    RegexTooLong { length: usize, limit: usize },

    /// Value constraint list too long.
    #[error(
        "Value constraint list too long: {length} items, limit is {limit}. \
        This may cause excessive memory consumption. \
        Consider reducing the list size or increasing the limit."
    )]
    ListTooLong { length: usize, limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ValidationLimits::default();
        assert_eq!(limits.max_recursion_depth, DEFAULT_MAX_RECURSION_DEPTH);
        assert_eq!(limits.max_shape_references, DEFAULT_MAX_SHAPE_REFERENCES);
        assert_eq!(
            limits.max_triples_examined,
            DEFAULT_MAX_TRIPLES_EXAMINED
        );
    }

    #[test]
    fn test_strict_limits() {
        let limits = ValidationLimits::strict();
        assert_eq!(limits.max_recursion_depth, 50);
        assert_eq!(limits.timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_permissive_limits() {
        let limits = ValidationLimits::permissive();
        assert_eq!(limits.max_recursion_depth, 500);
        assert_eq!(limits.timeout, None);
    }

    #[test]
    fn test_recursion_tracking() {
        let limits = ValidationLimits::default().with_max_recursion_depth(3);
        let mut ctx = ValidationContext::new(limits);

        assert_eq!(ctx.current_depth(), 0);
        assert!(ctx.enter_recursion().is_ok());
        assert_eq!(ctx.current_depth(), 1);
        assert!(ctx.enter_recursion().is_ok());
        assert_eq!(ctx.current_depth(), 2);
        assert!(ctx.enter_recursion().is_ok());
        assert_eq!(ctx.current_depth(), 3);

        // Should fail on 4th level
        assert!(ctx.enter_recursion().is_err());

        ctx.exit_recursion();
        assert_eq!(ctx.current_depth(), 3);
    }

    #[test]
    fn test_shape_reference_counting() {
        let limits = ValidationLimits::default().with_max_shape_references(3);
        let mut ctx = ValidationContext::new(limits);

        assert_eq!(ctx.shape_reference_count(), 0);
        assert!(ctx.record_shape_reference().is_ok());
        assert_eq!(ctx.shape_reference_count(), 1);
        assert!(ctx.record_shape_reference().is_ok());
        assert!(ctx.record_shape_reference().is_ok());
        assert_eq!(ctx.shape_reference_count(), 3);

        // Should fail on 4th reference
        assert!(ctx.record_shape_reference().is_err());
    }

    #[test]
    fn test_triples_examined_counting() {
        let limits = ValidationLimits::default().with_max_triples_examined(100);
        let mut ctx = ValidationContext::new(limits);

        assert!(ctx.record_triples_examined(50).is_ok());
        assert_eq!(ctx.triples_examined(), 50);
        assert!(ctx.record_triples_examined(50).is_ok());
        assert_eq!(ctx.triples_examined(), 100);

        // Should fail when exceeding limit
        assert!(ctx.record_triples_examined(1).is_err());
    }

    #[test]
    fn test_timeout_check() {
        use std::thread;

        let limits = ValidationLimits::default().with_timeout(Duration::from_millis(50));
        let ctx = ValidationContext::new(limits);

        assert!(ctx.check_timeout().is_ok());
        thread::sleep(Duration::from_millis(100));
        assert!(ctx.check_timeout().is_err());
    }

    #[test]
    fn test_regex_length_validation() {
        let limits = ValidationLimits::default().with_max_regex_length(10);
        let ctx = ValidationContext::new(limits);

        assert!(ctx.validate_regex_length("short").is_ok());
        assert!(ctx.validate_regex_length("exactly10c").is_ok());
        assert!(ctx.validate_regex_length("this is too long").is_err());
    }

    #[test]
    fn test_list_length_validation() {
        let limits = ValidationLimits::default().with_max_list_length(5);
        let ctx = ValidationContext::new(limits);

        assert!(ctx.validate_list_length(5).is_ok());
        assert!(ctx.validate_list_length(6).is_err());
    }

    #[test]
    fn test_builder_pattern() {
        let limits = ValidationLimits::new()
            .with_max_recursion_depth(75)
            .with_max_shape_references(2000)
            .with_timeout(Duration::from_secs(15))
            .without_timeout();

        assert_eq!(limits.max_recursion_depth, 75);
        assert_eq!(limits.max_shape_references, 2000);
        assert_eq!(limits.timeout, None);
    }
}
