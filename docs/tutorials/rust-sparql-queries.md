# SPARQL Queries in Oxigraph

This tutorial teaches you how to query and update RDF data using SPARQL (SPARQL Protocol and RDF Query Language), the standard query language for RDF.

## What is SPARQL?

SPARQL is to RDF what SQL is to relational databases. It allows you to:

- **SELECT**: Query and retrieve data
- **CONSTRUCT**: Create new RDF graphs from query results
- **ASK**: Check if a pattern exists
- **DESCRIBE**: Get information about a resource
- **INSERT/DELETE**: Modify data

## Basic SELECT Queries

### Your First SPARQL Query

Let's start with a simple query that retrieves all triples:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Add some data
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    store.insert(&Quad::new(
        alice.clone(),
        knows,
        bob.clone(),
        GraphName::DefaultGraph
    ))?;

    // Simple SPARQL query
    let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";

    // Execute and process results
    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Results:");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "Subject: {}, Predicate: {}, Object: {}",
                solution.get("s").unwrap(),
                solution.get("p").unwrap(),
                solution.get("o").unwrap()
            );
        }
    }

    Ok(())
}
```

### Using Prefixes

Prefixes make queries more readable:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Add data
    let alice = NamedNode::new("http://example.org/alice")?;
    let person = NamedNode::new("http://xmlns.com/foaf/0.1/Person")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    store.insert(&Quad::new(
        alice.clone(),
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        person,
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        alice,
        name,
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;

    // Query with prefixes
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

        SELECT ?person ?name WHERE {
            ?person rdf:type foaf:Person .
            ?person foaf:name ?name .
        }
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "Person: {}, Name: {}",
                solution.get("person").unwrap(),
                solution.get("name").unwrap()
            );
        }
    }

    Ok(())
}
```

### Filtering Results

Use `FILTER` to restrict results:

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::xsd;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Add people with ages
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let charlie = NamedNode::new("http://example.org/charlie")?;
    let age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;

    store.insert(&Quad::new(
        alice.clone(),
        age.clone(),
        Literal::new_typed_literal("30", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        bob.clone(),
        age.clone(),
        Literal::new_typed_literal("25", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        charlie.clone(),
        age.clone(),
        Literal::new_typed_literal("35", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    // Query: Find people older than 27
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?person ?age WHERE {
            ?person foaf:age ?age .
            FILTER(?age > 27)
        }
        ORDER BY DESC(?age)
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("People older than 27:");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {} (age: {})",
                solution.get("person").unwrap(),
                solution.get("age").unwrap()
            );
        }
    }

    Ok(())
}
```

### Optional Patterns

Use `OPTIONAL` to include data that might not exist:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let email = NamedNode::new("http://xmlns.com/foaf/0.1/mbox")?;

    // Alice has name and email
    store.insert(&Quad::new(
        alice.clone(),
        name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        email.clone(),
        Literal::new_simple_literal("alice@example.org"),
        GraphName::DefaultGraph
    ))?;

    // Bob only has name (no email)
    store.insert(&Quad::new(
        bob.clone(),
        name.clone(),
        Literal::new_simple_literal("Bob"),
        GraphName::DefaultGraph
    ))?;

    // Query with OPTIONAL email
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?person ?name ?email WHERE {
            ?person foaf:name ?name .
            OPTIONAL { ?person foaf:mbox ?email }
        }
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            let email_str = match solution.get("email") {
                Some(e) => e.to_string(),
                None => "no email".to_string(),
            };
            println!(
                "{} ({})",
                solution.get("name").unwrap(),
                email_str
            );
        }
    }

    Ok(())
}
```

## ASK Queries

ASK queries return a boolean indicating whether a pattern exists:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    store.insert(&Quad::new(
        alice,
        knows,
        bob,
        GraphName::DefaultGraph
    ))?;

    // ASK if Alice knows Bob
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        ASK {
            ex:alice foaf:knows ex:bob
        }
    "#;

    if let QueryResults::Boolean(result) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Does Alice know Bob? {}", result);
    }

    Ok(())
}
```

## CONSTRUCT Queries

CONSTRUCT creates new RDF graphs from query results:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        bob.clone(),
        name.clone(),
        Literal::new_simple_literal("Bob"),
        GraphName::DefaultGraph
    ))?;

    // CONSTRUCT a new graph with friend names
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        CONSTRUCT {
            ?person ex:friendName ?friendName .
        }
        WHERE {
            ?person foaf:knows ?friend .
            ?friend foaf:name ?friendName .
        }
    "#;

    if let QueryResults::Graph(triples) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Constructed triples:");
        for triple in triples {
            let triple = triple?;
            println!("  {} {} {}", triple.subject, triple.predicate, triple.object);
        }
    }

    Ok(())
}
```

