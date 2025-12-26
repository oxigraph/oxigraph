# Rust Integration Patterns

This guide demonstrates production-ready patterns for integrating Oxigraph into Rust applications, covering web frameworks, async runtimes, error handling, and advanced use cases.

## Table of Contents

1. [Axum REST API](#axum-rest-api)
2. [Actix-web SPARQL Endpoint](#actix-web-sparql-endpoint)
3. [Tokio Async Patterns](#tokio-async-patterns)
4. [Error Handling](#error-handling)
5. [Logging and Monitoring](#logging-and-monitoring)
6. [Production Deployment](#production-deployment)
7. [Advanced Patterns](#advanced-patterns)

## Axum REST API

Complete REST API using Axum with shared store, error handling, and CORS.

### Cargo.toml

```toml
[package]
name = "oxigraph-axum-api"
version = "0.1.0"
edition = "2021"

[dependencies]
oxigraph = "0.4"
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "1"
once_cell = "1"
```

### src/main.rs

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use once_cell::sync::Lazy;
use oxigraph::{
    io::{RdfFormat, RdfParser},
    model::*,
    sparql::{QueryResults, QuerySolution, SparqlEvaluator},
    store::Store,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, warn};

// Shared store instance
static STORE: Lazy<Store> = Lazy::new(|| {
    Store::open("./data/oxigraph").expect("Failed to open store")
});

// Application state
#[derive(Clone)]
struct AppState {
    store: &'static Store,
}

// Error types
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("Store error: {0}")]
    Store(#[from] oxigraph::store::StoreError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Query parse error: {0}")]
    QueryParse(#[from] oxigraph::sparql::SparqlParseError),

    #[error("Query evaluation error: {0}")]
    QueryEval(#[from] oxigraph::sparql::EvaluationError),

    #[error("Invalid IRI: {0}")]
    InvalidIri(#[from] oxigraph::model::IriParseError),

    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::QueryParse(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// Request/Response types
#[derive(Debug, Deserialize)]
struct QueryRequest {
    query: String,
    #[serde(default)]
    base_iri: Option<String>,
}

#[derive(Debug, Serialize)]
struct QueryResponse {
    results: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct TripleRequest {
    subject: String,
    predicate: String,
    object: String,
    #[serde(default)]
    object_type: ObjectType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ObjectType {
    Iri,
    Literal,
    TypedLiteral { datatype: String },
    LangLiteral { language: String },
}

impl Default for ObjectType {
    fn default() -> Self {
        ObjectType::Literal
    }
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    100
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oxigraph_axum_api=debug,tower_http=debug".into()),
        )
        .init();

    // Initialize store
    info!("Initializing Oxigraph store at ./data/oxigraph");
    let _ = &*STORE;

    // Build app state
    let state = AppState { store: &STORE };

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .route("/", get(health_check))
        .route("/query", post(execute_query))
        .route("/triples", post(add_triple))
        .route("/triples", get(get_triples))
        .route("/triples/:subject", get(get_triples_by_subject))
        .route("/stats", get(get_stats))
        .route("/load", post(load_data))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Handlers

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "oxigraph-api"
    }))
}

async fn execute_query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    info!("Executing SPARQL query");

    // Execute query in blocking task
    let store = state.store;
    let query = req.query.clone();

    let results = tokio::task::spawn_blocking(move || {
        SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(store)
            .execute()
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    // Convert results to JSON
    let json_results = match results {
        QueryResults::Solutions(solutions) => {
            let mut results = Vec::new();
            for solution in solutions {
                let solution = solution?;
                let mut binding = serde_json::Map::new();

                for (var, term) in solution.iter() {
                    binding.insert(
                        var.as_str().to_string(),
                        term_to_json(term),
                    );
                }
                results.push(serde_json::Value::Object(binding));
            }
            results
        }
        QueryResults::Boolean(b) => {
            vec![serde_json::json!({ "result": b })]
        }
        QueryResults::Graph(triples) => {
            let mut results = Vec::new();
            for triple in triples {
                let triple = triple?;
                results.push(serde_json::json!({
                    "subject": term_to_json(triple.subject.as_ref()),
                    "predicate": term_to_json(triple.predicate.as_ref()),
                    "object": term_to_json(triple.object.as_ref()),
                }));
            }
            results
        }
    };

    Ok(Json(QueryResponse {
        total: Some(json_results.len()),
        results: json_results,
    }))
}

async fn add_triple(
    State(state): State<AppState>,
    Json(req): Json<TripleRequest>,
) -> Result<StatusCode, ApiError> {
    info!("Adding triple");

    let subject = NamedNode::new(&req.subject)?;
    let predicate = NamedNode::new(&req.predicate)?;

    let object: Term = match req.object_type {
        ObjectType::Iri => {
            NamedNode::new(&req.object)?.into()
        }
        ObjectType::Literal => {
            Literal::new_simple_literal(&req.object).into()
        }
        ObjectType::TypedLiteral { datatype } => {
            Literal::new_typed_literal(&req.object, NamedNode::new(datatype)?).into()
        }
        ObjectType::LangLiteral { language } => {
            Literal::new_language_tagged_literal(&req.object, &language)
                .map_err(|e| ApiError::BadRequest(e.to_string()))?
                .into()
        }
    };

    let store = state.store;
    tokio::task::spawn_blocking(move || {
        store.insert(&Quad::new(
            subject,
            predicate,
            object,
            GraphName::DefaultGraph,
        ))
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    Ok(StatusCode::CREATED)
}

async fn get_triples(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<QueryResponse>, ApiError> {
    info!("Getting all triples with pagination");

    let store = state.store;
    let limit = params.limit;
    let offset = params.offset;

    let quads = tokio::task::spawn_blocking(move || {
        store
            .iter()
            .skip(offset)
            .take(limit)
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    let results = quads
        .iter()
        .map(|quad| {
            serde_json::json!({
                "subject": term_to_json(quad.subject.as_ref()),
                "predicate": term_to_json(quad.predicate.as_ref()),
                "object": term_to_json(quad.object.as_ref()),
                "graph": term_to_json(quad.graph_name.as_ref()),
            })
        })
        .collect();

    Ok(Json(QueryResponse {
        results,
        total: None,
    }))
}

async fn get_triples_by_subject(
    State(state): State<AppState>,
    Path(subject_iri): Path<String>,
) -> Result<Json<QueryResponse>, ApiError> {
    info!("Getting triples for subject: {}", subject_iri);

    let subject = NamedNode::new(&subject_iri)?;
    let store = state.store;

    let quads = tokio::task::spawn_blocking(move || {
        store
            .quads_for_pattern(Some(subject.as_ref().into()), None, None, None)
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    let results = quads
        .iter()
        .map(|quad| {
            serde_json::json!({
                "predicate": term_to_json(quad.predicate.as_ref()),
                "object": term_to_json(quad.object.as_ref()),
            })
        })
        .collect();

    Ok(Json(QueryResponse {
        results,
        total: Some(quads.len()),
    }))
}

async fn get_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.store;

    let count = tokio::task::spawn_blocking(move || store.len())
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    Ok(Json(serde_json::json!({
        "triple_count": count,
        "store_type": "persistent",
    })))
}

#[derive(Debug, Deserialize)]
struct LoadDataRequest {
    data: String,
    format: String,
    #[serde(default)]
    base_iri: Option<String>,
}

async fn load_data(
    State(state): State<AppState>,
    Json(req): Json<LoadDataRequest>,
) -> Result<StatusCode, ApiError> {
    info!("Loading data in format: {}", req.format);

    use oxigraph::io::RdfFormat;

    let format = match req.format.as_str() {
        "turtle" | "ttl" => RdfFormat::Turtle,
        "ntriples" | "nt" => RdfFormat::NTriples,
        "rdfxml" | "xml" => RdfFormat::RdfXml,
        "nquads" | "nq" => RdfFormat::NQuads,
        "trig" => RdfFormat::TriG,
        _ => return Err(ApiError::BadRequest("Unsupported format".to_string())),
    };

    let store = state.store;
    let data = req.data;
    let base_iri = req.base_iri;

    tokio::task::spawn_blocking(move || {
        let mut parser = RdfParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser.with_base_iri(&base_iri)
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        }
        store.load_from_reader(parser, data.as_bytes())
    })
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))??;

    Ok(StatusCode::CREATED)
}

// Utility functions

fn term_to_json(term: &Term) -> serde_json::Value {
    match term {
        Term::NamedNode(n) => serde_json::json!({
            "type": "NamedNode",
            "value": n.as_str()
        }),
        Term::BlankNode(b) => serde_json::json!({
            "type": "BlankNode",
            "value": b.as_str()
        }),
        Term::Literal(l) => {
            let mut obj = serde_json::json!({
                "type": "Literal",
                "value": l.value()
            });

            if let Some(lang) = l.language() {
                obj["language"] = serde_json::Value::String(lang.to_string());
            } else if l.datatype().as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                obj["datatype"] = serde_json::Value::String(l.datatype().as_str().to_string());
            }

            obj
        }
        Term::Triple(t) => serde_json::json!({
            "type": "Triple",
            "subject": term_to_json(t.subject.as_ref()),
            "predicate": term_to_json(t.predicate.as_ref()),
            "object": term_to_json(t.object.as_ref()),
        }),
    }
}
```

### Testing

```bash
# Start the server
cargo run

# Health check
curl http://localhost:3000/

# Add a triple
curl -X POST http://localhost:3000/triples \
  -H "Content-Type: application/json" \
  -d '{
    "subject": "http://example.org/alice",
    "predicate": "http://schema.org/name",
    "object": "Alice",
    "object_type": "literal"
  }'

# Query
curl -X POST http://localhost:3000/query \
  -H "Content-Type: application/json" \
  -d '{
    "query": "SELECT * WHERE { ?s ?p ?o } LIMIT 10"
  }'

# Load data
curl -X POST http://localhost:3000/load \
  -H "Content-Type: application/json" \
  -d '{
    "data": "<http://example.org/alice> <http://schema.org/name> \"Alice\" .",
    "format": "turtle"
  }'
```

## Actix-web SPARQL Endpoint

Full SPARQL endpoint with Actix-web, supporting multiple content types.

### Cargo.toml

```toml
[dependencies]
actix-web = "4"
actix-rt = "2"
oxigraph = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
env_logger = "0.11"
log = "0.4"
```

### src/main.rs

```rust
use actix_web::{
    get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use form_urlencoded;
use log::info;
use oxigraph::{
    io::{RdfFormat, RdfParser, RdfSerializer},
    model::GraphName,
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};
use std::sync::Arc;

struct AppData {
    store: Arc<Store>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    info!("Initializing Oxigraph store");
    let store = Arc::new(Store::open("./data/actix-store").expect("Failed to open store"));

    info!("Starting Actix-web server on http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppData {
                store: store.clone(),
            }))
            .service(health)
            .service(sparql_query)
            .service(sparql_update)
            .service(upload_data)
            .service(export_data)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

#[get("/")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "running",
        "endpoint": "SPARQL 1.1"
    }))
}

#[post("/sparql")]
async fn sparql_query(
    req: HttpRequest,
    body: web::Bytes,
    data: web::Data<AppData>,
) -> Result<HttpResponse> {
    let query_str = match req.content_type() {
        "application/sparql-query" => String::from_utf8_lossy(&body).to_string(),
        "application/x-www-form-urlencoded" => {
            let params = form_urlencoded::parse(&body)
                .find(|(k, _)| k == "query")
                .map(|(_, v)| v.to_string())
                .ok_or_else(|| {
                    actix_web::error::ErrorBadRequest("Missing 'query' parameter")
                })?;
            params
        }
        _ => return Ok(HttpResponse::UnsupportedMediaType().finish()),
    };

    info!("Executing query: {}", query_str);

    let store = data.store.clone();
    let results = web::block(move || {
        SparqlEvaluator::new()
            .parse_query(&query_str)?
            .on_store(&store)
            .execute()
    })
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?
    .map_err(actix_web::error::ErrorBadRequest)?;

    // Determine response format from Accept header
    let accept = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/sparql-results+json");

    match results {
        QueryResults::Solutions(solutions) => {
            if accept.contains("application/sparql-results+json") {
                let mut bindings = Vec::new();
                for solution in solutions {
                    let solution = solution.map_err(actix_web::error::ErrorInternalServerError)?;
                    let mut binding = serde_json::Map::new();
                    for (var, term) in solution.iter() {
                        binding.insert(var.as_str().to_string(), term_to_json_value(term));
                    }
                    bindings.push(serde_json::Value::Object(binding));
                }
                Ok(HttpResponse::Ok()
                    .content_type("application/sparql-results+json")
                    .json(serde_json::json!({
                        "head": { "vars": [] },
                        "results": { "bindings": bindings }
                    })))
            } else {
                Ok(HttpResponse::NotAcceptable().finish())
            }
        }
        QueryResults::Boolean(b) => Ok(HttpResponse::Ok()
            .content_type("application/sparql-results+json")
            .json(serde_json::json!({
                "head": {},
                "boolean": b
            }))),
        QueryResults::Graph(_) => {
            Ok(HttpResponse::Ok().body("CONSTRUCT results - implement serialization"))
        }
    }
}

#[post("/update")]
async fn sparql_update(body: web::Bytes, data: web::Data<AppData>) -> Result<HttpResponse> {
    let update_str = String::from_utf8_lossy(&body).to_string();
    info!("Executing update: {}", update_str);

    let store = data.store.clone();
    web::block(move || store.update(&update_str))
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
        .map_err(actix_web::error::ErrorBadRequest)?;

    Ok(HttpResponse::NoContent().finish())
}

#[post("/upload")]
async fn upload_data(
    req: HttpRequest,
    body: web::Bytes,
    data: web::Data<AppData>,
) -> Result<HttpResponse> {
    let content_type = req.content_type();
    let format = match content_type {
        "text/turtle" => RdfFormat::Turtle,
        "application/n-triples" => RdfFormat::NTriples,
        "application/rdf+xml" => RdfFormat::RdfXml,
        "application/n-quads" => RdfFormat::NQuads,
        "application/trig" => RdfFormat::TriG,
        _ => return Ok(HttpResponse::UnsupportedMediaType().finish()),
    };

    info!("Uploading data with format: {:?}", format);

    let store = data.store.clone();
    let data_bytes = body.to_vec();

    web::block(move || {
        store.load_from_reader(RdfParser::from_format(format), &data_bytes[..])
    })
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?
    .map_err(actix_web::error::ErrorBadRequest)?;

    Ok(HttpResponse::Created().finish())
}

#[get("/export")]
async fn export_data(req: HttpRequest, data: web::Data<AppData>) -> Result<HttpResponse> {
    let accept = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("text/turtle");

    let format = match accept {
        "text/turtle" => RdfFormat::Turtle,
        "application/n-triples" => RdfFormat::NTriples,
        "application/n-quads" => RdfFormat::NQuads,
        "application/trig" => RdfFormat::TriG,
        _ => RdfFormat::Turtle,
    };

    let store = data.store.clone();
    let buffer = web::block(move || {
        let mut buf = Vec::new();
        store.dump_graph_to_writer(
            GraphName::DefaultGraph,
            RdfSerializer::from_format(format),
            &mut buf
        )?;
        Ok::<_, std::io::Error>(buf)
    })
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(accept)
        .body(buffer))
}

fn term_to_json_value(term: &oxigraph::model::Term) -> serde_json::Value {
    use oxigraph::model::Term;

    match term {
        Term::NamedNode(n) => serde_json::json!({
            "type": "uri",
            "value": n.as_str()
        }),
        Term::BlankNode(b) => serde_json::json!({
            "type": "bnode",
            "value": b.as_str()
        }),
        Term::Literal(l) => {
            let mut obj = serde_json::json!({
                "type": "literal",
                "value": l.value()
            });
            if let Some(lang) = l.language() {
                obj["xml:lang"] = serde_json::Value::String(lang.to_string());
            } else {
                obj["datatype"] = serde_json::Value::String(l.datatype().as_str().to_string());
            }
            obj
        }
        _ => serde_json::json!({"type": "unknown"}),
    }
}
```

## Tokio Async Patterns

Proper async integration with Tokio runtime.

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use tokio::task;
use std::sync::Arc;

// Pattern 1: Spawn blocking for I/O operations
async fn query_async(store: Arc<Store>, query: String) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    task::spawn_blocking(move || {
        let results = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&store)
            .execute()?;

        // Process results
        if let QueryResults::Solutions(solutions) = results {
            let mut output = Vec::new();
            for solution in solutions {
                let sol = solution?;
                output.push(format!("{:?}", sol));
            }
            Ok(output)
        } else {
            Ok(Vec::new())
        }
    })
    .await?
}

// Pattern 2: Concurrent queries
async fn concurrent_queries(store: Arc<Store>) -> Result<(), Box<dyn std::error::Error>> {
    let queries = vec![
        "SELECT * WHERE { ?s ?p ?o } LIMIT 10",
        "SELECT * WHERE { ?s a ?type }",
        "SELECT (COUNT(*) as ?count) WHERE { ?s ?p ?o }",
    ];

    let mut handles = Vec::new();

    for query in queries {
        let store = store.clone();
        let query = query.to_string();

        let handle = task::spawn_blocking(move || {
            SparqlEvaluator::new()
                .parse_query(&query)?
                .on_store(&store)
                .execute()
        });

        handles.push(handle);
    }

    // Wait for all queries
    for handle in handles {
        let result = handle.await??;
        println!("Query result: {:?}", result);
    }

    Ok(())
}

// Pattern 3: Bulk operations with progress
async fn bulk_insert(
    store: Arc<Store>,
    triples: Vec<(String, String, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    const BATCH_SIZE: usize = 1000;

    for (i, batch) in triples.chunks(BATCH_SIZE).enumerate() {
        let store = store.clone();
        let batch = batch.to_vec();

        task::spawn_blocking(move || {
            for (s, p, o) in batch {
                let subject = NamedNode::new(s)?;
                let predicate = NamedNode::new(p)?;
                let object = Literal::new_simple_literal(o);

                store.insert(&Quad::new(
                    subject,
                    predicate,
                    object,
                    GraphName::DefaultGraph,
                ))?;
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await??;

        println!("Processed batch {}", i + 1);
    }

    Ok(())
}
```

## Error Handling

Production-ready error handling with thiserror.

```rust
use thiserror::Error;
use oxigraph::store::StoreError;
use oxigraph::sparql::{QueryResults, SparqlEvaluator, QueryParseError, EvaluationError};

#[derive(Error, Debug)]
pub enum OxigraphApiError {
    #[error("Store operation failed: {0}")]
    Store(#[from] StoreError),

    #[error("SPARQL parse error: {0}")]
    QueryParse(#[from] QueryParseError),

    #[error("Query evaluation failed: {0}")]
    QueryEval(#[from] EvaluationError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid IRI: {0}")]
    InvalidIri(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

// Usage example
pub fn safe_query(store: &Store, query: &str) -> Result<QueryResults, OxigraphApiError> {
    SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()
        .map_err(Into::into)
}
```

## Logging and Monitoring

Structured logging with tracing.

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use tracing::{info, warn, error, instrument, span, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

pub fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

#[instrument(skip(store), fields(query_len = query.len()))]
async fn instrumented_query(
    store: Arc<Store>,
    query: String,
) -> Result<QueryResults, OxigraphApiError> {
    let span = span!(Level::INFO, "query_execution");
    let _enter = span.enter();

    info!("Starting query execution");

    let start = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&store)
            .execute()
    })
    .await
    .map_err(|e| OxigraphApiError::Internal(e.to_string()))??;

    let duration = start.elapsed();
    info!(duration_ms = duration.as_millis(), "Query completed");

    Ok(result)
}
```

## Production Deployment

Docker configuration for production.

### Dockerfile

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/oxigraph-axum-api /usr/local/bin/

ENV RUST_LOG=info
EXPOSE 3000

CMD ["oxigraph-axum-api"]
```

### docker-compose.yml

```yaml
version: '3.8'

services:
  oxigraph-api:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped
```

## Advanced Patterns

### Custom SPARQL Functions

```rust
// This is a conceptual example - Oxigraph doesn't expose custom function registration yet
// But you can preprocess queries or use SPARQL 1.1 built-in functions

fn add_custom_prefixes(query: &str) -> String {
    let prefixes = r#"
        PREFIX schema: <http://schema.org/>
        PREFIX ex: <http://example.org/>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
    "#;

    format!("{}\n{}", prefixes, query)
}
```

### Transaction Patterns

```rust
use oxigraph::store::Store;

fn transactional_update(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    // Oxigraph operations are transactional by default
    // For bulk operations, group them together

    let updates = vec![
        "INSERT DATA { <http://example.org/s1> <http://example.org/p> \"value1\" }",
        "INSERT DATA { <http://example.org/s2> <http://example.org/p> \"value2\" }",
        "DELETE WHERE { <http://example.org/old> ?p ?o }",
    ];

    for update in updates {
        store.update(update)?;
    }

    Ok(())
}
```

### Streaming Large Results

```rust
use oxigraph::sparql::{QueryResults, QuerySolution, EvaluationError, SparqlEvaluator};
use std::sync::Arc;

async fn stream_results(
    store: Arc<Store>,
    query: String,
) -> impl Stream<Item = Result<QuerySolution, EvaluationError>> {
    tokio_stream::wrappers::ReceiverStream::new({
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::task::spawn_blocking(move || {
            if let Ok(QueryResults::Solutions(solutions)) = SparqlEvaluator::new()
                .parse_query(&query)
                .and_then(|q| q.on_store(&store).execute())
            {
                for solution in solutions {
                    if tx.blocking_send(solution).is_err() {
                        break;
                    }
                }
            }
        });

        rx
    })
}
```

---

These patterns provide a solid foundation for building production Rust applications with Oxigraph. Adapt them to your specific requirements!
