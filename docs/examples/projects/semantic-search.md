# Semantic Search Engine

A complete semantic search implementation using Oxigraph with text-to-SPARQL conversion, full-text search integration, result ranking, and faceted search.

## Architecture

```
┌──────────────┐      ┌─────────────────┐      ┌──────────────┐
│   User Query │─────▶│  Query Analyzer │─────▶│   SPARQL     │
│  (Natural)   │      │  (NLP/Template) │      │  Generator   │
└──────────────┘      └─────────────────┘      └──────┬───────┘
                                                       │
                      ┌────────────────────────────────┘
                      │
                 ┌────▼─────┐         ┌───────────────┐
                 │ Oxigraph │◀────────│  Full-text    │
                 │  Store   │         │  Index        │
                 └────┬─────┘         └───────────────┘
                      │
                 ┌────▼─────┐
                 │  Result  │
                 │  Ranker  │
                 └────┬─────┘
                      │
                 ┌────▼─────┐
                 │ Faceted  │
                 │ Results  │
                 └──────────┘
```

## Data Model

### Ontology (schema.ttl)

```turtle
@prefix search: <http://example.org/search/> .
@prefix schema: <http://schema.org/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# Core Classes
search:Article a owl:Class ;
    rdfs:label "Article" ;
    rdfs:comment "A searchable article or document" .

search:Author a owl:Class ;
    rdfs:label "Author" ;
    rdfs:subClassOf schema:Person .

search:Category a owl:Class ;
    rdfs:label "Category" ;
    rdfs:comment "Article category for faceted search" .

search:Tag a owl:Class ;
    rdfs:label "Tag" ;
    rdfs:comment "Article tag" .

# Properties
search:title a owl:DatatypeProperty ;
    rdfs:domain search:Article ;
    rdfs:range xsd:string ;
    rdfs:label "title" .

search:abstract a owl:DatatypeProperty ;
    rdfs:domain search:Article ;
    rdfs:range xsd:string ;
    rdfs:label "abstract" .

search:fullText a owl:DatatypeProperty ;
    rdfs:domain search:Article ;
    rdfs:range xsd:string ;
    rdfs:label "full text content" .

search:publishedDate a owl:DatatypeProperty ;
    rdfs:range xsd:dateTime ;
    rdfs:label "published date" .

search:viewCount a owl:DatatypeProperty ;
    rdfs:range xsd:integer ;
    rdfs:label "view count" .

search:score a owl:DatatypeProperty ;
    rdfs:range xsd:decimal ;
    rdfs:label "relevance score" .

search:author a owl:ObjectProperty ;
    rdfs:domain search:Article ;
    rdfs:range search:Author ;
    rdfs:label "author" .

search:category a owl:ObjectProperty ;
    rdfs:domain search:Article ;
    rdfs:range search:Category ;
    rdfs:label "category" .

search:tag a owl:ObjectProperty ;
    rdfs:domain search:Article ;
    rdfs:range search:Tag ;
    rdfs:label "tag" .

search:relatedTo a owl:ObjectProperty ;
    owl:inverseOf search:relatedTo ;
    rdfs:label "related to" .
```

## Implementation

### Rust Implementation

#### Cargo.toml

```toml
[package]
name = "semantic-search"
version = "0.1.0"
edition = "2021"

[dependencies]
oxigraph = "0.4"
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
regex = "1.10"
tantivy = "0.22"
chrono = "0.4"
unicode-segmentation = "1.11"
tower-http = { version = "0.5", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

#### src/main.rs

```rust
use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use oxigraph::store::Store;
use oxigraph::model::*;
use oxigraph::sparql::QueryResults;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use regex::Regex;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use std::sync::Mutex;

const SEARCH_NS: &str = "http://example.org/search/";

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    index: Arc<Mutex<Index>>,
    index_writer: Arc<Mutex<IndexWriter>>,
}

#[derive(Serialize, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    from_date: Option<String>,
    #[serde(default)]
    to_date: Option<String>,
}

fn default_limit() -> usize { 10 }

#[derive(Serialize)]
struct SearchResult {
    id: String,
    title: String,
    abstract_text: String,
    score: f32,
    author: Option<String>,
    category: Option<String>,
    published_date: Option<String>,
    tags: Vec<String>,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    total: usize,
    facets: Facets,
    query_info: QueryInfo,
}

