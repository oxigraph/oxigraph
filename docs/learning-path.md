# Oxigraph Learning Path

This guide provides a structured learning path from beginner to advanced. Each section builds on the previous one, with estimated time commitments and hands-on exercises.

---

## How to Use This Guide

1. **Choose your starting point** based on your experience level
2. **Follow the path sequentially** for best results
3. **Complete the exercises** to reinforce learning
4. **Check your understanding** with the verification tasks

**Time Investment:**
- Beginner: 1-2 hours
- Intermediate: 3-4 hours (half day)
- Advanced: 6-8 hours (full day)

---

## Beginner Path (1-2 hours)

**Goal:** Understand RDF basics, create your first store, and run simple queries.

**Prerequisites:**
- Oxigraph installed in at least one language
- Basic programming knowledge
- No prior knowledge of RDF/SPARQL required

### Module 1: What is RDF and SPARQL? (15 minutes)

#### Understanding RDF

RDF (Resource Description Framework) is a way to represent data as interconnected facts.

**Core Concept: The Triple**

Every piece of information is a statement with three parts:
```
Subject → Predicate → Object
```

Example:
```
Alice → knows → Bob
(who)   (relationship)  (whom)
```

In RDF syntax (Turtle):
```turtle
<http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/bob> .
```

**Why IRIs?**
- IRIs (like URLs) make concepts globally unique
- Allows data from different sources to connect
- Forms a "web of data"

**RDF Node Types:**

1. **Named Nodes (IRIs):** `<http://example.org/alice>`
2. **Literals:** `"Alice"`, `42`, `"2024-01-01"^^xsd:date`
3. **Blank Nodes:** `_:b1` (anonymous nodes)

#### Understanding SPARQL

SPARQL is the query language for RDF (like SQL for databases).

**Basic Query Pattern:**
```sparql
SELECT ?variable WHERE {
    ?variable <predicate> <object> .
}
```

Example:
```sparql
SELECT ?person WHERE {
    ?person <http://xmlns.com/foaf/0.1/knows> <http://example.org/bob> .
}
```

This finds everyone who knows Bob.

**Exercise:**
Read: [RDF Fundamentals](explanation/rdf-fundamentals.md)

---

### Module 2: Your First Store and Query (30 minutes)

#### Choose Your Language

<details>
<summary><b>Rust</b></summary>

**Create a new project:**
```bash
cargo new my-first-graph
cd my-first-graph
cargo add oxigraph
```

**Write your first program (`src/main.rs`):**
```rust
use oxigraph::model::*;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a store
    let store = Store::new()?;
    println!("✓ Store created");

    // 2. Add a triple: "Alice knows Bob"
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph,
    ))?;
    println!("✓ Data added");

    // 3. Query: "Who does Alice know?"
    let query = "SELECT ?friend WHERE {
        <http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> ?friend .
    }";

    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        println!("\n✓ Results:");
        while let Some(solution) = solutions.next() {
            println!("  Alice knows: {}", solution?.get("friend").unwrap());
        }
    }

    Ok(())
}
```

**Run:**
```bash
cargo run
```

</details>

<details>
<summary><b>Python</b></summary>

**Create a file `first_graph.py`:**
```python
from pyoxigraph import Store, NamedNode, Quad, DefaultGraph

# 1. Create a store
store = Store()
print("✓ Store created")

# 2. Add a triple: "Alice knows Bob"
alice = NamedNode("http://example.org/alice")
bob = NamedNode("http://example.org/bob")
knows = NamedNode("http://xmlns.com/foaf/0.1/knows")

store.add(Quad(alice, knows, bob, DefaultGraph()))
print("✓ Data added")

# 3. Query: "Who does Alice know?"
query = """
    SELECT ?friend WHERE {
        <http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> ?friend .
    }
"""

print("\n✓ Results:")
for solution in store.query(query):
    print(f"  Alice knows: {solution['friend']}")
```

**Run:**
```bash
python first_graph.py
```

</details>

