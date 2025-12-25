# Getting Started with Pyoxigraph

Pyoxigraph is a Python library for working with RDF graphs and executing SPARQL queries. It provides a fast, in-memory or persistent graph database with full SPARQL 1.1 support.

## Installation

### Using pip

The simplest way to install pyoxigraph is using pip:

```bash
pip install pyoxigraph
```

### Using conda

If you prefer conda, pyoxigraph is also available on conda-forge:

```bash
conda install -c conda-forge pyoxigraph
```

### Verifying the installation

You can verify the installation by importing the library:

```python
import pyoxigraph
print(f"Pyoxigraph version: {pyoxigraph.__version__}")
```

## Creating Your First Store

Pyoxigraph provides two types of stores:

1. **In-memory store** - Fast, but data is lost when the program ends
2. **Persistent store** - Data is saved to disk

### In-Memory Store

An in-memory store is perfect for quick experiments and temporary data:

```python
from pyoxigraph import Store

# Create an in-memory store
store = Store()
```

### Persistent Store

For data that needs to persist between sessions, create a store with a file path:

```python
from pyoxigraph import Store

# Create a persistent store
store = Store(path="my_database")
```

The data will be stored in the `my_database` directory. You can open the same store later to access your data:

```python
# Open an existing store
store = Store(path="my_database")
```

## Working with RDF Data

### Understanding RDF Basics

RDF (Resource Description Framework) represents information as triples:
- **Subject** - What you're describing
- **Predicate** - The property or relationship
- **Object** - The value or related resource

### Creating RDF Terms

Pyoxigraph provides three main types of RDF terms:

```python
from pyoxigraph import NamedNode, BlankNode, Literal

# Named nodes (IRIs) - for identifying resources
person = NamedNode("http://example.org/person/alice")
name_property = NamedNode("http://schema.org/name")

# Literals - for values
name_value = Literal("Alice")
age_value = Literal("30", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))

# Blank nodes - for anonymous resources
anonymous = BlankNode()
```

### Adding Triples to the Store

Use the `Quad` class to add data (a quad is a triple with an optional graph):

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

# Create a store
store = Store()

# Create terms
alice = NamedNode("http://example.org/person/alice")
name = NamedNode("http://schema.org/name")
age = NamedNode("http://schema.org/age")

# Add triples (quads with default graph)
store.add(Quad(alice, name, Literal("Alice")))
store.add(Quad(alice, age, Literal("30", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))))

print(f"Store now contains {len(store)} triples")
```

## Querying with SPARQL

SPARQL is the standard query language for RDF data. Let's start with a simple query.

### Your First SELECT Query

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

# Setup
store = Store()
alice = NamedNode("http://example.org/person/alice")
bob = NamedNode("http://example.org/person/bob")
name = NamedNode("http://schema.org/name")

store.add(Quad(alice, name, Literal("Alice")))
store.add(Quad(bob, name, Literal("Bob")))

# Query: Get all names
query = """
    SELECT ?person ?name
    WHERE {
        ?person <http://schema.org/name> ?name .
    }
"""

results = store.query(query)

# Iterate over results
for solution in results:
    print(f"Person: {solution['person']}, Name: {solution['name'].value}")
```

Output:
```
Person: http://example.org/person/alice, Name: Alice
Person: http://example.org/person/bob, Name: Bob
```

### ASK Queries

ASK queries return a boolean indicating whether a pattern exists:

```python
# Check if Alice exists
exists = store.query("ASK { <http://example.org/person/alice> ?p ?o }")
print(f"Alice exists: {exists}")  # True
```

### CONSTRUCT Queries

CONSTRUCT queries create new triples based on a template:

```python
# Create simplified triples
query = """
    CONSTRUCT {
        ?person <http://example.org/hasName> ?name .
    }
    WHERE {
        ?person <http://schema.org/name> ?name .
    }
"""

results = store.query(query)
for triple in results:
    print(f"{triple.subject} {triple.predicate} {triple.object}")
```

