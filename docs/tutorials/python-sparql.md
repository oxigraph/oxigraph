# SPARQL Queries in Pyoxigraph

This tutorial covers executing SPARQL queries in Pyoxigraph, from basic patterns to advanced techniques.

## Table of Contents

- [SPARQL Basics](#sparql-basics)
- [Query Types](#query-types)
- [Working with Query Results](#working-with-query-results)
- [SPARQL Updates](#sparql-updates)
- [Advanced Query Patterns](#advanced-query-patterns)
- [Parameterized Queries](#parameterized-queries)
- [Real-World Examples](#real-world-examples)

## SPARQL Basics

SPARQL (SPARQL Protocol and RDF Query Language) is the standard query language for RDF data. It's similar to SQL but designed for graph data.

### Your First Query

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

# Setup data
store = Store()
alice = NamedNode("http://example.org/alice")
name = NamedNode("http://schema.org/name")
store.add(Quad(alice, name, Literal("Alice")))

# Execute query
results = store.query("""
    SELECT ?subject ?name
    WHERE {
        ?subject <http://schema.org/name> ?name .
    }
""")

# Process results
for solution in results:
    print(f"Subject: {solution['subject']}")
    print(f"Name: {solution['name'].value}")
```

### SPARQL Syntax Essentials

```sparql
# Prefixes - shortcuts for long URIs
PREFIX schema: <http://schema.org/>
PREFIX ex: <http://example.org/>

# Query pattern
SELECT ?variable1 ?variable2
WHERE {
    # Triple patterns (. separates triples)
    ?variable1 schema:name ?variable2 .

    # Full URIs can be used in angle brackets
    ?variable1 <http://schema.org/age> ?age .

    # Multiple patterns about the same subject (; separator)
    ?person schema:name ?name ;
            schema:age ?age .
}
```

## Query Types

SPARQL has four main query types.

### SELECT Queries

Return tabular results with specified variables:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

store = Store()

# Add test data
people = [
    ("alice", "Alice", 30),
    ("bob", "Bob", 25),
    ("charlie", "Charlie", 35)
]

for id, name, age in people:
    person = NamedNode(f"http://example.org/{id}")
    store.add(Quad(person, NamedNode("http://schema.org/name"), Literal(name)))
    store.add(Quad(person, NamedNode("http://schema.org/age"), Literal(str(age))))

# SELECT query
query = """
    PREFIX schema: <http://schema.org/>

    SELECT ?name ?age
    WHERE {
        ?person schema:name ?name ;
                schema:age ?age .
        FILTER(?age >= 30)
    }
    ORDER BY DESC(?age)
"""

results = store.query(query)
for solution in results:
    print(f"{solution['name'].value} is {solution['age'].value} years old")

# Output:
# Charlie is 35 years old
# Alice is 30 years old
```

### ASK Queries

Return a boolean indicating if a pattern exists:

```python
# Check if any person is older than 40
exists = store.query("""
    PREFIX schema: <http://schema.org/>

    ASK {
        ?person schema:age ?age .
        FILTER(?age > 40)
    }
""")

print(f"Anyone over 40? {exists}")  # False

# Check if Alice exists
exists = store.query("""
    ASK { ?person <http://schema.org/name> "Alice" }
""")

print(f"Alice exists? {exists}")  # True
```

### CONSTRUCT Queries

Create new triples based on a template:

```python
# Transform data to a different schema
results = store.query("""
    PREFIX schema: <http://schema.org/>
    PREFIX ex: <http://example.org/vocab/>

    CONSTRUCT {
        ?person ex:fullName ?name ;
                ex:yearsOld ?age .
    }
    WHERE {
        ?person schema:name ?name ;
                schema:age ?age .
    }
""")

# Results is an iterator of Triple objects
for triple in results:
    print(f"{triple.subject}")
    print(f"  {triple.predicate} -> {triple.object.value}")
```

### DESCRIBE Queries

Return all triples about a resource:

```python
# Get all information about Alice
results = store.query("""
    DESCRIBE <http://example.org/alice>
""")

for triple in results:
    print(f"{triple.predicate} -> {triple.object.value}")
```

## Working with Query Results

### SELECT Results (QuerySolutions)

```python
query = """
    SELECT ?person ?name ?age
    WHERE {
        ?person <http://schema.org/name> ?name ;
                <http://schema.org/age> ?age .
    }
"""

results = store.query(query)

# Get variable names
print(f"Variables: {[str(v) for v in results.variables]}")

# Iterate over solutions
for solution in results:
    # Access by variable name (string)
    print(f"Name: {solution['name'].value}")

    # Access by Variable object
    from pyoxigraph import Variable
    name_var = Variable('name')
    print(f"Name: {solution[name_var].value}")

    # Access by index
    print(f"First variable: {solution[0]}")

    # Unpack solution
    person, name, age = solution
    print(f"{name.value} is {age.value}")

    # Check if variable is bound
    if 'email' in solution:
        print(f"Email: {solution['email'].value}")

    # Get with default
    email = solution.get('email', Literal('N/A'))
    print(f"Email: {email.value}")
```

### Result Set Modifiers

```python
# LIMIT - restrict number of results
query = """
    SELECT ?name
    WHERE { ?person <http://schema.org/name> ?name }
    LIMIT 10
"""

# OFFSET - skip first N results
query = """
    SELECT ?name
    WHERE { ?person <http://schema.org/name> ?name }
    OFFSET 5
    LIMIT 10
"""

# ORDER BY - sort results
query = """
    SELECT ?name ?age
    WHERE {
        ?person <http://schema.org/name> ?name ;
                <http://schema.org/age> ?age .
    }
    ORDER BY DESC(?age) ?name
"""

# DISTINCT - remove duplicates
query = """
    SELECT DISTINCT ?city
    WHERE {
        ?person <http://schema.org/address> ?addr .
        ?addr <http://schema.org/addressLocality> ?city .
    }
"""
```

### Aggregate Functions

```python
# COUNT, SUM, AVG, MIN, MAX
query = """
    SELECT
        (COUNT(?person) AS ?count)
        (AVG(?age) AS ?avgAge)
        (MIN(?age) AS ?minAge)
        (MAX(?age) AS ?maxAge)
        (SUM(?age) AS ?totalAge)
    WHERE {
        ?person <http://schema.org/age> ?age .
    }
"""

result = next(store.query(query))
print(f"Count: {result['count'].value}")
print(f"Average age: {result['avgAge'].value}")
print(f"Min age: {result['minAge'].value}")
print(f"Max age: {result['maxAge'].value}")

# GROUP BY and HAVING
query = """
    SELECT ?city (COUNT(?person) AS ?population)
    WHERE {
        ?person <http://schema.org/address> ?addr .
        ?addr <http://schema.org/addressLocality> ?city .
    }
    GROUP BY ?city
    HAVING (COUNT(?person) > 100)
    ORDER BY DESC(?population)
"""
```

### CONSTRUCT and DESCRIBE Results (QueryTriples)

```python
results = store.query("""
    CONSTRUCT { ?s ?p ?o }
    WHERE { ?s ?p ?o }
""")

# Iterate over triples
for triple in results:
    print(f"{triple.subject} {triple.predicate} {triple.object}")

# Convert to list
triples_list = list(results)
print(f"Total triples: {len(triples_list)}")
```

## SPARQL Updates

Modify data using SPARQL UPDATE operations.

### INSERT DATA

```python
# Insert new triples
store.update("""
    PREFIX schema: <http://schema.org/>
    PREFIX ex: <http://example.org/>

    INSERT DATA {
        ex:david schema:name "David" ;
                 schema:age 28 ;
                 schema:email "david@example.org" .
    }
""")
```

### DELETE DATA

```python
# Delete specific triples
store.update("""
    PREFIX schema: <http://schema.org/>
    PREFIX ex: <http://example.org/>

    DELETE DATA {
        ex:david schema:email "david@example.org" .
    }
""")
```

### DELETE/INSERT (Conditional Updates)

```python
# Update Alice's age
store.update("""
    PREFIX schema: <http://schema.org/>
    PREFIX ex: <http://example.org/>

    DELETE {
        ex:alice schema:age ?oldAge .
    }
    INSERT {
        ex:alice schema:age 31 .
    }
    WHERE {
        ex:alice schema:age ?oldAge .
    }
""")

# Add email to everyone who doesn't have one
store.update("""
    PREFIX schema: <http://schema.org/>

    INSERT {
        ?person schema:email ?defaultEmail .
    }
    WHERE {
        ?person schema:name ?name .
        FILTER NOT EXISTS { ?person schema:email ?email }
        BIND(CONCAT(STR(?name), "@example.org") AS ?defaultEmail)
    }
""")
```

### DELETE WHERE (Delete Matching Patterns)

```python
# Delete all people under 25
store.update("""
    PREFIX schema: <http://schema.org/>

    DELETE WHERE {
        ?person schema:age ?age .
        FILTER(?age < 25)
    }
""")
```

### CLEAR (Delete All Data)

```python
# Clear default graph
store.update("CLEAR DEFAULT")

# Clear specific graph
store.update("CLEAR GRAPH <http://example.org/graph1>")

# Clear all data
store.update("CLEAR ALL")
```

## Advanced Query Patterns

### OPTIONAL Patterns

Match patterns when they exist, but don't filter out results when they don't:

```python
query = """
    PREFIX schema: <http://schema.org/>

    SELECT ?name ?email
    WHERE {
        ?person schema:name ?name .
        OPTIONAL { ?person schema:email ?email }
    }
"""

# Returns all people, with email when available
for solution in store.query(query):
    name = solution['name'].value
    email = solution.get('email')
    if email:
        print(f"{name}: {email.value}")
    else:
        print(f"{name}: no email")
```

### UNION Patterns

Match one pattern or another:

```python
query = """
    PREFIX schema: <http://schema.org/>
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?person ?contact
    WHERE {
        ?person schema:name ?name .
        {
            ?person schema:email ?contact
        } UNION {
            ?person foaf:phone ?contact
        }
    }
"""
```

### FILTER Expressions

```python
query = """
    PREFIX schema: <http://schema.org/>

    SELECT ?name ?age
    WHERE {
        ?person schema:name ?name ;
                schema:age ?age .

        # Numeric comparisons
        FILTER(?age >= 25 && ?age < 35)

        # String operations
        FILTER(REGEX(?name, "^A", "i"))  # Starts with A (case-insensitive)

        # Type checking
        FILTER(isLiteral(?name))

        # Logical operators
        FILTER(?age > 30 || ?name = "Bob")
    }
"""
```

### Property Paths

```python
# Zero or more (/*)
query = """
    SELECT ?ancestor
    WHERE {
        <http://example.org/alice> <http://example.org/parent>* ?ancestor
    }
"""

# One or more (/+)
query = """
    SELECT ?descendant
    WHERE {
        <http://example.org/alice> <http://example.org/parent>+ ?descendant
    }
"""

# Alternative paths (|)
query = """
    PREFIX schema: <http://schema.org/>
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?contact
    WHERE {
        ?person (schema:email|foaf:mbox) ?contact
    }
"""

# Sequence (/)
query = """
    SELECT ?city
    WHERE {
        ?person <http://schema.org/address>/<http://schema.org/addressLocality> ?city
    }
"""
```

### BIND (Creating New Variables)

```python
query = """
    PREFIX schema: <http://schema.org/>

    SELECT ?name ?birthYear
    WHERE {
        ?person schema:name ?name ;
                schema:age ?age .

        # Calculate birth year
        BIND(2024 - ?age AS ?birthYear)

        # String concatenation
        BIND(CONCAT("Hello, ", ?name) AS ?greeting)
    }
"""
```

### Subqueries

```python
# Find people older than average
query = """
    PREFIX schema: <http://schema.org/>

    SELECT ?name ?age
    WHERE {
        {
            SELECT (AVG(?age) AS ?avgAge)
            WHERE {
                ?p schema:age ?age .
            }
        }

        ?person schema:name ?name ;
                schema:age ?age .

        FILTER(?age > ?avgAge)
    }
"""
```

### VALUES (Inline Data)

```python
query = """
    PREFIX ex: <http://example.org/>
    PREFIX schema: <http://schema.org/>

    SELECT ?name
    WHERE {
        VALUES ?person { ex:alice ex:bob }
        ?person schema:name ?name .
    }
"""

# With multiple variables
query = """
    SELECT ?greeting
    WHERE {
        VALUES (?name ?title) {
            ("Alice" "Dr.")
            ("Bob" "Mr.")
        }
        BIND(CONCAT(?title, " ", ?name) AS ?greeting)
    }
"""
```

## Parameterized Queries

While SPARQL doesn't have native parameterization, you can safely build queries in Python:

### String Formatting (Simple Cases)

```python
def get_person_info(person_id: str) -> dict:
    """Get information about a person by ID."""
    # Build the IRI
    person_iri = f"http://example.org/{person_id}"

    query = f"""
        PREFIX schema: <http://schema.org/>

        SELECT ?name ?age ?email
        WHERE {{
            <{person_iri}> schema:name ?name .
            OPTIONAL {{ <{person_iri}> schema:age ?age }}
            OPTIONAL {{ <{person_iri}> schema:email ?email }}
        }}
    """

    result = next(store.query(query), None)
    if result:
        return {
            'name': result['name'].value,
            'age': result.get('age', {}).value if result.get('age') else None,
            'email': result.get('email', {}).value if result.get('email') else None
        }
    return None

# Usage
info = get_person_info("alice")
print(info)
```

### Query Builder Pattern

```python
class QueryBuilder:
    """Build SPARQL queries programmatically."""

    def __init__(self, store):
        self.store = store
        self.prefixes = {
            'schema': 'http://schema.org/',
            'ex': 'http://example.org/'
        }
        self.patterns = []
        self.filters = []

    def add_pattern(self, subject, predicate, object):
        self.patterns.append(f"{subject} {predicate} {object}")
        return self

    def add_filter(self, condition):
        self.filters.append(condition)
        return self

    def build_select(self, variables):
        prefix_section = '\n'.join(
            f"PREFIX {name}: <{uri}>"
            for name, uri in self.prefixes.items()
        )

        patterns_section = ' .\n        '.join(self.patterns)
        filters_section = '\n        '.join(
            f"FILTER({f})" for f in self.filters
        )

        query = f"""
            {prefix_section}

            SELECT {' '.join(variables)}
            WHERE {{
                {patterns_section} .
                {filters_section}
            }}
        """

        return self.store.query(query)

# Usage
builder = QueryBuilder(store)
results = builder \
    .add_pattern("?person", "schema:name", "?name") \
    .add_pattern("?person", "schema:age", "?age") \
    .add_filter("?age > 25") \
    .build_select(["?name", "?age"])

for solution in results:
    print(f"{solution['name'].value}: {solution['age'].value}")
```

### VALUES-Based Parameterization

```python
def find_people_by_ids(person_ids: list[str]):
    """Find people by a list of IDs."""
    # Build VALUES clause
    iris = ' '.join(f"ex:{id}" for id in person_ids)

    query = f"""
        PREFIX schema: <http://schema.org/>
        PREFIX ex: <http://example.org/>

        SELECT ?person ?name
        WHERE {{
            VALUES ?person {{ {iris} }}
            ?person schema:name ?name .
        }}
    """

    return store.query(query)

# Usage
results = find_people_by_ids(["alice", "bob", "charlie"])
for solution in results:
    print(solution['name'].value)
```

## Real-World Examples

### Social Network Analysis

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

# Setup social network
store = Store()

# Add people and relationships
people = {
    'alice': 'Alice',
    'bob': 'Bob',
    'charlie': 'Charlie',
    'david': 'David',
    'eve': 'Eve'
}

# Add names
for id, name in people.items():
    person = NamedNode(f"http://example.org/{id}")
    store.add(Quad(person, NamedNode("http://xmlns.com/foaf/0.1/name"), Literal(name)))

# Add friendships
friendships = [
    ('alice', 'bob'),
    ('alice', 'charlie'),
    ('bob', 'charlie'),
    ('bob', 'david'),
    ('charlie', 'david'),
    ('david', 'eve')
]

for person1_id, person2_id in friendships:
    person1 = NamedNode(f"http://example.org/{person1_id}")
    person2 = NamedNode(f"http://example.org/{person2_id}")
    knows = NamedNode("http://xmlns.com/foaf/0.1/knows")
    store.add(Quad(person1, knows, person2))
    store.add(Quad(person2, knows, person1))  # Symmetric relationship

# Query 1: Find Alice's friends
query1 = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    SELECT ?friendName
    WHERE {
        ex:alice foaf:knows ?friend .
        ?friend foaf:name ?friendName .
    }
"""
print("Alice's friends:")
for solution in store.query(query1):
    print(f"  - {solution['friendName'].value}")

# Query 2: Find friends of friends (2 degrees)
query2 = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    SELECT DISTINCT ?fofName
    WHERE {
        ex:alice foaf:knows/foaf:knows ?fof .
        ?fof foaf:name ?fofName .
        FILTER(?fof != ex:alice)
    }
"""
print("\nFriends of Alice's friends:")
for solution in store.query(query2):
    print(f"  - {solution['fofName'].value}")

# Query 3: Most connected person
query3 = """
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?name (COUNT(?friend) AS ?friendCount)
    WHERE {
        ?person foaf:name ?name ;
                foaf:knows ?friend .
    }
    GROUP BY ?person ?name
    ORDER BY DESC(?friendCount)
    LIMIT 1
"""
result = next(store.query(query3))
print(f"\nMost connected: {result['name'].value} with {result['friendCount'].value} friends")
```

### Product Catalog with Filtering

```python
# Setup product catalog
store = Store()

products = [
    {"id": "laptop1", "name": "UltraBook Pro", "category": "Laptop", "price": 1299.99, "rating": 4.5},
    {"id": "laptop2", "name": "ThinkStation", "category": "Laptop", "price": 899.99, "rating": 4.2},
    {"id": "phone1", "name": "SmartPhone X", "category": "Phone", "price": 799.99, "rating": 4.7},
    {"id": "phone2", "name": "BudgetPhone", "category": "Phone", "price": 299.99, "rating": 3.9},
    {"id": "tablet1", "name": "TabletPro", "category": "Tablet", "price": 599.99, "rating": 4.3},
]

SCHEMA = "http://schema.org/"
EX = "http://example.org/"

for product_data in products:
    product = NamedNode(f"{EX}product/{product_data['id']}")
    store.add(Quad(product, NamedNode(f"{SCHEMA}name"), Literal(product_data['name'])))
    store.add(Quad(product, NamedNode(f"{SCHEMA}category"), Literal(product_data['category'])))
    store.add(Quad(product, NamedNode(f"{SCHEMA}price"),
                  Literal(str(product_data['price']), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal"))))
    store.add(Quad(product, NamedNode(f"{EX}rating"),
                  Literal(str(product_data['rating']), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal"))))

# Query: Find laptops under $1000 with rating > 4.0
query = """
    PREFIX schema: <http://schema.org/>
    PREFIX ex: <http://example.org/>

    SELECT ?name ?price ?rating
    WHERE {
        ?product schema:name ?name ;
                schema:category "Laptop" ;
                schema:price ?price ;
                ex:rating ?rating .
        FILTER(?price < 1000 && ?rating > 4.0)
    }
    ORDER BY DESC(?rating)
"""

print("Laptops under $1000 with rating > 4.0:")
for solution in store.query(query):
    print(f"  {solution['name'].value}: ${solution['price'].value} (⭐ {solution['rating'].value})")
```

### Time-Series Data Analysis

```python
from datetime import datetime, timedelta

# Add time-series sensor data
store = Store()

SENSOR = "http://example.org/sensor/"
SOSA = "http://www.w3.org/ns/sosa/"
XSD = "http://www.w3.org/2001/XMLSchema#"

base_time = datetime(2024, 1, 1, 0, 0)

# Generate sensor readings
for i in range(24):
    observation = NamedNode(f"{SENSOR}observation/{i}")
    timestamp = base_time + timedelta(hours=i)
    temperature = 20 + (i % 12)  # Simulated temperature cycle

    store.add(Quad(observation, NamedNode(f"{SOSA}resultTime"),
                  Literal(timestamp.isoformat(), datatype=NamedNode(f"{XSD}dateTime"))))
    store.add(Quad(observation, NamedNode(f"{SOSA}hasSimpleResult"),
                  Literal(str(temperature), datatype=NamedNode(f"{XSD}decimal"))))

# Query: Average temperature per 6-hour period
query = """
    PREFIX sosa: <http://www.w3.org/ns/sosa/>

    SELECT (AVG(?temp) AS ?avgTemp) (MIN(?time) AS ?periodStart)
    WHERE {
        ?obs sosa:resultTime ?time ;
             sosa:hasSimpleResult ?temp .
    }
    GROUP BY (FLOOR(HOURS(?time) / 6))
    ORDER BY ?periodStart
"""

print("Average temperature by 6-hour period:")
for solution in store.query(query):
    print(f"  {solution['periodStart'].value}: {solution['avgTemp'].value}°C")
```

## Performance Tips

### 1. Use Query Planning

SPARQL optimizers work better with certain patterns:

```python
# Good - specific patterns first
query = """
    SELECT ?name
    WHERE {
        ?person <http://schema.org/name> "Alice" .  # Specific
        ?person <http://schema.org/age> ?age .       # General
    }
"""

# Less efficient - general patterns first
query = """
    SELECT ?name
    WHERE {
        ?person <http://schema.org/age> ?age .       # General
        ?person <http://schema.org/name> "Alice" .  # Specific
    }
"""
```

### 2. Use LIMIT When Appropriate

```python
# If you only need a few results, use LIMIT
query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 100"
```

### 3. Use Indexes via Named Graphs

```python
# Store related data in named graphs for better performance
graph = NamedNode("http://example.org/graph/users")
store.add(Quad(subject, predicate, object, graph))

# Query specific graph
query = """
    SELECT ?s ?p ?o
    FROM <http://example.org/graph/users>
    WHERE { ?s ?p ?o }
"""
```

## Best Practices

1. **Use prefixes** to make queries more readable
2. **Add comments** to complex queries
3. **Test queries** incrementally
4. **Use LIMIT** during development
5. **Handle empty results** gracefully
6. **Validate IRIs** before constructing queries
7. **Use appropriate datatypes** for literals

## Resources

- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)
- [SPARQL Examples](https://www.w3.org/TR/sparql11-query/#examples)
- [Pyoxigraph Documentation](https://pyoxigraph.readthedocs.io/)

## Next Steps

- Explore [Getting Started](python-getting-started.md) for basics
- Learn about [RDF Data](python-rdf-data.md) structures
- Check out the [API Reference](https://pyoxigraph.readthedocs.io/)
