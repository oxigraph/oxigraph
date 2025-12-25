# Oxigraph Architecture

This document provides an in-depth explanation of Oxigraph's internal architecture, including how it stores data, processes queries, and how the different crates work together. The goal is to help you build a mental model of how Oxigraph works under the hood.

## Architectural Overview

Oxigraph follows a layered architecture, with each layer building on the one below it:

```
┌─────────────────────────────────────────────────────────┐
│  Application Layer                                       │
│  (CLI, Python bindings, JavaScript bindings)            │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│  oxigraph Library                                        │
│  (Store, SPARQL execution, transactions)                │
└─────────────────────────────────────────────────────────┘
                         ↓
┌────────────────────┬──────────────────┬─────────────────┐
│  I/O Layer         │  SPARQL Stack    │  Validation     │
│  (oxrdfio)         │  (spargebra,     │  (sparshacl)    │
│                    │   spareval,      │                 │
│                    │   sparopt)       │                 │
└────────────────────┴──────────────────┴─────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│  RDF Data Model (oxrdf)                                  │
│  (Triple, Quad, NamedNode, Literal, etc.)               │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│  Storage Layer (RocksDB)                                 │
│  (Persistent key-value store with indexes)              │
└─────────────────────────────────────────────────────────┘
```

### Design Philosophy

Oxigraph is designed around several key principles:

1. **Modularity**: Each crate has a single, well-defined responsibility
2. **Safety**: Use Rust's type system to prevent invalid states
3. **Performance**: Optimize for fast queries on large datasets
4. **Compliance**: Implement W3C standards correctly
5. **Flexibility**: Support multiple deployment modes (library, server, embedded)

## Storage Layer

The storage layer is the foundation of Oxigraph, responsible for persisting RDF data efficiently and enabling fast queries.

### Why RocksDB?

RocksDB is a high-performance, embedded key-value store developed by Facebook. Oxigraph uses RocksDB because:

- **Embedded**: No separate server process needed
- **Fast**: Optimized for SSD storage with LSM-tree architecture
- **Reliable**: Crash recovery, backups, transactions
- **Flexible**: Column families for multiple indexes
- **Proven**: Battle-tested at massive scale

### The Encoding Challenge

RDF quads consist of four variable-length components (subject, predicate, object, graph). To store them in RocksDB (a key-value store), Oxigraph must:

1. **Encode** quads as binary keys
2. **Choose indexes** to enable efficient lookups
3. **Decode** keys back to quads when reading

### String Interning

Before encoding, Oxigraph interns all strings (IRIs, literals, language tags):

```
"http://schema.org/Person" → ID: 42
"Alice" → ID: 137
"en" → ID: 8
```

**Benefits**:
- **Space efficiency**: Each string stored once
- **Fast comparisons**: Compare integer IDs instead of strings
- **Smaller indexes**: IDs are fixed-size (typically 8-16 bytes)

**Implementation**:
- String → ID mapping in the `id2str` column family
- ID → String reverse mapping for retrieval

### Index Structure

To enable fast queries, Oxigraph maintains **9 indexes** (column families in RocksDB), each ordering quads differently:

#### Indexes for Named Graphs

1. **SPOG** (Subject-Predicate-Object-Graph)
2. **POSG** (Predicate-Object-Subject-Graph)
3. **OSPG** (Object-Subject-Predicate-Graph)
4. **GSPO** (Graph-Subject-Predicate-Object)
5. **GPOS** (Graph-Predicate-Object-Subject)
6. **GOSP** (Graph-Object-Subject-Predicate)

#### Indexes for Default Graph

7. **DSPO** (Default-Subject-Predicate-Object)
8. **DPOS** (Default-Predicate-Object-Subject)
9. **DOSP** (Default-Object-Subject-Predicate)

### Why Multiple Indexes?

Different query patterns need different indexes for efficiency:

**Example 1**: Find all properties and values for a subject
```sparql
SELECT ?p ?o WHERE { ex:alice ?p ?o }
```
→ Uses **SPOG** index (subject is fixed, scan predicate and object)

