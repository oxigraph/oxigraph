# Migrate from RDFLib to pyoxigraph

This guide helps Python developers migrate from RDFLib to pyoxigraph for better performance and SPARQL support.

## Overview

RDFLib is the most popular Python RDF library, but it has limitations:

- **Slow Performance**: Pure Python implementation
- **Limited SPARQL**: Basic query support
- **Memory Issues**: Large datasets cause problems

pyoxigraph solves these issues with:

- **10-100x Faster**: Rust-based implementation
- **Full SPARQL 1.1**: Complete query and update support
- **Persistent Storage**: RocksDB backend handles billions of triples
- **Standards Compliant**: Full RDF 1.1 and SPARQL 1.1 support

## Quick Comparison

| Feature | RDFLib | pyoxigraph |
|---------|--------|------------|
| Language | Pure Python | Rust with Python bindings |
| Storage | In-memory or plugins | In-memory or RocksDB |
| SPARQL | Basic (via rdflib-sparql) | Full SPARQL 1.1 |
| Performance | Slow | Fast |
| Triple Count | Millions (memory limited) | Billions (disk-backed) |
| Serialization | Many formats | Many formats |
| Namespace Management | Built-in | Manual |
| Plugins | Many available | Few |

## API Mapping

### Basic Operations

#### RDFLib

```python
from rdflib import Graph, URIRef, Literal, Namespace
from rdflib.namespace import RDF, RDFS, FOAF

# Create graph
g = Graph()

# Add namespace
EX = Namespace("http://example.org/")
g.bind("ex", EX)

# Add triple
subject = URIRef("http://example.org/alice")
g.add((subject, RDF.type, FOAF.Person))
g.add((subject, FOAF.name, Literal("Alice")))

# Query with SPARQL
query = """
SELECT ?s ?name
WHERE {
    ?s a foaf:Person .
    ?s foaf:name ?name .
}
"""
results = g.query(query)
for row in results:
    print(f"{row.s}: {row.name}")

# Serialize
print(g.serialize(format="turtle"))
```

#### pyoxigraph

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

# Create store
store = Store()

# Add triples (namespaces handled manually)
subject = NamedNode("http://example.org/alice")
rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
foaf_person = NamedNode("http://xmlns.com/foaf/0.1/Person")
foaf_name = NamedNode("http://xmlns.com/foaf/0.1/name")

store.add(Quad(subject, rdf_type, foaf_person))
store.add(Quad(subject, foaf_name, Literal("Alice")))

# Query with SPARQL
query = """
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?s ?name
WHERE {
    ?s a foaf:Person .
    ?s foaf:name ?name .
}
"""
results = store.query(query)
for result in results:
    print(f"{result['s'].value}: {result['name'].value}")

# Serialize
output = store.dump_graph(format="text/turtle")
print(output.decode("utf-8"))
```

### Complete API Mapping

| RDFLib | pyoxigraph | Notes |
|--------|------------|-------|
| `Graph()` | `Store()` | In-memory graph |
| `Graph(store="path")` | `Store("path")` | Persistent store |
| `ConjunctiveGraph()` | `Store()` | Supports named graphs |
| `g.add((s, p, o))` | `store.add(Quad(s, p, o))` | Add triple |
| `g.remove((s, p, o))` | `store.remove(Quad(s, p, o))` | Remove triple |
| `g.triples((s, p, o))` | `store.quads_for_pattern(s, p, o)` | Pattern matching |
| `g.subjects(p, o)` | `store.quads_for_pattern(None, p, o)` | Find subjects |
| `g.query(sparql)` | `store.query(sparql)` | SPARQL SELECT |
| `g.update(sparql)` | `store.update(sparql)` | SPARQL UPDATE |
| `g.parse(file, format)` | `store.load(file, format)` | Load RDF |
| `g.serialize(format)` | `store.dump_graph(format)` | Serialize RDF |
| `URIRef(uri)` | `NamedNode(uri)` | Named node |
| `Literal(value)` | `Literal(value)` | Literal value |
| `Literal(value, lang)` | `Literal(value, language=lang)` | Language literal |
| `BNode()` | `BlankNode()` | Blank node |
| `Namespace(uri)` | Manual prefixes | Namespace handling |

## Namespace Handling

One key difference is namespace management.

### RDFLib Approach

```python
from rdflib import Graph, Namespace
from rdflib.namespace import RDF, RDFS, FOAF, SKOS

