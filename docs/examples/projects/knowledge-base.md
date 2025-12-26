# Knowledge Base Application

A complete implementation of a knowledge management system using Oxigraph, with RDFS/OWL ontology, data loading pipeline, and multi-language API.

## Architecture Overview

```
┌─────────────────┐      ┌──────────────────┐      ┌─────────────────┐
│  Data Sources   │─────▶│  ETL Pipeline    │─────▶│   Oxigraph      │
│  (CSV/JSON/RDF) │      │  (Validation)    │      │   Store         │
└─────────────────┘      └──────────────────┘      └────────┬────────┘
                                                             │
                         ┌───────────────────────────────────┤
                         │                                   │
                    ┌────▼─────┐                      ┌─────▼──────┐
                    │ REST API │                      │ SPARQL API │
                    └──────────┘                      └────────────┘
```

## Schema Design

### Ontology (schema.ttl)

```turtle
@prefix kb: <http://example.org/kb/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# Core Classes
kb:Document a owl:Class ;
    rdfs:label "Document" ;
    rdfs:comment "A document in the knowledge base" .

kb:Person a owl:Class ;
    rdfs:label "Person" ;
    rdfs:comment "An individual person" .

kb:Organization a owl:Class ;
    rdfs:label "Organization" ;
    rdfs:comment "An organization or company" .

kb:Topic a owl:Class ;
    rdfs:label "Topic" ;
    rdfs:comment "A subject or topic" .

kb:Project a owl:Class ;
    rdfs:label "Project" ;
    rdfs:comment "A project or initiative" .

# Properties
kb:title a owl:DatatypeProperty ;
    rdfs:domain kb:Document ;
    rdfs:range xsd:string ;
    rdfs:label "title" .

kb:content a owl:DatatypeProperty ;
    rdfs:domain kb:Document ;
    rdfs:range xsd:string ;
    rdfs:label "content" .

kb:created a owl:DatatypeProperty ;
    rdfs:range xsd:dateTime ;
    rdfs:label "created" .

kb:modified a owl:DatatypeProperty ;
    rdfs:range xsd:dateTime ;
    rdfs:label "modified" .

kb:author a owl:ObjectProperty ;
    rdfs:domain kb:Document ;
    rdfs:range kb:Person ;
    rdfs:label "author" .

kb:mentions a owl:ObjectProperty ;
    rdfs:domain kb:Document ;
    rdfs:label "mentions" .

kb:relatedTo a owl:ObjectProperty ;
    owl:inverseOf kb:relatedTo ;
    rdfs:label "related to" .

kb:worksFor a owl:ObjectProperty ;
    rdfs:domain kb:Person ;
    rdfs:range kb:Organization ;
    rdfs:label "works for" .

kb:participatesIn a owl:ObjectProperty ;
    rdfs:domain kb:Person ;
    rdfs:range kb:Project ;
    rdfs:label "participates in" .

kb:tag a owl:ObjectProperty ;
    rdfs:range kb:Topic ;
    rdfs:label "tag" .

kb:email a owl:DatatypeProperty ;
    rdfs:domain kb:Person ;
    rdfs:range xsd:string ;
    rdfs:label "email" .

kb:name a owl:DatatypeProperty ;
    rdfs:range xsd:string ;
    rdfs:label "name" .
```

## Implementation

### Rust Implementation

#### Cargo.toml

```toml
[package]
name = "knowledge-base"
version = "0.1.0"
edition = "2021"

[dependencies]
oxigraph = "0.4"
anyhow = "1.0"
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tower-http = { version = "0.5", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = "0.4"
csv = "1.3"
```

#### src/main.rs