**Example 2**: Find all subjects with a specific predicate-object pair
```sparql
SELECT ?s WHERE { ?s schema:name "Alice" }
```
→ Uses **POSG** index (predicate and object are fixed, scan subjects)

**Example 3**: Find all triples in a specific named graph
```sparql
SELECT ?s ?p ?o FROM <http://graph1> WHERE { ?s ?p ?o }
```
→ Uses **GSPO** index (graph is fixed, scan all SPO combinations)

Without multiple indexes, some queries would require full scans of all data.

### Index Selection Algorithm

For a triple pattern like `?s schema:name ?o`, the evaluator:

1. **Counts bound variables**: 1 (predicate)
2. **Checks which variables are bound**: Predicate
3. **Selects best index**: POS (or POSG for named graphs)
4. **Scans the index**: All entries with predicate `schema:name`

The more variables are bound, the more selective the lookup.

### Storage Efficiency Trade-offs

**Write amplification**: Each quad is stored 3-9 times (once per index)
- **Cost**: More disk space, slower writes
- **Benefit**: Much faster reads

**Optimization**: Only maintain indexes actually needed
- Named graph indexes only created when using named graphs
- Default graph indexes optimized for common case

### Transactions and ACID Properties

Oxigraph provides ACID transactions through RocksDB:

**Atomicity**: All changes in a transaction succeed or fail together
```rust
let mut transaction = store.transaction()?;
transaction.insert(&quad1)?;
transaction.insert(&quad2)?;
transaction.commit()?; // Both or neither
```

**Consistency**: Indexes stay synchronized
- All indexes updated atomically
- No partial writes visible

**Isolation**: Repeatable read isolation level
- Queries see a consistent snapshot
- No dirty reads

**Durability**: Changes persist to disk
- Write-ahead log (WAL)
- Configurable fsync policies

### Memory vs. Disk

Oxigraph supports two storage modes:

#### Persistent Store (RocksDB)
```rust
let store = Store::open("/path/to/data")?;
```
- Data persists across restarts
- Scales to datasets larger than RAM
- Includes transactions, backups

#### In-Memory Store
```rust
let store = Store::new()?;
```
- Faster for small datasets
- No disk I/O overhead
- Used in JavaScript/WASM builds
- Good for testing

## SPARQL Processing Pipeline

When you execute a SPARQL query, it goes through multiple stages:

### Stage 1: Parsing (spargebra)

**Input**: SPARQL query string

**Process**: Lexical analysis and parsing using a hand-written parser

**Output**: Abstract Syntax Tree (AST)

```rust
pub struct Query {
    pub dataset: QueryDataset,
    pub algebra: GraphPattern,
    pub base_iri: Option<Iri<String>>,
}

pub enum GraphPattern {
    Bgp { patterns: Vec<TriplePattern> },
    Join { left: Box<GraphPattern>, right: Box<GraphPattern> },
    LeftJoin { left: Box<GraphPattern>, right: Box<GraphPattern>, expression: Option<Expression> },
    Filter { expr: Expression, inner: Box<GraphPattern> },
    Union { left: Box<GraphPattern>, right: Box<GraphPattern> },
    // ... more variants
}
```

**Why a custom parser?**
- SPARQL grammar is complex
- Need precise error messages
- Want zero-copy parsing where possible
- Performance critical

**Error handling**: Syntax errors include position information for debugging

### Stage 2: Optimization (sparopt)

**Input**: Parsed query algebra

**Process**: Apply rewrite rules to improve execution efficiency

**Output**: Optimized algebra

#### Optimization Rules

**1. Join Reordering**

Move selective patterns earlier:
```
Before: Join(Join(A, B), C)
  where A matches 1M, B matches 100, C matches 10K

After: Join(Join(B, A), C)
  (start with B which matches only 100)
```

**Heuristics**:
- Patterns with bound terms are more selective
- Constants are more selective than variables
- Start with smallest expected result set

**2. Filter Pushdown**

Evaluate filters as early as possible:
```
Before:
  Filter(
    Join(A, B),
    expression
  )

After:
  Join(
    Filter(A, expression),  # Filter on A first
    B
  )
```

