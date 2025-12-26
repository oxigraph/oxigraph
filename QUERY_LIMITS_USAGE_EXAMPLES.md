# Query Execution Limits - Usage Examples

This document demonstrates how to use the query execution limits feature in Oxigraph's SPARQL evaluator.

## Basic Usage

### Using Preset Configurations

```rust
use oxrdf::Dataset;
use spareval::{QueryEvaluator, QueryExecutionLimits, QueryResults};
use spargebra::SparqlParser;

// Use strict limits for public endpoints
let evaluator = QueryEvaluator::new()
    .with_limits(QueryExecutionLimits::strict());

let query = SparqlParser::new().parse_query(
    "SELECT * WHERE { ?s ?p ?o }"
)?;

let dataset = Dataset::new();
let results = evaluator.prepare(&query).execute(&dataset)?;
```

### Available Presets

#### 1. Strict (Public Endpoints)
```rust
let limits = QueryExecutionLimits::strict();
// Timeout: 5 seconds
// Max rows: 1,000
// Max groups: 100
// Max depth: 100
// Max memory: 100 MB
```

#### 2. Default (Balanced)
```rust
let limits = QueryExecutionLimits::default();
// Timeout: 30 seconds
// Max rows: 10,000
// Max groups: 1,000
// Max depth: 1,000
// Max memory: 1 GB
```

#### 3. Permissive (Internal/Trusted)
```rust
let limits = QueryExecutionLimits::permissive();
// Timeout: 5 minutes
// Max rows: 100,000
// Max groups: 10,000
// Max depth: 10,000
// Max memory: 10 GB
```

#### 4. Unlimited (Development)
```rust
let limits = QueryExecutionLimits::unlimited();
// All limits disabled
```

## Custom Limit Configuration

### Example 1: Custom Timeout Only
```rust
use std::time::Duration;

let custom_limits = QueryExecutionLimits {
    timeout: Some(Duration::from_secs(10)),
    max_result_rows: None, // No limit
    max_groups: None,
    max_property_path_depth: None,
    max_memory_bytes: None,
};

let evaluator = QueryEvaluator::new().with_limits(custom_limits);
```

### Example 2: Custom Result Limit
```rust
let custom_limits = QueryExecutionLimits {
    timeout: Some(Duration::from_secs(30)),
    max_result_rows: Some(5_000), // Limit to 5,000 rows
    ..QueryExecutionLimits::unlimited()
};
```

### Example 3: Starting from Preset
```rust
let custom_limits = QueryExecutionLimits {
    max_result_rows: Some(50_000), // Override just one field
    ..QueryExecutionLimits::strict() // Start from strict preset
};
```

## Error Handling

### Handling Timeout Errors
```rust
use spareval::QueryEvaluationError;

match evaluator.prepare(&query).execute(&dataset) {
    Ok(results) => {
        // Process results
    }
    Err(QueryEvaluationError::Timeout(duration)) => {
        eprintln!("Query timed out after {:?}", duration);
        // Return 503 Service Unavailable
    }
    Err(e) => {
        eprintln!("Query error: {}", e);
    }
}
```

### Handling All Limit Errors
```rust
match evaluator.prepare(&query).execute(&dataset) {
    Ok(results) => process_results(results),
    Err(QueryEvaluationError::Timeout(d)) => {
        log::warn!("Query timeout: {:?}", d);
        respond_503_timeout()
    }
    Err(QueryEvaluationError::ResultLimitExceeded(limit)) => {
        log::warn!("Result limit exceeded: {} rows", limit);
        respond_413_payload_too_large()
    }
    Err(QueryEvaluationError::GroupLimitExceeded(limit)) => {
        log::warn!("Group limit exceeded: {} groups", limit);
        respond_413_payload_too_large()
    }
    Err(QueryEvaluationError::PropertyPathDepthExceeded(depth)) => {
        log::warn!("Property path too deep: {}", depth);
        respond_400_bad_request()
    }
    Err(QueryEvaluationError::MemoryLimitExceeded(bytes)) => {
        log::warn!("Memory limit exceeded: {} bytes", bytes);
        respond_507_insufficient_storage()
    }
    Err(e) => {
        log::error!("Query evaluation error: {}", e);
        respond_500_internal_error()
    }
}
```