#[derive(Serialize)]
struct Facets {
    categories: Vec<FacetItem>,
    authors: Vec<FacetItem>,
    tags: Vec<FacetItem>,
    date_ranges: Vec<DateRangeFacet>,
}

#[derive(Serialize)]
struct FacetItem {
    name: String,
    count: usize,
}

#[derive(Serialize)]
struct DateRangeFacet {
    range: String,
    count: usize,
}

#[derive(Serialize)]
struct QueryInfo {
    original_query: String,
    sparql_query: String,
    search_type: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize Oxigraph store
    let store = Store::new()?;
    load_schema(&store)?;

    // Initialize Tantivy full-text index
    let (index, index_writer) = create_fulltext_index()?;

    // Load sample data
    load_sample_data(&store, &index_writer)?;

    let state = AppState {
        store: Arc::new(store),
        index: Arc::new(Mutex::new(index)),
        index_writer: Arc::new(Mutex::new(index_writer)),
    };

    let app = Router::new()
        .route("/search", get(search))
        .route("/suggest", get(suggest))
        .route("/facets", get(get_facets))
        .route("/similar/:id", get(find_similar))
        .route("/index", post(index_document))
        .layer(CorsLayer::permissive())
        .with_state(state);

    info!("Semantic search server starting on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn load_schema(store: &Store) -> Result<()> {
    let schema = include_str!("../schema.ttl");
    let parser = oxigraph::io::RdfParser::from_format(oxigraph::io::RdfFormat::Turtle);
    store.load_from_reader(parser, schema.as_bytes())?;
    info!("Schema loaded");
    Ok(())
}

fn create_fulltext_index() -> Result<(Index, IndexWriter)> {
    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field("id", STRING | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("abstract", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT);
    schema_builder.add_text_field("author", STRING | STORED);
    schema_builder.add_text_field("category", STRING | STORED);

    let schema = schema_builder.build();
    let index = Index::create_in_ram(schema);
    let index_writer = index.writer(50_000_000)?;

    info!("Full-text index created");
    Ok((index, index_writer))
}

fn load_sample_data(store: &Store, index_writer: &IndexWriter) -> Result<()> {
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;

    // Create author
    let author = NamedNode::new(format!("{}author/john-smith", SEARCH_NS))?;
    let author_class = NamedNode::new(format!("{}Author", SEARCH_NS))?;
    let name_pred = NamedNode::new("http://schema.org/name")?;

    store.insert(&Quad::new(
        author.clone(),
        rdf_type.clone(),
        author_class,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        author.clone(),
        name_pred,
        Literal::new_simple_literal("John Smith"),
        GraphName::DefaultGraph,
    ))?;

    // Create category
    let category = NamedNode::new(format!("{}category/technology", SEARCH_NS))?;
    let category_class = NamedNode::new(format!("{}Category", SEARCH_NS))?;
    let label_pred = NamedNode::new("http://www.w3.org/2000/01/rdf-schema#label")?;

    store.insert(&Quad::new(
        category.clone(),
        rdf_type.clone(),
        category_class,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        category.clone(),
        label_pred.clone(),
        Literal::new_simple_literal("Technology"),
        GraphName::DefaultGraph,
    ))?;

    // Create article
    let article = NamedNode::new(format!("{}article/1", SEARCH_NS))?;
    let article_class = NamedNode::new(format!("{}Article", SEARCH_NS))?;
    let title_pred = NamedNode::new(format!("{}title", SEARCH_NS))?;
    let abstract_pred = NamedNode::new(format!("{}abstract", SEARCH_NS))?;
    let fulltext_pred = NamedNode::new(format!("{}fullText", SEARCH_NS))?;
    let author_pred = NamedNode::new(format!("{}author", SEARCH_NS))?;
    let category_pred = NamedNode::new(format!("{}category", SEARCH_NS))?;

    store.insert(&Quad::new(
        article.clone(),
        rdf_type,
        article_class,
        GraphName::DefaultGraph,
    ))?;

    let title = "Introduction to Semantic Search and Knowledge Graphs";
    let abstract_text = "This article explores the intersection of semantic search and knowledge graphs...";
    let content = "Full article content about semantic search, RDF, SPARQL, and knowledge representation...";

    store.insert(&Quad::new(
        article.clone(),
        title_pred,
        Literal::new_simple_literal(title),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        article.clone(),
        abstract_pred,
        Literal::new_simple_literal(abstract_text),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        article.clone(),
        fulltext_pred,
        Literal::new_simple_literal(content),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        article.clone(),
        author_pred,
        author,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        article,
        category_pred,
        category,
        GraphName::DefaultGraph,
    ))?;

    // Index in Tantivy
    let schema = index_writer.index().schema();
    let id_field = schema.get_field("id").unwrap();
    let title_field = schema.get_field("title").unwrap();
    let abstract_field = schema.get_field("abstract").unwrap();
    let content_field = schema.get_field("content").unwrap();
    let author_field = schema.get_field("author").unwrap();
    let category_field = schema.get_field("category").unwrap();

    index_writer.add_document(doc!(
        id_field => format!("{}article/1", SEARCH_NS),
        title_field => title,
        abstract_field => abstract_text,
        content_field => content,
        author_field => "John Smith",
        category_field => "Technology"
    ))?;

    index_writer.commit()?;

    info!("Sample data loaded");
    Ok(())
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    info!("Search query: {}", params.q);

    // Parse and convert natural language query to SPARQL
    let sparql_query = generate_sparql_query(&params);

    // Execute full-text search
    let fulltext_results = execute_fulltext_search(&state, &params.q)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Execute SPARQL query
    let sparql_results = state.store.query(&sparql_query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Combine and rank results
    let mut results = combine_results(sparql_results, fulltext_results)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    results.truncate(params.limit);

    // Get facets
    let facets = get_facets_from_store(&state.store)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SearchResponse {
        total: results.len(),
        results,
        facets,
        query_info: QueryInfo {
            original_query: params.q.clone(),
            sparql_query: sparql_query.clone(),
            search_type: "hybrid".to_string(),
        },
    }))
}

fn generate_sparql_query(params: &SearchQuery) -> String {
    let search_term = params.q.to_lowercase();

    // Simple pattern matching for query intent
    let query_type = detect_query_type(&search_term);

    match query_type {
        QueryType::ByAuthor(author) => {
            format!(r#"
                PREFIX search: <{SEARCH_NS}>
                PREFIX schema: <http://schema.org/>

                SELECT ?article ?title ?abstract ?author ?category WHERE {{
                    ?article a search:Article ;
                             search:title ?title ;
                             search:abstract ?abstract ;
                             search:author ?authorNode .
                    ?authorNode schema:name ?author .
                    OPTIONAL {{ ?article search:category ?categoryNode .
                                ?categoryNode rdfs:label ?category }}
                    FILTER(CONTAINS(LCASE(?author), "{author}"))
                }}
                ORDER BY DESC(?article)
                LIMIT 100
            "#)
        }
        QueryType::ByCategory(category) => {
            format!(r#"
                PREFIX search: <{SEARCH_NS}>
                PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

                SELECT ?article ?title ?abstract ?category WHERE {{
                    ?article a search:Article ;
                             search:title ?title ;
                             search:abstract ?abstract ;
                             search:category ?categoryNode .
                    ?categoryNode rdfs:label ?category .
                    FILTER(CONTAINS(LCASE(?category), "{category}"))
                }}
                ORDER BY DESC(?article)
                LIMIT 100
            "#)
        }
        QueryType::General => {
            format!(r#"
                PREFIX search: <{SEARCH_NS}>
                PREFIX schema: <http://schema.org/>
                PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

                SELECT ?article ?title ?abstract ?author ?category WHERE {{
                    ?article a search:Article ;
                             search:title ?title ;
                             search:abstract ?abstract .
                    OPTIONAL {{ ?article search:author ?authorNode .
                                ?authorNode schema:name ?author }}
                    OPTIONAL {{ ?article search:category ?categoryNode .
                                ?categoryNode rdfs:label ?category }}
                    FILTER(
                        CONTAINS(LCASE(?title), "{search_term}") ||
                        CONTAINS(LCASE(?abstract), "{search_term}")
                    )
                }}
                ORDER BY DESC(?article)
                LIMIT 100
            "#)
        }
    }
}

enum QueryType {
    ByAuthor(String),
    ByCategory(String),
    General,
}

fn detect_query_type(query: &str) -> QueryType {
    let author_patterns = vec![
        Regex::new(r"by (\w+)").unwrap(),
        Regex::new(r"author:(\w+)").unwrap(),
        Regex::new(r"written by (\w+)").unwrap(),
    ];

    let category_patterns = vec![
        Regex::new(r"category:(\w+)").unwrap(),
        Regex::new(r"in (\w+)").unwrap(),
    ];

    for pattern in author_patterns {
        if let Some(captures) = pattern.captures(query) {
            if let Some(author) = captures.get(1) {
                return QueryType::ByAuthor(author.as_str().to_string());
            }
        }
    }

    for pattern in category_patterns {
        if let Some(captures) = pattern.captures(query) {
            if let Some(category) = captures.get(1) {
                return QueryType::ByCategory(category.as_str().to_string());
            }
        }
    }

    QueryType::General
}

fn execute_fulltext_search(state: &AppState, query: &str) -> Result<Vec<(String, f32)>> {
    let index = state.index.lock().unwrap();
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;

    let searcher = reader.searcher();
    let schema = index.schema();

    let title_field = schema.get_field("title").unwrap();
    let content_field = schema.get_field("content").unwrap();

    let query_parser = QueryParser::for_index(&index, vec![title_field, content_field]);
    let query = query_parser.parse_query(query)?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(100))?;

    let mut results = Vec::new();
    let id_field = schema.get_field("id").unwrap();

    for (score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address)?;
        if let Some(id_value) = retrieved_doc.get_first(id_field) {
            if let Some(id) = id_value.as_str() {
                results.push((id.to_string(), score));
            }
        }
    }

    Ok(results)
}

fn combine_results(
    sparql_results: QueryResults,
    fulltext_results: Vec<(String, f32)>,
) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let mut fulltext_scores: std::collections::HashMap<String, f32> =
        fulltext_results.into_iter().collect();

    if let QueryResults::Solutions(solutions) = sparql_results {
        for solution in solutions {
            let solution = solution?;

            let id = solution
                .get("article")
                .and_then(|t| t.as_ref().as_named_node())
                .map(|n| n.as_str().to_string())
                .unwrap_or_default();

            let score = fulltext_scores.remove(&id).unwrap_or(0.5);

            results.push(SearchResult {
                id: id.clone(),
                title: solution
                    .get("title")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                abstract_text: solution
                    .get("abstract")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                score,
                author: solution
                    .get("author")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
                category: solution
                    .get("category")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
                published_date: None,
                tags: Vec::new(),
            });
        }
    }

    // Sort by score
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    Ok(results)
}

fn get_facets_from_store(store: &Store) -> Result<Facets> {
    // Get category facets
    let category_query = format!(r#"
        PREFIX search: <{SEARCH_NS}>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?category (COUNT(?article) AS ?count) WHERE {{
            ?article search:category ?categoryNode .
            ?categoryNode rdfs:label ?category .
        }}
        GROUP BY ?category
        ORDER BY DESC(?count)
    "#);

    let mut categories = Vec::new();
    if let QueryResults::Solutions(solutions) = store.query(&category_query)? {
        for solution in solutions {
            let solution = solution?;
            if let (Some(name), Some(count)) = (
                solution.get("category").and_then(|t| t.as_ref().as_literal()).map(|l| l.value().to_string()),
                solution.get("count").and_then(|t| t.as_ref().as_literal()).and_then(|l| l.value().parse::<usize>().ok()),
            ) {
                categories.push(FacetItem { name, count });
            }
        }
    }

    // Get author facets
    let author_query = format!(r#"
        PREFIX search: <{SEARCH_NS}>
        PREFIX schema: <http://schema.org/>

        SELECT ?author (COUNT(?article) AS ?count) WHERE {{
            ?article search:author ?authorNode .
            ?authorNode schema:name ?author .
        }}
        GROUP BY ?author
        ORDER BY DESC(?count)
    "#);

    let mut authors = Vec::new();
    if let QueryResults::Solutions(solutions) = store.query(&author_query)? {
        for solution in solutions {
            let solution = solution?;
            if let (Some(name), Some(count)) = (
                solution.get("author").and_then(|t| t.as_ref().as_literal()).map(|l| l.value().to_string()),
                solution.get("count").and_then(|t| t.as_ref().as_literal()).and_then(|l| l.value().parse::<usize>().ok()),
            ) {
                authors.push(FacetItem { name, count });
            }
        }
    }

    Ok(Facets {
        categories,
        authors,
        tags: Vec::new(),
        date_ranges: Vec::new(),
    })
}

async fn suggest(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let prefix = params.q.to_lowercase();

    let query = format!(r#"
        PREFIX search: <{SEARCH_NS}>

        SELECT DISTINCT ?title WHERE {{
            ?article a search:Article ;
                     search:title ?title .
            FILTER(CONTAINS(LCASE(?title), "{prefix}"))
        }}
        LIMIT 10
    "#);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut suggestions = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if let Some(title) = solution.get("title")
                .and_then(|t| t.as_ref().as_literal())
                .map(|l| l.value().to_string())
            {
                suggestions.push(title);
            }
        }
    }

    Ok(Json(suggestions))
}

async fn get_facets(State(state): State<AppState>) -> Result<Json<Facets>, StatusCode> {
    let facets = get_facets_from_store(&state.store)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(facets))
}

async fn find_similar(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<SearchResult>>, StatusCode> {
    // Find similar articles based on shared tags, categories, or authors
    let query = format!(r#"
        PREFIX search: <{SEARCH_NS}>

        SELECT DISTINCT ?similar ?title ?abstract ?score WHERE {{
            <{SEARCH_NS}article/{id}> search:category ?cat .
            ?similar a search:Article ;
                     search:category ?cat ;
                     search:title ?title ;
                     search:abstract ?abstract .
            FILTER(?similar != <{SEARCH_NS}article/{id}>)
            BIND(1.0 AS ?score)
        }}
        LIMIT 5
    "#);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut similar = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            similar.push(SearchResult {
                id: solution.get("similar")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
                title: solution.get("title")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                abstract_text: solution.get("abstract")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                score: 1.0,
                author: None,
                category: None,
                published_date: None,
                tags: Vec::new(),
            });
        }
    }

    Ok(Json(similar))
}

