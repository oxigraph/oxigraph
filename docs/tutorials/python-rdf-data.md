# Working with RDF Data in Pyoxigraph

This tutorial covers the RDF data model in Pyoxigraph and shows you how to create, manipulate, and persist RDF data using Pythonic patterns.

## Table of Contents

- [RDF Data Model Overview](#rdf-data-model-overview)
- [Creating RDF Terms](#creating-rdf-terms)
- [Working with Triples and Quads](#working-with-triples-and-quads)
- [In-Memory Datasets](#in-memory-datasets)
- [Loading and Saving RDF Files](#loading-and-saving-rdf-files)
- [Graph Operations](#graph-operations)
- [Integration with Python Ecosystem](#integration-with-python-ecosystem)

## RDF Data Model Overview

RDF (Resource Description Framework) represents information as graphs. The basic building blocks are:

- **Triples**: Subject-Predicate-Object statements
- **Quads**: Triples with an optional named graph
- **Terms**: The nodes and values in your graph

```
Subject --Predicate--> Object
```

Example: "Alice knows Bob"
```
<http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/bob>
```

## Creating RDF Terms

Pyoxigraph provides classes for all RDF term types.

### Named Nodes (IRIs)

Named nodes are IRIs (Internationalized Resource Identifiers) that uniquely identify resources:

```python
from pyoxigraph import NamedNode

# Basic IRI
person = NamedNode("http://example.org/person/alice")

# Using namespace pattern
FOAF = "http://xmlns.com/foaf/0.1/"
knows = NamedNode(f"{FOAF}knows")
name = NamedNode(f"{FOAF}name")

# Common vocabularies
rdf_type = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
rdfs_label = NamedNode("http://www.w3.org/2000/01/rdf-schema#label")
```

### Literals

Literals represent values like strings, numbers, and dates:

```python
from pyoxigraph import NamedNode, Literal

# Simple string literal
name = Literal("Alice")

# Literal with language tag
label_en = Literal("Hello", language="en")
label_fr = Literal("Bonjour", language="fr")

# Typed literals (numbers, dates, etc.)
XSD = "http://www.w3.org/2001/XMLSchema#"

age = Literal("30", datatype=NamedNode(f"{XSD}integer"))
height = Literal("1.75", datatype=NamedNode(f"{XSD}decimal"))
is_active = Literal("true", datatype=NamedNode(f"{XSD}boolean"))
birthday = Literal("1993-05-15", datatype=NamedNode(f"{XSD}date"))
timestamp = Literal("2024-01-15T10:30:00Z", datatype=NamedNode(f"{XSD}dateTime"))

# Accessing literal properties
print(name.value)           # "Alice"
print(label_en.language)    # "en"
print(age.datatype)         # NamedNode('http://www.w3.org/2001/XMLSchema#integer')
```

### Blank Nodes

Blank nodes are anonymous resources without a global identifier:

```python
from pyoxigraph import BlankNode

# Auto-generated blank node
bn1 = BlankNode()
bn2 = BlankNode()

# Named blank node (useful for round-tripping)
bn3 = BlankNode("person1")

# Each blank node is unique
print(bn1 == bn2)  # False
```

Blank nodes are useful for intermediate or internal structures:

```python
from pyoxigraph import Store, NamedNode, BlankNode, Literal, Quad

store = Store()

# Representing a mailing address as a blank node
alice = NamedNode("http://example.org/person/alice")
address_pred = NamedNode("http://schema.org/address")
street_pred = NamedNode("http://schema.org/streetAddress")
city_pred = NamedNode("http://schema.org/addressLocality")

address = BlankNode()
store.add(Quad(alice, address_pred, address))
store.add(Quad(address, street_pred, Literal("123 Main St")))
store.add(Quad(address, city_pred, Literal("Springfield")))
```

## Working with Triples and Quads

### Triple vs Quad

- **Triple**: Subject, Predicate, Object (in the default graph)
- **Quad**: Subject, Predicate, Object, Graph (in a named graph)

```python
from pyoxigraph import Triple, Quad, NamedNode, Literal, DefaultGraph

alice = NamedNode("http://example.org/alice")
name = NamedNode("http://schema.org/name")
alice_lit = Literal("Alice")

# Triple
triple = Triple(alice, name, alice_lit)
print(f"Subject: {triple.subject}")
print(f"Predicate: {triple.predicate}")
print(f"Object: {triple.object}")

# Quad in default graph (equivalent to triple)
quad1 = Quad(alice, name, alice_lit)
quad2 = Quad(alice, name, alice_lit, DefaultGraph())
print(quad1 == quad2)  # True

# Quad in named graph
graph = NamedNode("http://example.org/graph/social")
quad3 = Quad(alice, name, alice_lit, graph)
```

### Pattern Matching with Quads

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

store = Store()

# Add some data
alice = NamedNode("http://example.org/alice")
bob = NamedNode("http://example.org/bob")
name = NamedNode("http://schema.org/name")
age = NamedNode("http://schema.org/age")

store.add(Quad(alice, name, Literal("Alice")))
store.add(Quad(alice, age, Literal("30")))
store.add(Quad(bob, name, Literal("Bob")))
store.add(Quad(bob, age, Literal("25")))

# Match all quads
all_quads = list(store.quads_for_pattern(None, None, None))
print(f"Total quads: {len(all_quads)}")

# Match all quads about Alice
alice_quads = list(store.quads_for_pattern(alice, None, None))
for quad in alice_quads:
    print(f"  {quad.predicate} -> {quad.object}")

# Match all name properties
name_quads = list(store.quads_for_pattern(None, name, None))
for quad in name_quads:
    print(f"  {quad.subject} has name {quad.object.value}")

# Match specific quad
specific = list(store.quads_for_pattern(alice, name, Literal("Alice")))
print(f"Found exact match: {len(specific) > 0}")
```

## In-Memory Datasets

The `Dataset` class provides a pure Python collection for RDF quads, similar to a set.

```python
from pyoxigraph import Dataset, NamedNode, Literal, Quad

# Create an empty dataset
dataset = Dataset()

# Add quads
alice = NamedNode("http://example.org/alice")
name = NamedNode("http://schema.org/name")
dataset.add(Quad(alice, name, Literal("Alice")))

# Dataset operations
print(len(dataset))  # 1
print(Quad(alice, name, Literal("Alice")) in dataset)  # True

# Iterate over quads
for quad in dataset:
    print(quad)

# Create dataset from iterable
quads = [
    Quad(NamedNode("http://example.org/alice"), name, Literal("Alice")),
    Quad(NamedNode("http://example.org/bob"), name, Literal("Bob"))
]
dataset = Dataset(quads)

# Set operations
dataset1 = Dataset([Quad(alice, name, Literal("Alice"))])
dataset2 = Dataset([Quad(alice, name, Literal("Alice"))])
print(dataset1 == dataset2)  # True
```

## Loading and Saving RDF Files

Pyoxigraph supports multiple RDF formats.

### Supported Formats

```python
from pyoxigraph import RdfFormat

# Available formats
formats = {
    "Turtle": RdfFormat.TURTLE,
    "N-Triples": RdfFormat.N_TRIPLES,
    "N-Quads": RdfFormat.N_QUADS,
    "TriG": RdfFormat.TRIG,
    "RDF/XML": RdfFormat.RDF_XML,
    "JSON-LD": RdfFormat.JSON_LD,
}
```

### Loading RDF Data into a Store

```python
from pyoxigraph import Store

store = Store()

# Load from file
store.load("data.ttl", format=RdfFormat.TURTLE)

# Load from file with base IRI
store.load("data.ttl", format=RdfFormat.TURTLE, base_iri="http://example.org/")

# Load from string
turtle_data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.org/> .

ex:alice schema:name "Alice" ;
         schema:age 30 .
"""
store.load(turtle_data.encode('utf-8'), format=RdfFormat.TURTLE)

# Load into a named graph
graph = NamedNode("http://example.org/graph/social")
store.load("social-data.ttl", format=RdfFormat.TURTLE, to_graph=graph)
```

### Saving RDF Data

```python
from pyoxigraph import Store, RdfFormat

store = Store()
# ... add some data ...

# Save to file
store.dump("output.ttl", format=RdfFormat.TURTLE)

# Save to bytes
turtle_bytes = store.dump(format=RdfFormat.TURTLE)
print(turtle_bytes.decode('utf-8'))

# Save from a specific graph
graph = NamedNode("http://example.org/graph/social")
store.dump("social-output.ttl", format=RdfFormat.TURTLE, from_graph=graph)
```

### Parsing RDF Without a Store

For simple parsing tasks, use the `parse` function:

```python
from pyoxigraph import parse, RdfFormat

rdf_data = """
@prefix schema: <http://schema.org/> .
<http://example.org/alice> schema:name "Alice" .
"""

# Parse to iterator of quads
quads = parse(rdf_data.encode('utf-8'), format=RdfFormat.TURTLE)

for quad in quads:
    print(f"Subject: {quad.subject}")
    print(f"Predicate: {quad.predicate}")
    print(f"Object: {quad.object}")
    print()
```

### Serializing RDF Data

```python
from pyoxigraph import serialize, RdfFormat, NamedNode, Literal, Triple

# Create some triples
triples = [
    Triple(NamedNode("http://example.org/alice"),
           NamedNode("http://schema.org/name"),
           Literal("Alice")),
    Triple(NamedNode("http://example.org/bob"),
           NamedNode("http://schema.org/name"),
           Literal("Bob"))
]

# Serialize to Turtle
turtle = serialize(triples, format=RdfFormat.TURTLE)
print(turtle.decode('utf-8'))

# Serialize to N-Triples
ntriples = serialize(triples, format=RdfFormat.N_TRIPLES)
print(ntriples.decode('utf-8'))
```

## Graph Operations

### Bulk Loading Data

For large datasets, use bulk operations for better performance:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

store = Store()

# Create many quads
quads = []
for i in range(10000):
    person = NamedNode(f"http://example.org/person/{i}")
    name = NamedNode("http://schema.org/name")
    quads.append(Quad(person, name, Literal(f"Person {i}")))

# Bulk extend (faster than adding one by one)
store.bulk_extend(quads)
print(f"Store contains {len(store)} quads")
```

### Removing Data

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

store = Store()

alice = NamedNode("http://example.org/alice")
name = NamedNode("http://schema.org/name")
age = NamedNode("http://schema.org/age")

store.add(Quad(alice, name, Literal("Alice")))
store.add(Quad(alice, age, Literal("30")))

# Remove a specific quad
store.remove(Quad(alice, age, Literal("30")))

# Check if it's removed
print(Quad(alice, age, Literal("30")) in store)  # False
print(Quad(alice, name, Literal("Alice")) in store)  # True
```

### Clearing the Store

```python
# Clear all data
store.clear()
print(len(store))  # 0

# Clear only a specific graph
graph = NamedNode("http://example.org/graph/social")
store.clear_graph(graph)
```

## Integration with Python Ecosystem

### Working with Pandas

Convert RDF query results to a pandas DataFrame:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad
import pandas as pd

# Create store with data
store = Store()
people = [
    ("alice", "Alice", 30, "alice@example.org"),
    ("bob", "Bob", 25, "bob@example.org"),
    ("charlie", "Charlie", 35, "charlie@example.org")
]

for id, name, age, email in people:
    person = NamedNode(f"http://example.org/person/{id}")
    store.add(Quad(person, NamedNode("http://schema.org/name"), Literal(name)))
    store.add(Quad(person, NamedNode("http://schema.org/age"), Literal(str(age))))
    store.add(Quad(person, NamedNode("http://schema.org/email"), Literal(email)))

# Query and convert to DataFrame
query = """
    SELECT ?name ?age ?email
    WHERE {
        ?person <http://schema.org/name> ?name ;
               <http://schema.org/age> ?age ;
               <http://schema.org/email> ?email .
    }
    ORDER BY ?name
"""

results = store.query(query)
data = [{var: solution[var].value for var in ['name', 'age', 'email']}
        for solution in results]

df = pd.DataFrame(data)
print(df)
#        name age               email
# 0     Alice  30   alice@example.org
# 1       Bob  25     bob@example.org
# 2   Charlie  35  charlie@example.org

# Analyze with pandas
print(f"Average age: {df['age'].astype(int).mean()}")
```

### Creating RDF from Python Data Structures

Convert Python dictionaries and lists to RDF:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

def dict_to_rdf(data, subject_iri, store, namespace="http://example.org/property/"):
    """Convert a Python dict to RDF triples."""
    subject = NamedNode(subject_iri)

    for key, value in data.items():
        predicate = NamedNode(f"{namespace}{key}")

        if isinstance(value, str):
            obj = Literal(value)
        elif isinstance(value, bool):
            obj = Literal(str(value).lower(),
                         datatype=NamedNode("http://www.w3.org/2001/XMLSchema#boolean"))
        elif isinstance(value, int):
            obj = Literal(str(value),
                         datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))
        elif isinstance(value, float):
            obj = Literal(str(value),
                         datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal"))
        else:
            obj = Literal(str(value))

        store.add(Quad(subject, predicate, obj))

# Example usage
store = Store()
person_data = {
    "name": "Alice",
    "age": 30,
    "height": 1.75,
    "is_active": True,
    "city": "Springfield"
}

dict_to_rdf(person_data, "http://example.org/person/alice", store)

# Verify
for quad in store:
    print(f"{quad.predicate} -> {quad.object.value}")
```

### Working with JSON-LD

JSON-LD is JSON with RDF semantics, perfect for web APIs:

```python
from pyoxigraph import Store, RdfFormat

store = Store()

# JSON-LD data
jsonld_data = """
{
  "@context": "http://schema.org/",
  "@id": "http://example.org/alice",
  "@type": "Person",
  "name": "Alice",
  "age": 30,
  "knows": {
    "@id": "http://example.org/bob",
    "name": "Bob"
  }
}
"""

# Load JSON-LD
store.load(jsonld_data.encode('utf-8'), format=RdfFormat.JSON_LD)

# Query the data
query = """
    SELECT ?person ?name ?age
    WHERE {
        ?person <http://schema.org/name> ?name .
        OPTIONAL { ?person <http://schema.org/age> ?age }
    }
"""

for solution in store.query(query):
    print(f"{solution['name'].value}: {solution.get('age', 'unknown')}")
```

### Integration with rdflib

If you're already using rdflib, check out [oxrdflib](https://github.com/oxigraph/oxrdflib), which provides an rdflib store backed by Pyoxigraph:

```python
# pip install oxrdflib
from rdflib import Graph
from oxrdflib import OxigraphStore

# Create an rdflib graph with Pyoxigraph backend
graph = Graph(store=OxigraphStore())
graph.parse("data.ttl", format="turtle")

# Use standard rdflib API with Pyoxigraph performance
for s, p, o in graph:
    print(f"{s} {p} {o}")
```

## Best Practices

### 1. Use Bulk Operations for Large Datasets

```python
# Slow
for quad in large_list:
    store.add(quad)

# Fast
store.bulk_extend(large_list)
```

### 2. Define Vocabulary Namespaces

```python
# Good practice
SCHEMA = "http://schema.org/"
FOAF = "http://xmlns.com/foaf/0.1/"
EX = "http://example.org/"

name = NamedNode(f"{SCHEMA}name")
knows = NamedNode(f"{FOAF}knows")
person = NamedNode(f"{EX}person/alice")
```

### 3. Use Appropriate Datatypes

```python
# Strings can be compared and sorted correctly
name = Literal("Alice")

# Numbers need proper datatypes for numeric operations in SPARQL
age = Literal("30", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))
```

### 4. Choose the Right Store Type

```python
# For temporary operations - in-memory
temp_store = Store()

# For persistent data - disk-based
persistent_store = Store(path="my_data")
```

## Next Steps

- Learn advanced [SPARQL queries](python-sparql.md)
- Explore the [API reference](https://pyoxigraph.readthedocs.io/)
- Check out the [Oxigraph website](https://oxigraph.org/)

## Resources

- [RDF 1.1 Primer](https://www.w3.org/TR/rdf11-primer/)
- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
- [JSON-LD Specification](https://www.w3.org/TR/json-ld/)
- [Turtle Specification](https://www.w3.org/TR/turtle/)