## INSERT and DELETE Operations

### INSERT DATA

Add new triples to the store:

```rust
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // INSERT new data
    let update = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        INSERT DATA {
            ex:alice foaf:name "Alice" .
            ex:alice foaf:knows ex:bob .
            ex:bob foaf:name "Bob" .
        }
    "#;

    SparqlEvaluator::new()
        .parse_update(update)?
        .on_store(&store)
        .execute()?;

    println!("Data inserted! Store now has {} quads", store.len()?);

    Ok(())
}
```

### DELETE DATA

Remove specific triples:

```rust
use oxigraph::model::*;
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // First insert some data
    let insert = r#"
        PREFIX ex: <http://example.org/>
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        INSERT DATA {
            ex:alice foaf:name "Alice" .
            ex:alice foaf:age 30 .
        }
    "#;

    SparqlEvaluator::new()
        .parse_update(insert)?
        .on_store(&store)
        .execute()?;

    println!("After INSERT: {} quads", store.len()?);

    // Delete specific data
    let delete = r#"
        PREFIX ex: <http://example.org/>
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        DELETE DATA {
            ex:alice foaf:age 30 .
        }
    "#;

    SparqlEvaluator::new()
        .parse_update(delete)?
        .on_store(&store)
        .execute()?;

    println!("After DELETE: {} quads", store.len()?);

    Ok(())
}
```

### DELETE/INSERT (Update with Patterns)

Modify data based on patterns:

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::xsd;
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Insert initial data
    let alice = NamedNode::new("http://example.org/alice")?;
    let age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;

    store.insert(&Quad::new(
        alice.clone(),
        age.clone(),
        Literal::new_typed_literal("30", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    println!("Initial age: 30");

    // Update Alice's age
    let update = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        DELETE {
            ex:alice foaf:age ?oldAge .
        }
        INSERT {
            ex:alice foaf:age 31 .
        }
        WHERE {
            ex:alice foaf:age ?oldAge .
        }
    "#;

    SparqlEvaluator::new()
        .parse_update(update)?
        .on_store(&store)
        .execute()?;

    println!("Age updated to 31!");

    Ok(())
}
```

## Advanced Query Patterns

### Aggregation

Count, sum, average, etc.:

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::xsd;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;

    // Add multiple people with ages
    for (name, age_val) in [("alice", "30"), ("bob", "25"), ("charlie", "35")] {
        let person = NamedNode::new(&format!("http://example.org/{}", name))?;
        store.insert(&Quad::new(
            person,
            age.clone(),
            Literal::new_typed_literal(age_val, xsd::INTEGER),
            GraphName::DefaultGraph
        ))?;
    }

    // Query with aggregation
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT (COUNT(?person) AS ?count) (AVG(?age) AS ?avgAge) WHERE {
            ?person foaf:age ?age .
        }
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        if let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "Count: {}, Average age: {}",
                solution.get("count").unwrap(),
                solution.get("avgAge").unwrap()
            );
        }
    }

    Ok(())
}
```

### GROUP BY

Group results and aggregate:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    // Alice knows 2 people, Bob knows 1
    store.insert(&Quad::new(
        NamedNode::new("http://example.org/alice")?,
        knows.clone(),
        NamedNode::new("http://example.org/bob")?,
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        NamedNode::new("http://example.org/alice")?,
        knows.clone(),
        NamedNode::new("http://example.org/charlie")?,
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        NamedNode::new("http://example.org/bob")?,
        knows.clone(),
        NamedNode::new("http://example.org/charlie")?,
        GraphName::DefaultGraph
    ))?;

    // Count friends per person
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?person (COUNT(?friend) AS ?friendCount) WHERE {
            ?person foaf:knows ?friend .
        }
        GROUP BY ?person
        ORDER BY DESC(?friendCount)
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Friend counts:");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {}: {} friends",
                solution.get("person").unwrap(),
                solution.get("friendCount").unwrap()
            );
        }
    }

    Ok(())
}
```

### UNION

Query multiple patterns:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let alice = NamedNode::new("http://example.org/alice")?;
    let email = NamedNode::new("http://xmlns.com/foaf/0.1/mbox")?;
    let phone = NamedNode::new("http://xmlns.com/foaf/0.1/phone")?;

    store.insert(&Quad::new(
        alice.clone(),
        email,
        Literal::new_simple_literal("alice@example.org"),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        phone,
        Literal::new_simple_literal("+1-555-0123"),
        GraphName::DefaultGraph
    ))?;

    // Find either email or phone
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        SELECT ?contactType ?value WHERE {
            {
                ex:alice foaf:mbox ?value .
                BIND("email" AS ?contactType)
            }
            UNION
            {
                ex:alice foaf:phone ?value .
                BIND("phone" AS ?contactType)
            }
        }
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Contact information:");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {}: {}",
                solution.get("contactType").unwrap(),
                solution.get("value").unwrap()
            );
        }
    }

    Ok(())
}
```

