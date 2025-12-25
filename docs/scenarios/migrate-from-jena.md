# Migrate from Apache Jena to Oxigraph

This guide helps you migrate from Apache Jena to Oxigraph, covering API equivalence, data migration, and performance optimization.

## Overview

Apache Jena is a popular Java RDF framework with TDB and TDB2 storage backends. Oxigraph provides similar functionality with:

- **Better Performance**: Optimized RocksDB storage
- **Modern API**: Idiomatic Rust with language bindings
- **Standards Compliance**: Full SPARQL 1.1 support
- **Smaller Footprint**: Less memory and disk usage

## Quick Comparison

| Feature | Apache Jena | Oxigraph |
|---------|-------------|----------|
| Language | Java | Rust (+ Python, JavaScript) |
| Storage Backend | TDB/TDB2 | RocksDB |
| SPARQL Version | 1.1 | 1.1 |
| Transactions | Yes | Yes |
| Federation | SERVICE | SERVICE |
| Inference | Yes | No (planned) |
| Text Search | Yes (with plugin) | No (planned) |

## API Mapping

### Basic Operations

#### Jena (Java)

```java
import org.apache.jena.rdf.model.*;
import org.apache.jena.tdb2.TDB2Factory;

// Create dataset
Dataset dataset = TDB2Factory.connectDataset("./data");
Model model = dataset.getDefaultModel();

// Add triple
Resource subject = model.createResource("http://example.org/subject");
Property predicate = model.createProperty("http://example.org/predicate");
RDFNode object = model.createLiteral("value");
model.add(subject, predicate, object);

// Query
String sparql = "SELECT * WHERE { ?s ?p ?o } LIMIT 10";
try (QueryExecution qexec = QueryExecutionFactory.create(sparql, model)) {
    ResultSet results = qexec.execSelect();
    while (results.hasNext()) {
        QuerySolution soln = results.nextSolution();
        System.out.println(soln);
    }
}

dataset.close();
```

#### Oxigraph (Rust)

```rust
use oxigraph::store::Store;
use oxigraph::model::*;
use oxigraph::sparql::QueryResults;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create store
    let store = Store::open("./data")?;

    // Add triple
    let subject = NamedNode::new("http://example.org/subject")?;
    let predicate = NamedNode::new("http://example.org/predicate")?;
    let object = Literal::new_simple_literal("value");

    store.insert(&Triple::new(subject, predicate, object))?;

    // Query
    let sparql = "SELECT * WHERE { ?s ?p ?o } LIMIT 10";
    if let QueryResults::Solutions(mut solutions) = store.query(sparql)? {
        for solution in solutions {
            println!("{:?}", solution?);
        }
    }

    Ok(())
}
```

### Complete Mapping Table

| Jena Class/Method | Oxigraph Equivalent | Notes |
|-------------------|---------------------|-------|
| `TDB2Factory.connectDataset()` | `Store::open()` | Opens persistent store |
| `DatasetFactory.createTxnMem()` | `MemoryStore::new()` | In-memory store |
| `Model.add()` | `Store::insert()` | Add single triple |
| `Model.remove()` | `Store::remove()` | Remove triple |
| `Model.listStatements()` | `Store::iter()` | Iterate triples |
| `QueryExecutionFactory.create()` | `Store::query()` | Execute SPARQL |
| `UpdateExecutionFactory.create()` | `Store::update()` | Execute SPARQL UPDATE |
| `dataset.begin()` | `Store::transaction()` | Start transaction |
| `ReadWrite.commit()` | `Transaction::commit()` | Commit transaction |
| `RDFDataMgr.loadModel()` | `Store::bulk_loader()` | Load RDF files |
| `RDFDataMgr.write()` | `Store::dump_graph()` | Export RDF |
| `Resource` | `NamedNode` or `BlankNode` | RDF resources |
| `Property` | `NamedNode` | RDF properties |
| `Literal` | `Literal` | RDF literals |
| `Statement` | `Triple` or `Quad` | RDF statements |

## Data Migration

### Step 1: Export Data from Jena

```java
import org.apache.jena.riot.RDFDataMgr;
import org.apache.jena.riot.RDFFormat;

// Export to N-Quads (recommended for full dataset)
Dataset dataset = TDB2Factory.connectDataset("./jena-data");
try (OutputStream out = new FileOutputStream("export.nq")) {
    RDFDataMgr.write(out, dataset, RDFFormat.NQUADS);
}
dataset.close();
```

### Step 2: Import into Oxigraph

#### Option A: Command-line Tool

