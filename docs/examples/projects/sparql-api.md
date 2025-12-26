# RESTful and GraphQL API over SPARQL

A complete API server implementation that provides RESTful and GraphQL interfaces over SPARQL, with caching, rate limiting, and authentication.

## Architecture

```
┌──────────────┐      ┌────────────────┐      ┌──────────────┐
│   REST API   │─────▶│  API Gateway   │─────▶│   SPARQL     │
│   Client     │      │  (Rate Limit,  │      │   Translator │
└──────────────┘      │   Auth, Cache) │      └──────┬───────┘
                      └────────────────┘             │
┌──────────────┐             │                       │
│  GraphQL     │─────────────┘                       │
│   Client     │                                     │
└──────────────┘                                     │
                                                     │
                      ┌──────────────────────────────┘
                      │
                 ┌────▼─────┐         ┌───────────────┐
                 │ Oxigraph │◀────────│  Redis Cache  │
                 │  Store   │         │               │
                 └──────────┘         └───────────────┘
```

## Implementation

### Rust Implementation

#### Cargo.toml

```toml
[package]
name = "sparql-api-server"
version = "0.1.0"
edition = "2021"

[dependencies]
oxigraph = "0.4"
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace", "limit"] }
async-graphql = "7.0"
async-graphql-axum = "7.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
jsonwebtoken = "9.2"
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
sha2 = "0.10"
hex = "0.4"
chrono = "0.4"
governor = "0.6"
nonzero_ext = "0.3"
```

#### src/main.rs