**Benefit**: Reduce intermediate result sizes

**3. Optional Optimization**

Transform expensive OPTIONAL patterns:
```
Before:
  LeftJoin(A, B)  # B evaluated for every A solution

After:
  If B is expensive and A is small:
    Use hash join with B materialized
```

**4. Common Subexpression Elimination**

Avoid recomputing the same expression:
```
Before:
  Filter(?x = fn(?y) && ?z > fn(?y))

After:
  Let ?temp = fn(?y)
  Filter(?x = ?temp && ?z > ?temp)
```

**5. Projection Pushdown**

Only compute variables actually needed:
```
SELECT ?x WHERE {
  ?x :prop ?y .
  ?y :other ?z .
}

# Don't need to keep ?z after checking pattern
```

#### Limitations

**No statistics**: Oxigraph doesn't maintain statistics about data distribution
- Optimization is heuristic-based
- Some pathological cases can't be detected
- Future enhancement opportunity

**Cost model**: Simple rule-based rather than cost-based
- Fast optimization
- Predictable behavior
- Sometimes suboptimal plans

### Stage 3: Evaluation (spareval)

**Input**: Optimized query algebra

**Process**: Execute against the storage layer

**Output**: Query results (solutions, graph, or boolean)

#### Evaluation Strategies

**Basic Graph Pattern (BGP) Evaluation**

For a BGP with multiple triple patterns:

1. **Start with the first pattern** (hopefully most selective)
2. **For each solution, evaluate next pattern** with bindings
3. **Filter out incompatible combinations**
4. **Iterate until all patterns evaluated**

**Pseudocode**:
```
solutions = evaluate_pattern(patterns[0])
for pattern in patterns[1..]:
    new_solutions = []
    for solution in solutions:
        bindings = evaluate_pattern(pattern, solution)
        for binding in bindings:
            if compatible(solution, binding):
                new_solutions.append(merge(solution, binding))
    solutions = new_solutions
return solutions
```

**Join Evaluation**

Different strategies depending on join type:

**Nested Loop Join**:
```rust
for left in evaluate(left_pattern) {
    for right in evaluate(right_pattern, left.bindings()) {
        if compatible(left, right) {
            yield merge(left, right);
        }
    }
}
```
- Simple
- Good for small result sets
- Used for highly selective joins

**Hash Join**:
```rust
// Build phase
let mut hash_table = HashMap::new();
for left in evaluate(left_pattern) {
    hash_table.insert(left.key(), left);
}

// Probe phase
for right in evaluate(right_pattern) {
    if let Some(left) = hash_table.get(&right.key()) {
        if compatible(left, right) {
            yield merge(left, right);
        }
    }
}
```
- Better for larger result sets
- Requires memory for hash table
- Used for less selective joins

**OPTIONAL Evaluation**

Left join semantics:
```rust
for left in evaluate(left_pattern) {
    let mut matched = false;
    for right in evaluate(right_pattern, left.bindings()) {
        if compatible(left, right) {
            yield merge(left, right);
            matched = true;
        }
    }
    if !matched {
        yield left;  // Left without right
    }
}
```

**UNION Evaluation**

Concatenate results from alternatives:
```rust
for solution in evaluate(left_pattern) {
    yield solution;
}
for solution in evaluate(right_pattern) {
    yield solution;
}
```

**FILTER Evaluation**

Only keep solutions satisfying the expression:
```rust
for solution in evaluate(pattern) {
    if evaluate_expression(filter_expr, solution) {
        yield solution;
    }
}
```

#### Streaming Evaluation

Oxigraph uses **iterator-based evaluation**:
- Results produced lazily
- Low memory footprint
- Can process large result sets
- Enables early termination (LIMIT)

```rust
pub struct SolutionIter {
    inner: Box<dyn Iterator<Item = Result<Solution>>>,
}
```

Benefits:
- Handle millions of results without loading all into memory
- Start returning results before query fully completes
- LIMIT 10 only computes 10 results

#### Property Path Evaluation

Property paths require special handling:

**Fixed length path** `schema:parent{3}`:
- Expand to 3 joins
- Efficient

