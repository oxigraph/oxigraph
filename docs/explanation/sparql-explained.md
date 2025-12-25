# SPARQL Explained

This document explains SPARQL (SPARQL Protocol and RDF Query Language) - the standard query language for RDF data. We'll explore what SPARQL is, how it works, and how Oxigraph evaluates SPARQL queries.

## What is SPARQL?

SPARQL is to RDF what SQL is to relational databases - a powerful query language for retrieving and manipulating data. But while SQL works with tables, SPARQL works with graphs.

### The SQL Analogy

If you're familiar with SQL:

```sql
SELECT name, email
FROM users
WHERE age > 18
```

The SPARQL equivalent finds patterns in a graph:

```sparql
SELECT ?name ?email
WHERE {
    ?person schema:age ?age .
    ?person schema:name ?name .
    ?person schema:email ?email .
    FILTER (?age > 18)
}
```

### Why SPARQL Exists

SPARQL solves unique challenges in graph data:

1. **Flexible schemas**: Data doesn't need to fit rigid table structures
2. **Pattern matching**: Find complex relationships across the graph
3. **Open-world assumption**: Missing data is not an error
4. **Federated queries**: Query across multiple databases seamlessly
5. **Inference**: Derive new facts from existing data

## Query Types

SPARQL defines four main query types, each serving a different purpose.

### SELECT Queries

**Purpose**: Extract specific values from the graph.

**Returns**: A table of variable bindings.

**Example**:
```sparql
SELECT ?name ?email
WHERE {
    ?person rdf:type schema:Person .
    ?person schema:name ?name .
    ?person schema:email ?email .
}
```

**Result**:
```
+----------+---------------------+
| name     | email               |
+----------+---------------------+
| "Alice"  | "alice@example.com" |
| "Bob"    | "bob@example.com"   |
+----------+---------------------+
```

**Use cases**:
- Displaying data in applications
- Generating reports
- Extracting specific fields

**In Oxigraph**:
```rust
if let QueryResults::Solutions(solutions) = query_results {
    for solution in solutions {
        let name = solution?.get("name");
        println!("Name: {:?}", name);
    }
}
```

### CONSTRUCT Queries

**Purpose**: Create new RDF graphs from existing data.

**Returns**: A set of triples/quads.

**Example**:
```sparql
CONSTRUCT {
    ?person ex:fullProfile ?profile
}
WHERE {
    ?person schema:name ?name .
    ?person schema:email ?email .
    BIND(CONCAT(?name, " - ", ?email) AS ?profile)
}
```

**Result**: New triples you can store or process further.

**Use cases**:
- Transforming data between schemas
- Creating views or derived datasets
- Data integration and ETL

**In Oxigraph**:
```rust
if let QueryResults::Graph(triples) = query_results {
    for triple in triples {
        dataset.insert(&triple?);
    }
}
```

### ASK Queries

**Purpose**: Check if a pattern exists in the graph.

**Returns**: Boolean (true/false).

**Example**:
```sparql
ASK {
    ?person schema:name "Alice" .
    ?person schema:age ?age .
    FILTER (?age >= 18)
}
```

**Result**: `true` if Alice exists and is 18 or older, `false` otherwise.

**Use cases**:
- Access control checks
- Validation
- Conditional logic

**In Oxigraph**:
```rust
if let QueryResults::Boolean(exists) = query_results {
    if exists {
        println!("Pattern found!");
    }
}
```

### DESCRIBE Queries

**Purpose**: Get all information about a resource.

**Returns**: A graph describing the resource.

**Example**:
```sparql
DESCRIBE <http://example.com/person/alice>
```

**Result**: All triples where Alice is either subject or object.

**Use cases**:
- Resource exploration
- Debugging
- Building resource pages

## Graph Patterns: The Heart of SPARQL

### Basic Graph Patterns (BGP)

A **Basic Graph Pattern** is a set of triple patterns. Variables (starting with `?` or `$`) act as wildcards.

```sparql
WHERE {
    ?person rdf:type schema:Person .    # Triple pattern 1
    ?person schema:name ?name .         # Triple pattern 2
    ?person schema:age ?age .           # Triple pattern 3
}
```

This finds all combinations of `?person`, `?name`, and `?age` that satisfy ALL three patterns.