```rust
use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
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

const KB_NS: &str = "http://example.org/kb/";

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize store
    let store = Store::new()?;

    // Load schema
    load_schema(&store)?;

    // Load initial data
    load_sample_data(&store)?;

    let state = AppState {
        store: Arc::new(store),
    };

    // Build router
    let app = Router::new()
        .route("/documents", get(list_documents).post(create_document))
        .route("/documents/:id", get(get_document).delete(delete_document))
        .route("/search", get(search))
        .route("/sparql", post(sparql_query))
        .route("/persons", get(list_persons))
        .route("/graph", get(get_graph))
        .layer(CorsLayer::permissive())
        .with_state(state);

    info!("Starting server on http://localhost:3000");
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

fn load_sample_data(store: &Store) -> Result<()> {
    // Create sample person
    let alice = NamedNode::new(format!("{}person/alice", KB_NS))?;
    let name_pred = NamedNode::new(format!("{}name", KB_NS))?;
    let email_pred = NamedNode::new(format!("{}email", KB_NS))?;
    let person_class = NamedNode::new(format!("{}Person", KB_NS))?;
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;

    store.insert(&Quad::new(
        alice.clone(),
        rdf_type.clone(),
        person_class,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        alice.clone(),
        name_pred,
        Literal::new_simple_literal("Alice Smith"),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        alice.clone(),
        email_pred,
        Literal::new_simple_literal("alice@example.com"),
        GraphName::DefaultGraph,
    ))?;

    // Create sample document
    let doc = NamedNode::new(format!("{}document/1", KB_NS))?;
    let doc_class = NamedNode::new(format!("{}Document", KB_NS))?;
    let title_pred = NamedNode::new(format!("{}title", KB_NS))?;
    let content_pred = NamedNode::new(format!("{}content", KB_NS))?;
    let author_pred = NamedNode::new(format!("{}author", KB_NS))?;

    store.insert(&Quad::new(
        doc.clone(),
        rdf_type,
        doc_class,
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        doc.clone(),
        title_pred,
        Literal::new_simple_literal("Introduction to Knowledge Graphs"),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        doc.clone(),
        content_pred,
        Literal::new_simple_literal("Knowledge graphs represent information as interconnected entities..."),
        GraphName::DefaultGraph,
    ))?;

    store.insert(&Quad::new(
        doc,
        author_pred,
        alice,
        GraphName::DefaultGraph,
    ))?;

    info!("Sample data loaded");
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Document {
    id: String,
    title: String,
    content: String,
    author: Option<String>,
    created: Option<String>,
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize { 10 }

async fn list_documents(State(state): State<AppState>) -> Result<Json<Vec<Document>>, StatusCode> {
    let query = format!(r#"
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author ?created WHERE {{
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL {{ ?doc kb:author ?author }}
            OPTIONAL {{ ?doc kb:created ?created }}
        }}
        ORDER BY DESC(?created)
        LIMIT 100
    "#);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut documents = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            documents.push(Document {
                id: solution.get("doc")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
                title: solution.get("title")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                content: solution.get("content")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                author: solution.get("author")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string()),
                created: solution.get("created")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
                tags: Vec::new(),
            });
        }
    }

    Ok(Json(documents))
}

async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Document>, StatusCode> {
    let doc_iri = format!("{KB_NS}document/{id}");

    let query = format!(r#"
        PREFIX kb: <{KB_NS}>

        SELECT ?title ?content ?author ?created WHERE {{
            <{doc_iri}> kb:title ?title ;
                        kb:content ?content .
            OPTIONAL {{ <{doc_iri}> kb:author ?author }}
            OPTIONAL {{ <{doc_iri}> kb:created ?created }}
        }}
    "#);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let QueryResults::Solutions(mut solutions) = results {
        if let Some(Ok(solution)) = solutions.next() {
            return Ok(Json(Document {
                id: doc_iri,
                title: solution.get("title")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                content: solution.get("content")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                author: solution.get("author")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string()),
                created: solution.get("created")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
                tags: Vec::new(),
            }));
        }
    }

    Err(StatusCode::NOT_FOUND)
}

#[derive(Deserialize)]
struct CreateDocumentRequest {
    title: String,
    content: String,
    author_id: Option<String>,
    tags: Option<Vec<String>>,
}

async fn create_document(
    State(state): State<AppState>,
    Json(req): Json<CreateDocumentRequest>,
) -> Result<Json<Document>, StatusCode> {
    let doc_id = uuid::Uuid::new_v4().to_string();
    let doc_iri = NamedNode::new(format!("{KB_NS}document/{doc_id}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let doc_class = NamedNode::new(format!("{KB_NS}Document"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let title_pred = NamedNode::new(format!("{KB_NS}title"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let content_pred = NamedNode::new(format!("{KB_NS}content"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        doc_iri.clone(),
        rdf_type,
        doc_class,
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        doc_iri.clone(),
        title_pred,
        Literal::new_simple_literal(&req.title),
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.store.insert(&Quad::new(
        doc_iri.clone(),
        content_pred,
        Literal::new_simple_literal(&req.content),
        GraphName::DefaultGraph,
    )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(author_id) = req.author_id {
        let author_pred = NamedNode::new(format!("{KB_NS}author"))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let author = NamedNode::new(&author_id)
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        state.store.insert(&Quad::new(
            doc_iri.clone(),
            author_pred,
            author,
            GraphName::DefaultGraph,
        )).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(Document {
        id: doc_iri.as_str().to_string(),
        title: req.title,
        content: req.content,
        author: req.author_id,
        created: None,
        tags: req.tags.unwrap_or_default(),
    }))
}

async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let doc_iri = NamedNode::new(format!("{KB_NS}document/{id}"))
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Delete all triples with this document as subject
    let quads_to_delete: Vec<_> = state.store
        .quads_for_pattern(Some(doc_iri.as_ref()), None, None, None)
        .collect();

    for quad in quads_to_delete {
        let quad = quad.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state.store.remove(&quad)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<Document>>, StatusCode> {
    let search_term = params.q.to_lowercase();

    let query = format!(r#"
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author WHERE {{
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL {{ ?doc kb:author ?author }}
            FILTER(
                CONTAINS(LCASE(?title), "{search_term}") ||
                CONTAINS(LCASE(?content), "{search_term}")
            )
        }}
        LIMIT {}
    "#, params.limit);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut documents = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            documents.push(Document {
                id: solution.get("doc")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
                title: solution.get("title")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                content: solution.get("content")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string())
                    .unwrap_or_default(),
                author: solution.get("author")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string()),
                created: None,
                tags: Vec::new(),
            });
        }
    }

    Ok(Json(documents))
}

#[derive(Deserialize)]
struct SparqlRequest {
    query: String,
}

async fn sparql_query(
    State(state): State<AppState>,
    Json(req): Json<SparqlRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let results = state.store.query(&req.query)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    match results {
        QueryResults::Solutions(solutions) => {
            let mut bindings = Vec::new();
            for solution in solutions {
                let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let mut binding = serde_json::Map::new();

                for (var, term) in solution.iter() {
                    binding.insert(
                        var.as_str().to_string(),
                        serde_json::Value::String(term.to_string()),
                    );
                }
                bindings.push(serde_json::Value::Object(binding));
            }

            Ok(Json(serde_json::json!({
                "results": { "bindings": bindings }
            })))
        }
        QueryResults::Boolean(b) => {
            Ok(Json(serde_json::json!({ "boolean": b })))
        }
        QueryResults::Graph(_) => {
            Ok(Json(serde_json::json!({ "type": "graph" })))
        }
    }
}

async fn list_persons(State(state): State<AppState>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let query = format!(r#"
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?person ?name ?email WHERE {{
            ?person rdf:type kb:Person .
            OPTIONAL {{ ?person kb:name ?name }}
            OPTIONAL {{ ?person kb:email ?email }}
        }}
    "#);

    let results = state.store.query(&query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut persons = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            persons.push(serde_json::json!({
                "id": solution.get("person")
                    .and_then(|t| t.as_ref().as_named_node())
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
                "name": solution.get("name")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
                "email": solution.get("email")
                    .and_then(|t| t.as_ref().as_literal())
                    .map(|l| l.value().to_string()),
            }));
        }
    }

    Ok(Json(persons))
}

async fn get_graph(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let query = r#"
        SELECT ?s ?p ?o WHERE {
            ?s ?p ?o
        }
        LIMIT 1000
    "#;

    let results = state.store.query(query)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut triples = Vec::new();

    if let QueryResults::Solutions(solutions) = results {
        for solution in solutions {
            let solution = solution.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            triples.push(serde_json::json!({
                "subject": solution.get("s").map(|t| t.to_string()).unwrap_or_default(),
                "predicate": solution.get("p").map(|t| t.to_string()).unwrap_or_default(),
                "object": solution.get("o").map(|t| t.to_string()).unwrap_or_default(),
            }));
        }
    }

    Ok(Json(serde_json::json!({ "triples": triples })))
}
```