## Real-World Example: Social Network

Here's a complete example modeling a social network:

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Social Network Example ===\n");

    let store = Store::new()?;

    // Build the social network
    build_social_network(&store)?;

    // Run various queries
    println!("1. Find all users:");
    find_all_users(&store)?;

    println!("\n2. Find friends of Alice:");
    find_friends_of(&store, "alice")?;

    println!("\n3. Find mutual friends:");
    find_mutual_friends(&store, "alice", "charlie")?;

    println!("\n4. Find users by age range:");
    find_users_by_age_range(&store, 25, 35)?;

    println!("\n5. Count posts per user:");
    count_posts_per_user(&store)?;

    Ok(())
}

fn build_social_network(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let update = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>
        PREFIX schema: <http://schema.org/>

        INSERT DATA {
            # Users
            ex:alice a foaf:Person ;
                foaf:name "Alice Smith" ;
                foaf:age 30 ;
                foaf:knows ex:bob, ex:charlie .

            ex:bob a foaf:Person ;
                foaf:name "Bob Jones" ;
                foaf:age 25 ;
                foaf:knows ex:alice, ex:david .

            ex:charlie a foaf:Person ;
                foaf:name "Charlie Brown" ;
                foaf:age 35 ;
                foaf:knows ex:alice, ex:david .

            ex:david a foaf:Person ;
                foaf:name "David Wilson" ;
                foaf:age 28 ;
                foaf:knows ex:bob, ex:charlie .

            # Posts
            ex:post1 a schema:BlogPosting ;
                schema:author ex:alice ;
                schema:headline "My First Post" .

            ex:post2 a schema:BlogPosting ;
                schema:author ex:alice ;
                schema:headline "Another Post" .

            ex:post3 a schema:BlogPosting ;
                schema:author ex:bob ;
                schema:headline "Bob's Thoughts" .
        }
    "#;

    SparqlEvaluator::new()
        .parse_update(update)?
        .on_store(store)
        .execute()?;

    println!("Social network created with {} quads\n", store.len()?);

    Ok(())
}

fn find_all_users(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?user ?name WHERE {
            ?user a foaf:Person ;
                  foaf:name ?name .
        }
        ORDER BY ?name
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  {}", solution.get("name").unwrap());
        }
    }

    Ok(())
}

fn find_friends_of(store: &Store, username: &str) -> Result<(), Box<dyn std::error::Error>> {
    let query = format!(
        r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        SELECT ?friendName WHERE {{
            ex:{} foaf:knows ?friend .
            ?friend foaf:name ?friendName .
        }}
        ORDER BY ?friendName
        "#,
        username
    );

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(&query)?
        .on_store(store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  {}", solution.get("friendName").unwrap());
        }
    }

    Ok(())
}

fn find_mutual_friends(
    store: &Store,
    user1: &str,
    user2: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = format!(
        r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        SELECT ?mutualFriendName WHERE {{
            ex:{} foaf:knows ?mutualFriend .
            ex:{} foaf:knows ?mutualFriend .
            ?mutualFriend foaf:name ?mutualFriendName .
        }}
        "#,
        user1, user2
    );

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(&query)?
        .on_store(store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("  {}", solution.get("mutualFriendName").unwrap());
        }
    }

    Ok(())
}