### How Pattern Matching Works

Think of it as a constraint satisfaction problem:

1. Start with the first pattern: find all matches
2. For each match, find solutions for the second pattern
3. Keep only combinations that satisfy all patterns
4. Return the surviving variable bindings

**Example**:

Given data:
```turtle
ex:alice rdf:type schema:Person ;
         schema:name "Alice" ;
         schema:age 30 .

ex:bob rdf:type schema:Person ;
       schema:name "Bob" .
```

Query:
```sparql
SELECT ?person ?age WHERE {
    ?person rdf:type schema:Person .
    ?person schema:age ?age .
}
```

**Step by step**:
1. First pattern matches: `ex:alice` and `ex:bob`
2. Second pattern: only `ex:alice` has an age
3. Result: One solution with `?person = ex:alice, ?age = 30`

### Optional Patterns

**OPTIONAL** makes patterns non-mandatory:

```sparql
SELECT ?person ?name ?email WHERE {
    ?person schema:name ?name .
    OPTIONAL {
        ?person schema:email ?email .
    }
}
```

This returns people with names, including their email *if available*. Missing email doesn't disqualify a result.

**Why this matters**: RDF follows the "open-world assumption" - absence of data doesn't mean it's false, just unknown.

### Union Patterns

**UNION** creates alternatives:

```sparql
SELECT ?person ?contact WHERE {
    ?person schema:name ?name .
    {
        ?person schema:email ?contact .
    } UNION {
        ?person schema:telephone ?contact .
    }
}
```

Returns people with their email OR phone (or both).

### Filter Patterns

**FILTER** adds constraints:

```sparql
SELECT ?person ?age WHERE {
    ?person schema:age ?age .
    FILTER (?age >= 18 && ?age < 65)
}
```

Only returns persons with age between 18 and 64.

**Filter expressions** can use:
- Comparison: `=`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Functions: `STRLEN`, `CONTAINS`, `REGEX`, etc.
- Existence: `EXISTS`, `NOT EXISTS`

### Subqueries

Nest queries within queries:

```sparql
SELECT ?person ?avgAge WHERE {
    ?person schema:age ?age .
    {
        SELECT (AVG(?age) AS ?avgAge) WHERE {
            ?p schema:age ?age .
        }
    }
    FILTER (?age > ?avgAge)
}
```

Finds people older than the average age.

### Property Paths

Navigate relationships of arbitrary length:

```sparql
# Find all ancestors (transitive knows relationship)
SELECT ?person ?ancestor WHERE {
    ?person schema:parent+ ?ancestor .
}
```

Path operators:
- `+` : One or more
- `*` : Zero or more
- `?` : Zero or one
- `{n,m}` : Between n and m times
- `/` : Sequence
- `|` : Alternative
- `^` : Inverse direction

## How Oxigraph Evaluates Queries

### The Evaluation Pipeline

```
SPARQL String
    ↓
[1. Parsing]
    ↓
Query Algebra (spargebra)
    ↓
[2. Optimization]
    ↓
Optimized Algebra (sparopt)
    ↓
[3. Evaluation]
    ↓
Query Results (spareval)
```

### 1. Parsing: String to Algebra

**Input**: SPARQL query string

**Process**: The `spargebra` crate parses the query into an abstract syntax tree (AST).

```rust
use spargebra::SparqlParser;

let query = SparqlParser::new()
    .parse_query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")
    .unwrap();
```

**Output**: Structured representation of the query

The algebra represents the query as data structures:
- `GraphPattern` for WHERE clauses
- `Expression` for filters and calculations
- `OrderExpression` for ORDER BY clauses

### 2. Optimization: Making Queries Fast

**Input**: Query algebra

**Process**: The `sparopt` crate rewrites the query to execute more efficiently.

**Optimizations include**:

#### Reordering Patterns
Place selective patterns first to reduce intermediate results:

```sparql
# Before optimization:
WHERE {
    ?person schema:name ?name .           # Matches many
    ?person schema:email "rare@test.com". # Matches few
}

# After optimization (conceptual):
WHERE {
    ?person schema:email "rare@test.com". # Start with selective pattern
    ?person schema:name ?name .
}
```

#### Filter Pushdown
Move filters as early as possible:

```sparql
# Before:
SELECT ?person WHERE {
    ?person schema:age ?age .
    ?person schema:name ?name .
} FILTER (?age > 18)

# After (conceptual):
SELECT ?person WHERE {
    ?person schema:age ?age .
    FILTER (?age > 18)  # Pushed down
    ?person schema:name ?name .
}
```

#### Join Elimination
Remove unnecessary joins when possible.

#### Common Subexpression Elimination
Avoid evaluating the same expression multiple times.

**Output**: Optimized algebra ready for execution

### 3. Evaluation: Executing the Query

**Input**: Optimized query algebra

**Process**: The `spareval` crate executes the query against the data.

**Evaluation strategies**:

#### Nested Loop Join (NLJ)
For each solution from the left pattern, find matching solutions from the right:

```
for left_solution in left_pattern_results {
    for right_solution in right_pattern_results {
        if compatible(left_solution, right_solution) {
            yield merge(left_solution, right_solution);
        }
    }
}
```

**Good for**: Small result sets

#### Hash Join
Build a hash table from one side, probe with the other:

```
hash_table = build_hash_table(left_pattern_results);
for right_solution in right_pattern_results {
    if (matching = hash_table.lookup(right_solution)) {
        yield merge(matching, right_solution);
    }
}
```

**Good for**: Large result sets with good join selectivity

#### Index Scans
Use database indexes to quickly find matching quads:

```sparql
WHERE { ?s schema:name "Alice" }
```

Oxigraph uses its indexes to directly find all quads with predicate `schema:name` and object `"Alice"`, rather than scanning all data.

### Oxigraph's Index-Based Evaluation

Oxigraph uses its multiple indexes (SPO, POS, OSP, etc.) to efficiently evaluate triple patterns:

**Example**: Pattern `?person schema:name ?name`
- Uses the POS index (Predicate-Object-Subject)
- Looks up predicate `schema:name`
- Returns all matching subject-object pairs

**Example**: Pattern `ex:alice ?p ?o`
- Uses the SPO index (Subject-Predicate-Object)
- Looks up subject `ex:alice`
- Returns all matching predicate-object pairs

The query evaluator chooses the most efficient index based on which parts of the pattern are bound.

## Query Results Formats

SPARQL defines multiple result formats:

### For SELECT and ASK

- **SPARQL JSON**: Widely used, good for web APIs
- **SPARQL XML**: Official W3C format
- **CSV**: Simple tabular format
- **TSV**: Tab-separated values

**Oxigraph support**:
```rust
use oxigraph::sparql::results::QueryResultsFormat;

let json_results = QueryResultsFormat::Json;
let xml_results = QueryResultsFormat::Xml;
```

### For CONSTRUCT and DESCRIBE

Results are RDF graphs, serialized in any RDF format:
- Turtle
- N-Triples
- RDF/XML
- JSON-LD

## Advanced Features

### Aggregation

Group and aggregate data:

```sparql
SELECT ?department (AVG(?salary) AS ?avgSalary) WHERE {
    ?person schema:department ?department .
    ?person schema:salary ?salary .
}
GROUP BY ?department
HAVING (AVG(?salary) > 50000)
```

**Aggregate functions**:
- `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`
- `GROUP_CONCAT`, `SAMPLE`

### Sorting and Limiting

```sparql
SELECT ?person ?age WHERE {
    ?person schema:age ?age .
}
ORDER BY DESC(?age)
LIMIT 10
OFFSET 20
```

**ORDER BY**: Sort results
**LIMIT**: Return only first N results
**OFFSET**: Skip first N results

### Federated Queries

Query multiple SPARQL endpoints:

```sparql
SELECT ?person ?friend WHERE {
    ?person schema:name "Alice" .
    SERVICE <http://remote-endpoint.com/sparql> {
        ?person foaf:knows ?friend .
    }
}
```

The `SERVICE` clause sends part of the query to a remote endpoint.

### Update Operations

SPARQL Update allows modifying data:

```sparql
# Insert new data
INSERT DATA {
    ex:alice schema:age 31 .
}

# Delete data
DELETE DATA {
    ex:alice schema:age 30 .
}

# Delete and insert (update)
DELETE {
    ?person schema:age ?oldAge .
}
INSERT {
    ?person schema:age ?newAge .
}
WHERE {
    ?person schema:name "Alice" .
    ?person schema:age ?oldAge .
    BIND(?oldAge + 1 AS ?newAge)
}
```

