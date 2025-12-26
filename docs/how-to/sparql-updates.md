# SPARQL Updates

A comprehensive guide to modifying RDF data using SPARQL Update operations in Oxigraph.

## Table of Contents

- [Introduction](#introduction)
- [INSERT DATA](#insert-data)
- [DELETE DATA](#delete-data)
- [INSERT/DELETE WHERE](#insertdelete-where)
- [LOAD and CLEAR](#load-and-clear)
- [CREATE and DROP](#create-and-drop)
- [COPY, MOVE, and ADD](#copy-move-and-add)
- [Transaction Handling](#transaction-handling)
- [Best Practices](#best-practices)

## Introduction

SPARQL Update is the part of the SPARQL specification for modifying RDF data. It provides operations to insert, delete, and manage RDF triples and graphs.

### Update vs Query

- **SPARQL Query**: Retrieves data (SELECT, ASK, CONSTRUCT, DESCRIBE)
- **SPARQL Update**: Modifies data (INSERT, DELETE, LOAD, CLEAR, etc.)

## INSERT DATA

Inserts specific triples into the graph store.

### Basic Syntax

```sparql
INSERT DATA {
  triples
}
```

### Simple Insert

**SPARQL:**
```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  ex:alice foaf:name "Alice" ;
           foaf:age 30 .
}
```

**Rust:**
```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

let update = r#"
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  ex:alice foaf:name "Alice" ;
           foaf:age 30 .
}
"#;

SparqlEvaluator::new()
    .parse_update(update)?
    .on_store(&store)
    .execute()?;
```

**JavaScript:**
```javascript
import { Store } from 'oxigraph';

const store = new Store();

store.update(`
  PREFIX ex: <http://example.com/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  INSERT DATA {
    ex:alice foaf:name "Alice" ;
             foaf:age 30 .
  }
`);
```

**Python:**
```python
from pyoxigraph import Store

store = Store()

store.update("""
  PREFIX ex: <http://example.com/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  INSERT DATA {
    ex:alice foaf:name "Alice" ;
             foaf:age 30 .
  }
""")
```

### Multiple Triples

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  ex:alice foaf:name "Alice" ;
           foaf:age 30 ;
           foaf:knows ex:bob .

  ex:bob foaf:name "Bob" ;
         foaf:age 28 .
}
```

### Different Data Types

```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {
  ex:product1 ex:name "Laptop" ;
              ex:price 999.99 ;
              ex:quantity 5 ;
              ex:inStock true ;
              ex:releaseDate "2024-01-15"^^xsd:date ;
              ex:description "A powerful laptop"@en .
}
```

### Inserting into Named Graphs

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  GRAPH ex:users {
    ex:alice foaf:name "Alice" ;
             foaf:mbox <mailto:alice@example.com> .
  }

  GRAPH ex:metadata {
    ex:users ex:createdDate "2024-01-01"^^xsd:date .
  }
}
```

### Practical Example: Bulk Insert

```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

// Insert multiple users at once
let update = r#"
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  ex:alice foaf:name "Alice" ; foaf:age 30 .
  ex:bob foaf:name "Bob" ; foaf:age 28 .
  ex:charlie foaf:name "Charlie" ; foaf:age 35 .
  ex:diana foaf:name "Diana" ; foaf:age 32 .

  ex:alice foaf:knows ex:bob, ex:charlie .
  ex:bob foaf:knows ex:alice, ex:diana .
  ex:charlie foaf:knows ex:alice, ex:diana .
  ex:diana foaf:knows ex:bob, ex:charlie .
}
"#;

SparqlEvaluator::new()
    .parse_update(update)?
    .on_store(&store)
    .execute()?;

println!("Inserted {} quads", store.len()?);
```

## DELETE DATA

Removes specific triples from the graph store.

### Basic Syntax

```sparql
DELETE DATA {
  triples
}
```

### Simple Delete

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE DATA {
  ex:alice foaf:age 30 .
}
```

### Delete Multiple Triples

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE DATA {
  ex:alice foaf:knows ex:bob .
  ex:bob foaf:knows ex:alice .
}
```

### Delete from Named Graph

```sparql
PREFIX ex: <http://example.com/>

DELETE DATA {
  GRAPH ex:oldData {
    ex:obsoleteItem ex:property "old value" .
  }
}
```

### Practical Example: Delete User

```python
from pyoxigraph import Store

store = Store()

# First, add some data
store.update("""
  PREFIX ex: <http://example.com/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  INSERT DATA {
    ex:tempUser foaf:name "Temp User" ;
                foaf:mbox <mailto:temp@example.com> ;
                foaf:knows ex:alice .
  }
""")

# Now delete it
store.update("""
  PREFIX ex: <http://example.com/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  DELETE DATA {
    ex:tempUser foaf:name "Temp User" ;
                foaf:mbox <mailto:temp@example.com> ;
                foaf:knows ex:alice .
  }
""")
```

## INSERT/DELETE WHERE

Conditionally insert or delete triples based on pattern matching.

### DELETE WHERE

Deletes triples matching a pattern.

**Basic Syntax:**
```sparql
DELETE WHERE {
  pattern
}
```

**Example:**
```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE WHERE {
  ?person foaf:age ?age .
  FILTER(?age < 0)
}
```

Deletes all negative ages (invalid data).

### DELETE/INSERT

Combines deletion and insertion in a single operation.

**Basic Syntax:**
```sparql
DELETE {
  delete_pattern
}
INSERT {
  insert_pattern
}
WHERE {
  match_pattern
}
```

### Update Example: Change Property

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE {
  ?person foaf:firstName ?oldFirst ;
          foaf:lastName ?oldLast .
}
INSERT {
  ?person foaf:name ?fullName .
}
WHERE {
  ?person foaf:firstName ?oldFirst ;
          foaf:lastName ?oldLast .
  BIND(CONCAT(?oldFirst, " ", ?oldLast) AS ?fullName)
}
```

Replaces firstName/lastName with a combined name.

### Update with Calculations

```sparql
PREFIX ex: <http://example.com/>

DELETE {
  ?product ex:price ?oldPrice .
}
INSERT {
  ?product ex:price ?newPrice .
}
WHERE {
  ?product ex:price ?oldPrice ;
           ex:category ex:Electronics .
  BIND(?oldPrice * 0.9 AS ?newPrice)  # 10% discount
}
```

### Increment Counter

```sparql
PREFIX ex: <http://example.com/>

DELETE {
  ?item ex:viewCount ?oldCount .
}
INSERT {
  ?item ex:viewCount ?newCount .
}
WHERE {
  ?item ex:id "item123" ;
        ex:viewCount ?oldCount .
  BIND(?oldCount + 1 AS ?newCount)
}
```

### Practical Example: Update User Profile

```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

// Update a user's email and last modified date
let update = r#"
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

DELETE {
  ex:alice foaf:mbox ?oldEmail ;
           ex:lastModified ?oldDate .
}
INSERT {
  ex:alice foaf:mbox <mailto:alice.new@example.com> ;
           ex:lastModified ?now .
}
WHERE {
  ex:alice foaf:mbox ?oldEmail .
  OPTIONAL { ex:alice ex:lastModified ?oldDate }
  BIND(NOW() AS ?now)
}
"#;

SparqlEvaluator::new()
    .parse_update(update)?
    .on_store(&store)
    .execute()?;
```

### Complex Example: Normalize Data

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE {
  ?person foaf:name ?name .
}
INSERT {
  ?person foaf:name ?normalizedName .
}
WHERE {
  ?person foaf:name ?name .
  FILTER(STRLEN(?name) > 0)
  BIND(CONCAT(
    UCASE(SUBSTR(?name, 1, 1)),
    LCASE(SUBSTR(?name, 2))
  ) AS ?normalizedName)
  FILTER(?name != ?normalizedName)
}
```

Capitalizes first letter and lowercases the rest.

### Conditional Updates

```sparql
PREFIX ex: <http://example.com/>

DELETE {
  ?product ex:status ?oldStatus .
}
INSERT {
  ?product ex:status ?newStatus .
}
WHERE {
  ?product ex:stock ?stock ;
           ex:status ?oldStatus .

  BIND(
    IF(?stock = 0, "Out of Stock",
    IF(?stock < 10, "Low Stock", "In Stock"))
    AS ?newStatus
  )

  FILTER(?oldStatus != ?newStatus)
}
```

## LOAD and CLEAR

### LOAD

Loads RDF data from a URI into a graph.

**Syntax:**
```sparql
LOAD <source-uri> [INTO GRAPH <target-graph>]
```

**Example:**
```sparql
# Load into default graph
LOAD <http://example.com/data.ttl>

# Load into named graph
LOAD <http://example.com/data.ttl> INTO GRAPH <http://example.com/graph1>
```

**Load SILENT:**
```sparql
# Don't fail if loading fails
LOAD SILENT <http://example.com/data.ttl> INTO GRAPH <http://example.com/graph1>
```

### CLEAR

Removes all triples from a graph.

**Syntax:**
```sparql
CLEAR [SILENT] (DEFAULT | NAMED | ALL | GRAPH <uri>)
```

**Examples:**
```sparql
# Clear default graph
CLEAR DEFAULT

# Clear a specific named graph
CLEAR GRAPH <http://example.com/graph1>

# Clear all named graphs (keeping default)
CLEAR NAMED

# Clear everything
CLEAR ALL

# Don't fail if graph doesn't exist
CLEAR SILENT GRAPH <http://example.com/graph1>
```

### Practical Example: Replace Graph Data

```javascript
import { Store } from 'oxigraph';

const store = new Store();

// Clear old data and load new data
store.update(`
  PREFIX ex: <http://example.com/>

  # Clear the graph
  CLEAR GRAPH ex:productCatalog ;

  # Insert new data
  INSERT DATA {
    GRAPH ex:productCatalog {
      ex:product1 ex:name "New Product" ;
                  ex:price 99.99 .
      ex:product2 ex:name "Another Product" ;
                  ex:price 149.99 .
    }
  }
`);
```

## CREATE and DROP

### CREATE

Creates a new empty graph.

**Syntax:**
```sparql
CREATE [SILENT] GRAPH <uri>
```

**Example:**
```sparql
CREATE GRAPH <http://example.com/newGraph>

# Don't fail if graph already exists
CREATE SILENT GRAPH <http://example.com/newGraph>
```

### DROP

Removes a graph and all its triples.

**Syntax:**
```sparql
DROP [SILENT] (DEFAULT | NAMED | ALL | GRAPH <uri>)
```

**Examples:**
```sparql
# Drop a specific graph
DROP GRAPH <http://example.com/oldGraph>

# Drop default graph
DROP DEFAULT

# Drop all named graphs
DROP NAMED

# Drop everything
DROP ALL

# Don't fail if graph doesn't exist
DROP SILENT GRAPH <http://example.com/maybeGraph>
```

### Practical Example: Manage Graphs

```python
from pyoxigraph import Store

store = Store()

# Create a new graph for user data
store.update("""
  CREATE SILENT GRAPH <http://example.com/users>
""")

# Add data to it
store.update("""
  PREFIX ex: <http://example.com/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  INSERT DATA {
    GRAPH ex:users {
      ex:alice foaf:name "Alice" .
      ex:bob foaf:name "Bob" .
    }
  }
""")

# Later, if needed, drop the graph
store.update("""
  DROP GRAPH <http://example.com/users>
""")
```

## COPY, MOVE, and ADD

### COPY

Copies all triples from one graph to another, replacing the target.

**Syntax:**
```sparql
COPY [SILENT] (DEFAULT | GRAPH <source>) TO (DEFAULT | GRAPH <target>)
```

**Example:**
```sparql
PREFIX ex: <http://example.com/>

# Copy from one graph to another
COPY GRAPH ex:source TO GRAPH ex:target

# Copy default graph to named graph
COPY DEFAULT TO GRAPH ex:backup
```

### MOVE

Moves all triples from source to target, removing them from source.

**Syntax:**
```sparql
MOVE [SILENT] (DEFAULT | GRAPH <source>) TO (DEFAULT | GRAPH <target>)
```

**Example:**
```sparql
PREFIX ex: <http://example.com/>

# Move from one graph to another
MOVE GRAPH ex:temporary TO GRAPH ex:permanent
```

### ADD

Adds all triples from source to target (target keeps existing data).

**Syntax:**
```sparql
ADD [SILENT] (DEFAULT | GRAPH <source>) TO (DEFAULT | GRAPH <target>)
```

**Example:**
```sparql
PREFIX ex: <http://example.com/>

# Add triples from one graph to another
ADD GRAPH ex:additional TO GRAPH ex:main
```

### Practical Example: Backup and Restore

```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

// Create a backup
let backup = r#"
PREFIX ex: <http://example.com/>

# Copy main data to backup
COPY GRAPH ex:mainData TO GRAPH ex:backup
"#;

SparqlEvaluator::new()
    .parse_update(backup)?
    .on_store(&store)
    .execute()?;

// Later, restore from backup
let restore = r#"
PREFIX ex: <http://example.com/>

# Restore from backup
COPY GRAPH ex:backup TO GRAPH ex:mainData
"#;

SparqlEvaluator::new()
    .parse_update(restore)?
    .on_store(&store)
    .execute()?;
```

## Transaction Handling

Oxigraph ensures ACID properties for updates:

- **Atomicity**: Updates either complete fully or not at all
- **Consistency**: Database remains in a valid state
- **Isolation**: Concurrent operations don't interfere
- **Durability**: Committed changes persist

### Single Update Transaction

Each update operation is atomic:

```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

// This entire update is atomic
let update = r#"
PREFIX ex: <http://example.com/>

DELETE {
  ?person ex:status "pending" .
}
INSERT {
  ?person ex:status "active" ;
          ex:activatedAt ?now .
}
WHERE {
  ?person ex:status "pending" .
  BIND(NOW() AS ?now)
}
"#;

// Either all matching persons are updated, or none
SparqlEvaluator::new()
    .parse_update(update)?
    .on_store(&store)
    .execute()?;
```

### Multiple Updates

You can chain multiple update operations in one request:

```sparql
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

# First operation
INSERT DATA {
  ex:alice foaf:name "Alice" .
} ;

# Second operation
INSERT DATA {
  ex:bob foaf:name "Bob" .
} ;

# Third operation
INSERT DATA {
  ex:alice foaf:knows ex:bob .
}
```

**Note:** Each operation separated by `;` is executed in sequence.

### Error Handling

```rust
use oxigraph::store::Store;
use oxigraph::sparql::SparqlEvaluator;

let store = Store::new()?;

let update = r#"
PREFIX ex: <http://example.com/>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  ex:newUser foaf:name "New User" ;
             foaf:age 25 .
}
"#;

match SparqlEvaluator::new()
    .parse_update(update)?
    .on_store(&store)
    .execute()
{
    Ok(_) => println!("Update successful"),
    Err(e) => eprintln!("Update failed: {}", e),
}
```

### Async Updates (JavaScript)

```javascript
import { Store } from 'oxigraph';

const store = new Store();

async function updateUser(userId, newEmail) {
  try {
    await store.updateAsync(`
      PREFIX ex: <http://example.com/>
      PREFIX foaf: <http://xmlns.com/foaf/0.1/>

      DELETE {
        ex:${userId} foaf:mbox ?oldEmail .
      }
      INSERT {
        ex:${userId} foaf:mbox <mailto:${newEmail}> ;
                     ex:updated ?now .
      }
      WHERE {
        ex:${userId} foaf:mbox ?oldEmail .
        BIND(NOW() AS ?now)
      }
    `);
    console.log('User updated successfully');
  } catch (error) {
    console.error('Update failed:', error);
  }
}

await updateUser('alice', 'alice.new@example.com');
```

## Best Practices

### 1. Use DELETE/INSERT Instead of Separate Operations

**Good:**
```sparql
DELETE {
  ?person ex:status ?oldStatus .
}
INSERT {
  ?person ex:status ?newStatus .
}
WHERE {
  ?person ex:id "123" ;
          ex:status ?oldStatus .
  BIND("active" AS ?newStatus)
}
```

**Avoid:**
```sparql
DELETE WHERE {
  ?person ex:id "123" ;
          ex:status ?status .
} ;

INSERT DATA {
  ex:person123 ex:status "active" .
}
```

### 2. Use SILENT for Optional Operations

```sparql
# Won't fail if graph doesn't exist
CLEAR SILENT GRAPH <http://example.com/maybeGraph> ;

# Won't fail if file can't be loaded
LOAD SILENT <http://example.com/optional-data.ttl>
```

### 3. Validate Before Updating

```sparql
PREFIX ex: <http://example.com/>

DELETE {
  ?product ex:price ?oldPrice .
}
INSERT {
  ?product ex:price ?newPrice .
}
WHERE {
  ?product ex:id ?id ;
           ex:price ?oldPrice .

  # Validate that new price is positive
  BIND(?oldPrice * 1.1 AS ?newPrice)
  FILTER(?newPrice > 0)
}
```

### 4. Use Timestamps for Audit Trail

```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

DELETE {
  ?item ex:status ?oldStatus .
}
INSERT {
  ?item ex:status ?newStatus ;
        ex:statusChangedAt ?now ;
        ex:statusChangedBy ?user .
}
WHERE {
  VALUES (?item ?newStatus ?user) {
    (ex:item1 "completed" ex:user123)
  }
  ?item ex:status ?oldStatus .
  BIND(NOW() AS ?now)
}
```

### 5. Batch Updates When Possible

```sparql
PREFIX ex: <http://example.com/>

# Update multiple items at once
DELETE {
  ?product ex:discount ?oldDiscount .
}
INSERT {
  ?product ex:discount 0.20 .
}
WHERE {
  ?product ex:category ex:Electronics ;
           ex:discount ?oldDiscount .
  FILTER(?oldDiscount < 0.20)
}
```

### 6. Use Named Graphs for Organization

```sparql
PREFIX ex: <http://example.com/>

INSERT DATA {
  # User data in one graph
  GRAPH ex:users {
    ex:alice ex:name "Alice" .
  }

  # Metadata in another
  GRAPH ex:metadata {
    ex:users ex:lastUpdated "2024-01-01"^^xsd:date .
  }

  # Application data in another
  GRAPH ex:application {
    ex:setting1 ex:value "enabled" .
  }
}
```

## Common Update Patterns

### Pattern 1: Upsert (Update or Insert)

```sparql
PREFIX ex: <http://example.com/>

DELETE {
  ?item ex:property ?oldValue .
}
INSERT {
  ?item ex:property ?newValue .
}
WHERE {
  BIND(ex:item123 AS ?item)
  BIND("new value" AS ?newValue)
  OPTIONAL { ?item ex:property ?oldValue }
}
```

### Pattern 2: Conditional Delete

```sparql
PREFIX ex: <http://example.com/>

DELETE WHERE {
  ?item ex:expiryDate ?date .
  FILTER(?date < NOW())
}
```

### Pattern 3: Data Migration

```sparql
PREFIX ex: <http://example.com/>
PREFIX old: <http://old.example.com/>
PREFIX new: <http://new.example.com/>

DELETE {
  ?person old:emailAddress ?email .
}
INSERT {
  ?person new:email ?email .
}
WHERE {
  ?person old:emailAddress ?email .
}
```

### Pattern 4: Denormalization

```sparql
PREFIX ex: <http://example.com/>

INSERT {
  ?product ex:categoryName ?categoryName .
}
WHERE {
  ?product ex:category ?category .
  ?category ex:name ?categoryName .

  # Only add if not already present
  FILTER NOT EXISTS { ?product ex:categoryName ?existing }
}
```

### Pattern 5: Data Cleanup

```sparql
PREFIX ex: <http://example.com/>

# Remove duplicate properties, keeping only one
DELETE {
  ?s ?p ?o2 .
}
WHERE {
  ?s ?p ?o1, ?o2 .
  FILTER(?o1 < ?o2)
  FILTER(?p = ex:uniqueProperty)
}
```

## Performance Considerations

1. **Batch operations**: Update multiple items in one operation when possible
2. **Use specific patterns**: More specific WHERE patterns execute faster
3. **Index-friendly updates**: Oxigraph maintains SPO, POS, OSP indexes automatically
4. **Avoid large transactions**: Very large updates may require more memory
5. **Use CLEAR instead of DELETE WHERE**: `CLEAR GRAPH` is more efficient than `DELETE WHERE { GRAPH ?g { ?s ?p ?o } }`

## Troubleshooting

### Update Doesn't Match Anything

```sparql
# Check what would match first
PREFIX ex: <http://example.com/>

SELECT ?item ?oldValue
WHERE {
  ?item ex:property ?oldValue .
  FILTER(?oldValue = "expected value")
}

# Then run update
DELETE {
  ?item ex:property ?oldValue .
}
INSERT {
  ?item ex:property "new value" .
}
WHERE {
  ?item ex:property ?oldValue .
  FILTER(?oldValue = "expected value")
}
```

### Update Affects Too Many Items

```sparql
# Use LIMIT in subquery to be cautious
PREFIX ex: <http://example.com/>

DELETE {
  ?item ex:status ?oldStatus .
}
INSERT {
  ?item ex:status "archived" .
}
WHERE {
  {
    SELECT ?item ?oldStatus
    WHERE {
      ?item ex:status ?oldStatus ;
            ex:lastActive ?date .
      FILTER(?date < "2020-01-01"^^xsd:date)
    }
    LIMIT 100  # Process in batches
  }
}
```

## Next Steps

- [SPARQL Introduction](../tutorials/sparql-introduction.md) - Learn basic SPARQL queries
- [Advanced SPARQL Queries](sparql-advanced-queries.md) - Complex query patterns
- [SPARQL Functions Reference](../reference/sparql-functions.md) - All available functions

## Resources

- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)
- [Oxigraph Store API](https://docs.rs/oxigraph/latest/oxigraph/store/)
- [SPARQL Update Examples](https://www.w3.org/TR/sparql11-update/#examples)
