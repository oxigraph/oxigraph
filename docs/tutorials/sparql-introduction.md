# SPARQL Introduction

A beginner's guide to querying RDF data with SPARQL in Oxigraph.

## What is SPARQL?

SPARQL (SPARQL Protocol and RDF Query Language) is the standard query language for RDF graph databases. Think of it as SQL for graph data - it allows you to query, retrieve, and manipulate data stored in RDF format.

## Basic Concepts

### RDF Triples

RDF data is stored as **triples** consisting of:
- **Subject**: What you're talking about
- **Predicate**: The property or relationship
- **Object**: The value or related resource

Example triple:
```
<http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
```

This reads as: "Alice has the name 'Alice'".

### URIs and Prefixes

URIs (Uniform Resource Identifiers) uniquely identify resources. To make queries more readable, we use prefixes:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT * WHERE {
  ex:alice foaf:name ?name .
}
```

## Getting Started with Oxigraph

### Loading Data

First, let's create a store and add some data:

**Rust:**
```rust
use oxigraph::model::*;
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;

// Define some URIs
let ex = NamedNode::new("http://example.com/")?;
let foaf_name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
let foaf_knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
let alice = NamedNode::new("http://example.com/alice")?;
let bob = NamedNode::new("http://example.com/bob")?;

// Insert triples
store.insert(&Quad::new(
    alice.clone(),
    foaf_name.clone(),
    Literal::new_simple_literal("Alice"),
    GraphName::DefaultGraph,
))?;

store.insert(&Quad::new(
    bob.clone(),
    foaf_name.clone(),
    Literal::new_simple_literal("Bob"),
    GraphName::DefaultGraph,
))?;

store.insert(&Quad::new(
    alice.clone(),
    foaf_knows.clone(),
    bob.clone(),
    GraphName::DefaultGraph,
))?;
```

**JavaScript:**
```javascript
import { Store } from 'oxigraph';

const store = new Store();

store.add({
  subject: { termType: 'NamedNode', value: 'http://example.com/alice' },
  predicate: { termType: 'NamedNode', value: 'http://xmlns.com/foaf/0.1/name' },
  object: { termType: 'Literal', value: 'Alice' },
  graph: { termType: 'DefaultGraph' }
});

store.add({
  subject: { termType: 'NamedNode', value: 'http://example.com/bob' },
  predicate: { termType: 'NamedNode', value: 'http://xmlns.com/foaf/0.1/name' },
  object: { termType: 'Literal', value: 'Bob' },
  graph: { termType: 'DefaultGraph' }
});

store.add({
  subject: { termType: 'NamedNode', value: 'http://example.com/alice' },
  predicate: { termType: 'NamedNode', value: 'http://xmlns.com/foaf/0.1/knows' },
  object: { termType: 'NamedNode', value: 'http://example.com/bob' },
  graph: { termType: 'DefaultGraph' }
});
```

**Python:**
```python
from pyoxigraph import Store, NamedNode, Literal, Quad

store = Store()

alice = NamedNode("http://example.com/alice")
bob = NamedNode("http://example.com/bob")
foaf_name = NamedNode("http://xmlns.com/foaf/0.1/name")
foaf_knows = NamedNode("http://xmlns.com/foaf/0.1/knows")

store.add(Quad(alice, foaf_name, Literal("Alice")))
store.add(Quad(bob, foaf_name, Literal("Bob")))
store.add(Quad(alice, foaf_knows, bob))
```

## SPARQL Query Structure

A basic SPARQL query has this structure:

```sparql
PREFIX prefix: <namespace>

SELECT variables
WHERE {
  # Triple patterns
}
```

### Your First Query: SELECT

The `SELECT` query retrieves data matching a pattern:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
}
```

**Executing in Rust:**
```rust
let query = "
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
}";

if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()?
{
    for solution in solutions {
        let solution = solution?;
        println!("Person: {}, Name: {}",
            solution.get("person").unwrap(),
            solution.get("name").unwrap()
        );
    }
}
```

**Executing in JavaScript:**
```javascript
const results = store.query(`
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  SELECT ?person ?name
  WHERE {
    ?person foaf:name ?name .
  }