**Arbitrary length path** `schema:parent*`:
- Breadth-first search from starting nodes
- Track visited nodes to avoid cycles
- Can be expensive on large graphs

**Example implementation concept**:
```rust
fn evaluate_star_path(predicate, start_nodes) {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from(start_nodes);

    while let Some(node) = queue.pop_front() {
        if visited.insert(node) {
            yield node;
            let next = find_objects(node, predicate);
            queue.extend(next);
        }
    }
}
```

### Stage 4: Result Serialization

**Input**: Query results

**Process**: Format according to requested MIME type

**Output**: Serialized results

Formats handled by `sparesults` crate:
- SPARQL JSON (application/sparql-results+json)
- SPARQL XML (application/sparql-results+xml)
- CSV (text/csv)
- TSV (text/tab-separated-values)

For CONSTRUCT/DESCRIBE, uses RDF serializers from `oxrdfio`.

## Crate Organization and Dependencies

### Core Crates

#### oxrdf
**Purpose**: RDF data model types

**Key types**:
- `NamedNode`, `BlankNode`, `Literal`
- `Triple`, `Quad`
- `Graph`, `Dataset`

**Dependencies**: Minimal (oxiri, oxilangtag)

**Why separate?** Can be used independently of Oxigraph for RDF processing

#### oxigraph
**Purpose**: The database itself

**Key types**:
- `Store` - main entry point
- Transaction support
- Bulk loading

**Dependencies**:
- `oxrdf` - data model
- `oxrdfio` - parsing/serialization
- `spargebra`, `spareval`, `sparopt` - SPARQL
- `rocksdb` - storage (optional)

#### spargebra
**Purpose**: SPARQL parsing

**Key types**:
- `Query`, `Update`
- `GraphPattern`, `Expression`

**Dependencies**: `oxrdf`

**Why separate?** Useful for tools that need to parse but not execute SPARQL

#### sparopt
**Purpose**: Query optimization

**Input**: Query algebra from spargebra
**Output**: Optimized algebra

**Dependencies**: `spargebra`

#### spareval
**Purpose**: Query evaluation

**Key types**:
- `QueryEvaluator`
- `QueryResults`

**Dependencies**: `spargebra`, `sparopt`, `oxrdf`

**Why separate?** Can evaluate SPARQL against custom storage backends

#### oxrdfio
**Purpose**: Unified RDF I/O interface

**Key types**:
- `RdfParser`, `RdfSerializer`
- `RdfFormat` enum

**Dependencies**:
- `oxrdf` - data model
- `oxttl` - Turtle family
- `oxrdfxml` - RDF/XML
- `oxjsonld` - JSON-LD (optional)

#### oxttl
**Purpose**: Turtle, TriG, N-Triples, N-Quads parsing/serialization

**Why separate?** Widely used, self-contained

#### sparesults
**Purpose**: SPARQL results format handling

**Supports**: JSON, XML, CSV, TSV

### Dependency Graph

```
oxigraph
  ├─ oxrdf
  │   ├─ oxiri
  │   ├─ oxilangtag
  │   └─ oxsdatatypes
  ├─ oxrdfio
  │   ├─ oxrdf
  │   ├─ oxttl
  │   ├─ oxrdfxml
  │   └─ oxjsonld (optional)
  ├─ spargebra
  │   └─ oxrdf
  ├─ sparopt
  │   └─ spargebra
  ├─ spareval
  │   ├─ spargebra
  │   ├─ sparopt
  │   └─ oxrdf
  ├─ sparesults
  │   └─ oxrdf
  └─ rocksdb (optional)
```

### Language Bindings

#### Python (pyoxigraph)
Built with PyO3 and maturin:
- Wraps `oxigraph` Rust library
- Pythonic API
- Publishes to PyPI

#### JavaScript (oxigraph npm package)
Built with wasm-pack:
- Compiles to WebAssembly
- Runs in browsers and Node.js
- Uses in-memory storage (no RocksDB in WASM)

#### CLI (oxigraph-cli)
Command-line interface:
- SPARQL endpoint server
- Bulk loading tools
- Built on `oxigraph` library