<details>
<summary><b>JavaScript</b></summary>

**Create `first-graph.js`:**
```javascript
const oxigraph = require('oxigraph');

// 1. Create a store
const store = new oxigraph.Store();
console.log("✓ Store created");

// 2. Add a triple: "Alice knows Bob"
const alice = oxigraph.namedNode("http://example.org/alice");
const bob = oxigraph.namedNode("http://example.org/bob");
const knows = oxigraph.namedNode("http://xmlns.com/foaf/0.1/knows");

store.add(oxigraph.triple(alice, knows, bob));
console.log("✓ Data added");

// 3. Query: "Who does Alice know?"
const query = `
    SELECT ?friend WHERE {
        <http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> ?friend .
    }
`;

console.log("\n✓ Results:");
for (const solution of store.query(query)) {
    console.log(`  Alice knows: ${solution.get("friend").value}`);
}
```

**Run:**
```bash
node first-graph.js
```

</details>

**Expected Output (all languages):**
```
✓ Store created
✓ Data added

✓ Results:
  Alice knows: <http://example.org/bob>
```

---

### Module 3: Loading Sample Data (30 minutes)

Instead of adding triples one by one, load data from RDF files.

#### Create Sample Data File

Create `sample-data.ttl` (Turtle format):

```turtle
@prefix ex: <http://example.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix schema: <http://schema.org/> .

ex:alice a foaf:Person ;
    foaf:name "Alice Smith" ;
    foaf:age 30 ;
    foaf:knows ex:bob, ex:charlie .

ex:bob a foaf:Person ;
    foaf:name "Bob Jones" ;
    foaf:age 25 .

ex:charlie a foaf:Person ;
    foaf:name "Charlie Brown" ;
    foaf:age 35 ;
    foaf:knows ex:alice .
```

#### Load and Query the Data

<details>
<summary><b>Rust</b></summary>

```rust
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Load from file
    store.load_from_path("sample-data.ttl")?;
    println!("✓ Data loaded");

    // Query: Find all people and their names
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        SELECT ?person ?name WHERE {
            ?person a foaf:Person ;
                    foaf:name ?name .
        }
    "#;

    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        println!("\nPeople in the database:");
        while let Some(solution) = solutions.next() {
            let sol = solution?;
            println!("  - {}", sol.get("name").unwrap());
        }
    }

    Ok(())
}
```

</details>

<details>
<summary><b>Python</b></summary>

```python
from pyoxigraph import Store

store = Store()

# Load from file
store.load(path="sample-data.ttl", format=RdfFormat.TURTLE)
print("✓ Data loaded")

# Query: Find all people and their names
query = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?person ?name WHERE {
        ?person a foaf:Person ;
                foaf:name ?name .
    }
"""

print("\nPeople in the database:")
for solution in store.query(query):
    print(f"  - {solution['name'].value}")
```

</details>

<details>
<summary><b>JavaScript</b></summary>

```javascript
const oxigraph = require('oxigraph');
const fs = require('fs');

const store = new oxigraph.Store();

// Load from file
const data = fs.readFileSync('sample-data.ttl', 'utf-8');
store.load(input=data, { format: "text/turtle" });
console.log("✓ Data loaded");

// Query: Find all people and their names
const query = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?person ?name WHERE {
        ?person a foaf:Person ;
                foaf:name ?name .
    }
`;

console.log("\nPeople in the database:");
for (const solution of store.query(query)) {
    console.log(`  - ${solution.get("name").value}`);
}
```

</details>

**Expected Output:**
```
✓ Data loaded

People in the database:
  - "Alice Smith"
  - "Bob Jones"
  - "Charlie Brown"
```

---

### Module 4: Practice Exercises (30 minutes)

Using the `sample-data.ttl` from above, write queries to answer these questions:

**Exercise 1:** Find everyone Alice knows
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?friend ?friendName WHERE {
    <http://example.org/alice> foaf:knows ?friend .
    ?friend foaf:name ?friendName .
}
```