#[derive(Deserialize)]
struct IndexRequest {
    id: String,
    title: String,
    abstract_text: String,
    content: String,
    author: Option<String>,
    category: Option<String>,
}

async fn index_document(
    State(state): State<AppState>,
    Json(req): Json<IndexRequest>,
) -> Result<StatusCode, StatusCode> {
    let article = NamedNode::new(format!("{SEARCH_NS}article/{}", req.id))
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let article_class = NamedNode::new(format!("{}Article", SEARCH_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let title_pred = NamedNode::new(format!("{}title", SEARCH_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let abstract_pred = NamedNode::new(format!("{}abstract", SEARCH_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let fulltext_pred = NamedNode::new(format!("{}fullText", SEARCH_NS))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Add to RDF store
    state.store.insert(&Quad::new(
        article.clone(),
        rdf_type,
        article_class,
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        article.clone(),
        title_pred,
        Literal::new_simple_literal(&req.title),
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        article.clone(),
        abstract_pred,
        Literal::new_simple_literal(&req.abstract_text),
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        article,
        fulltext_pred,
        Literal::new_simple_literal(&req.content),
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Add to full-text index
    let mut index_writer = state.index_writer.lock().unwrap();
    let schema = index_writer.index().schema();

    let id_field = schema.get_field("id").unwrap();
    let title_field = schema.get_field("title").unwrap();
    let abstract_field = schema.get_field("abstract").unwrap();
    let content_field = schema.get_field("content").unwrap();

    index_writer.add_document(doc!(
        id_field => format!("{SEARCH_NS}article/{}", req.id),
        title_field => req.title,
        abstract_field => req.abstract_text,
        content_field => req.content
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    index_writer.commit()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}
```

### Python Implementation

#### requirements.txt

```txt
pyoxigraph>=0.4.0
flask>=3.0.0
flask-cors>=4.0.0
whoosh>=2.7.4
nltk>=3.8
```

#### search_server.py

```python
from flask import Flask, request, jsonify
from flask_cors import CORS
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph, RdfFormat
from whoosh.index import create_in
from whoosh.fields import Schema, TEXT, ID, STORED
from whoosh.qparser import QueryParser, MultifieldParser
from whoosh import scoring
import os
import re
from collections import defaultdict

app = Flask(__name__)
CORS(app)

SEARCH_NS = "http://example.org/search/"
store = Store()

# Create Whoosh schema
whoosh_schema = Schema(
    id=ID(stored=True),
    title=TEXT(stored=True),
    abstract=TEXT(stored=True),
    content=TEXT,
    author=TEXT(stored=True),
    category=TEXT(stored=True)
)

# Create index
if not os.path.exists("indexdir"):
    os.mkdir("indexdir")

ix = create_in("indexdir", whoosh_schema)

def load_schema():
    """Load search ontology"""
    schema = """
    @prefix search: <http://example.org/search/> .
    @prefix schema: <http://schema.org/> .
    @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
    @prefix owl: <http://www.w3.org/2002/07/owl#> .

    search:Article a owl:Class .
    search:Author a owl:Class .
    search:Category a owl:Class .

    search:title a owl:DatatypeProperty .
    search:abstract a owl:DatatypeProperty .
    search:fullText a owl:DatatypeProperty .
    search:author a owl:ObjectProperty .
    search:category a owl:ObjectProperty .
    """
    store.load(input=schema.encode(), format=RdfFormat.TURTLE)
    print("Schema loaded")

def load_sample_data():
    """Load sample articles"""
    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")

    # Create author
    author = NamedNode(f"{SEARCH_NS}author/john-smith")
    author_class = NamedNode(f"{SEARCH_NS}Author")
    name_pred = NamedNode("http://schema.org/name")

    store.add(Quad(author, rdf_type, author_class, DefaultGraph()))
    store.add(Quad(author, name_pred, Literal("John Smith"), DefaultGraph()))

    # Create category
    category = NamedNode(f"{SEARCH_NS}category/technology")
    category_class = NamedNode(f"{SEARCH_NS}Category")
    label_pred = NamedNode("http://www.w3.org/2000/01/rdf-schema#label")

    store.add(Quad(category, rdf_type, category_class, DefaultGraph()))
    store.add(Quad(category, label_pred, Literal("Technology"), DefaultGraph()))

    # Create article
    article = NamedNode(f"{SEARCH_NS}article/1")
    article_class = NamedNode(f"{SEARCH_NS}Article")
    title_pred = NamedNode(f"{SEARCH_NS}title")
    abstract_pred = NamedNode(f"{SEARCH_NS}abstract")
    fulltext_pred = NamedNode(f"{SEARCH_NS}fullText")
    author_pred = NamedNode(f"{SEARCH_NS}author")
    category_pred = NamedNode(f"{SEARCH_NS}category")

    title = "Introduction to Semantic Search and Knowledge Graphs"
    abstract = "This article explores the intersection of semantic search and knowledge graphs..."
    content = "Full article content about semantic search, RDF, SPARQL, and knowledge representation..."

    store.add(Quad(article, rdf_type, article_class, DefaultGraph()))
    store.add(Quad(article, title_pred, Literal(title), DefaultGraph()))
    store.add(Quad(article, abstract_pred, Literal(abstract), DefaultGraph()))
    store.add(Quad(article, fulltext_pred, Literal(content), DefaultGraph()))
    store.add(Quad(article, author_pred, author, DefaultGraph()))
    store.add(Quad(article, category_pred, category, DefaultGraph()))

    # Index in Whoosh
    writer = ix.writer()
    writer.add_document(
        id=f"{SEARCH_NS}article/1",
        title=title,
        abstract=abstract,
        content=content,
        author="John Smith",
        category="Technology"
    )
    writer.commit()

    print("Sample data loaded")

def detect_query_intent(query):
    """Detect the type of query"""
    query_lower = query.lower()

    # Author query
    author_match = re.search(r'by (\w+)|author:(\w+)|written by (\w+)', query_lower)
    if author_match:
        author = next(g for g in author_match.groups() if g)
        return 'author', author

    # Category query
    category_match = re.search(r'category:(\w+)|in (\w+)', query_lower)
    if category_match:
        category = next(g for g in category_match.groups() if g)
        return 'category', category

    return 'general', query

def generate_sparql_query(query_text):
    """Generate SPARQL query from natural language"""
    query_type, param = detect_query_intent(query_text)

    if query_type == 'author':
        return f"""
            PREFIX search: <{SEARCH_NS}>
            PREFIX schema: <http://schema.org/>

            SELECT ?article ?title ?abstract ?author ?category WHERE {{
                ?article a search:Article ;
                         search:title ?title ;
                         search:abstract ?abstract ;
                         search:author ?authorNode .
                ?authorNode schema:name ?author .
                OPTIONAL {{ ?article search:category ?categoryNode .
                            ?categoryNode rdfs:label ?category }}
                FILTER(CONTAINS(LCASE(?author), "{param}"))
            }}
            LIMIT 100
        """
    elif query_type == 'category':
        return f"""
            PREFIX search: <{SEARCH_NS}>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT ?article ?title ?abstract ?category WHERE {{
                ?article a search:Article ;
                         search:title ?title ;
                         search:abstract ?abstract ;
                         search:category ?categoryNode .
                ?categoryNode rdfs:label ?category .
                FILTER(CONTAINS(LCASE(?category), "{param}"))
            }}
            LIMIT 100
        """
    else:
        return f"""
            PREFIX search: <{SEARCH_NS}>
            PREFIX schema: <http://schema.org/>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT ?article ?title ?abstract ?author ?category WHERE {{
                ?article a search:Article ;
                         search:title ?title ;
                         search:abstract ?abstract .
                OPTIONAL {{ ?article search:author ?authorNode .
                            ?authorNode schema:name ?author }}
                OPTIONAL {{ ?article search:category ?categoryNode .
                            ?categoryNode rdfs:label ?category }}
                FILTER(
                    CONTAINS(LCASE(?title), "{param}") ||
                    CONTAINS(LCASE(?abstract), "{param}")
                )
            }}
            LIMIT 100
        """

@app.route('/search', methods=['GET'])
def search():
    """Hybrid search endpoint"""
    query_text = request.args.get('q', '')
    limit = int(request.args.get('limit', 10))

    # Generate SPARQL query
    sparql_query = generate_sparql_query(query_text)

    # Execute SPARQL
    sparql_results = list(store.query(sparql_query))

    # Execute full-text search
    with ix.searcher(weighting=scoring.BM25F()) as searcher:
        parser = MultifieldParser(["title", "content"], schema=ix.schema)
        query = parser.parse(query_text)
        fulltext_results = searcher.search(query, limit=100)

        # Build score map
        scores = {hit['id']: hit.score for hit in fulltext_results}

    # Combine results
    combined = []
    for row in sparql_results:
        article_id = str(row['article'])
        score = scores.get(article_id, 0.5)

        combined.append({
            'id': article_id,
            'title': str(row['title']),
            'abstract': str(row['abstract']),
            'author': str(row['author']) if row.get('author') else None,
            'category': str(row['category']) if row.get('category') else None,
            'score': float(score)
        })

    # Sort by score and limit
    combined.sort(key=lambda x: x['score'], reverse=True)
    combined = combined[:limit]

    # Get facets
    facets = get_facets()

    return jsonify({
        'results': combined,
        'total': len(combined),
        'facets': facets,
        'query_info': {
            'original_query': query_text,
            'sparql_query': sparql_query,
            'search_type': 'hybrid'
        }
    })

def get_facets():
    """Get facets for filtering"""
    # Category facets
    category_query = f"""
        PREFIX search: <{SEARCH_NS}>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?category (COUNT(?article) AS ?count) WHERE {{
            ?article search:category ?categoryNode .
            ?categoryNode rdfs:label ?category .
        }}
        GROUP BY ?category
        ORDER BY DESC(?count)
    """

    categories = []
    for row in store.query(category_query):
        categories.append({
            'name': str(row['category']),
            'count': int(str(row['count']))
        })

    # Author facets
    author_query = f"""
        PREFIX search: <{SEARCH_NS}>
        PREFIX schema: <http://schema.org/>

        SELECT ?author (COUNT(?article) AS ?count) WHERE {{
            ?article search:author ?authorNode .
            ?authorNode schema:name ?author .
        }}
        GROUP BY ?author
        ORDER BY DESC(?count)
    """

    authors = []
    for row in store.query(author_query):
        authors.append({
            'name': str(row['author']),
            'count': int(str(row['count']))
        })

    return {
        'categories': categories,
        'authors': authors,
        'tags': [],
        'date_ranges': []
    }

@app.route('/suggest', methods=['GET'])
def suggest():
    """Autocomplete suggestions"""
    prefix = request.args.get('q', '').lower()

    query = f"""
        PREFIX search: <{SEARCH_NS}>

        SELECT DISTINCT ?title WHERE {{
            ?article a search:Article ;
                     search:title ?title .
            FILTER(CONTAINS(LCASE(?title), "{prefix}"))
        }}
        LIMIT 10
    """

    suggestions = []
    for row in store.query(query):
        suggestions.append(str(row['title']))

    return jsonify(suggestions)

@app.route('/similar/<article_id>', methods=['GET'])
def find_similar(article_id):
    """Find similar articles"""
    query = f"""
        PREFIX search: <{SEARCH_NS}>

        SELECT DISTINCT ?similar ?title ?abstract WHERE {{
            <{SEARCH_NS}article/{article_id}> search:category ?cat .
            ?similar a search:Article ;
                     search:category ?cat ;
                     search:title ?title ;
                     search:abstract ?abstract .
            FILTER(?similar != <{SEARCH_NS}article/{article_id}>)
        }}
        LIMIT 5
    """

    similar = []
    for row in store.query(query):
        similar.append({
            'id': str(row['similar']),
            'title': str(row['title']),
            'abstract': str(row['abstract']),
            'score': 1.0
        })

    return jsonify(similar)

@app.route('/index', methods=['POST'])
def index_document():
    """Index a new document"""
    data = request.json
    article_id = data['id']
    article = NamedNode(f"{SEARCH_NS}article/{article_id}")

    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    article_class = NamedNode(f"{SEARCH_NS}Article")
    title_pred = NamedNode(f"{SEARCH_NS}title")
    abstract_pred = NamedNode(f"{SEARCH_NS}abstract")
    fulltext_pred = NamedNode(f"{SEARCH_NS}fullText")

    # Add to RDF store
    store.add(Quad(article, rdf_type, article_class, DefaultGraph()))
    store.add(Quad(article, title_pred, Literal(data['title']), DefaultGraph()))
    store.add(Quad(article, abstract_pred, Literal(data['abstract']), DefaultGraph()))
    store.add(Quad(article, fulltext_pred, Literal(data['content']), DefaultGraph()))

    # Add to full-text index
    writer = ix.writer()
    writer.add_document(
        id=f"{SEARCH_NS}article/{article_id}",
        title=data['title'],
        abstract=data['abstract'],
        content=data['content'],
        author=data.get('author', ''),
        category=data.get('category', '')
    )
    writer.commit()

    return '', 201

if __name__ == '__main__':
    load_schema()
    load_sample_data()
    app.run(debug=True, port=3000)
```

## Setup and Usage

### Rust

```bash
cargo build --release
cargo run --release
```

### Python

```bash
pip install -r requirements.txt
python search_server.py
```

## API Examples

### Basic Search

```bash
curl "http://localhost:3000/search?q=semantic+search&limit=10"
```

### Author Search

```bash
curl "http://localhost:3000/search?q=by+john"
```

### Category Search

```bash
curl "http://localhost:3000/search?q=category:technology"
```

### Autocomplete

```bash
curl "http://localhost:3000/suggest?q=intro"
```

### Similar Articles

```bash
curl "http://localhost:3000/similar/1"
```

## Features

1. **Text-to-SPARQL**: Natural language query parsing
2. **Hybrid Search**: Combines structured SPARQL with full-text search
3. **Result Ranking**: BM25 scoring with semantic relevance
4. **Faceted Search**: Dynamic facets for categories, authors, tags
5. **Autocomplete**: Real-time search suggestions
6. **Similar Items**: Content-based recommendations
7. **Multi-field Search**: Search across title, abstract, and content

## Performance Tips

1. Use full-text index for large content
2. Cache frequent queries
3. Pre-compute facets
4. Index asynchronously
5. Use query pagination