```rust
use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::info;
use redis::AsyncCommands;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

const API_NS: &str = "http://example.org/api/";

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    redis: Arc<redis::Client>,
    rate_limiter: Arc<RateLimiter<String, DashMapStateStore, DefaultClock>>,
    jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    role: String,
}

use governor::state::keyed::DashMapStateStore;
use governor::clock::DefaultClock;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize store
    let store = Store::new()?;
    load_sample_data(&store)?;

    // Initialize Redis
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;

    // Initialize rate limiter (100 requests per minute per user)
    let quota = Quota::per_minute(NonZeroU32::new(100).unwrap());
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    let state = AppState {
        store: Arc::new(store),
        redis: Arc::new(redis_client),
        rate_limiter,
        jwt_secret: "your-secret-key".to_string(),
    };

    // Build GraphQL schema
    let graphql_schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(state.clone())
        .finish();

    // Build REST router
    let rest_api = Router::new()
        .route("/products", get(list_products).post(create_product))
        .route("/products/:id", get(get_product).delete(delete_product))
        .route("/categories", get(list_categories))
        .route("/search", get(search))
        .route("/sparql", post(sparql_endpoint))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    // Build GraphQL router
    let graphql_api = Router::new()
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Build main app
    let app = Router::new()
        .nest("/api", rest_api)
        .nest("/", graphql_api)
        .route("/auth/login", post(login))
        .route("/health", get(health_check))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .layer(tower_http::trace::TraceLayer::new_for_http()),
        )
        .with_state(state)
        .with_state(graphql_schema);

    info!("Server starting on http://localhost:3000");
    info!("GraphQL Playground: http://localhost:3000/graphql");
    info!("REST API: http://localhost:3000/api");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn load_sample_data(store: &Store) -> Result<()> {
    use oxigraph::model::*;

    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;

    // Product 1
    let p1 = NamedNode::new(format!("{}product/1", API_NS))?;
    let product_class = NamedNode::new(format!("{}Product", API_NS))?;
    let name_pred = NamedNode::new(format!("{}name", API_NS))?;
    let price_pred = NamedNode::new(format!("{}price", API_NS))?;
    let category_pred = NamedNode::new(format!("{}category", API_NS))?;

    store.insert(&Quad::new(p1.clone(), rdf_type.clone(), product_class.clone(), GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p1.clone(), name_pred.clone(), Literal::new_simple_literal("Laptop"), GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p1.clone(), price_pred.clone(), Literal::new_typed_literal("999.99", xsd::DECIMAL), GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p1, category_pred.clone(), NamedNode::new(format!("{}category/electronics", API_NS))?, GraphName::DefaultGraph))?;

    // Product 2
    let p2 = NamedNode::new(format!("{}product/2", API_NS))?;
    store.insert(&Quad::new(p2.clone(), rdf_type, product_class, GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p2.clone(), name_pred, Literal::new_simple_literal("Mouse"), GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p2.clone(), price_pred, Literal::new_typed_literal("29.99", xsd::DECIMAL), GraphName::DefaultGraph))?;
    store.insert(&Quad::new(p2, category_pred, NamedNode::new(format!("{}category/electronics", API_NS))?, GraphName::DefaultGraph))?;

    info!("Sample data loaded");
    Ok(())
}

// Authentication Middleware
async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<impl IntoResponse, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Store claims in request extensions
    request.extensions_mut().insert(token_data.claims);

    Ok(next.run(request).await)
}

// Rate Limiting Middleware
async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<impl IntoResponse, StatusCode> {
    // Extract user ID from request (simplified)
    let user_id = "default_user".to_string();

    if state.rate_limiter.check_key(&user_id).is_err() {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(request).await)
}

// REST API Endpoints

#[derive(Serialize, Deserialize)]
struct Product {
    id: String,
    name: String,
    price: f64,
    category: Option<String>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

async fn list_products(State(state): State<AppState>) -> Result<Json<Vec<Product>>, StatusCode> {
    // Check cache first
    let cache_key = "products:all";
    if let Ok(cached) = get_from_cache(&state.redis, cache_key).await {
        return Ok(Json(cached));
    }

    let query = format!(
        r#"
        PREFIX api: <{API_NS}>

        SELECT ?id ?name ?price ?category WHERE {{
            ?id a api:Product ;
                api:name ?name ;
                api:price ?price .
            OPTIONAL {{ ?id api:category ?category }}
        }}
        ORDER BY ?name
        LIMIT 100
    "#
    );

    let results = state
        .store
        .query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let products = extract_products(results)?;

    // Cache results
    set_in_cache(&state.redis, cache_key, &products, 300).await?;

    Ok(Json(products))
}

async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Product>, StatusCode> {
    let cache_key = format!("product:{}", id);
    if let Ok(cached) = get_from_cache(&state.redis, &cache_key).await {
        return Ok(Json(cached));
    }

    let product_iri = format!("{}product/{}", API_NS, id);

    let query = format!(
        r#"
        PREFIX api: <{API_NS}>

        SELECT ?name ?price ?category WHERE {{
            <{product_iri}> api:name ?name ;
                            api:price ?price .
            OPTIONAL {{ <{product_iri}> api:category ?category }}
        }}
    "#
    );

    let results = state
        .store
        .query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let QueryResults::Solutions(mut solutions) = results {
        if let Some(Ok(solution)) = solutions.next() {
            let product = Product {
                id: product_iri,
                name: solution
                    .get("name")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                price: solution
                    .get("price")
                    .and_then(|t| t.as_ref().as_literal())
                    .and_then(|l| l.value().parse().ok())
                    .unwrap_or(0.0),
                category: solution
                    .get("category")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string()),
            };

            set_in_cache(&state.redis, &cache_key, &product, 300).await?;

            return Ok(Json(product));
        }
    }

    Err(StatusCode::NOT_FOUND)
}

#[derive(Deserialize)]
struct CreateProductRequest {
    name: String,
    price: f64,
    category: Option<String>,
}

async fn create_product(
    State(state): State<AppState>,
    Json(req): Json<CreateProductRequest>,
) -> Result<Json<Product>, StatusCode> {
    use oxigraph::model::*;

    let id = uuid::Uuid::new_v4().to_string();
    let product = NamedNode::new(format!("{}product/{}", API_NS, id))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let product_class = NamedNode::new(format!("{}Product", API_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let name_pred = NamedNode::new(format!("{}name", API_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let price_pred = NamedNode::new(format!("{}price", API_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .store
        .insert(&Quad::new(product.clone(), rdf_type, product_class, GraphName::DefaultGraph))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .store
        .insert(&Quad::new(
            product.clone(),
            name_pred,
            Literal::new_simple_literal(&req.name),
            GraphName::DefaultGraph,
        ))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .store
        .insert(&Quad::new(
            product.clone(),
            price_pred,
            Literal::new_typed_literal(req.price.to_string(), xsd::DECIMAL),
            GraphName::DefaultGraph,
        ))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Invalidate cache
    invalidate_cache(&state.redis, "products:all").await?;

    Ok(Json(Product {
        id: product.as_str().to_string(),
        name: req.name,
        price: req.price,
        category: req.category,
    }))
}

async fn delete_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    use oxigraph::model::*;

    let product = NamedNode::new(format!("{}product/{}", API_NS, id))
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let quads: Vec<_> = state
        .store
        .quads_for_pattern(Some(product.as_ref()), None, None, None)
        .collect();

    for quad in quads {
        let quad = quad.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .store
            .remove(&quad)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Invalidate cache
    invalidate_cache(&state.redis, "products:all").await?;
    invalidate_cache(&state.redis, &format!("product:{}", id)).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_categories(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let query = format!(
        r#"
        PREFIX api: <{API_NS}>

        SELECT DISTINCT ?category WHERE {{
            ?product api:category ?category .
        }}
        ORDER BY ?category
    "#
    );

    let results = state
        .store
        .query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut categories = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if let Some(category) = solution
                .get("category")
                .and_then(|t| t.as_ref().as_named_node())
                .map(|n| n.as_str().to_string())
            {
                categories.push(category);
            }
        }
    }

    Ok(Json(categories))
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<Product>>, StatusCode> {
    let search_term = params.q.to_lowercase();

    let query = format!(
        r#"
        PREFIX api: <{API_NS}>

        SELECT ?id ?name ?price ?category WHERE {{
            ?id a api:Product ;
                api:name ?name ;
                api:price ?price .
            OPTIONAL {{ ?id api:category ?category }}
            FILTER(CONTAINS(LCASE(?name), "{search_term}"))
        }}
        LIMIT {}
    "#,
        params.limit
    );

    let results = state
        .store
        .query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let products = extract_products(results)?;

    Ok(Json(products))
}

#[derive(Deserialize)]
struct SparqlRequest {
    query: String,
}

async fn sparql_endpoint(
    State(state): State<AppState>,
    Json(req): Json<SparqlRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Generate cache key from query
    let cache_key = format!("sparql:{}", sha2_hash(&req.query));

    if let Ok(cached) = get_from_cache(&state.redis, &cache_key).await {
        return Ok(Json(cached));
    }

    let results = state
        .store
        .query(&req.query)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let response = match results {
        QueryResults::Solutions(solutions) => {
            let mut bindings = Vec::new();
            for solution in solutions {
                let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let mut binding = serde_json::Map::new();

                for (var, term) in solution.iter() {
                    binding.insert(var.as_str().to_string(), serde_json::json!(term.to_string()));
                }
                bindings.push(serde_json::Value::Object(binding));
            }
            serde_json::json!({ "results": { "bindings": bindings } })
        }
        QueryResults::Boolean(b) => serde_json::json!({ "boolean": b }),
        QueryResults::Graph(_) => serde_json::json!({ "type": "graph" }),
    };

    // Cache for 5 minutes
    set_in_cache(&state.redis, &cache_key, &response, 300).await?;

    Ok(Json(response))
}

// GraphQL API

struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn products(&self, ctx: &Context<'_>) -> Vec<Product> {
        let state = ctx.data::<AppState>().unwrap();

        let query = format!(
            r#"
            PREFIX api: <{API_NS}>

            SELECT ?id ?name ?price ?category WHERE {{
                ?id a api:Product ;
                    api:name ?name ;
                    api:price ?price .
                OPTIONAL {{ ?id api:category ?category }}
            }}
            ORDER BY ?name
            LIMIT 100
        "#
        );

        if let Ok(results) = state.store.query(&query) {
            extract_products(results).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    async fn product(&self, ctx: &Context<'_>, id: String) -> Option<Product> {
        let state = ctx.data::<AppState>().unwrap();
        let product_iri = format!("{}product/{}", API_NS, id);

        let query = format!(
            r#"
            PREFIX api: <{API_NS}>

            SELECT ?name ?price ?category WHERE {{
                <{product_iri}> api:name ?name ;
                                api:price ?price .
                OPTIONAL {{ <{product_iri}> api:category ?category }}
            }}
        "#
        );

        if let Ok(QueryResults::Solutions(mut solutions)) = state.store.query(&query) {
            if let Some(Ok(solution)) = solutions.next() {
                return Some(Product {
                    id: product_iri,
                    name: solution
                        .get("name")
                        .and_then(|t| t.as_ref().as_literal())
                        .map(|l| l.value().to_string())
                        .unwrap_or_default(),
                    price: solution
                        .get("price")
                        .and_then(|t| t.as_ref().as_literal())
                        .and_then(|l| l.value().parse().ok())
                        .unwrap_or(0.0),
                    category: solution
                        .get("category")
                        .and_then(|t| t.as_ref().as_named_node())
                        .map(|n| n.as_str().to_string()),
                });
            }
        }

        None
    }

    async fn search(&self, ctx: &Context<'_>, query: String) -> Vec<Product> {
        let state = ctx.data::<AppState>().unwrap();
        let search_term = query.to_lowercase();

        let sparql = format!(
            r#"
            PREFIX api: <{API_NS}>

            SELECT ?id ?name ?price ?category WHERE {{
                ?id a api:Product ;
                    api:name ?name ;
                    api:price ?price .
                OPTIONAL {{ ?id api:category ?category }}
                FILTER(CONTAINS(LCASE(?name), "{search_term}"))
            }}
            LIMIT 10
        "#
        );

        if let Ok(results) = state.store.query(&sparql) {
            extract_products(results).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

async fn graphql_handler(
    State(schema): State<Schema<QueryRoot, EmptyMutation, EmptySubscription>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground() -> impl IntoResponse {
    axum::response::Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    ))
}

// Authentication

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Simplified authentication (in production, verify against database)
    if req.username == "admin" && req.password == "password" {
        let claims = Claims {
            sub: req.username,
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
            role: "admin".to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(state.jwt_secret.as_ref()),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(LoginResponse { token }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn health_check() -> &'static str {
    "OK"
}

// Helper Functions

fn extract_products(results: QueryResults) -> Result<Vec<Product>, StatusCode> {
    let mut products = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            products.push(Product {
                id: solution
                    .get("id")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
                name: solution
                    .get("name")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                price: solution
                    .get("price")
                    .and_then(|t| t.as_ref().as_literal())
                    .and_then(|l| l.value().parse().ok())
                    .unwrap_or(0.0),
                category: solution
                    .get("category")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string()),
            });
        }
    }

    Ok(products)
}

async fn get_from_cache<T: serde::de::DeserializeOwned>(
    redis: &redis::Client,
    key: &str,
) -> Result<T, ()> {
    let mut conn = redis.get_multiplexed_async_connection().await.map_err(|_| ())?;
    let cached: String = conn.get(key).await.map_err(|_| ())?;
    serde_json::from_str(&cached).map_err(|_| ())
}

async fn set_in_cache<T: serde::Serialize>(
    redis: &redis::Client,
    key: &str,
    value: &T,
    ttl: usize,
) -> Result<(), StatusCode> {
    let mut conn = redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let json = serde_json::to_string(value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    conn.set_ex(key, json, ttl)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

async fn invalidate_cache(redis: &redis::Client, key: &str) -> Result<(), StatusCode> {
    let mut conn = redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    conn.del(key)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

fn sha2_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}
```