**Exercise 2:** Find people older than 28
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?name ?age WHERE {
    ?person foaf:name ?name ;
            foaf:age ?age .
    FILTER (?age > 28)
}
```

**Exercise 3:** Count total people
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT (COUNT(?person) AS ?count) WHERE {
    ?person a foaf:Person .
}
```

**Exercise 4:** Find mutual friendships (who knows who back)
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?person1Name ?person2Name WHERE {
    ?person1 foaf:knows ?person2 .
    ?person2 foaf:knows ?person1 .
    ?person1 foaf:name ?person1Name .
    ?person2 foaf:name ?person2Name .
}
```

---

### Beginner Verification

You're ready for the intermediate level if you can:

- [ ] Explain what a triple is
- [ ] Create a store and add data
- [ ] Load data from a Turtle file
- [ ] Write a basic SELECT query
- [ ] Use FILTER in a query
- [ ] Use PREFIX to shorten IRIs

**Time Invested:** ~2 hours

---

## Intermediate Path (3-4 hours)

**Goal:** Build real-world data models, write complex queries, understand performance basics.

**Prerequisites:** Completed beginner path or equivalent knowledge.

### Module 5: Data Modeling (1 hour)

#### Real-World Example: Building a Book Catalog

Let's model a library catalog with books, authors, and publishers.

**Create `library.ttl`:**

```turtle
@prefix ex: <http://example.org/library/> .
@prefix schema: <http://schema.org/> .
@prefix dcterms: <http://purl.org/dc/terms/> .

# Books
ex:book1 a schema:Book ;
    schema:name "The Rust Programming Language" ;
    schema:author ex:author1 ;
    schema:publisher ex:publisher1 ;
    dcterms:issued "2023-02-01"^^xsd:date ;
    schema:numberOfPages 550 ;
    schema:isbn "978-1718503106" .

ex:book2 a schema:Book ;
    schema:name "Programming Python" ;
    schema:author ex:author2 ;
    schema:publisher ex:publisher2 ;
    dcterms:issued "2013-01-15"^^xsd:date ;
    schema:numberOfPages 1628 .

ex:book3 a schema:Book ;
    schema:name "JavaScript: The Good Parts" ;
    schema:author ex:author3 ;
    schema:publisher ex:publisher2 ;
    dcterms:issued "2008-05-08"^^xsd:date ;
    schema:numberOfPages 176 .

# Authors
ex:author1 a schema:Person ;
    schema:name "Steve Klabnik" ;
    schema:nationality "American" .

ex:author2 a schema:Person ;
    schema:name "Mark Lutz" ;
    schema:nationality "American" .

ex:author3 a schema:Person ;
    schema:name "Douglas Crockford" ;
    schema:nationality "American" .

# Publishers
ex:publisher1 a schema:Organization ;
    schema:name "No Starch Press" ;
    schema:location "San Francisco, CA" .

ex:publisher2 a schema:Organization ;
    schema:name "O'Reilly Media" ;
    schema:location "Sebastopol, CA" .
```

#### Load and Explore

Write a program to load this data and run various queries:

```python
# Python example
from pyoxigraph import Store

store = Store()
store.load(path="library.ttl", format=RdfFormat.TURTLE)

# Query 1: All books
print("=== All Books ===")
for solution in store.query("""
    PREFIX schema: <http://schema.org/>
    SELECT ?title ?pages WHERE {
        ?book a schema:Book ;
              schema:name ?title ;
              schema:numberOfPages ?pages .
    }
    ORDER BY ?title
"""):
    print(f"{solution['title'].value} ({solution['pages'].value} pages)")

# Query 2: Books by publisher
print("\n=== Books by O'Reilly ===")
for solution in store.query("""
    PREFIX schema: <http://schema.org/>
    SELECT ?title ?authorName WHERE {
        ?book a schema:Book ;
              schema:name ?title ;
              schema:author ?author ;
              schema:publisher ?publisher .
        ?author schema:name ?authorName .
        ?publisher schema:name "O'Reilly Media" .
    }
"""):
    print(f"{solution['title'].value} by {solution['authorName'].value}")