## Integration with Oxigraph Store

### Example 1: Using with Oxigraph Store
```rust
use oxigraph::store::Store;
use spareval::{QueryEvaluator, QueryExecutionLimits};
use spargebra::SparqlParser;

// Open store
let store = Store::open("./my_database")?;

// Create evaluator with limits
let evaluator = QueryEvaluator::new()
    .with_limits(QueryExecutionLimits::strict());

// Parse query
let query = SparqlParser::new().parse_query(
    "SELECT ?person ?name WHERE {
        ?person a <http://schema.org/Person> .
        ?person <http://schema.org/name> ?name .
    } LIMIT 100"
)?;

// Execute with limits
let results = evaluator.prepare(&query).execute(&store)?;

if let QueryResults::Solutions(solutions) = results {
    for solution in solutions {
        let solution = solution?;
        println!("Found: {:?}", solution);
    }
}
```

### Example 2: Different Limits for Different Endpoints
```rust
struct QueryService {
    public_evaluator: QueryEvaluator,
    admin_evaluator: QueryEvaluator,
}

impl QueryService {
    fn new() -> Self {
        Self {
            public_evaluator: QueryEvaluator::new()
                .with_limits(QueryExecutionLimits::strict()),
            admin_evaluator: QueryEvaluator::new()
                .with_limits(QueryExecutionLimits::permissive()),
        }
    }

    fn execute_public_query(&self, query: &Query, store: &Store)
        -> Result<QueryResults, QueryEvaluationError>
    {
        self.public_evaluator.prepare(query).execute(store)
    }

    fn execute_admin_query(&self, query: &Query, store: &Store)
        -> Result<QueryResults, QueryEvaluationError>
    {
        self.admin_evaluator.prepare(query).execute(store)
    }
}
```

## Combining with Other Features

### With Cancellation Token
```rust
use spareval::CancellationToken;
use std::time::Duration;

let token = CancellationToken::new();
let evaluator = QueryEvaluator::new()
    .with_limits(QueryExecutionLimits::strict())
    .with_cancellation_token(token.clone());

// Spawn thread to cancel after 2 seconds
let cancel_token = token.clone();
std::thread::spawn(move || {
    std::thread::sleep(Duration::from_secs(2));
    cancel_token.cancel();
});

// Query will be cancelled or timeout, whichever comes first
match evaluator.prepare(&query).execute(&dataset) {
    Err(QueryEvaluationError::Cancelled) => {
        println!("Query was cancelled");
    }
    Err(QueryEvaluationError::Timeout(_)) => {
        println!("Query timed out");
    }
    Ok(results) => {
        println!("Query completed successfully");
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### With Statistics
```rust
let evaluator = QueryEvaluator::new()
    .with_limits(QueryExecutionLimits::default())
    .compute_statistics();

let (result, explanation) = evaluator.prepare(&query).explain(&dataset);

// View execution statistics
println!("Query explanation: {:?}", explanation);
```

## HTTP Server Integration

### Example: SPARQL Endpoint with Limits
```rust
use axum::{
    Router,
    routing::post,
    extract::State,
    response::{Response, IntoResponse},
    http::StatusCode,
};
use oxigraph::store::Store;
use spareval::{QueryEvaluator, QueryExecutionLimits, QueryEvaluationError};

struct AppState {
    store: Store,
    evaluator: QueryEvaluator,
}