### Python Implementation

#### requirements.txt

```txt
pyoxigraph>=0.4.0
flask>=3.0.0
flask-cors>=4.0.0
pandas>=2.0.0
rdflib>=7.0.0
```

#### app.py

```python
from flask import Flask, request, jsonify
from flask_cors import CORS
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph, RdfFormat
import uuid
from datetime import datetime

app = Flask(__name__)
CORS(app)

KB_NS = "http://example.org/kb/"
store = Store()

def load_schema():
    """Load RDFS/OWL schema"""
    schema = """
    @prefix kb: <http://example.org/kb/> .
    @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
    @prefix owl: <http://www.w3.org/2002/07/owl#> .
    @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

    kb:Document a owl:Class ;
        rdfs:label "Document" .

    kb:Person a owl:Class ;
        rdfs:label "Person" .

    kb:title a owl:DatatypeProperty ;
        rdfs:domain kb:Document ;
        rdfs:range xsd:string .

    kb:content a owl:DatatypeProperty ;
        rdfs:domain kb:Document ;
        rdfs:range xsd:string .

    kb:author a owl:ObjectProperty ;
        rdfs:domain kb:Document ;
        rdfs:range kb:Person .

    kb:name a owl:DatatypeProperty ;
        rdfs:range xsd:string .

    kb:email a owl:DatatypeProperty ;
        rdfs:range xsd:string .
    """

    store.load(input=schema.encode(), format=RdfFormat.TURTLE)
    print("Schema loaded")

def load_sample_data():
    """Load sample data"""
    # Create person
    alice = NamedNode(f"{KB_NS}person/alice")
    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    person_class = NamedNode(f"{KB_NS}Person")
    name_pred = NamedNode(f"{KB_NS}name")
    email_pred = NamedNode(f"{KB_NS}email")

    store.add(Quad(alice, rdf_type, person_class, DefaultGraph()))
    store.add(Quad(alice, name_pred, Literal("Alice Smith"), DefaultGraph()))
    store.add(Quad(alice, email_pred, Literal("alice@example.com"), DefaultGraph()))

    # Create document
    doc = NamedNode(f"{KB_NS}document/1")
    doc_class = NamedNode(f"{KB_NS}Document")
    title_pred = NamedNode(f"{KB_NS}title")
    content_pred = NamedNode(f"{KB_NS}content")
    author_pred = NamedNode(f"{KB_NS}author")

    store.add(Quad(doc, rdf_type, doc_class, DefaultGraph()))
    store.add(Quad(doc, title_pred, Literal("Introduction to Knowledge Graphs"), DefaultGraph()))
    store.add(Quad(doc, content_pred,
                  Literal("Knowledge graphs represent information as interconnected entities..."),
                  DefaultGraph()))
    store.add(Quad(doc, author_pred, alice, DefaultGraph()))

    print("Sample data loaded")

@app.route('/documents', methods=['GET'])
def list_documents():
    """List all documents"""
    query = f"""
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author WHERE {{
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL {{ ?doc kb:author ?author }}
        }}
        ORDER BY ?title
        LIMIT 100
    """

    results = store.query(query)
    documents = []

    for row in results:
        documents.append({
            'id': str(row['doc']),
            'title': str(row['title']),
            'content': str(row['content']),
            'author': str(row['author']) if row.get('author') else None
        })

    return jsonify(documents)

@app.route('/documents/<doc_id>', methods=['GET'])
def get_document(doc_id):
    """Get a specific document"""
    doc_iri = f"{KB_NS}document/{doc_id}"

    query = f"""
        PREFIX kb: <{KB_NS}>

        SELECT ?title ?content ?author WHERE {{
            <{doc_iri}> kb:title ?title ;
                        kb:content ?content .
            OPTIONAL {{ <{doc_iri}> kb:author ?author }}
        }}
    """

    results = list(store.query(query))

    if not results:
        return jsonify({'error': 'Document not found'}), 404

    row = results[0]
    return jsonify({
        'id': doc_iri,
        'title': str(row['title']),
        'content': str(row['content']),
        'author': str(row['author']) if row.get('author') else None
    })

@app.route('/documents', methods=['POST'])
def create_document():
    """Create a new document"""
    data = request.json
    doc_id = str(uuid.uuid4())
    doc_iri = NamedNode(f"{KB_NS}document/{doc_id}")

    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    doc_class = NamedNode(f"{KB_NS}Document")
    title_pred = NamedNode(f"{KB_NS}title")
    content_pred = NamedNode(f"{KB_NS}content")

    store.add(Quad(doc_iri, rdf_type, doc_class, DefaultGraph()))
    store.add(Quad(doc_iri, title_pred, Literal(data['title']), DefaultGraph()))
    store.add(Quad(doc_iri, content_pred, Literal(data['content']), DefaultGraph()))

    if 'author_id' in data:
        author_pred = NamedNode(f"{KB_NS}author")
        author = NamedNode(data['author_id'])
        store.add(Quad(doc_iri, author_pred, author, DefaultGraph()))

    return jsonify({
        'id': str(doc_iri),
        'title': data['title'],
        'content': data['content'],
        'author': data.get('author_id')
    }), 201

@app.route('/documents/<doc_id>', methods=['DELETE'])
def delete_document(doc_id):
    """Delete a document"""
    doc_iri = NamedNode(f"{KB_NS}document/{doc_id}")

    # Remove all quads with this document as subject
    for quad in store.quads_for_pattern(doc_iri, None, None, None):
        store.remove(quad)

    return '', 204

@app.route('/search', methods=['GET'])
def search():
    """Search documents"""
    q = request.args.get('q', '').lower()
    limit = int(request.args.get('limit', 10))

    query = f"""
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author WHERE {{
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL {{ ?doc kb:author ?author }}
            FILTER(
                CONTAINS(LCASE(?title), "{q}") ||
                CONTAINS(LCASE(?content), "{q}")
            )
        }}
        LIMIT {limit}
    """

    results = store.query(query)
    documents = []

    for row in results:
        documents.append({
            'id': str(row['doc']),
            'title': str(row['title']),
            'content': str(row['content']),
            'author': str(row['author']) if row.get('author') else None
        })

    return jsonify(documents)

@app.route('/sparql', methods=['POST'])
def sparql_query():
    """Execute SPARQL query"""
    data = request.json
    query = data.get('query')

    if not query:
        return jsonify({'error': 'No query provided'}), 400

    try:
        results = store.query(query)

        # Handle different result types
        if hasattr(results, '__iter__'):
            bindings = []
            for row in results:
                binding = {}
                for var in row:
                    binding[var] = str(row[var])
                bindings.append(binding)
            return jsonify({'results': {'bindings': bindings}})
        else:
            return jsonify({'boolean': bool(results)})
    except Exception as e:
        return jsonify({'error': str(e)}), 400

@app.route('/persons', methods=['GET'])
def list_persons():
    """List all persons"""
    query = f"""
        PREFIX kb: <{KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?person ?name ?email WHERE {{
            ?person rdf:type kb:Person .
            OPTIONAL {{ ?person kb:name ?name }}
            OPTIONAL {{ ?person kb:email ?email }}
        }}
    """

    results = store.query(query)
    persons = []

    for row in results:
        persons.append({
            'id': str(row['person']),
            'name': str(row['name']) if row.get('name') else None,
            'email': str(row['email']) if row.get('email') else None
        })

    return jsonify(persons)

if __name__ == '__main__':
    load_schema()
    load_sample_data()
    app.run(debug=True, port=3000)
```