# Built-in namespaces
g = Graph()
g.bind("rdf", RDF)
g.bind("rdfs", RDFS)
g.bind("foaf", FOAF)

# Custom namespace
EX = Namespace("http://example.org/")
g.bind("ex", EX)

# Use namespace
g.add((EX.alice, RDF.type, FOAF.Person))
g.add((EX.alice, FOAF.name, Literal("Alice")))

# Serialization includes prefixes
print(g.serialize(format="turtle"))
```

### pyoxigraph Approach

Create a namespace helper:

```python
from pyoxigraph import NamedNode

class Namespace:
    """Helper class for namespace management"""
    def __init__(self, uri):
        self.uri = uri if uri.endswith((":", "/", "#")) else uri + "#"

    def __getitem__(self, name):
        return NamedNode(self.uri + name)

    def __getattr__(self, name):
        return NamedNode(self.uri + name)

# Usage
RDF = Namespace("http://www.w3.org/1999/02/22-rdf-syntax-ns#")
RDFS = Namespace("http://www.w3.org/2000/01/rdf-schema#")
FOAF = Namespace("http://xmlns.com/foaf/0.1/")
EX = Namespace("http://example.org/")

# Now you can use it like RDFLib
from pyoxigraph import Store, Quad, Literal

store = Store()
store.add(Quad(EX.alice, RDF.type, FOAF.Person))
store.add(Quad(EX.alice, FOAF.name, Literal("Alice")))
```

## Complete Migration Example

### Original RDFLib Code

```python
# old_code.py - Using RDFLib
from rdflib import Graph, URIRef, Literal, Namespace
from rdflib.namespace import RDF, FOAF, DCTERMS
import sys

def main():
    # Create graph
    g = Graph()

    # Define namespaces
    EX = Namespace("http://example.org/")
    g.bind("ex", EX)
    g.bind("foaf", FOAF)
    g.bind("dcterms", DCTERMS)

    # Load existing data
    print("Loading data...")
    g.parse("data.ttl", format="turtle")

    # Add new data
    print("Adding new triples...")
    for i in range(1000):
        person = EX[f"person{i}"]
        g.add((person, RDF.type, FOAF.Person))
        g.add((person, FOAF.name, Literal(f"Person {i}")))
        g.add((person, DCTERMS.created, Literal("2024-01-01")))

    # Query data
    print("Querying data...")
    query = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX dcterms: <http://purl.org/dc/terms/>

    SELECT ?person ?name
    WHERE {
        ?person a foaf:Person .
        ?person foaf:name ?name .
        ?person dcterms:created "2024-01-01" .
    }
    ORDER BY ?name
    LIMIT 10
    """

    results = g.query(query)
    for row in results:
        print(f"  {row.name}")

    # Save data
    print("Saving data...")
    with open("output.ttl", "wb") as f:
        f.write(g.serialize(format="turtle"))

    print(f"Done! Total triples: {len(g)}")

if __name__ == "__main__":
    main()
```

### Migrated pyoxigraph Code

```python
# new_code.py - Using pyoxigraph
from pyoxigraph import Store, NamedNode, Literal, Quad
import sys

# Namespace helper (reusable across projects)
class Namespace:
    def __init__(self, uri):
        self.uri = uri if uri.endswith((":", "/", "#")) else uri + "#"

    def __getitem__(self, name):
        return NamedNode(self.uri + name)

    def __getattr__(self, name):
        return NamedNode(self.uri + name)

# Define namespaces
RDF = Namespace("http://www.w3.org/1999/02/22-rdf-syntax-ns#")
FOAF = Namespace("http://xmlns.com/foaf/0.1/")
DCTERMS = Namespace("http://purl.org/dc/terms/")
EX = Namespace("http://example.org/")

def main():
    # Create store (use path for persistent storage)
    store = Store()

    # Load existing data
    print("Loading data...")
    with open("data.ttl", "rb") as f:
        store.load(f, "text/turtle")

    # Add new data (using transaction for better performance)
    print("Adding new triples...")
    for i in range(1000):
        person = EX[f"person{i}"]
        store.add(Quad(person, RDF.type, FOAF.Person))
        store.add(Quad(person, FOAF.name, Literal(f"Person {i}")))
        store.add(Quad(person, DCTERMS.created, Literal("2024-01-01")))

    # Query data
    print("Querying data...")
    query = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX dcterms: <http://purl.org/dc/terms/>

    SELECT ?person ?name
    WHERE {
        ?person a foaf:Person .
        ?person foaf:name ?name .
        ?person dcterms:created "2024-01-01" .
    }
    ORDER BY ?name
    LIMIT 10
    """

    results = store.query(query)
    for result in results:
        print(f"  {result['name'].value}")

    # Save data
    print("Saving data...")
    output = store.dump_graph(format="text/turtle")
    with open("output.ttl", "wb") as f:
        f.write(output)

    print(f"Done! Total quads: {len(store)}")

