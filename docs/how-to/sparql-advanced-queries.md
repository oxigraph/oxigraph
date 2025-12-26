# Advanced SPARQL Queries

This guide covers advanced SPARQL query patterns and techniques for complex data retrieval in Oxigraph.

## Table of Contents

- [OPTIONAL Patterns](#optional-patterns)
- [UNION Queries](#union-queries)
- [Subqueries](#subqueries)
- [Aggregation](#aggregation)
- [Property Paths](#property-paths)
- [Federated Queries (SERVICE)](#federated-queries-service)
- [BIND and VALUES](#bind-and-values)
- [Named Graphs](#named-graphs)
- [Negation (FILTER NOT EXISTS, MINUS)](#negation)

## OPTIONAL Patterns

Use `OPTIONAL` to include information when available without requiring it to match.

### Basic OPTIONAL

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
}
```

This returns all people with names, and includes email addresses when they exist.

### Multiple OPTIONAL Blocks

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?email ?phone
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
  OPTIONAL { ?person ex:phone ?phone }
}
```

Each `OPTIONAL` is independent - results include people with names, optionally with emails and/or phones.

### Nested OPTIONAL

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?friend ?friendEmail
WHERE {
  ?person foaf:name ?name .
  OPTIONAL {
    ?person foaf:knows ?friend .
    OPTIONAL { ?friend foaf:mbox ?friendEmail }
  }
}
```

### OPTIONAL with FILTER

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?age
WHERE {
  ?person foaf:name ?name .
  OPTIONAL {
    ?person ex:age ?age .
    FILTER(?age >= 18)
  }
}
```

Returns all people with names, and age only if they're 18 or older.

### Practical Example: Profile with Optional Fields

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;

let query = r#"
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?email ?homepage ?bio
WHERE {
  ?person a foaf:Person ;
          foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
  OPTIONAL { ?person foaf:homepage ?homepage }
  OPTIONAL { ?person ex:bio ?bio }
}
ORDER BY ?name
"#;

if let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()?
{
    for solution in solutions {
        let sol = solution?;
        println!("Name: {}", sol.get("name").unwrap());
        if let Some(email) = sol.get("email") {
            println!("  Email: {}", email);
        }
        if let Some(homepage) = sol.get("homepage") {
            println!("  Homepage: {}", homepage);
        }
    }
}
```

## UNION Queries

`UNION` combines results from alternative patterns.

### Basic UNION

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX vcard: <http://www.w3.org/2006/vcard/ns#>

SELECT ?person ?name
WHERE {
  {
    ?person foaf:name ?name .
  }
  UNION
  {
    ?person vcard:fn ?name .
  }
}
```

Returns names using either FOAF or vCard vocabulary.

### Multiple UNIONs

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX vcard: <http://www.w3.org/2006/vcard/ns#>
PREFIX schema: <http://schema.org/>

SELECT ?person ?name
WHERE {
  { ?person foaf:name ?name }
  UNION
  { ?person vcard:fn ?name }
  UNION
  { ?person schema:name ?name }
}
```

### UNION with Different Variables

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX org: <http://www.w3.org/ns/org#>

SELECT ?entity ?label
WHERE {
  {
    ?entity a foaf:Person ;
            foaf:name ?label .
  }
  UNION
  {
    ?entity a org:Organization ;
            org:name ?label .
  }
}
```

Returns both people and organizations with their names.

### UNION with Additional Constraints

```sparql
PREFIX ex: <http://example.com/>

SELECT ?item ?value
WHERE {
  {
    ?item ex:price ?value .
    FILTER(?value < 100)
  }
  UNION
  {
    ?item ex:rating ?value .
    FILTER(?value >= 4.0)
  }
}
```

### Practical Example: Contact Information

```python
from pyoxigraph import Store

store = Store()

query = """
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX vcard: <http://www.w3.org/2006/vcard/ns#>

SELECT DISTINCT ?person ?contact
WHERE {
  ?person foaf:name ?name .
  {
    ?person foaf:mbox ?contact .
  }
  UNION
  {
    ?person foaf:phone ?contact .
  }
  UNION
  {
    ?person vcard:email ?contact .
  }
}
"""

for solution in store.query(query):
    print(f"{solution['person']}: {solution['contact']}")
```

## Subqueries

Subqueries allow nested SELECT queries for complex operations.

### Basic Subquery

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?friendCount
WHERE {
  ?person foaf:name ?name .
  {
    SELECT ?person (COUNT(?friend) AS ?friendCount)
    WHERE {
      ?person foaf:knows ?friend .
    }
    GROUP BY ?person
  }
}
```

### Subquery with LIMIT

```sparql
PREFIX ex: <http://example.com/>

SELECT ?topProduct ?price
WHERE {
  {
    SELECT ?topProduct ?price
    WHERE {
      ?topProduct ex:price ?price .
    }
    ORDER BY DESC(?price)
    LIMIT 5
  }
  ?topProduct ex:inStock true .
}
```

Finds the top 5 most expensive products that are in stock.

### Subquery for Filtering

```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?salary
WHERE {
  ?person ex:salary ?salary .
  {
    SELECT (AVG(?s) AS ?avgSalary)
    WHERE {
      ?p ex:salary ?s .
    }
  }
  FILTER(?salary > ?avgSalary)
}
```

Finds people earning above average.

### Practical Example: Top Contributors

```javascript
import { Store } from 'oxigraph';

const store = new Store();

const query = `
PREFIX ex: <http://example.com/>

SELECT ?contributor ?name ?contributions
WHERE {
  {
    SELECT ?contributor (COUNT(?contribution) AS ?contributions)
    WHERE {
      ?contributor ex:contributed ?contribution .
    }
    GROUP BY ?contributor
    ORDER BY DESC(?contributions)
    LIMIT 10
  }
  ?contributor ex:name ?name .
}
ORDER BY DESC(?contributions)
`;

for (const result of store.query(query)) {
  console.log(\`\${result.get('name').value}: \${result.get('contributions').value} contributions\`);
}
```

## Aggregation

Perform calculations across groups of results.

### COUNT

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT (COUNT(?person) AS ?totalPeople)
WHERE {
  ?person a foaf:Person .
}
```

Count with GROUP BY:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?department (COUNT(?person) AS ?employeeCount)
WHERE {
  ?person ex:worksIn ?department .
}
GROUP BY ?department
```

### SUM, AVG, MIN, MAX

```sparql
PREFIX ex: <http://example.com/>

SELECT ?category
       (COUNT(?product) AS ?productCount)
       (AVG(?price) AS ?avgPrice)
       (MIN(?price) AS ?minPrice)
       (MAX(?price) AS ?maxPrice)
       (SUM(?price) AS ?totalValue)
WHERE {
  ?product ex:category ?category ;
           ex:price ?price .
}
GROUP BY ?category
```

### GROUP_CONCAT

Concatenate values into a single string:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (GROUP_CONCAT(?friendName; SEPARATOR=", ") AS ?friends)
WHERE {
  ?person foaf:name ?name ;
          foaf:knows ?friend .
  ?friend foaf:name ?friendName .
}
GROUP BY ?person
```

### HAVING Clause

Filter groups based on aggregate values:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?author (COUNT(?article) AS ?articleCount)
WHERE {
  ?article ex:author ?author .
}
GROUP BY ?author
HAVING (COUNT(?article) > 5)
ORDER BY DESC(?articleCount)
```

Finds authors with more than 5 articles.

### Practical Example: Sales Analysis

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;

let query = r#"
PREFIX ex: <http://example.com/>

SELECT ?product ?productName
       (COUNT(?sale) AS ?salesCount)
       (SUM(?amount) AS ?totalRevenue)
       (AVG(?amount) AS ?avgSale)
WHERE {
  ?sale ex:product ?product ;
        ex:amount ?amount .
  ?product ex:name ?productName .
}
GROUP BY ?product ?productName
HAVING (SUM(?amount) > 10000)
ORDER BY DESC(?totalRevenue)
LIMIT 20
"#;

if let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()?
{
    for solution in solutions {
        let sol = solution?;
        println!("Product: {}", sol.get("productName").unwrap());
        println!("  Sales: {}", sol.get("salesCount").unwrap());
        println!("  Revenue: {}", sol.get("totalRevenue").unwrap());
        println!("  Average: {}", sol.get("avgSale").unwrap());
    }
}
```

## Property Paths

Property paths allow matching complex graph patterns in a concise syntax.

### Sequence Path (/)

Follow a path of properties in sequence:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?city
WHERE {
  ?person foaf:based_near/foaf:name ?city .
}
```

Equivalent to:
```sparql
SELECT ?person ?city
WHERE {
  ?person foaf:based_near ?location .
  ?location foaf:name ?city .
}
```

### Alternative Path (|)

Match one of several properties:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX vcard: <http://www.w3.org/2006/vcard/ns#>

SELECT ?person ?name
WHERE {
  ?person (foaf:name|vcard:fn) ?name .
}
```

### Zero or More (*)

Follow a property any number of times:

```sparql
PREFIX org: <http://www.w3.org/ns/org#>

SELECT ?person ?ancestor
WHERE {
  ?person org:reportsTo* ?ancestor .
}
```

Finds all ancestors in the reporting hierarchy (including the person themselves).

### One or More (+)

Follow a property at least once:

```sparql
PREFIX org: <http://www.w3.org/ns/org#>

SELECT ?person ?manager
WHERE {
  ?person org:reportsTo+ ?manager .
}
```

Finds managers at any level (excluding the person themselves).

### Zero or One (?)

Optional property:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?friend ?friendOfFriend
WHERE {
  ?person foaf:knows ?friend .
  ?friend foaf:knows? ?friendOfFriend .
}
```

### Inverse Path (^)

Traverse a property in reverse:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?knownBy
WHERE {
  ?person ^foaf:knows ?knownBy .
}
```

Equivalent to: `?knownBy foaf:knows ?person`

### Negated Property Set (!)

Exclude specific properties:

```sparql
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?subject ?object
WHERE {
  ?subject !rdf:type ?object .
}
```

### Practical Example: Social Network Analysis

```python
from pyoxigraph import Store

store = Store()

# Find all people in extended network (friends of friends of friends...)
query = """
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT DISTINCT ?person
WHERE {
  ex:alice foaf:knows+ ?person .
}
"""

# Find all managers up the chain
query2 = """
PREFIX org: <http://www.w3.org/ns/org#>
PREFIX ex: <http://example.com/>

SELECT ?person ?allManagers
WHERE {
  ex:employee123 org:reportsTo+ ?allManagers .
}
"""

# Find items and their containers at any level
query3 = """
PREFIX ex: <http://example.com/>

SELECT ?item ?container
WHERE {
  ?item ex:containedIn+ ?container .
  ?container a ex:TopLevelContainer .
}
"""

for solution in store.query(query):
    print(solution)
```

### Complex Path Example

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?colleague
WHERE {
  ?person ex:worksIn/^ex:worksIn ?colleague .
  FILTER(?person != ?colleague)
}
```

Finds colleagues (people working in the same department).

## Federated Queries (SERVICE)

Query remote SPARQL endpoints.

### Basic SERVICE

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  SERVICE <http://dbpedia.org/sparql> {
    ?person foaf:homepage ?homepage .
  }
}
```

### SERVICE with LOCAL Data

```sparql
PREFIX ex: <http://example.com/>
PREFIX dbr: <http://dbpedia.org/resource/>

SELECT ?localPerson ?name ?abstract
WHERE {
  ?localPerson ex:sameAs ?dbpediaPerson .

  SERVICE <http://dbpedia.org/sparql> {
    ?dbpediaPerson rdfs:label ?name ;
                   dbo:abstract ?abstract .
    FILTER(LANG(?abstract) = "en")
  }
}
```

Enriches local data with information from DBpedia.

### SERVICE SILENT

Ignore errors from remote endpoint:

```sparql
SELECT ?person ?info
WHERE {
  ?person a foaf:Person .

  SERVICE SILENT <http://example.com/sparql> {
    ?person ex:additionalInfo ?info .
  }
}
```

If the remote service fails, the query continues without the remote data.

### Practical Example: Data Enrichment

```rust
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;

// Note: Oxigraph supports federated queries with the http-client feature
let query = r#"
PREFIX dbo: <http://dbpedia.org/ontology/>
PREFIX dbr: <http://dbpedia.org/resource/>
PREFIX ex: <http://example.com/>

SELECT ?city ?population
WHERE {
  ?localCity ex:name ?cityName ;
             ex:linkedTo ?dbpediaCity .

  SERVICE <http://dbpedia.org/sparql> {
    ?dbpediaCity dbo:populationTotal ?population .
  }
}
"#;
```

## BIND and VALUES

### BIND - Creating Variables

Create new variables from expressions:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?price ?priceWithTax
WHERE {
  ?product ex:price ?price .
  BIND(?price * 1.2 AS ?priceWithTax)
}
```

Multiple BINDs:

```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?person ?fullName ?ageYears
WHERE {
  ?person ex:firstName ?first ;
          ex:lastName ?last ;
          ex:birthDate ?birthDate .

  BIND(CONCAT(?first, " ", ?last) AS ?fullName)
  BIND(YEAR(NOW()) - YEAR(?birthDate) AS ?ageYears)
}
```

### VALUES - Inline Data

Provide a list of values to query:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?person ?name
WHERE {
  VALUES ?person {
    ex:alice
    ex:bob
    ex:charlie
  }
  ?person ex:name ?name .
}
```

Multiple variables:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?country ?capital ?population
WHERE {
  VALUES (?country ?capital) {
    (ex:USA ex:WashingtonDC)
    (ex:UK ex:London)
    (ex:France ex:Paris)
  }
  ?capital ex:population ?population .
}
```

### Practical Example: Calculated Fields

```javascript
const query = `
PREFIX ex: <http://example.com/>

SELECT ?employee ?name ?salary ?grade ?bonus
WHERE {
  ?employee ex:name ?name ;
            ex:salary ?salary .

  BIND(
    IF(?salary < 50000, "Junior",
    IF(?salary < 80000, "Mid",
    IF(?salary < 120000, "Senior", "Executive")))
    AS ?grade
  )

  BIND(
    IF(?salary < 50000, ?salary * 0.05,
    IF(?salary < 80000, ?salary * 0.10,
    ?salary * 0.15))
    AS ?bonus
  )
}
ORDER BY DESC(?salary)
`;
```

## Named Graphs

Work with multiple graphs in a dataset.

### Querying a Specific Graph

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  GRAPH <http://example.com/graph/users> {
    ?person foaf:name ?name .
  }
}
```

### Querying All Graphs

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?graph ?person ?name
WHERE {
  GRAPH ?graph {
    ?person foaf:name ?name .
  }
}
```

### Combining Default and Named Graphs

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.com/>

SELECT ?person ?name ?metadata
WHERE {
  # From default graph
  ?person foaf:name ?name .

  # From named graph
  GRAPH ex:metadata {
    ?person ex:verified ?metadata .
  }
}
```

### Practical Example: Multi-tenant Data

```rust
use oxigraph::model::*;
use oxigraph::store::Store;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let store = Store::new()?;

// Query across multiple tenant graphs
let query = r#"
PREFIX ex: <http://example.com/>

SELECT ?tenant ?user ?action ?timestamp
WHERE {
  GRAPH ?tenant {
    ?user ex:performed ?action ;
          ex:at ?timestamp .
  }
  FILTER(STRSTARTS(STR(?tenant), "http://example.com/tenant/"))
}
ORDER BY DESC(?timestamp)
LIMIT 100
"#;
```

## Negation

### FILTER NOT EXISTS

Exclude results matching a pattern:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER NOT EXISTS {
    ?person foaf:mbox ?email .
  }
}
```

Finds people without email addresses.

### MINUS

Remove matching solutions:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  MINUS {
    ?person foaf:mbox ?email .
  }
}
```

Similar to `FILTER NOT EXISTS` but with different semantics.

### Difference Between NOT EXISTS and MINUS

```sparql
# NOT EXISTS - checks pattern existence
SELECT ?person
WHERE {
  ?person a foaf:Person .
  FILTER NOT EXISTS { ?person foaf:age ?age }
}

# MINUS - removes solutions
SELECT ?person
WHERE {
  ?person a foaf:Person .
  MINUS { ?person foaf:age ?age }
}
```

### Practical Example: Finding Gaps

```python
from pyoxigraph import Store

store = Store()

# Find products without reviews
query = """
PREFIX ex: <http://example.com/>

SELECT ?product ?name ?price
WHERE {
  ?product a ex:Product ;
           ex:name ?name ;
           ex:price ?price .
  FILTER NOT EXISTS {
    ?review ex:reviewsProduct ?product .
  }
}
ORDER BY DESC(?price)
"""

# Find employees not assigned to any project
query2 = """
PREFIX ex: <http://example.com/>

SELECT ?employee ?name
WHERE {
  ?employee a ex:Employee ;
            ex:name ?name .
  MINUS {
    ?project ex:assignedTo ?employee .
  }
}
"""

for solution in store.query(query):
    print(f"Product: {solution['name']}, Price: {solution['price']}")
```

## Performance Tips

1. **Filter Early**: Place filters close to the patterns they filter
2. **Use LIMIT**: During development, limit results to avoid overwhelming output
3. **Property Paths**: Use judiciously - complex paths can be expensive
4. **Subqueries**: Can improve performance by reducing intermediate results
5. **DISTINCT**: Can be expensive on large result sets - only use when necessary
6. **ORDER BY**: Consider ordering only the final results, not intermediate ones
7. **Indexes**: Oxigraph automatically maintains SPO, POS, OSP indexes

## Common Patterns

### Pagination

```sparql
SELECT ?item ?label
WHERE {
  ?item rdfs:label ?label .
}
ORDER BY ?label
LIMIT 20
OFFSET 40
```

### Conditional Values

```sparql
PREFIX ex: <http://example.com/>

SELECT ?product ?name ?status
WHERE {
  ?product ex:name ?name ;
           ex:stock ?stock .

  BIND(
    IF(?stock = 0, "Out of Stock",
    IF(?stock < 10, "Low Stock", "In Stock"))
    AS ?status
  )
}
```

### String Concatenation

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person (CONCAT(?firstName, " ", ?lastName) AS ?fullName)
WHERE {
  ?person foaf:firstName ?firstName ;
          foaf:lastName ?lastName .
}
```

### Date Filtering

```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?event ?title ?date
WHERE {
  ?event ex:title ?title ;
         ex:date ?date .
  FILTER(?date >= "2024-01-01"^^xsd:date &&
         ?date < "2025-01-01"^^xsd:date)
}
```

## Next Steps

- [SPARQL Functions Reference](../reference/sparql-functions.md) - Complete function list
- [SPARQL Updates](sparql-updates.md) - Modifying data
- [SPARQL Introduction](../tutorials/sparql-introduction.md) - Basic concepts

## Resources

- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)
- [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/)
- [Property Paths](https://www.w3.org/TR/sparql11-query/#propertypaths)