### JavaScript Implementation

#### package.json

```json
{
  "name": "knowledge-base",
  "version": "1.0.0",
  "type": "module",
  "dependencies": {
    "oxigraph": "^0.4.0",
    "express": "^4.18.0",
    "cors": "^2.8.5",
    "uuid": "^9.0.0"
  }
}
```

#### server.js

```javascript
import express from 'express';
import cors from 'cors';
import { Store, NamedNode, Literal, DataFactory } from 'oxigraph';
import { v4 as uuidv4 } from 'uuid';

const app = express();
app.use(cors());
app.use(express.json());

const store = new Store();
const KB_NS = "http://example.org/kb/";
const { quad, namedNode, literal, defaultGraph } = DataFactory;

// Load schema
const schema = `
@prefix kb: <http://example.org/kb/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

kb:Document a owl:Class ;
    rdfs:label "Document" .

kb:Person a owl:Class ;
    rdfs:label "Person" .

kb:title a owl:DatatypeProperty ;
    rdfs:domain kb:Document ;
    rdfs:range xsd:string .

kb:content a owl:DatatypeProperty ;
    rdfs:domain kb:Document ;
    rdfs:range xsd:string .

kb:author a owl:ObjectProperty ;
    rdfs:domain kb:Document ;
    rdfs:range kb:Person .