async fn sparql_endpoint(
    State(state): State<AppState>,
    query: String,
) -> Response {
    // Parse query
    let query = match SparqlParser::new().parse_query(&query) {
        Ok(q) => q,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Parse error: {}", e))
                .into_response();
        }
    };

    // Execute with limits
    match state.evaluator.prepare(&query).execute(&state.store) {
        Ok(results) => {
            // Serialize results (JSON, XML, etc.)
            serialize_results(results).into_response()
        }
        Err(QueryEvaluationError::Timeout(duration)) => {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("Query timeout after {:?}", duration)
            ).into_response()
        }
        Err(QueryEvaluationError::ResultLimitExceeded(limit)) => {
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Result set exceeds {} rows", limit)
            ).into_response()
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Query error: {}", e)
            ).into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    let store = Store::open("./data").expect("Failed to open store");
    let evaluator = QueryEvaluator::new()
        .with_limits(QueryExecutionLimits::strict());

    let app_state = AppState { store, evaluator };

    let app = Router::new()
        .route("/sparql", post(sparql_endpoint))
        .with_state(app_state);

    // Run server...
}
```

## Testing with Limits

### Example: Testing Limit Enforcement
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_respects_limits() {
        let dataset = create_large_dataset(10_000);
        let query = SparqlParser::new()
            .parse_query("SELECT * WHERE { ?s ?p ?o }")
            .unwrap();

        let evaluator = QueryEvaluator::new()
            .with_limits(QueryExecutionLimits {
                max_result_rows: Some(100),
                ..QueryExecutionLimits::unlimited()
            });

        let results = evaluator.prepare(&query).execute(&dataset).unwrap();

        if let QueryResults::Solutions(solutions) = results {
            let count = solutions.count();
            assert!(count <= 100, "Expected at most 100 results, got {}", count);
        }
    }
}
```

## Best Practices

### 1. Choose Appropriate Limits for Your Use Case
```rust
// Public SPARQL endpoint
let public_limits = QueryExecutionLimits::strict();

// Internal analytics
let analytics_limits = QueryExecutionLimits::permissive();

// Development/testing
let dev_limits = QueryExecutionLimits::unlimited();
```

### 2. Log Limit Violations
```rust
match evaluator.prepare(&query).execute(&store) {
    Err(QueryEvaluationError::Timeout(d)) => {
        log::warn!(
            "Query timeout: {:?}, query: {}",
            d,
            query.to_string().chars().take(100).collect::<String>()
        );
    }
    Err(e) => log::error!("Query error: {}", e),
    Ok(r) => { /* process */ }
}
```

### 3. Provide Helpful Error Messages
```rust
match evaluator.prepare(&query).execute(&store) {
    Err(QueryEvaluationError::ResultLimitExceeded(limit)) => {
        eprintln!(
            "Your query returned more than {} results. \
             Please add a LIMIT clause or use more specific filters.",
            limit
        );
    }
    // ...
}
```

### 4. Consider Query Complexity
```rust
// Simpler queries get more permissive limits
fn get_limits_for_query(query: &Query) -> QueryExecutionLimits {
    let complexity = estimate_query_complexity(query);

    if complexity < 10 {
        QueryExecutionLimits::permissive()
    } else if complexity < 50 {
        QueryExecutionLimits::default()
    } else {
        QueryExecutionLimits::strict()
    }
}
```

## Migration Guide

If you're currently using `QueryEvaluator` without limits:

### Before
```rust
let evaluator = QueryEvaluator::new();
let results = evaluator.prepare(&query).execute(&dataset)?;
```

### After (with limits)
```rust
let evaluator = QueryEvaluator::new()
    .with_limits(QueryExecutionLimits::default());
let results = evaluator.prepare(&query).execute(&dataset)?;
```

No breaking changes - limits are optional! If you don't call `with_limits()`, no limits are enforced (same behavior as before).

## Performance Considerations

- Limit checking adds minimal overhead (<5% expected)
- Timeouts are checked periodically, not continuously
- Result counting is incremental
- No additional memory allocations for limit tracking

## See Also

- [QueryExecutionLimits API Documentation](https://docs.rs/spareval/latest/spareval/struct.QueryExecutionLimits.html)
- [QueryEvaluationError](https://docs.rs/spareval/latest/spareval/enum.QueryEvaluationError.html)
- [SPARQL Query Specification](https://www.w3.org/TR/sparql11-query/)