### Python Implementation

#### requirements.txt

```txt
pyoxigraph>=0.4.0
fastapi>=0.109.0
uvicorn>=0.27.0
pyjwt>=2.8.0
redis>=5.0.0
strawberry-graphql>=0.219.0
slowapi>=0.1.9
passlib>=1.7.4
python-multipart>=0.0.6
```

#### api_server.py

```python
from fastapi import FastAPI, HTTPException, Depends, status, Request
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from fastapi.middleware.cors import CORSMiddleware
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph
import strawberry
from strawberry.fastapi import GraphQLRouter
from pydantic import BaseModel
from typing import Optional, List
import jwt
from datetime import datetime, timedelta
import redis
import json
import hashlib
from slowapi import Limiter, _rate_limit_exceeded_handler
from slowapi.util import get_remote_address
from slowapi.errors import RateLimitExceeded

# Configuration
API_NS = "http://example.org/api/"
JWT_SECRET = "your-secret-key"
JWT_ALGORITHM = "HS256"
REDIS_URL = "redis://localhost:6379"

app = FastAPI(title="SPARQL API Server")
security = HTTPBearer()
limiter = Limiter(key_func=get_remote_address)
app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)

# CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Initialize services
store = Store()
redis_client = redis.from_url(REDIS_URL, decode_responses=True)

def load_sample_data():
    """Load sample data"""
    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    product_class = NamedNode(f"{API_NS}Product")
    name_pred = NamedNode(f"{API_NS}name")
    price_pred = NamedNode(f"{API_NS}price")
    category_pred = NamedNode(f"{API_NS}category")

    # Product 1
    p1 = NamedNode(f"{API_NS}product/1")
    store.add(Quad(p1, rdf_type, product_class, DefaultGraph()))
    store.add(Quad(p1, name_pred, Literal("Laptop"), DefaultGraph()))
    store.add(Quad(p1, price_pred, Literal("999.99", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal")), DefaultGraph()))
    store.add(Quad(p1, category_pred, NamedNode(f"{API_NS}category/electronics"), DefaultGraph()))

    # Product 2
    p2 = NamedNode(f"{API_NS}product/2")
    store.add(Quad(p2, rdf_type, product_class, DefaultGraph()))
    store.add(Quad(p2, name_pred, Literal("Mouse"), DefaultGraph()))
    store.add(Quad(p2, price_pred, Literal("29.99", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal")), DefaultGraph()))
    store.add(Quad(p2, category_pred, NamedNode(f"{API_NS}category/electronics"), DefaultGraph()))

    print("Sample data loaded")

load_sample_data()

# Models
class Product(BaseModel):
    id: str
    name: str
    price: float
    category: Optional[str] = None

class CreateProductRequest(BaseModel):
    name: str
    price: float
    category: Optional[str] = None

class LoginRequest(BaseModel):
    username: str
    password: str

class LoginResponse(BaseModel):
    token: str

class SparqlRequest(BaseModel):
    query: str

# Authentication
def create_token(username: str, role: str = "user") -> str:
    """Create JWT token"""
    payload = {
        "sub": username,
        "role": role,
        "exp": datetime.utcnow() + timedelta(hours=24)
    }
    return jwt.encode(payload, JWT_SECRET, algorithm=JWT_ALGORITHM)

def verify_token(credentials: HTTPAuthorizationCredentials = Depends(security)) -> dict:
    """Verify JWT token"""
    try:
        payload = jwt.decode(credentials.credentials, JWT_SECRET, algorithms=[JWT_ALGORITHM])
        return payload
    except jwt.ExpiredSignatureError:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Token expired")
    except jwt.InvalidTokenError:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid token")

# Cache helpers
def get_from_cache(key: str):
    """Get from Redis cache"""
    cached = redis_client.get(key)
    if cached:
        return json.loads(cached)
    return None

def set_in_cache(key: str, value: any, ttl: int = 300):
    """Set in Redis cache"""
    redis_client.setex(key, ttl, json.dumps(value))

def invalidate_cache(key: str):
    """Invalidate cache key"""
    redis_client.delete(key)

def sha2_hash(text: str) -> str:
    """SHA256 hash"""
    return hashlib.sha256(text.encode()).hexdigest()

# REST API Endpoints

@app.post("/auth/login", response_model=LoginResponse)
def login(req: LoginRequest):
    """Login endpoint"""
    # Simplified authentication
    if req.username == "admin" and req.password == "password":
        token = create_token(req.username, "admin")
        return LoginResponse(token=token)
    raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid credentials")

@app.get("/health")
def health_check():
    """Health check"""
    return {"status": "OK"}

@app.get("/api/products", response_model=List[Product])
@limiter.limit("100/minute")
def list_products(request: Request, user: dict = Depends(verify_token)):
    """List all products"""
    cache_key = "products:all"
    cached = get_from_cache(cache_key)
    if cached:
        return cached

    query = f"""
        PREFIX api: <{API_NS}>

        SELECT ?id ?name ?price ?category WHERE {{
            ?id a api:Product ;
                api:name ?name ;
                api:price ?price .
            OPTIONAL {{ ?id api:category ?category }}
        }}
        ORDER BY ?name
        LIMIT 100
    """

    products = []
    for row in store.query(query):
        products.append({
            "id": str(row['id']),
            "name": str(row['name']),
            "price": float(str(row['price'])),
            "category": str(row['category']) if row.get('category') else None
        })

    set_in_cache(cache_key, products, 300)
    return products

@app.get("/api/products/{product_id}", response_model=Product)
@limiter.limit("100/minute")
def get_product(request: Request, product_id: str, user: dict = Depends(verify_token)):
    """Get product by ID"""
    cache_key = f"product:{product_id}"
    cached = get_from_cache(cache_key)
    if cached:
        return cached

    product_iri = f"{API_NS}product/{product_id}"

    query = f"""
        PREFIX api: <{API_NS}>

        SELECT ?name ?price ?category WHERE {{
            <{product_iri}> api:name ?name ;
                            api:price ?price .
            OPTIONAL {{ <{product_iri}> api:category ?category }}
        }}
    """

    results = list(store.query(query))
    if not results:
        raise HTTPException(status_code=404, detail="Product not found")

    row = results[0]
    product = {
        "id": product_iri,
        "name": str(row['name']),
        "price": float(str(row['price'])),
        "category": str(row['category']) if row.get('category') else None
    }

    set_in_cache(cache_key, product, 300)
    return product

@app.post("/api/products", response_model=Product)
@limiter.limit("20/minute")
def create_product(request: Request, req: CreateProductRequest, user: dict = Depends(verify_token)):
    """Create new product"""
    import uuid
    product_id = str(uuid.uuid4())
    product = NamedNode(f"{API_NS}product/{product_id}")

    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    product_class = NamedNode(f"{API_NS}Product")
    name_pred = NamedNode(f"{API_NS}name")
    price_pred = NamedNode(f"{API_NS}price")

    store.add(Quad(product, rdf_type, product_class, DefaultGraph()))
    store.add(Quad(product, name_pred, Literal(req.name), DefaultGraph()))
    store.add(Quad(product, price_pred,
                  Literal(str(req.price), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal")),
                  DefaultGraph()))

    invalidate_cache("products:all")

    return Product(
        id=str(product),
        name=req.name,
        price=req.price,
        category=req.category
    )

@app.delete("/api/products/{product_id}", status_code=status.HTTP_204_NO_CONTENT)
@limiter.limit("20/minute")
def delete_product(request: Request, product_id: str, user: dict = Depends(verify_token)):
    """Delete product"""
    product = NamedNode(f"{API_NS}product/{product_id}")

    for quad in store.quads_for_pattern(product, None, None, None):
        store.remove(quad)

    invalidate_cache("products:all")
    invalidate_cache(f"product:{product_id}")

@app.get("/api/search", response_model=List[Product])
@limiter.limit("100/minute")
def search(request: Request, q: str, limit: int = 10, user: dict = Depends(verify_token)):
    """Search products"""
    search_term = q.lower()

    query = f"""
        PREFIX api: <{API_NS}>

        SELECT ?id ?name ?price ?category WHERE {{
            ?id a api:Product ;
                api:name ?name ;
                api:price ?price .
            OPTIONAL {{ ?id api:category ?category }}
            FILTER(CONTAINS(LCASE(?name), "{search_term}"))
        }}
        LIMIT {limit}
    """

    products = []
    for row in store.query(query):
        products.append({
            "id": str(row['id']),
            "name": str(row['name']),
            "price": float(str(row['price'])),
            "category": str(row['category']) if row.get('category') else None
        })

    return products

@app.post("/api/sparql")
@limiter.limit("50/minute")
def sparql_endpoint(request: Request, req: SparqlRequest, user: dict = Depends(verify_token)):
    """SPARQL endpoint"""
    cache_key = f"sparql:{sha2_hash(req.query)}"
    cached = get_from_cache(cache_key)
    if cached:
        return cached

    try:
        results = store.query(req.query)
        bindings = []

        for row in results:
            binding = {}
            for var in row:
                binding[var] = str(row[var])
            bindings.append(binding)

        response = {"results": {"bindings": bindings}}
        set_in_cache(cache_key, response, 300)

        return response
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))

# GraphQL API

@strawberry.type
class ProductType:
    id: str
    name: str
    price: float
    category: Optional[str]

@strawberry.type
class Query:
    @strawberry.field
    def products(self) -> List[ProductType]:
        query = f"""
            PREFIX api: <{API_NS}>

            SELECT ?id ?name ?price ?category WHERE {{
                ?id a api:Product ;
                    api:name ?name ;
                    api:price ?price .
                OPTIONAL {{ ?id api:category ?category }}
            }}
            LIMIT 100
        """

        products = []
        for row in store.query(query):
            products.append(ProductType(
                id=str(row['id']),
                name=str(row['name']),
                price=float(str(row['price'])),
                category=str(row['category']) if row.get('category') else None
            ))
        return products

    @strawberry.field
    def product(self, id: str) -> Optional[ProductType]:
        product_iri = f"{API_NS}product/{id}"

        query = f"""
            PREFIX api: <{API_NS}>

            SELECT ?name ?price ?category WHERE {{
                <{product_iri}> api:name ?name ;
                                api:price ?price .
                OPTIONAL {{ <{product_iri}> api:category ?category }}
            }}
        """

        results = list(store.query(query))
        if results:
            row = results[0]
            return ProductType(
                id=product_iri,
                name=str(row['name']),
                price=float(str(row['price'])),
                category=str(row['category']) if row.get('category') else None
            )
        return None

schema = strawberry.Schema(query=Query)
graphql_app = GraphQLRouter(schema)

app.include_router(graphql_app, prefix="/graphql")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=3000)
```