# Query 3: Average pages per publisher
print("\n=== Average Pages by Publisher ===")
for solution in store.query("""
    PREFIX schema: <http://schema.org/>
    SELECT ?publisherName (AVG(?pages) AS ?avgPages) WHERE {
        ?book a schema:Book ;
              schema:numberOfPages ?pages ;
              schema:publisher ?publisher .
        ?publisher schema:name ?publisherName .
    }
    GROUP BY ?publisherName
"""):
    print(f"{solution['publisherName'].value}: {float(solution['avgPages'].value):.0f} pages")
```

**Exercises:**

1. Add more books to the catalog
2. Query for books published after 2010
3. Find authors who have written more than one book
4. Calculate the total number of pages by publisher

---

### Module 6: Complex Queries (1.5 hours)

#### OPTIONAL Patterns

Find books that may or may not have an ISBN:

```sparql
PREFIX schema: <http://schema.org/>
SELECT ?title ?isbn WHERE {
    ?book schema:name ?title .
    OPTIONAL { ?book schema:isbn ?isbn }
}
```

#### UNION Patterns

Find all people (authors OR people mentioned):

```sparql
PREFIX schema: <http://schema.org/>
SELECT DISTINCT ?name WHERE {
    {
        ?person a schema:Person ;
                schema:name ?name .
    } UNION {
        ?author a schema:Author ;
                schema:name ?name .
    }
}
```

#### Property Paths

Find all transitive relationships:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT ?person ?connection WHERE {
    ?person foaf:knows+ ?connection .
}
```

The `+` means "one or more hops" (friends of friends).

#### Aggregations

```sparql
# Count, sum, average, min, max
PREFIX schema: <http://schema.org/>
SELECT
    (COUNT(?book) AS ?totalBooks)
    (SUM(?pages) AS ?totalPages)
    (AVG(?pages) AS ?avgPages)
    (MIN(?pages) AS ?shortestBook)
    (MAX(?pages) AS ?longestBook)
WHERE {
    ?book a schema:Book ;
          schema:numberOfPages ?pages .
}
```

#### Subqueries

Find authors who wrote the longest books:

```sparql
PREFIX schema: <http://schema.org/>
SELECT ?authorName ?bookTitle ?pages WHERE {
    ?book schema:author ?author ;
          schema:name ?bookTitle ;
          schema:numberOfPages ?pages .
    ?author schema:name ?authorName .

    {
        SELECT (MAX(?p) AS ?maxPages) WHERE {
            ?b schema:numberOfPages ?p .
        }
    }

    FILTER(?pages = ?maxPages)
}
```

**Practice Exercises:**

1. Find books with or without an ISBN number
2. List all organizations (publishers) and count their books
3. Find the most prolific author (most books)
4. Create a query using a property path

---

### Module 7: Performance Basics (1 hour)

#### Understanding Indexes

Oxigraph maintains three indexes:
- **SPO** (Subject-Predicate-Object)
- **POS** (Predicate-Object-Subject)
- **OSP** (Object-Subject-Predicate)

Queries are optimized based on which patterns you provide.

#### Query Optimization Tips

**1. Use LIMIT when exploring:**
```sparql
SELECT * WHERE { ?s ?p ?o } LIMIT 100
```

**2. Filter early:**
```sparql
# Good: Filter first
SELECT ?person ?name WHERE {
    ?person a foaf:Person .  # Filter by type first
    ?person foaf:name ?name .
}

# Less optimal: Filter last
SELECT ?person ?name WHERE {
    ?person foaf:name ?name .
    ?person a foaf:Person .  # Type filter last
}
```

**3. Be specific:**
```sparql
# Good: Specific predicate
SELECT ?value WHERE {
    <http://example.org/alice> foaf:name ?value .
}

# Slower: Scan all predicates
SELECT ?p ?value WHERE {
    <http://example.org/alice> ?p ?value .
}
```