```bash
# Using oxigraph-server
oxigraph load --location ./oxigraph-data --file export.nq --format nquads
```

#### Option B: Rust Code

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use std::fs::File;
use std::io::BufReader;

fn migrate_data() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open("./oxigraph-data")?;

    // Use bulk loader for best performance
    let file = File::open("export.nq")?;
    let reader = BufReader::new(file);

    store.bulk_loader()
        .on_progress(|count| {
            if count % 100_000 == 0 {
                println!("Loaded {} quads", count);
            }
        })
        .load_from_reader(reader, RdfFormat::NQuads)?;

    println!("Migration complete!");
    println!("Total quads: {}", store.len()?);

    Ok(())
}
```

#### Option C: Python Script

```python
from pyoxigraph import Store

def migrate_data():
    store = Store("./oxigraph-data")

    # Load exported data
    with open("export.nq", "rb") as f:
        store.bulk_load(f, "application/n-quads")

    print(f"Migration complete! Total quads: {len(store)}")

if __name__ == "__main__":
    migrate_data()
```

### Step 3: Verify Migration

Create a verification script to compare data:

```rust
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;

fn verify_migration() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open("./oxigraph-data")?;

    // Check total count
    println!("Total quads: {}", store.len()?);

    // Sample some data
    let query = r#"
        SELECT ?g (COUNT(*) as ?count)
        WHERE {
            GRAPH ?g { ?s ?p ?o }
        }
        GROUP BY ?g
        ORDER BY DESC(?count)
    "#;

    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        println!("\nQuads per graph:");
        for solution in solutions {
            let solution = solution?;
            println!("  {}: {}",
                solution.get("g").unwrap(),
                solution.get("count").unwrap()
            );
        }
    }

    Ok(())
}
```

## Query Compatibility

### SPARQL Differences

Most SPARQL queries work identically, but note these differences:

#### Property Paths

Both support SPARQL 1.1 property paths:

```sparql
# Works in both
SELECT ?person ?ancestor
WHERE {
    ?person foaf:knows+/foaf:parent* ?ancestor
}
```

#### Aggregates

```sparql
# Works in both
SELECT ?type (COUNT(*) as ?count)
WHERE {
    ?s a ?type
}
GROUP BY ?type
HAVING (COUNT(*) > 10)
```

#### Federation (SERVICE)

Both support SERVICE, but Oxigraph requires the `http-client` feature:

```sparql
# Works in both (with http-client in Oxigraph)
SELECT ?item ?itemLabel
WHERE {
    SERVICE <https://query.wikidata.org/sparql> {
        ?item wdt:P31 wd:Q5.
        ?item rdfs:label ?itemLabel.
        FILTER(LANG(?itemLabel) = "en")
    }
}
LIMIT 10
```

#### Jena-Specific Extensions

These Jena features are NOT supported in Oxigraph:

```sparql
# NOT SUPPORTED: Text search (requires jena-text plugin)
PREFIX text: <http://jena.apache.org/text#>
SELECT ?s
WHERE {
    ?s text:query "search term"
}

# NOT SUPPORTED: Built-in inference
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?subclass
WHERE {
    ?subclass rdfs:subClassOf* ex:BaseClass
}
```

**Workaround**: Use property paths for transitive queries:

```sparql
# Alternative using property paths (works in both)
SELECT ?subclass
WHERE {
    ?subclass rdfs:subClassOf* ex:BaseClass
}
```

## Performance Optimization

### Bulk Loading

Jena TDB2 has a bulk loader; Oxigraph's is often faster:

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use std::time::Instant;

fn benchmark_load() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open("./data")?;

    let start = Instant::now();

    store.bulk_loader()
        .on_progress(|count| {
            if count % 1_000_000 == 0 {
                let elapsed = start.elapsed().as_secs();
                let rate = count / elapsed.max(1);
                println!("{} quads, {} quads/sec", count, rate);
            }
        })
        .load_from_path("large-dataset.nq", RdfFormat::NQuads)?;

    println!("Loading took: {:?}", start.elapsed());

    Ok(())
}
```

### Transaction Batching

Instead of Jena's `executeWrite`:

```java
// Jena
dataset.executeWrite(() -> {
    for (Triple t : triples) {
        model.add(t);
    }
});
```

Use Oxigraph transactions:

```rust
// Oxigraph
let mut transaction = store.transaction()?;
for triple in triples {
    transaction.insert(triple)?;
}
transaction.commit()?;
```

### Memory Configuration

Jena uses JVM heap settings; Oxigraph uses RocksDB settings:

```rust
use oxigraph::store::{Store, StoreOptions};

// Configure RocksDB
let options = StoreOptions::default()
    .write_buffer_size(256 * 1024 * 1024)  // 256 MB write buffer
    .max_open_files(1000);

let store = Store::open_with_options("./data", options)?;
```

## Complete Migration Example

Here's a complete application showing the migration:

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use oxigraph::sparql::QueryResults;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Jena to Oxigraph Migration ===\n");

    // Step 1: Create Oxigraph store
    println!("Creating Oxigraph store...");
    let store = Store::open("./oxigraph-data")?;

    // Step 2: Import Jena export
    println!("Importing data from Jena export...");
    let file = File::open("jena-export.nq")?;
    let reader = BufReader::new(file);

    store.bulk_loader()
        .on_progress(|count| {
            if count % 100_000 == 0 {
                println!("  Loaded {} quads", count);
            }
        })
        .load_from_reader(reader, RdfFormat::NQuads)?;

    println!("✓ Import complete!\n");

    // Step 3: Verify data
    println!("Verifying migration...");
    println!("  Total quads: {}", store.len()?);

    // Run a test query
    let query = "SELECT (COUNT(*) as ?count) WHERE { ?s ?p ?o }";
    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        if let Some(solution) = solutions.next() {
            let count = solution?.get("count").unwrap();
            println!("  Triple count via SPARQL: {}", count);
        }
    }

    // Step 4: Test a more complex query
    println!("\nTesting complex query...");
    let query = r#"
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

        SELECT ?type (COUNT(*) as ?count)
        WHERE {
            ?s rdf:type ?type
        }
        GROUP BY ?type
        ORDER BY DESC(?count)
        LIMIT 10
    "#;

    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        println!("  Top 10 types:");
        for solution in solutions {
            let solution = solution?;
            println!("    {}: {}",
                solution.get("type").unwrap(),
                solution.get("count").unwrap()
            );
        }
    }

    println!("\n✓ Migration successful!");

    Ok(())
}
```

## Deployment

### Docker Deployment

Replace Jena Fuseki with Oxigraph server:

**Jena Fuseki:**
```yaml
# docker-compose.yml (old)
version: '3'
services:
  fuseki:
    image: stain/jena-fuseki
    ports:
      - "3030:3030"
    volumes:
      - ./fuseki-data:/fuseki
    environment:
      - ADMIN_PASSWORD=admin
```

**Oxigraph:**
```yaml
# docker-compose.yml (new)
version: '3'
services:
  oxigraph:
    image: oxigraph/oxigraph:latest
    command: serve --location /data --bind 0.0.0.0:7878
    ports:
      - "7878:7878"
    volumes:
      - ./oxigraph-data:/data
```

### Running the Server

```bash
# Build and start
docker-compose up -d

# Check logs
docker-compose logs -f

# Query endpoint
curl -X POST http://localhost:7878/query \
  -H "Content-Type: application/sparql-query" \
  --data "SELECT * WHERE { ?s ?p ?o } LIMIT 10"
```

## Troubleshooting

### Issue: Different Query Results

**Cause**: Jena may include inference; Oxigraph doesn't.

**Solution**: Materialize inferences in Jena before export:

```java
// In Jena, before export
Reasoner reasoner = ReasonerRegistry.getRDFSReasoner();
InfModel inf = ModelFactory.createInfModel(reasoner, model);
// Export inf model instead
```

### Issue: Missing Text Search

**Cause**: Oxigraph doesn't support text indexing yet.

**Solution**: Use FILTER with regex or integrate Elasticsearch:

```sparql
# Regex approach (slower)
SELECT ?s ?label
WHERE {
    ?s rdfs:label ?label
    FILTER(REGEX(?label, "search term", "i"))
}
```

### Issue: Performance Differences

**Cause**: Different query planners and storage engines.

**Solution**: Use EXPLAIN-style debugging:

```rust
// Enable query logging
std::env::set_var("RUST_LOG", "spareval=debug");
env_logger::init();

store.query(sparql)?;
// Check logs for query plan
```

## Next Steps

- Review [Performance Tuning](../how-to/performance-tuning.md)
- Explore [SPARQL Features](../reference/sparql.md)
- Check [API Documentation](https://docs.rs/oxigraph)

## Additional Resources

- [Jena Migration Checklist](https://github.com/oxigraph/oxigraph/wiki/Jena-Migration)
- [Performance Benchmarks](https://github.com/oxigraph/oxigraph/wiki/Benchmarks)
- [Community Forum](https://github.com/oxigraph/oxigraph/discussions)