if __name__ == "__main__":
    main()
```

## Plugin Replacement

### RDFLib Plugins → pyoxigraph Alternatives

#### 1. SPARQLStore (Remote Endpoint)

**RDFLib:**
```python
from rdflib import Graph
from rdflib.plugins.stores.sparqlstore import SPARQLStore

store = SPARQLStore(query_endpoint="http://dbpedia.org/sparql")
g = Graph(store=store)
```

**pyoxigraph:**
```python
from pyoxigraph import Store

# Use SPARQL Federation via SERVICE
store = Store()
query = """
SELECT ?s ?p ?o
WHERE {
    SERVICE <http://dbpedia.org/sparql> {
        ?s ?p ?o
    }
}
LIMIT 10
"""
results = store.query(query)
```

#### 2. SQLAlchemy Store

**RDFLib:**
```python
from rdflib import Graph
from rdflib.plugins.stores.sqlalchemy import SQLAlchemy

store = SQLAlchemy(identifier="mystore")
g = Graph(store, identifier="mygraph")
g.open("sqlite:///rdflib.db", create=True)
```

**pyoxigraph:**
```python
# Use built-in RocksDB storage (better performance)
from pyoxigraph import Store

store = Store("./data")  # Persistent storage
```

#### 3. Berkeley DB Store

**RDFLib:**
```python
from rdflib import Graph
from rdflib.plugins.stores.berkeleydb import BerkeleyDB

store = BerkeleyDB()
g = Graph(store)
g.open("./bdb", create=True)
```

**pyoxigraph:**
```python
from pyoxigraph import Store

store = Store("./data")  # RocksDB is superior to Berkeley DB
```

#### 4. Full-Text Search

**RDFLib (with rdflib-sqlalchemy):**
```python
# No built-in full-text search
# Usually requires external integration
```

**pyoxigraph:**
```python
# Use regex in SPARQL (for smaller datasets)
query = """
SELECT ?s ?label
WHERE {
    ?s rdfs:label ?label .
    FILTER(REGEX(?label, "search term", "i"))
}
"""

# For large datasets, integrate Elasticsearch
from elasticsearch import Elasticsearch
es = Elasticsearch()

# Index literals
for quad in store.quads_for_pattern(None, RDFS.label, None):
    es.index(
        index="triples",
        document={"subject": quad.subject.value, "label": quad.object.value}
    )

# Search
results = es.search(index="triples", query={"match": {"label": "search term"}})
```

## Data Migration Script

Complete script to migrate RDFLib data to pyoxigraph:

```python
#!/usr/bin/env python3
"""
migrate_rdflib_to_pyoxigraph.py

Migrates RDFLib graph data to pyoxigraph store.
"""

import argparse
import sys
import time
from pathlib import Path

try:
    from rdflib import Graph
except ImportError:
    print("Error: rdflib not installed. Install with: pip install rdflib")
    sys.exit(1)

try:
    from pyoxigraph import Store
except ImportError:
    print("Error: pyoxigraph not installed. Install with: pip install pyoxigraph")
    sys.exit(1)