#### Bulk Loading

For large datasets, use bulk loading instead of inserting one triple at a time:

**Rust:**
```rust
let mut loader = store.bulk_loader();
loader.load_from_path("large-file.nq")?;
```

**Python:**
```python
store.bulk_load(path="large-file.nq", format=RdfFormat.N_QUADS)
```

#### Persistent vs In-Memory

**In-Memory (faster, limited by RAM):**
```python
store = Store()  # Python
Store::new()     # Rust
```

**Persistent (slower, scales to disk):**
```python
store = Store("./my-database")  # Python
Store::open("./my-database")    # Rust
```

**Exercise:**

1. Create a large dataset (10,000+ triples)
2. Compare bulk loading vs. individual inserts
3. Measure query performance with and without LIMIT
4. Try persistent vs. in-memory storage

---

### Intermediate Verification

You're ready for advanced topics if you can:

- [ ] Model real-world data with appropriate vocabularies
- [ ] Write queries with OPTIONAL, UNION, and FILTER
- [ ] Use aggregation functions (COUNT, SUM, AVG, etc.)
- [ ] Understand when to use bulk loading
- [ ] Explain the difference between in-memory and persistent storage
- [ ] Optimize simple queries for performance

**Time Invested:** ~4 hours (half day)

---

## Advanced Path (6-8 hours)

**Goal:** Deploy to production, optimize for scale, extend functionality.

**Prerequisites:** Completed intermediate path or equivalent experience.

### Module 8: Production Deployment (2 hours)

#### Running the Server

**Docker Production Setup:**

```bash
# Create production directories
mkdir -p /opt/oxigraph/{data,backups,logs}

# Run with resource limits and restart policy
docker run -d \
  --name oxigraph-prod \
  --restart unless-stopped \
  --memory="4g" \
  --cpus="2.0" \
  -v /opt/oxigraph/data:/data \
  -v /opt/oxigraph/logs:/logs \
  -p 7878:7878 \
  ghcr.io/oxigraph/oxigraph:latest \
  serve --location /data --bind 0.0.0.0:7878

# View logs
docker logs -f oxigraph-prod
```

**Nginx Reverse Proxy:**

```nginx
server {
    listen 80;
    server_name sparql.example.com;

    location / {
        proxy_pass http://localhost:7878;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # Timeout for long queries
        proxy_read_timeout 300s;
    }
}
```

**Systemd Service (Linux):**

Create `/etc/systemd/system/oxigraph.service`:

```ini
[Unit]
Description=Oxigraph SPARQL Server
After=network.target

[Service]
Type=simple
User=oxigraph
WorkingDirectory=/opt/oxigraph
ExecStart=/usr/local/bin/oxigraph serve --location /opt/oxigraph/data --bind 127.0.0.1:7878
Restart=on-failure
RestartSec=10s

# Resource limits
MemoryMax=4G
CPUQuota=200%

# Logging
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable oxigraph
sudo systemctl start oxigraph
sudo systemctl status oxigraph
```

**Kubernetes Deployment:**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: oxigraph
spec:
  replicas: 1
  selector:
    matchLabels:
      app: oxigraph
  template:
    metadata:
      labels:
        app: oxigraph
    spec:
      containers:
      - name: oxigraph
        image: ghcr.io/oxigraph/oxigraph:latest
        args: ["serve", "--location", "/data", "--bind", "0.0.0.0:7878"]
        ports:
        - containerPort: 7878
        volumeMounts:
        - name: data
          mountPath: /data
        resources:
          limits:
            memory: "4Gi"
            cpu: "2"
          requests:
            memory: "2Gi"
            cpu: "1"
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: oxigraph-data
---
apiVersion: v1
kind: Service
metadata:
  name: oxigraph
spec:
  selector:
    app: oxigraph
  ports:
  - port: 80
    targetPort: 7878
  type: LoadBalancer
```

#### Monitoring

**Health Check Endpoint:**

```bash
# Check if server is responding
curl http://localhost:7878/