**Oxigraph support**:
```rust
use spargebra::SparqlParser;

let update = SparqlParser::new()
    .parse_update("INSERT DATA { <ex:alice> <ex:age> 31 }")
    .unwrap();

store.update(&update)?;
```

## Query Optimization Concepts

### Selectivity

**Selectivity** measures how much a pattern reduces the solution space.

**High selectivity** (good): Matches few results
```sparql
?person schema:email "rare@example.com" .
```

**Low selectivity** (bad for early evaluation): Matches many results
```sparql
?s rdf:type ?type .  # Almost everything has a type
```

The optimizer tries to evaluate high-selectivity patterns first.

### Cardinality Estimation

The optimizer estimates how many results each pattern will return to decide the best join order.

**Challenges**:
- Without statistics, estimation is hard
- RDF's open-world assumption makes it harder

**Oxigraph approach**: Uses heuristics based on bound variables and pattern structure.

### Cost-Based vs. Rule-Based

**Rule-based optimization** (Oxigraph's primary approach):
- Apply known good transformations
- Use heuristics (bound variables are selective)
- Fast, predictable

**Cost-based optimization**:
- Estimate cost of different plans
- Choose cheapest plan
- Requires statistics, more complex

## Common Patterns and Best Practices

### Start Specific, End General

```sparql
# Good: Start with specific pattern
SELECT ?email WHERE {
    ?person schema:name "Alice" .     # Specific
    ?person schema:email ?email .     # General
}

# Less efficient: Start with general pattern
SELECT ?name WHERE {
    ?person schema:email ?email .     # General
    ?person schema:name "Alice" .     # Specific
}
```

### Use OPTIONAL Wisely

```sparql
# Good: Required data first
SELECT ?name ?email WHERE {
    ?person schema:name ?name .
    OPTIONAL { ?person schema:email ?email }
}

# Bad: OPTIONAL on required data
SELECT ?name ?email WHERE {
    OPTIONAL { ?person schema:name ?name }
    ?person schema:email ?email .
}
```

### Prefer Property Paths

```sparql
# Instead of multiple patterns:
?person schema:parent ?p1 .
?p1 schema:parent ?p2 .
?p2 schema:parent ?grandparent .

# Use property path:
?person schema:parent/schema:parent/schema:parent ?grandparent .
```

### Use BIND for Calculations

```sparql
SELECT ?person ?nextAge WHERE {
    ?person schema:age ?currentAge .
    BIND(?currentAge + 1 AS ?nextAge)
}
```

### Limit Results When Exploring

```sparql
SELECT * WHERE {
    ?s ?p ?o .
} LIMIT 100
```

## Performance Considerations

### Query Complexity

**Fast queries**:
- Few triple patterns
- Specific values (bound variables)
- Use indexes effectively

**Slow queries**:
- Many triple patterns
- All variables (e.g., `?s ?p ?o`)
- Complex filters on string operations
- Large OPTIONAL or UNION clauses

### Index Usage

Oxigraph automatically chooses the best index. Understanding indexes helps write better queries:

- Pattern `ex:alice ?p ?o` → SPO index
- Pattern `?s schema:name ?o` → POS index
- Pattern `?s ?p "Alice"` → OSP index

### Result Streaming

Oxigraph streams results rather than materializing all at once:

```rust
for solution in solutions {
    // Process one at a time
    println!("{:?}", solution?);
}
```

This allows handling large result sets efficiently.

## Summary

SPARQL is a powerful pattern-matching query language for RDF:

- **Four query types**: SELECT, CONSTRUCT, ASK, DESCRIBE
- **Graph patterns**: Match complex structures in RDF graphs
- **Flexible**: Optional patterns, unions, filters, property paths
- **Optimized**: Query optimization improves performance

Oxigraph evaluates SPARQL through:
- **Parsing**: SPARQL → Algebra (spargebra)
- **Optimization**: Algebra → Optimized Algebra (sparopt)
- **Evaluation**: Execute against indexes (spareval)

Understanding SPARQL's pattern matching and evaluation helps you write efficient queries and build better applications on Oxigraph.