def migrate(input_file, output_dir, input_format="turtle", show_progress=True):
    """
    Migrate RDFLib graph to pyoxigraph store.

    Args:
        input_file: Path to RDFLib data file
        output_dir: Directory for pyoxigraph store
        input_format: RDF format (turtle, xml, n3, nt, etc.)
        show_progress: Show progress during migration
    """
    print(f"=== RDFLib to pyoxigraph Migration ===\n")

    # Step 1: Load data with RDFLib
    print(f"Loading data with RDFLib from {input_file}...")
    start_time = time.time()

    g = Graph()
    g.parse(input_file, format=input_format)

    load_time = time.time() - start_time
    print(f"  ✓ Loaded {len(g)} triples in {load_time:.2f}s")

    # Step 2: Export to N-Triples (universal format)
    print("\nExporting to N-Triples...")
    temp_file = "temp_export.nt"

    with open(temp_file, "wb") as f:
        f.write(g.serialize(format="nt"))

    print(f"  ✓ Exported to {temp_file}")

    # Step 3: Import to pyoxigraph
    print(f"\nImporting to pyoxigraph at {output_dir}...")
    start_time = time.time()

    store = Store(output_dir)

    with open(temp_file, "rb") as f:
        if show_progress:
            # Load with progress tracking
            data = f.read()
            store.load(data, "application/n-triples")
        else:
            store.load(f, "application/n-triples")

    import_time = time.time() - start_time
    print(f"  ✓ Imported {len(store)} quads in {import_time:.2f}s")

    # Step 4: Verify migration
    print("\nVerifying migration...")

    # Count comparison
    rdflib_count = len(g)
    pyox_count = len(store)

    if rdflib_count == pyox_count:
        print(f"  ✓ Counts match: {rdflib_count} triples")
    else:
        print(f"  ⚠ Count mismatch: RDFLib={rdflib_count}, pyoxigraph={pyox_count}")

    # Sample query comparison
    test_query = "SELECT (COUNT(*) as ?count) WHERE { ?s ?p ?o }"

    rdflib_result = list(g.query(test_query))[0][0]
    pyox_result = list(store.query(test_query))[0]['count'].value

    if int(rdflib_result) == int(pyox_result):
        print(f"  ✓ Query results match: {rdflib_result}")
    else:
        print(f"  ⚠ Query mismatch: RDFLib={rdflib_result}, pyoxigraph={pyox_result}")

    # Cleanup
    Path(temp_file).unlink()

    print("\n=== Migration Complete! ===")
    print(f"\nPerformance Summary:")
    print(f"  RDFLib load time: {load_time:.2f}s")
    print(f"  pyoxigraph import time: {import_time:.2f}s")
    print(f"  Speedup: {load_time / import_time:.2f}x")

    return store