## Complete Working Example

Here's a complete example that demonstrates the basics:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad

def main():
    # Create an in-memory store
    store = Store()

    # Define vocabularies
    EX = "http://example.org/"
    SCHEMA = "http://schema.org/"

    # Create some people
    people = [
        {
            "id": "alice",
            "name": "Alice Smith",
            "age": 30,
            "email": "alice@example.org"
        },
        {
            "id": "bob",
            "name": "Bob Jones",
            "age": 25,
            "email": "bob@example.org"
        },
        {
            "id": "charlie",
            "name": "Charlie Brown",
            "age": 35,
            "email": "charlie@example.org"
        }
    ]

    # Add data to the store
    for person_data in people:
        person = NamedNode(f"{EX}person/{person_data['id']}")
        store.add(Quad(person, NamedNode(f"{SCHEMA}name"), Literal(person_data["name"])))
        store.add(Quad(person, NamedNode(f"{SCHEMA}age"),
                      Literal(str(person_data["age"]),
                             datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))))
        store.add(Quad(person, NamedNode(f"{SCHEMA}email"), Literal(person_data["email"])))

    print(f"Added {len(store)} triples to the store\n")

    # Query 1: Find all people
    print("All people:")
    query1 = """
        SELECT ?person ?name
        WHERE {
            ?person <http://schema.org/name> ?name .
        }
        ORDER BY ?name
    """
    for solution in store.query(query1):
        print(f"  - {solution['name'].value}")

    # Query 2: Find people older than 28
    print("\nPeople older than 28:")
    query2 = """
        SELECT ?name ?age
        WHERE {
            ?person <http://schema.org/name> ?name ;
                   <http://schema.org/age> ?age .
            FILTER(?age > 28)
        }
        ORDER BY DESC(?age)
    """
    for solution in store.query(query2):
        print(f"  - {solution['name'].value}: {solution['age'].value} years old")

    # Query 3: Count total people
    query3 = """
        SELECT (COUNT(?person) AS ?count)
        WHERE {
            ?person <http://schema.org/name> ?name .
        }
    """
    result = next(store.query(query3))
    print(f"\nTotal number of people: {result['count'].value}")

if __name__ == "__main__":
    main()
```

Output:
```
Added 9 triples to the store

All people:
  - Alice Smith
  - Bob Jones
  - Charlie Brown

People older than 28:
  - Charlie Brown: 35 years old
  - Alice Smith: 30 years old

Total number of people: 3
```

## Next Steps

Now that you understand the basics, you can:

- Learn more about [RDF data manipulation](python-rdf-data.md)
- Explore advanced [SPARQL queries](python-sparql.md)
- Read the [API reference](https://pyoxigraph.readthedocs.io/)

## Common Pitfalls

### 1. Forgetting to specify datatypes for numbers

```python
# Wrong - stored as string
store.add(Quad(person, age, Literal("30")))

# Right - stored as integer
store.add(Quad(person, age, Literal("30", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))))
```

### 2. Using `.value` when you don't need to

```python
# Query result
for solution in store.query("SELECT ?name WHERE { ?s <http://schema.org/name> ?name }"):
    # This gives you the Literal object
    name_literal = solution['name']

    # This gives you the string value
    name_string = solution['name'].value
```

### 3. Persistent stores are automatically managed

Persistent stores in pyoxigraph are automatically managed and don't require explicit closing:

```python
# Store is automatically flushed in background threads
store = Store(path="my_database")
store.add(Quad(subject, predicate, object))
# Data is persisted automatically

# You can force a flush if needed
store.flush()
```

## Resources

- [Pyoxigraph Documentation](https://pyoxigraph.readthedocs.io/)
- [SPARQL Tutorial](https://www.w3.org/TR/sparql11-query/)
- [RDF Primer](https://www.w3.org/TR/rdf11-primer/)
- [GitHub Repository](https://github.com/oxigraph/oxigraph)