kb:name a owl:DatatypeProperty ;
    rdfs:range xsd:string .

kb:email a owl:DatatypeProperty ;
    rdfs:range xsd:string .
`;

store.load(schema, {format: "text/turtle"});
console.log("Schema loaded");

// Load sample data
const alice = namedNode(`${KB_NS}person/alice`);
const rdfType = namedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
const personClass = namedNode(`${KB_NS}Person`);

store.add(quad(alice, rdfType, personClass, defaultGraph()));
store.add(quad(alice, namedNode(`${KB_NS}name`), literal("Alice Smith"), defaultGraph()));
store.add(quad(alice, namedNode(`${KB_NS}email`), literal("alice@example.com"), defaultGraph()));

const doc = namedNode(`${KB_NS}document/1`);
const docClass = namedNode(`${KB_NS}Document`);

store.add(quad(doc, rdfType, docClass, defaultGraph()));
store.add(quad(doc, namedNode(`${KB_NS}title`), literal("Introduction to Knowledge Graphs"), defaultGraph()));
store.add(quad(doc, namedNode(`${KB_NS}content`),
    literal("Knowledge graphs represent information as interconnected entities..."), defaultGraph()));
store.add(quad(doc, namedNode(`${KB_NS}author`), alice, defaultGraph()));

console.log("Sample data loaded");

// Routes
app.get('/documents', (req, res) => {
    const query = `
        PREFIX kb: <${KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author WHERE {
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL { ?doc kb:author ?author }
        }
        ORDER BY ?title
        LIMIT 100
    `;

    const results = store.query(query);
    const documents = [];

    for (const row of results) {
        documents.push({
            id: row.get('doc').value,
            title: row.get('title').value,
            content: row.get('content').value,
            author: row.has('author') ? row.get('author').value : null
        });
    }

    res.json(documents);
});

app.get('/documents/:id', (req, res) => {
    const docIri = `${KB_NS}document/${req.params.id}`;

    const query = `
        PREFIX kb: <${KB_NS}>

        SELECT ?title ?content ?author WHERE {
            <${docIri}> kb:title ?title ;
                        kb:content ?content .
            OPTIONAL { <${docIri}> kb:author ?author }
        }
    `;

    const results = Array.from(store.query(query));

    if (results.length === 0) {
        return res.status(404).json({ error: 'Document not found' });
    }

    const row = results[0];
    res.json({
        id: docIri,
        title: row.get('title').value,
        content: row.get('content').value,
        author: row.has('author') ? row.get('author').value : null
    });
});

app.post('/documents', (req, res) => {
    const docId = uuidv4();
    const docIri = namedNode(`${KB_NS}document/${docId}`);
    const { title, content, author_id } = req.body;

    store.add(quad(docIri, rdfType, docClass, defaultGraph()));
    store.add(quad(docIri, namedNode(`${KB_NS}title`), literal(title), defaultGraph()));
    store.add(quad(docIri, namedNode(`${KB_NS}content`), literal(content), defaultGraph()));

    if (author_id) {
        const author = namedNode(author_id);
        store.add(quad(docIri, namedNode(`${KB_NS}author`), author, defaultGraph()));
    }

    res.status(201).json({
        id: docIri.value,
        title,
        content,
        author: author_id || null
    });
});

app.delete('/documents/:id', (req, res) => {
    const docIri = namedNode(`${KB_NS}document/${req.params.id}`);

    for (const q of store.match(docIri, null, null, null)) {
        store.delete(q);
    }

    res.status(204).send();
});

app.get('/search', (req, res) => {
    const q = (req.query.q || '').toLowerCase();
    const limit = parseInt(req.query.limit || '10');

    const query = `
        PREFIX kb: <${KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?doc ?title ?content ?author WHERE {
            ?doc rdf:type kb:Document ;
                 kb:title ?title ;
                 kb:content ?content .
            OPTIONAL { ?doc kb:author ?author }
            FILTER(
                CONTAINS(LCASE(?title), "${q}") ||
                CONTAINS(LCASE(?content), "${q}")
            )
        }
        LIMIT ${limit}
    `;

    const results = store.query(query);
    const documents = [];

    for (const row of results) {
        documents.push({
            id: row.get('doc').value,
            title: row.get('title').value,
            content: row.get('content').value,
            author: row.has('author') ? row.get('author').value : null
        });
    }

    res.json(documents);
});

app.post('/sparql', (req, res) => {
    const { query } = req.body;

    if (!query) {
        return res.status(400).json({ error: 'No query provided' });
    }

    try {
        const results = store.query(query);
        const bindings = [];

        for (const row of results) {
            const binding = {};
            for (const [varName, term] of row) {
                binding[varName] = term.value;
            }
            bindings.push(binding);
        }

        res.json({ results: { bindings } });
    } catch (error) {
        res.status(400).json({ error: error.message });
    }
});

app.get('/persons', (req, res) => {
    const query = `
        PREFIX kb: <${KB_NS}>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?person ?name ?email WHERE {
            ?person rdf:type kb:Person .
            OPTIONAL { ?person kb:name ?name }
            OPTIONAL { ?person kb:email ?email }
        }
    `;

    const results = store.query(query);
    const persons = [];

    for (const row of results) {
        persons.push({
            id: row.get('person').value,
            name: row.has('name') ? row.get('name').value : null,
            email: row.has('email') ? row.get('email').value : null
        });
    }

    res.json(persons);
});

const PORT = 3000;
app.listen(PORT, () => {
    console.log(`Knowledge base server running on http://localhost:${PORT}`);
});
```

## Data Loading Pipeline

### CSV to RDF Converter (load_csv.py)

```python
import csv
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph
import sys