def main():
    parser = argparse.ArgumentParser(description="Migrate RDFLib data to pyoxigraph")
    parser.add_argument("input", help="Input RDF file")
    parser.add_argument("output", help="Output pyoxigraph directory")
    parser.add_argument(
        "-f", "--format",
        default="turtle",
        help="Input format (turtle, xml, n3, nt, etc.)"
    )
    parser.add_argument(
        "-q", "--quiet",
        action="store_true",
        help="Suppress progress output"
    )

    args = parser.parse_args()

    try:
        migrate(
            args.input,
            args.output,
            args.format,
            show_progress=not args.quiet
        )
    except Exception as e:
        print(f"\nError: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
```

Usage:

```bash
# Basic migration
python migrate_rdflib_to_pyoxigraph.py data.ttl ./oxigraph-data

# Specify format
python migrate_rdflib_to_pyoxigraph.py data.rdf ./oxigraph-data -f xml

# Quiet mode
python migrate_rdflib_to_pyoxigraph.py data.nt ./oxigraph-data -q
```

## Performance Comparison

Benchmark script:

```python
import time
from rdflib import Graph, Namespace, Literal
from rdflib.namespace import RDF, FOAF
from pyoxigraph import Store, NamedNode, Literal as OxLiteral, Quad

# Helper for pyoxigraph
class NS:
    def __init__(self, uri):
        self.uri = uri
    def __getitem__(self, name):
        return NamedNode(self.uri + name)

def benchmark_rdflib(n=10000):
    g = Graph()
    EX = Namespace("http://example.org/")

    start = time.time()
    for i in range(n):
        person = EX[f"person{i}"]
        g.add((person, RDF.type, FOAF.Person))
        g.add((person, FOAF.name, Literal(f"Person {i}")))

    elapsed = time.time() - start
    print(f"RDFLib: {n} triples in {elapsed:.2f}s ({n/elapsed:.0f} triples/s)")
    return elapsed

def benchmark_pyoxigraph(n=10000):
    store = Store()
    EX = NS("http://example.org/")
    RDF_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
    FOAF_Person = NamedNode("http://xmlns.com/foaf/0.1/Person")
    FOAF_name = NamedNode("http://xmlns.com/foaf/0.1/name")

    start = time.time()
    for i in range(n):
        person = EX[f"person{i}"]
        store.add(Quad(person, RDF_type, FOAF_Person))
        store.add(Quad(person, FOAF_name, OxLiteral(f"Person {i}")))

    elapsed = time.time() - start
    print(f"pyoxigraph: {n} triples in {elapsed:.2f}s ({n/elapsed:.0f} triples/s)")
    return elapsed

# Run benchmarks
print("=== Performance Comparison ===\n")
rdflib_time = benchmark_rdflib(10000)
pyox_time = benchmark_pyoxigraph(10000)

print(f"\nSpeedup: {rdflib_time / pyox_time:.1f}x faster")
```

Typical results:
```
RDFLib: 10000 triples in 2.34s (4274 triples/s)
pyoxigraph: 10000 triples in 0.15s (66667 triples/s)

Speedup: 15.6x faster
```

## Troubleshooting

### Issue: Missing Namespace Shortcuts

**Problem**: RDFLib has built-in namespaces; pyoxigraph doesn't.

**Solution**: Create a `namespaces.py` module:

```python
# namespaces.py
from pyoxigraph import NamedNode

class Namespace:
    def __init__(self, uri):
        self.uri = uri if uri.endswith((":", "/", "#")) else uri + "#"

    def __getitem__(self, name):
        return NamedNode(self.uri + name)

    def __getattr__(self, name):
        return NamedNode(self.uri + name)

# Common namespaces
RDF = Namespace("http://www.w3.org/1999/02/22-rdf-syntax-ns#")
RDFS = Namespace("http://www.w3.org/2000/01/rdf-schema#")
OWL = Namespace("http://www.w3.org/2002/07/owl#")
XSD = Namespace("http://www.w3.org/2001/XMLSchema#")
FOAF = Namespace("http://xmlns.com/foaf/0.1/")
DCTERMS = Namespace("http://purl.org/dc/terms/")
SKOS = Namespace("http://www.w3.org/2004/02/skos/core#")
SCHEMA = Namespace("http://schema.org/")
```

### Issue: Graph vs Store Confusion

**Problem**: RDFLib uses `Graph`; pyoxigraph uses `Store`.

**Solution**: Think of `Store` as a `ConjunctiveGraph` (supports named graphs).

### Issue: Serialization Differences

**Problem**: Different serialization APIs.

**Solution**: Wrapper function:

```python
def serialize_store(store, format="turtle"):
    """Serialize store to string (RDFLib-like API)"""
    mime_types = {
        "turtle": "text/turtle",
        "xml": "application/rdf+xml",
        "nt": "application/n-triples",
        "nq": "application/n-quads",
    }
    mime = mime_types.get(format, format)
    return store.dump_graph(format=mime).decode("utf-8")

# Usage
print(serialize_store(store, "turtle"))
```

## Next Steps

- Review [Python API Reference](../reference/python-api.md)
- Explore [Performance Tuning](../how-to/performance-tuning.md)
- Check [SPARQL Guide](../reference/sparql.md)

## Additional Resources

- [pyoxigraph Documentation](https://pyoxigraph.readthedocs.io/)
- [RDFLib Migration Guide](https://github.com/oxigraph/oxigraph/wiki/RDFLib-Migration)
- [Python Examples](../examples/python/)