## Usage Examples

### Authentication

```bash
# Login
curl -X POST http://localhost:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password"}'

# Response: {"token": "eyJ..."}
```

### REST API

```bash
# List products (with auth)
curl http://localhost:3000/api/products \
  -H "Authorization: Bearer eyJ..."

# Get product
curl http://localhost:3000/api/products/1 \
  -H "Authorization: Bearer eyJ..."

# Create product
curl -X POST http://localhost:3000/api/products \
  -H "Authorization: Bearer eyJ..." \
  -H "Content-Type: application/json" \
  -d '{"name": "Keyboard", "price": 79.99, "category": "electronics"}'

# Search
curl "http://localhost:3000/api/search?q=laptop&limit=5" \
  -H "Authorization: Bearer eyJ..."

# SPARQL query
curl -X POST http://localhost:3000/api/sparql \
  -H "Authorization: Bearer eyJ..." \
  -H "Content-Type: application/json" \
  -d '{"query": "PREFIX api: <http://example.org/api/> SELECT * WHERE { ?s a api:Product } LIMIT 10"}'
```

### GraphQL

```graphql
query {
  products {
    id
    name
    price
    category
  }
}

query {
  product(id: "1") {
    id
    name
    price
  }
}

query {
  search(query: "laptop") {
    id
    name
    price
  }
}
```

## Features

1. **RESTful API**: Standard REST endpoints over SPARQL
2. **GraphQL API**: Flexible GraphQL interface
3. **Authentication**: JWT-based auth
4. **Rate Limiting**: Per-user rate limits
5. **Caching**: Redis-based query caching
6. **CORS**: Cross-origin support
7. **Error Handling**: Comprehensive error responses
8. **Health Checks**: Service health monitoring

## Production Deployment

### Docker Compose

```yaml
version: '3.8'

services:
  api:
    build: .
    ports:
      - "3000:3000"
    environment:
      - JWT_SECRET=your-secret-key
      - REDIS_URL=redis://redis:6379
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

### Run

```bash
# Rust
cargo build --release
./target/release/sparql-api-server

# Python
pip install -r requirements.txt
uvicorn api_server:app --host 0.0.0.0 --port 3000

# Docker
docker-compose up
```

## API Documentation

- REST API docs: http://localhost:3000/docs
- GraphQL Playground: http://localhost:3000/graphql
- Health check: http://localhost:3000/health