def load_documents_from_csv(store, csv_file):
    """Load documents from CSV file"""
    KB_NS = "http://example.org/kb/"
    rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    doc_class = NamedNode(f"{KB_NS}Document")
    title_pred = NamedNode(f"{KB_NS}title")
    content_pred = NamedNode(f"{KB_NS}content")

    with open(csv_file, 'r') as f:
        reader = csv.DictReader(f)
        count = 0

        for row in reader:
            doc_id = row['id']
            doc_iri = NamedNode(f"{KB_NS}document/{doc_id}")

            store.add(Quad(doc_iri, rdf_type, doc_class, DefaultGraph()))
            store.add(Quad(doc_iri, title_pred, Literal(row['title']), DefaultGraph()))
            store.add(Quad(doc_iri, content_pred, Literal(row['content']), DefaultGraph()))

            count += 1
            if count % 1000 == 0:
                print(f"Loaded {count} documents...")

    print(f"Total documents loaded: {count}")

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python load_csv.py <csv_file>")
        sys.exit(1)

    store = Store()
    load_documents_from_csv(store, sys.argv[1])

    # Save to file
    with open("kb_data.nq", "wb") as f:
        store.dump(f, format=RdfFormat.N_QUADS)
    print("Data saved to kb_data.nq")
```

## Setup and Usage

### Rust

```bash
# Build
cargo build --release