fn find_users_by_age_range(
    store: &Store,
    min_age: i32,
    max_age: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = format!(
        r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?name ?age WHERE {{
            ?user a foaf:Person ;
                  foaf:name ?name ;
                  foaf:age ?age .
            FILTER(?age >= {} && ?age <= {})
        }}
        ORDER BY ?age
        "#,
        min_age, max_age
    );

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(&query)?
        .on_store(store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {} (age {})",
                solution.get("name").unwrap(),
                solution.get("age").unwrap()
            );
        }
    }

    Ok(())
}

fn count_posts_per_user(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX schema: <http://schema.org/>

        SELECT ?authorName (COUNT(?post) AS ?postCount) WHERE {
            ?post a schema:BlogPosting ;
                  schema:author ?author .
            ?author foaf:name ?authorName .
        }
        GROUP BY ?authorName
        ORDER BY DESC(?postCount)
    "#;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {}: {} posts",
                solution.get("authorName").unwrap(),
                solution.get("postCount").unwrap()
            );
        }
    }

    Ok(())
}
```

## Query Results Handling

### Iterating Through Solutions

```rust
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    // ... add data ...

    let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        // Get variable names
        let variables: Vec<_> = solutions.variables().iter()
            .map(|v| v.as_str())
            .collect();
        println!("Variables: {:?}", variables);

        // Iterate solutions
        while let Some(solution) = solutions.next() {
            let solution = solution?;

            // Access by variable name
            if let Some(value) = solution.get("s") {
                println!("Subject: {}", value);
            }

            // Iterate all bindings
            for (var, value) in solution.iter() {
                println!("{}: {}", var, value);
            }
        }
    }

    Ok(())
}
```

### Handling Different Result Types

```rust
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn execute_query(store: &Store, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    match SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(store)
        .execute()?
    {
        QueryResults::Solutions(mut solutions) => {
            println!("SELECT results:");
            while let Some(solution) = solutions.next() {
                println!("  {:?}", solution?);
            }
        }
        QueryResults::Graph(triples) => {
            println!("CONSTRUCT results:");
            for triple in triples {
                println!("  {:?}", triple?);
            }
        }
        QueryResults::Boolean(result) => {
            println!("ASK result: {}", result);
        }
    }

    Ok(())
}
```

## Best Practices

### 1. Use Prepared Queries for Reusability

```rust
use oxigraph::sparql::SparqlEvaluator;

// Create evaluator with common prefixes
let evaluator = SparqlEvaluator::new()
    .with_prefix("foaf", "http://xmlns.com/foaf/0.1/")?
    .with_prefix("ex", "http://example.org/")?;

// Reuse for multiple queries
let query1 = evaluator.parse_query("SELECT * WHERE { ?s foaf:name ?name }")?;
let query2 = evaluator.parse_query("SELECT * WHERE { ?s foaf:age ?age }")?;
```

### 2. Use LIMIT for Large Result Sets

```rust
let query = r#"
    SELECT ?s ?p ?o WHERE {
        ?s ?p ?o
    }
    LIMIT 100
"#;
```

### 3. Use DISTINCT to Remove Duplicates

```rust
let query = r#"
    SELECT DISTINCT ?person WHERE {
        { ?person foaf:knows ?friend1 }
        UNION
        { ?friend2 foaf:knows ?person }
    }
"#;
```

### 4. Error Handling

```rust
match SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()
{
    Ok(QueryResults::Solutions(mut solutions)) => {
        while let Some(solution) = solutions.next() {
            match solution {
                Ok(s) => println!("{:?}", s),
                Err(e) => eprintln!("Error reading solution: {}", e),
            }
        }
    }
    Err(e) => eprintln!("Query execution error: {}", e),
}
```

## Summary

You've learned how to:

- ✅ Write SELECT queries with filters and patterns
- ✅ Use ASK to check for pattern existence
- ✅ Create new graphs with CONSTRUCT
- ✅ Modify data with INSERT and DELETE
- ✅ Use advanced patterns (OPTIONAL, UNION, aggregation)
- ✅ Handle different query result types
- ✅ Build real-world queries for a social network

## Next Steps

- **SPARQL 1.1 Specification**: Read the [official W3C SPARQL 1.1 specification](https://www.w3.org/TR/sparql11-query/) for complete details
- **Federated Queries**: Explore SERVICE calls for querying remote SPARQL endpoints
- **Property Paths**: Learn about advanced graph traversal with property paths
- **Custom Functions**: Add custom SPARQL functions for domain-specific operations
- **Oxigraph Documentation**: Check the [API docs](https://docs.rs/oxigraph) for more features

Happy querying!