`);

for (const binding of results) {
  console.log(`Person: ${binding.get('person').value}, Name: ${binding.get('name').value}`);
}
```

**Executing in Python:**
```python
results = store.query("""
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  SELECT ?person ?name
  WHERE {
    ?person foaf:name ?name .
  }
""")

for solution in results:
    print(f"Person: {solution['person']}, Name: {solution['name']}")
```

## Variables and Bindings

### Variables

Variables in SPARQL start with `?` or `$`:

```sparql
SELECT ?subject ?predicate ?object
WHERE {
  ?subject ?predicate ?object .
}
```

This query returns all triples in the database.

### Selecting Specific Variables

You can select specific variables:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name
WHERE {
  ?person foaf:name ?name .
}
```

### SELECT *

Use `*` to select all variables:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT *
WHERE {
  ?person foaf:name ?name .
}
```

## Triple Patterns

Triple patterns are the building blocks of SPARQL queries. Each pattern can have variables in any position.

### Fixed Subject

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?predicate ?object
WHERE {
  ex:alice ?predicate ?object .
}
```

Returns all properties and values for Alice.

### Fixed Predicate

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
}
```

Returns all people and their names.

### Fixed Object

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person
WHERE {
  ?person foaf:name "Alice" .
}
```

Finds all people named "Alice".

### Multiple Patterns

Combine multiple patterns to create more complex queries:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?friend
WHERE {
  ?person foaf:name ?name .
  ?person foaf:knows ?friend .
}
```

This finds people, their names, and their friends.

### Sharing Variables

Variables shared across patterns create joins:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person1 ?person2
WHERE {
  ?person1 foaf:knows ?person2 .
  ?person2 foaf:name "Bob" .
}
```

Finds all people who know someone named Bob.

## FILTER Expressions

`FILTER` allows you to constrain query results based on conditions.

### String Filters

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER(STRLEN(?name) > 3)
}
```

Finds people with names longer than 3 characters.

### Comparison Operators

```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?age
WHERE {
  ?person ex:age ?age .
  FILTER(?age >= 18 && ?age < 65)
}
```

Supported operators: `=`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`

### String Matching

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER(STRSTARTS(?name, "A"))
}
```

Finds people whose names start with "A".

### REGEX for Pattern Matching

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?email
WHERE {
  ?person foaf:mbox ?email .
  FILTER(REGEX(?email, "@example\\.com$"))
}
```

Finds people with email addresses ending in @example.com.

## Built-in Functions

SPARQL provides many built-in functions for data manipulation.

### String Functions

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (UCASE(?name) AS ?uppercaseName)
WHERE {
  ?person foaf:name ?name .
}
```

Common string functions:
- `UCASE(?str)` - Convert to uppercase
- `LCASE(?str)` - Convert to lowercase
- `STRLEN(?str)` - String length
- `CONCAT(?str1, ?str2, ...)` - Concatenate strings
- `CONTAINS(?str, ?substring)` - Check if contains substring
- `STRSTARTS(?str, ?prefix)` - Check if starts with prefix
- `STRENDS(?str, ?suffix)` - Check if ends with suffix

### Numeric Functions

```sparql
PREFIX ex: <http://example.com/>

SELECT ?item ?price (ROUND(?price * 1.1) AS ?priceWithTax)
WHERE {
  ?item ex:price ?price .
}
```

Functions: `ABS`, `ROUND`, `CEIL`, `FLOOR`, `RAND`

### Type Checking

```sparql
SELECT ?s ?o
WHERE {
  ?s ?p ?o .
  FILTER(ISIRI(?o))
}
```

Functions: `ISIRI`, `ISBLANK`, `ISLITERAL`, `ISNUMERIC`, `BOUND`

## Query Forms

SPARQL has four query forms:

### SELECT

Returns variable bindings (shown above).

### ASK

Returns true/false if a pattern exists:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

