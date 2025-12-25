# SPARQL Support Reference

This document details Oxigraph's SPARQL implementation, including query and update support, built-in functions, and extensions.

## SPARQL Standards Support

### SPARQL 1.1 (Default)

Oxigraph implements the complete SPARQL 1.1 specification suite:

| Specification | Status | W3C Recommendation |
|---------------|--------|-------------------|
| SPARQL 1.1 Query | Full | [TR](https://www.w3.org/TR/sparql11-query/) |
| SPARQL 1.1 Update | Full | [TR](https://www.w3.org/TR/sparql11-update/) |
| SPARQL 1.1 Federated Query | Full | [TR](https://www.w3.org/TR/sparql11-federated-query/) |
| SPARQL 1.1 Service Description | Partial | [TR](https://www.w3.org/TR/sparql11-service-description/) |
| SPARQL 1.1 Protocol | Full (via HTTP server) | [TR](https://www.w3.org/TR/sparql11-protocol/) |

### SPARQL 1.2 (Feature Flag)

Enable SPARQL 1.2 support with the `sparql-12` feature flag:

```toml
[dependencies]
oxigraph = { version = "*", features = ["sparql-12"] }
# Or for specific crates:
spargebra = { version = "*", features = ["sparql-12"] }
spareval = { version = "*", features = ["sparql-12"] }
```

**Status**: Experimental (following [SPARQL 1.2 Working Drafts](https://www.w3.org/TR/sparql12-query/))

---

## SPARQL Query Forms

### SELECT Queries

Return tabular results with variable bindings.

**Example**:
```sparql
SELECT ?person ?name WHERE {
    ?person a schema:Person ;
            schema:name ?name .
}
```

**Result Format**: Solutions sequence (table of variable bindings)

**Supported Features**:
- DISTINCT / REDUCED modifiers
- ORDER BY, LIMIT, OFFSET
- Aggregates (COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT, SAMPLE)
- GROUP BY, HAVING
- Subqueries

---

### CONSTRUCT Queries

Return an RDF graph constructed from template.

**Example**:
```sparql
CONSTRUCT {
    ?person foaf:name ?name .
} WHERE {
    ?person schema:name ?name .
}
```

**Result Format**: RDF graph (triples or quads)

**Use Cases**:
- Data transformation
- Schema mapping
- Graph extraction

---

### ASK Queries

Return boolean result indicating pattern presence.

**Example**:
```sparql
ASK {
    ?person a schema:Person ;
            schema:name "Alice" .
}
```

**Result Format**: Boolean (true/false)

**Use Cases**:
- Existence checks
- Validation
- Conditional logic

---

### DESCRIBE Queries

Return RDF data describing resources.

**Example**:
```sparql
DESCRIBE <http://example.org/Alice>
```

**Result Format**: RDF graph

**Behavior**: Implementation-specific (Oxigraph returns CBD - Concise Bounded Description)

---

## SPARQL Update Operations

### INSERT DATA

Insert concrete triples/quads into the store.

**Example**:
```sparql
INSERT DATA {
    <http://example.org/Alice> a schema:Person ;
                                schema:name "Alice" .
}
```

---

### DELETE DATA

Delete concrete triples/quads from the store.

**Example**:
```sparql
DELETE DATA {
    <http://example.org/Alice> schema:age 29 .
}
```

---

### INSERT/DELETE WHERE

Modify data based on pattern matching.

**Example**:
```sparql
DELETE { ?person schema:age ?oldAge }
INSERT { ?person schema:age ?newAge }
WHERE {
    ?person schema:age ?oldAge .
    BIND(?oldAge + 1 AS ?newAge)
}
```

---

### LOAD

Load RDF data from URI into graph.

**Example**:
```sparql
LOAD <http://example.org/data.ttl> INTO GRAPH <http://example.org/graph>
```

**Note**: Requires `http-client` feature flag

---

### CLEAR / DROP / CREATE

Graph management operations.

**Examples**:
```sparql
CLEAR GRAPH <http://example.org/graph>
DROP GRAPH <http://example.org/graph>
CREATE GRAPH <http://example.org/graph>
```

---

### COPY / MOVE / ADD

Graph content operations.

**Examples**:
```sparql
COPY <http://example.org/graph1> TO <http://example.org/graph2>
MOVE <http://example.org/graph1> TO <http://example.org/graph2>
ADD <http://example.org/graph1> TO <http://example.org/graph2>
```

---

## SPARQL Query Features

### Graph Patterns

#### Basic Graph Patterns (BGP)

```sparql
SELECT * WHERE {
    ?person a schema:Person .
    ?person schema:name ?name .
    ?person schema:age ?age .
}
```

#### Optional Patterns

```sparql
SELECT * WHERE {
    ?person a schema:Person .
    OPTIONAL { ?person schema:email ?email }
}
```

#### Union Patterns

```sparql
SELECT * WHERE {
    { ?x a schema:Person }
    UNION
    { ?x a schema:Organization }
}
```

#### Graph Patterns (Named Graphs)

```sparql
SELECT * WHERE {
    GRAPH <http://example.org/graph> {
        ?s ?p ?o .
    }
}
```

#### Filters

```sparql
SELECT * WHERE {
    ?person schema:age ?age .
    FILTER(?age >= 18)
}
```

---

### Property Paths

| Path Type | Syntax | Example |
|-----------|--------|---------|
| Predicate | `<uri>` | `?x foaf:knows ?y` |
| Inverse | `^<uri>` | `?x ^foaf:knows ?y` |
| Sequence | `/` | `?x foaf:knows/foaf:name ?name` |
| Alternative | `\|` | `?x foaf:name\|schema:name ?name` |
| Zero or more | `*` | `?x foaf:knows* ?y` |
| One or more | `+` | `?x foaf:knows+ ?y` |
| Zero or one | `?` | `?x foaf:knows? ?y` |
| Negated | `!` | `?x !rdf:type ?o` |

**Example**:
```sparql
SELECT ?ancestor WHERE {
    <http://example.org/Alice> foaf:knows+ ?ancestor .
}
```

---

### Solution Modifiers

#### ORDER BY

```sparql
SELECT * WHERE { ?s ?p ?o }
ORDER BY DESC(?s) ?p
```

#### DISTINCT / REDUCED

```sparql
SELECT DISTINCT ?type WHERE {
    ?s a ?type .
}
```

#### LIMIT / OFFSET

```sparql
SELECT * WHERE { ?s ?p ?o }
LIMIT 10
OFFSET 20
```

---

### Aggregates

Supported aggregate functions:

| Function | Description | Example |
|----------|-------------|---------|
| COUNT | Count results | `COUNT(*)`, `COUNT(?var)` |
| SUM | Sum numeric values | `SUM(?amount)` |
| AVG | Average of values | `AVG(?age)` |
| MIN | Minimum value | `MIN(?price)` |
| MAX | Maximum value | `MAX(?score)` |
| GROUP_CONCAT | Concatenate strings | `GROUP_CONCAT(?name; SEPARATOR=", ")` |
| SAMPLE | Arbitrary value | `SAMPLE(?value)` |

**Example**:
```sparql
SELECT ?type (COUNT(*) AS ?count) WHERE {
    ?s a ?type .
}
GROUP BY ?type
HAVING (COUNT(*) > 10)
ORDER BY DESC(?count)
```

---

### Subqueries

```sparql
SELECT ?person ?avgAge WHERE {
    ?person a schema:Person .
    {
        SELECT (AVG(?age) AS ?avgAge) WHERE {
            ?p schema:age ?age .
        }
    }
}
```

---

### VALUES Clause

```sparql
SELECT * WHERE {
    ?person schema:name ?name .
    VALUES ?name { "Alice" "Bob" }
}
```

---

### BIND Clause

```sparql
SELECT ?person ?fullName WHERE {
    ?person schema:givenName ?first ;
            schema:familyName ?last .
    BIND(CONCAT(?first, " ", ?last) AS ?fullName)
}
```

---

### Negation

#### MINUS

```sparql
SELECT ?person WHERE {
    ?person a schema:Person .
    MINUS { ?person schema:email ?email }
}
```

#### NOT EXISTS / EXISTS

```sparql
SELECT ?person WHERE {
    ?person a schema:Person .
    FILTER NOT EXISTS { ?person schema:email ?email }
}
```

---

## SPARQL Federated Query (SERVICE)

Query remote SPARQL endpoints.

**Example**:
```sparql
SELECT * WHERE {
    ?person a schema:Person .
    SERVICE <http://dbpedia.org/sparql> {
        ?person owl:sameAs ?dbpediaUri .
    }
}
```

**Requirements**:
- Enable `http-client` feature flag
- Network access to remote endpoint
- Remote endpoint must be accessible

**Configuration**:
```toml
[dependencies]
oxigraph = { version = "*", features = ["http-client"] }
```

---

## Built-in Functions

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| STRLEN | String length | `STRLEN("hello")` → 5 |
| SUBSTR | Substring | `SUBSTR("hello", 2, 3)` → "ell" |
| UCASE | Uppercase | `UCASE("hello")` → "HELLO" |
| LCASE | Lowercase | `LCASE("HELLO")` → "hello" |
| STRSTARTS | Starts with | `STRSTARTS("hello", "he")` → true |
| STRENDS | Ends with | `STRENDS("hello", "lo")` → true |
| CONTAINS | Contains substring | `CONTAINS("hello", "ell")` → true |
| STRBEFORE | String before | `STRBEFORE("hello world", " ")` → "hello" |
| STRAFTER | String after | `STRAFTER("hello world", " ")` → "world" |
| CONCAT | Concatenate | `CONCAT("hello", " ", "world")` → "hello world" |
| REPLACE | Replace substring | `REPLACE("hello", "l", "r")` → "herro" |
| REGEX | Regular expression | `REGEX("test", "^t.*t$")` → true |

---

### Numeric Functions

| Function | Description | Example |
|----------|-------------|---------|
| ABS | Absolute value | `ABS(-5)` → 5 |
| ROUND | Round to integer | `ROUND(3.7)` → 4 |
| CEIL | Ceiling | `CEIL(3.2)` → 4 |
| FLOOR | Floor | `FLOOR(3.7)` → 3 |
| RAND | Random number | `RAND()` → 0.0 to 1.0 |

---

### Date/Time Functions

| Function | Description | Example |
|----------|-------------|---------|
| NOW | Current datetime | `NOW()` |
| YEAR | Extract year | `YEAR("2024-01-15"^^xsd:date)` → 2024 |
| MONTH | Extract month | `MONTH("2024-01-15"^^xsd:date)` → 1 |
| DAY | Extract day | `DAY("2024-01-15"^^xsd:date)` → 15 |
| HOURS | Extract hours | `HOURS("12:30:00"^^xsd:time)` → 12 |
| MINUTES | Extract minutes | `MINUTES("12:30:00"^^xsd:time)` → 30 |
| SECONDS | Extract seconds | `SECONDS("12:30:45"^^xsd:time)` → 45 |
| TIMEZONE | Extract timezone | `TIMEZONE(NOW())` |
| TZ | Timezone string | `TZ(NOW())` |

---

### Hash Functions

| Function | Description | Example |
|----------|-------------|---------|
| MD5 | MD5 hash | `MD5("hello")` |
| SHA1 | SHA-1 hash | `SHA1("hello")` |
| SHA256 | SHA-256 hash | `SHA256("hello")` |
| SHA512 | SHA-512 hash | `SHA512("hello")` |

---

### RDF Term Functions

| Function | Description | Example |
|----------|-------------|---------|
| STR | Convert to string | `STR(<http://example.org/>)` |
| LANG | Language tag | `LANG("hello"@en)` → "en" |
| DATATYPE | Datatype IRI | `DATATYPE("5"^^xsd:integer)` |
| IRI / URI | Create IRI | `IRI("http://example.org/")` |
| BNODE | Create blank node | `BNODE()` |
| STRDT | Create typed literal | `STRDT("5", xsd:integer)` |
| STRLANG | Create lang-tagged literal | `STRLANG("hello", "en")` |
| UUID | Generate UUID IRI | `UUID()` |
| STRUUID | Generate UUID string | `STRUUID()` |

---

### Type Checking Functions

| Function | Description | Example |
|----------|-------------|---------|
| isIRI / isURI | Check if IRI | `isIRI(<http://example.org/>)` → true |
| isBlank | Check if blank node | `isBlank(_:b1)` → true |
| isLiteral | Check if literal | `isLiteral("hello")` → true |
| isNumeric | Check if numeric | `isNumeric(5)` → true |

---

### Logical Functions

| Function | Description | Example |
|----------|-------------|---------|
| BOUND | Check if variable bound | `BOUND(?var)` |
| IF | Conditional | `IF(?x > 5, "large", "small")` |
| COALESCE | First non-error value | `COALESCE(?email, ?phone, "N/A")` |
| sameTerm | Same RDF term | `sameTerm(?x, ?y)` |

---

## SPARQL Extensions

### GeoSPARQL (Optional)

Enable with `geosparql` feature flag (JavaScript bindings):

```javascript
// In JavaScript/WebAssembly build
// Enabled with geosparql feature
```

**Supported Functions**: Partial implementation of [GeoSPARQL](https://docs.ogc.org/is/22-047r1/22-047r1.html)

**Status**: Work in progress, slow, partial coverage

---

### SPARQL Enhancement Proposals (SEPs)

#### SEP-0002: Temporal Functions

Enable with `sep-0002` feature flag in `spareval`:

**Features**:
- `ADJUST` function for timezone adjustment
- Arithmetic on date/time types:
  - `xsd:date`
  - `xsd:time`
  - `xsd:yearMonthDuration`
  - `xsd:dayTimeDuration`

**Example**:
```sparql
SELECT ?date ?adjusted WHERE {
    BIND("2024-01-15"^^xsd:date AS ?date)
    BIND(ADJUST(?date, "PT5H"^^xsd:dayTimeDuration) AS ?adjusted)
}
```

---

#### SEP-0006: LATERAL Joins

Enable with `sep-0006` feature flag in `spareval`:

**Feature**: `LATERAL` keyword for dependent joins

**Example**:
```sparql
SELECT * WHERE {
    ?person schema:name ?name .
    LATERAL {
        ?person schema:friend ?friend .
        FILTER(?friend > ?person)
    }
}
```

---

#### Calendar Extensions

Enable with `calendar-ext` feature flag in `spareval`:

**Feature**: Arithmetic on additional date types:
- `xsd:gYear`
- `xsd:gYearMonth`
- `xsd:gMonth`
- `xsd:gMonthDay`
- `xsd:gDay`

---

## Query Results Formats

Oxigraph supports all standard SPARQL results formats:

| Format | MIME Type | Extension | Use Case |
|--------|-----------|-----------|----------|
| XML | `application/sparql-results+xml` | `.srx` | Legacy systems |
| JSON | `application/sparql-results+json` | `.srj` | Web APIs |
| CSV | `text/csv` | `.csv` | Spreadsheets |
| TSV | `text/tab-separated-values` | `.tsv` | Data processing |

### Format Details

#### SPARQL Results XML
- W3C Standard: [TR](https://www.w3.org/TR/rdf-sparql-XMLres/)
- Full support for boolean and solutions
- XML-based systems

#### SPARQL Results JSON
- W3C Standard: [TR](https://www.w3.org/TR/sparql11-results-json/)
- Most common for web applications
- Easy JavaScript integration

#### SPARQL Results CSV/TSV
- W3C Standard: [TR](https://www.w3.org/TR/sparql11-results-csv-tsv/)
- Spreadsheet compatibility
- Simple data exchange

---

## Query Optimization

Oxigraph includes a SPARQL optimizer (`sparopt` crate) that:

- Rewrites queries for better performance
- Eliminates redundant operations
- Optimizes join order
- Simplifies expressions

**Note**: Optimizer preserves query semantics but may discard some errors

**Configuration**: Optimization is automatic; no configuration needed

---

## Query Execution

### Store API (Rust)

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;
let evaluator = SparqlEvaluator::new();
let results = evaluator
    .parse_query("SELECT * WHERE { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?;

match results {
    QueryResults::Solutions(mut solutions) => {
        for solution in solutions {
            println!("{:?}", solution?);
        }
    }
    QueryResults::Graph(triples) => {
        for triple in triples {
            println!("{:?}", triple?);
        }
    }
    QueryResults::Boolean(result) => {
        println!("Result: {}", result);
    }
}
```

---

### Python API

```python
from pyoxigraph import Store

store = Store()
results = store.query("SELECT * WHERE { ?s ?p ?o }")

for solution in results:
    print(solution)
```

---

### JavaScript API

```javascript
const store = new Store();
const results = store.query("SELECT * WHERE { ?s ?p ?o }");

for (const solution of results) {
    console.log(solution);
}
```

---

## Query Options

### Union Default Graph

Treat all named graphs as part of the default graph:

**CLI**:
```bash
oxigraph serve --union-default-graph
oxigraph query --union-default-graph
```

**Rust API**: Set via `SparqlEvaluator` options

**Effect**: Makes `{ ?s ?p ?o }` query across all graphs

---

### Query Timeout

Set timeout for query execution:

**CLI**:
```bash
oxigraph serve --timeout-s 30
```

**Effect**: Queries exceeding timeout will be terminated

---

### Base IRI

Set base IRI for relative IRIs in queries:

**CLI**:
```bash
oxigraph query --query-base http://example.org/
```

**SPARQL**:
```sparql
BASE <http://example.org/>
SELECT * WHERE { <alice> ?p ?o }
```

---

### Prefixes

Define namespace prefixes:

**SPARQL**:
```sparql
PREFIX schema: <http://schema.org/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT * WHERE {
    ?person a schema:Person ;
            foaf:name ?name .
}
```

---

## Performance Considerations

### Indexing

Oxigraph maintains three RDF indexes:
- SPO (Subject-Predicate-Object)
- POS (Predicate-Object-Subject)
- OSP (Object-Subject-Predicate)

These indexes enable efficient query evaluation for different patterns.

### Query Planning

- Oxigraph automatically selects optimal index based on query pattern
- Filters are pushed down when possible
- Joins are reordered for efficiency

### Best Practices

1. **Use LIMIT**: For large result sets, use LIMIT to avoid memory issues
2. **Selective Filters**: Apply filters early to reduce intermediate results
3. **Avoid SELECT ***: Select only needed variables
4. **Property Paths**: Use carefully; can be expensive on large graphs
5. **Aggregates**: Use GROUP BY to reduce data before aggregation

---

## References

- [SPARQL 1.1 Overview](https://www.w3.org/TR/sparql11-overview/)
- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)
- [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/)
- [SPARQL 1.2 Query (Draft)](https://www.w3.org/TR/sparql12-query/)
- [Oxigraph SPARQL Documentation](https://docs.rs/oxigraph/latest/oxigraph/sparql/)