## Design Decisions and Tradeoffs

### Storage: RocksDB vs. Alternatives

**Choice**: RocksDB

**Alternatives considered**:
- **LMDB**: Simpler but less flexible
- **Sled**: Pure Rust but less mature
- **Custom**: Too much work

**Tradeoffs**:
- ✅ Battle-tested, high performance
- ✅ Great tooling and documentation
- ❌ C++ dependency (complicates builds)
- ❌ Large binary size

### Multiple Indexes vs. Fewer Indexes

**Choice**: 9 indexes for comprehensive coverage

**Alternatives**:
- Fewer indexes: Requires more scans
- Index everything: Even more storage

**Tradeoffs**:
- ✅ Fast queries for all access patterns
- ✅ Predictable performance
- ❌ 3-9x storage overhead
- ❌ Slower writes

### String Interning

**Choice**: Intern all strings

**Alternatives**:
- Store strings inline: Huge waste
- Compression: Complex

**Tradeoffs**:
- ✅ Massive space savings
- ✅ Faster comparisons
- ❌ Extra indirection on reads
- ❌ Complicates deletion

### Streaming vs. Materialized Results

**Choice**: Iterator-based streaming

**Alternatives**:
- Materialize all results: Simple but memory-hungry
- Cursor-based: Stateful

**Tradeoffs**:
- ✅ Constant memory usage
- ✅ Fast time to first result
- ❌ Can't easily get count without consuming
- ❌ Results consumed once

### Modular Crate Design

**Choice**: Many small crates

**Alternatives**:
- Monolithic crate: Simpler
- Even more crates: Too granular

**Tradeoffs**:
- ✅ Reusable components
- ✅ Clear boundaries
- ✅ Easier testing
- ❌ More complex dependencies
- ❌ Potential version skew

### Type Safety

**Choice**: Extensive use of Rust's type system

**Examples**:
- Separate types for owned vs. borrowed
- Enums for RDF term types
- Result types for errors

**Tradeoffs**:
- ✅ Catch errors at compile time
- ✅ Clear API
- ✅ Performance (no runtime checks)
- ❌ More verbose
- ❌ Steeper learning curve

## Performance Characteristics

### Query Performance

**Strengths**:
- Fast lookups via indexes
- Streaming reduces memory
- Rust's zero-cost abstractions

**Weaknesses**:
- Large intermediate results can be slow
- No query statistics for optimization
- Property paths can be expensive

### Write Performance

**Bulk loading**: Use `bulk_loader()` for imports
- Optimized for large inserts
- Bypasses some overhead
- 10-100x faster than individual inserts

**Individual inserts**: Moderate speed
- Must update all indexes
- Transaction overhead

**Updates**: Slower than inserts
- Must delete old data
- Then insert new data

### Memory Usage

**Query execution**:
- Streaming: O(1) for result size
- Hash joins: O(n) for one side
- Property paths: O(visited nodes)

**Storage**:
- In-memory store: O(data size)
- RocksDB: Configurable caches, mostly on disk

### Scalability

**Dataset size**:
- Tested with billions of triples
- Limited by disk space
- RocksDB handles large key spaces well

**Concurrent access**:
- Multiple readers: Full parallelism
- Writers: Serialized (RocksDB limitation)
- Readers during writes: See snapshot

## Summary

Oxigraph's architecture is built on:

**Layered design**: Clean separation of concerns
- Storage layer (RocksDB with multiple indexes)
- Data model (oxrdf)
- SPARQL processing (parse, optimize, evaluate)
- I/O and serialization

**Performance through indexing**: 9 indexes cover all query patterns
- Fast lookups regardless of bound variables
- Storage overhead for query speed

**Modular crates**: Reusable, composable components
- Each crate has clear responsibility
- Can be used independently

**Streaming evaluation**: Iterator-based query execution
- Low memory footprint
- Fast time to first result

**Type safety**: Rust's type system prevents bugs
- Invalid RDF caught at compile time
- Clear API boundaries

Understanding this architecture helps you:
- Write efficient queries
- Debug performance issues
- Extend Oxigraph with new features
- Choose the right tools for your use case