# Run
cargo run --release

# Test
curl http://localhost:3000/documents
```

### Python

```bash
# Install dependencies
pip install -r requirements.txt

# Run
python app.py

# Test
curl http://localhost:3000/documents
```

### JavaScript

```bash
# Install dependencies
npm install

# Run
node server.js

# Test
curl http://localhost:3000/documents
```

## API Endpoints

- `GET /documents` - List all documents
- `GET /documents/:id` - Get specific document
- `POST /documents` - Create document
- `DELETE /documents/:id` - Delete document
- `GET /search?q=term&limit=10` - Search documents
- `POST /sparql` - Execute SPARQL query
- `GET /persons` - List all persons
- `GET /graph` - Get graph data

## Example Queries

### Create Document

```bash
curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Getting Started with SPARQL",
    "content": "SPARQL is a powerful query language for RDF data...",
    "author_id": "http://example.org/kb/person/alice"
  }'
```

### Search

```bash
curl "http://localhost:3000/search?q=knowledge&limit=5"
```

### SPARQL Query

```bash
curl -X POST http://localhost:3000/sparql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "PREFIX kb: <http://example.org/kb/> SELECT ?doc ?title WHERE { ?doc kb:title ?title } LIMIT 10"
  }'
```

## Production Considerations

1. **Persistence**: Use file-based store instead of in-memory
2. **Authentication**: Add JWT or OAuth2
3. **Validation**: Add input validation with SHACL
4. **Caching**: Add Redis for query results
5. **Full-text Search**: Integrate with Elasticsearch
6. **Rate Limiting**: Add rate limiting middleware
7. **Logging**: Add structured logging
8. **Monitoring**: Add Prometheus metrics
9. **Backup**: Regular RDF dumps
10. **Scaling**: Consider read replicas for queries