# Query endpoint test
curl -X POST \
  -H 'Content-Type: application/sparql-query' \
  --data 'ASK { ?s ?p ?o }' \
  http://localhost:7878/query
```

**Prometheus Monitoring (future feature):**

Track query latency, throughput, and error rates.

---

### Module 9: Optimization Strategies (2 hours)

#### RocksDB Tuning

Set environment variables for better performance:

```bash
# Increase write buffer size
export ROCKSDB_TOTAL_WRITE_BUFFER_SIZE=2147483648  # 2GB

# Increase background jobs
export ROCKSDB_MAX_BACKGROUND_JOBS=8

# Enable compression
export ROCKSDB_COMPRESSION_TYPE=lz4
```

#### Query Optimization

**Use EXPLAIN (when available):**

Analyze query execution plans to understand performance.

**Index Selection:**

Write queries that leverage indexes:

```sparql
# Uses POS index efficiently
SELECT ?subject WHERE {
    ?subject <http://schema.org/name> "Alice" .
}

# Uses OSP index efficiently
SELECT ?predicate WHERE {
    ?subject ?predicate <http://example.org/object> .
}
```

**Pagination:**

```sparql
SELECT ?s ?p ?o WHERE {
    ?s ?p ?o .
}
ORDER BY ?s
LIMIT 100 OFFSET 0  # First page

# Next page
LIMIT 100 OFFSET 100
```

#### Caching Strategies

**Application-Level Caching:**

```python
from functools import lru_cache
from pyoxigraph import Store

store = Store("./data")

@lru_cache(maxsize=1000)
def cached_query(query_string):
    return list(store.query(query_string))

# Subsequent calls use cache
results = cached_query("SELECT * WHERE { ?s ?p ?o } LIMIT 100")
```

#### Data Loading Strategies

**For Initial Load (Fastest):**

```bash
# Stop server, load offline, start server
oxigraph load --location ./data --file huge-dataset.nq
oxigraph serve --location ./data
```

**For Incremental Updates:**

```python
# Use transactions for batch updates
store.update("""
    INSERT DATA {
        # Multiple triples here...
    }
""")
```

---

### Module 10: Custom Extensions (2-3 hours)

#### Creating a Custom SPARQL Service

Embed Oxigraph in a web application:

**Rust Example (using Axum):**

```rust
use axum::{extract::State, routing::post, Json, Router};
use oxigraph::store::Store;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
}

async fn query_handler(
    State(state): State<AppState>,
    body: String,
) -> Result<String, String> {
    match state.store.query(&body) {
        Ok(results) => Ok(format!("{:?}", results)),
        Err(e) => Err(e.to_string()),
    }
}