ASK {
  ex:alice foaf:knows ex:bob .
}
```

**Rust:**
```rust
if let QueryResults::Boolean(exists) = SparqlEvaluator::new()
    .parse_query("ASK { <http://example.com/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.com/bob> }")?
    .on_store(&store)
    .execute()?
{
    println!("Alice knows Bob: {}", exists);
}
```

### CONSTRUCT

Creates new RDF triples from query results:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

CONSTRUCT {
  ?person ex:hasName ?name .
}
WHERE {
  ?person foaf:name ?name .
}
```

This transforms `foaf:name` predicates into `ex:hasName`.

### DESCRIBE

Returns RDF data about a resource:

```sparql
PREFIX ex: <http://example.com/>

DESCRIBE ex:alice
```

Returns all triples where Alice is the subject.

## Modifiers and Solution Sequences

### DISTINCT

Remove duplicate results:

```sparql
SELECT DISTINCT ?name
WHERE {
  ?person <http://xmlns.com/foaf/0.1/name> ?name .
}
```

### ORDER BY

Sort results:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
}
ORDER BY ?name
```

Order descending:
```sparql
ORDER BY DESC(?name)
```

### LIMIT and OFFSET

Paginate results:

```sparql
SELECT ?person ?name
WHERE {
  ?person <http://xmlns.com/foaf/0.1/name> ?name .
}
ORDER BY ?name
LIMIT 10
OFFSET 0
```

Gets the first 10 results.

## Practical Examples

### Example 1: Finding Friends of Friends

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?fof ?fofName
WHERE {
  ex:alice foaf:knows ?friend .
  ?friend foaf:knows ?fof .
  ?fof foaf:name ?fofName .
  FILTER(?fof != ex:alice)  # Exclude Alice herself
}
```

### Example 2: Counting Relationships

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (COUNT(?friend) AS ?friendCount)
WHERE {
  ?person foaf:knows ?friend .
}
GROUP BY ?person
```

### Example 3: Optional Information

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
}
```

Returns people with their names and email addresses (if available).

### Example 4: Combining Filters

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?age
WHERE {
  ?person foaf:name ?name .
  ?person ex:age ?age .
  FILTER(?age >= 18 && STRLEN(?name) >= 3)
}
ORDER BY DESC(?age)
LIMIT 10
```

## Best Practices

1. **Use Prefixes**: Make queries more readable with namespace prefixes
2. **Be Specific**: Add as many constraints as possible for better performance
3. **Filter Early**: Place filters close to the patterns they constrain
4. **Limit Results**: Use `LIMIT` during development to avoid overwhelming results
5. **Use DISTINCT**: Remove duplicates when joining multiple patterns
6. **Index on Variables**: Put most selective patterns first

## Common Pitfalls

### Cartesian Products

Be careful with disconnected patterns:

```sparql
# BAD: Creates a cartesian product
SELECT ?name1 ?name2
WHERE {
  ?person1 foaf:name ?name1 .
  ?person2 foaf:name ?name2 .
}
```

Connect patterns with shared variables:

```sparql
# GOOD: Connected via ?person1 knows ?person2
SELECT ?name1 ?name2
WHERE {
  ?person1 foaf:name ?name1 .
  ?person1 foaf:knows ?person2 .
  ?person2 foaf:name ?name2 .
}
```

### Unbound Variables in FILTER

```sparql
# BAD: ?age might be unbound
FILTER(?age > 18)
```

Use BOUND to check:

```sparql
# GOOD
FILTER(BOUND(?age) && ?age > 18)
```

Or use OPTIONAL properly:

```sparql
OPTIONAL { ?person ex:age ?age }
FILTER(!BOUND(?age) || ?age > 18)
```

## Next Steps

Now that you understand the basics, explore:

- [Advanced SPARQL Queries](../how-to/sparql-advanced-queries.md) - OPTIONAL, UNION, aggregation, property paths
- [SPARQL Functions Reference](../reference/sparql-functions.md) - Complete list of supported functions
- [SPARQL Updates](../how-to/sparql-updates.md) - Modifying data with INSERT and DELETE

## Resources

- [SPARQL 1.1 Query Language Specification](https://www.w3.org/TR/sparql11-query/)
- [Oxigraph Documentation](https://oxigraph.org/)
- [RDF 1.1 Primer](https://www.w3.org/TR/rdf11-primer/)