#[tokio::main]
async fn main() {
    let store = Arc::new(Store::open("./data").unwrap());
    let app_state = AppState { store };

    let app = Router::new()
        .route("/query", post(query_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
```

**Python Example (using Flask):**

```python
from flask import Flask, request, jsonify
from pyoxigraph import Store

app = Flask(__name__)
store = Store("./data")

@app.route('/query', methods=['POST'])
def query():
    sparql_query = request.data.decode('utf-8')
    try:
        results = []
        for solution in store.query(sparql_query):
            results.append({k: str(v) for k, v in solution.items()})
        return jsonify(results)
    except Exception as e:
        return jsonify({"error": str(e)}), 400

if __name__ == '__main__':
    app.run(port=5000)
```

#### Custom Vocabulary

Create your own ontology:

```turtle
@prefix myonto: <http://example.org/ontology/> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

# Define classes
myonto:Product a owl:Class ;
    rdfs:label "Product" ;
    rdfs:comment "A product in our catalog" .

myonto:Customer a owl:Class ;
    rdfs:label "Customer" ;
    rdfs:comment "A customer of our business" .

# Define properties
myonto:purchased a owl:ObjectProperty ;
    rdfs:domain myonto:Customer ;
    rdfs:range myonto:Product ;
    rdfs:label "purchased" .

myonto:price a owl:DatatypeProperty ;
    rdfs:domain myonto:Product ;
    rdfs:range xsd:decimal ;
    rdfs:label "price" .
```

#### SHACL Validation

Validate your data against shapes:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .

ex:PersonShape a sh:NodeShape ;
    sh:targetClass foaf:Person ;
    sh:property [
        sh:path foaf:name ;
        sh:minCount 1 ;
        sh:datatype xsd:string ;
    ] ;
    sh:property [
        sh:path foaf:age ;
        sh:datatype xsd:integer ;
        sh:minInclusive 0 ;
        sh:maxInclusive 150 ;
    ] .
```

**Validate with Python:**

```python
from pyoxigraph import Store

store = Store()
store.load(path="data.ttl", format=RdfFormat.TURTLE)
store.load(path="shapes.ttl", format=RdfFormat.TURTLE)

# Validation (when SHACL support is available)
# validation_report = store.validate_with_shacl("shapes.ttl")
```

---

### Module 11: Real-World Project (2 hours)

Build a complete application using Oxigraph.

**Project: Personal Knowledge Base**

Features:
1. Import notes from markdown files
2. Link notes together
3. Query with SPARQL
4. Export to various formats

**Schema:**

```turtle
@prefix kb: <http://example.org/kb/> .
@prefix dcterms: <http://purl.org/dc/terms/> .

kb:note1 a kb:Note ;
    dcterms:title "Introduction to RDF" ;
    dcterms:created "2024-01-15T10:00:00Z"^^xsd:dateTime ;
    kb:content "RDF is a framework for representing data..." ;
    kb:links kb:note2, kb:note3 ;
    kb:tag "rdf", "semantic-web" .
```

**Implementation Steps:**

1. Define your schema
2. Create import scripts
3. Build query interface
4. Add export functionality
5. Deploy

**Exercise:**

Build a smaller version:
- Create a bibliography manager
- Import BibTeX data
- Query by author, year, topic
- Export citations

---

### Advanced Verification

You've mastered Oxigraph if you can:

- [ ] Deploy Oxigraph to production with proper monitoring
- [ ] Optimize queries for large datasets
- [ ] Tune RocksDB for your workload
- [ ] Build custom applications embedding Oxigraph
- [ ] Create and use custom vocabularies
- [ ] Validate data with SHACL
- [ ] Design schemas for real-world problems
- [ ] Handle millions of triples efficiently

**Time Invested:** ~8 hours (full day)

---

## Continuous Learning

### Stay Updated

- Follow [Oxigraph Blog/Releases](https://github.com/oxigraph/oxigraph/releases)
- Join [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
- Chat on [Gitter](https://gitter.im/oxigraph/community)

### Contribute Back

- Report bugs and request features
- Improve documentation
- Submit pull requests
- Help others in discussions

### Further Resources

**RDF & Semantic Web:**
- [RDF Primer](https://www.w3.org/TR/rdf11-primer/)
- [Linked Data Book](http://linkeddatabook.com/)
- [Semantic Web for the Working Ontologist](https://www.workingontologist.org/)

**SPARQL:**
- [SPARQL 1.1 Specification](https://www.w3.org/TR/sparql11-query/)
- [Learn SPARQL](http://www.learningsparql.com/)
- [SPARQL by Example](https://www.w3.org/2009/Talks/0615-qbe/)

**Advanced Topics:**
- [RDF-star and SPARQL-star](https://w3c.github.io/rdf-star/)
- [SHACL Specification](https://www.w3.org/TR/shacl/)
- [GeoSPARQL](https://www.ogc.org/standards/geosparql)

---

## Next Steps

You've completed the learning path! Here's what to do next:

1. **Build something real:** Apply what you've learned to a project
2. **Optimize:** Measure and improve performance
3. **Share:** Write about your experience
4. **Contribute:** Help improve Oxigraph

**Keep the [Cheatsheet](cheatsheet.md) handy for quick reference!**

---

**Congratulations on completing the Oxigraph learning path!**
