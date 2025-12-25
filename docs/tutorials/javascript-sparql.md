# Advanced SPARQL with Oxigraph for JavaScript

This tutorial covers advanced SPARQL querying techniques, async operations, result handling, and building semantic web applications with Oxigraph.

## Overview

Oxigraph provides full SPARQL 1.1 support including:

- SELECT, ASK, CONSTRUCT, and DESCRIBE queries
- SPARQL UPDATE operations (INSERT, DELETE)
- Named graphs and graph management
- Query options (base IRI, prefixes, dataset specification)
- Both synchronous and asynchronous query execution
- Multiple result serialization formats

## SPARQL Query Types

### SELECT Queries

SELECT queries return variable bindings:

```javascript
const oxigraph = require('oxigraph');
const store = new oxigraph.Store();

// Load sample data
store.load(`
    @prefix foaf: <http://xmlns.com/foaf/0.1/> .
    @prefix ex: <http://example.org/> .

    ex:alice foaf:name "Alice" ;
             foaf:age 30 ;
             foaf:knows ex:bob, ex:charlie .

    ex:bob foaf:name "Bob" ;
           foaf:age 25 .

    ex:charlie foaf:name "Charlie" ;
               foaf:age 35 ;
               foaf:knows ex:alice .
`, { format: 'ttl' });

// Simple SELECT
const query = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?person ?name WHERE {
        ?person foaf:name ?name .
    }
    ORDER BY ?name
`;

const results = store.query(query);
// Results is an array of Maps

for (const binding of results) {
    console.log('Person:', binding.get('person').value);
    console.log('Name:', binding.get('name').value);
    console.log('---');
}
```

### ASK Queries

ASK queries return a boolean:

```javascript
// Check if pattern exists
const askQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    ASK {
        ?person foaf:age ?age .
        FILTER(?age > 30)
    }
`;

const hasOlderPerson = store.query(askQuery);
console.log('Has person over 30:', hasOlderPerson); // true or false

if (hasOlderPerson) {
    console.log('Found at least one person over 30');
}
```

### CONSTRUCT Queries

CONSTRUCT queries return a graph (array of quads):

```javascript
// Create new triples from existing patterns
const constructQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    CONSTRUCT {
        ?person a ex:Adult .
        ?person ex:canVote true .
    }
    WHERE {
        ?person foaf:age ?age .
        FILTER(?age >= 18)
    }
`;

const newTriples = store.query(constructQuery);
console.log(`Generated ${newTriples.length} new triples`);

// Create a new store with the constructed data
const adultsStore = new oxigraph.Store(newTriples);

// Or add to existing store
for (const triple of newTriples) {
    store.add(triple);
}
```

### DESCRIBE Queries

DESCRIBE returns all information about a resource:

```javascript
const describeQuery = `
    PREFIX ex: <http://example.org/>

    DESCRIBE ex:alice
`;

const description = store.query(describeQuery);
console.log(`Description contains ${description.length} triples`);

for (const triple of description) {
    console.log(triple.toString());
}
```

## Query Options

### Setting Base IRI

```javascript
// Using string base IRI
const results = store.query(
    'SELECT * WHERE { <alice> ?p ?o }',
    { baseIri: 'http://example.org/' }
);

// Using NamedNode base IRI
const base = oxigraph.namedNode('http://example.org/');
const results2 = store.query(
    'SELECT * WHERE { <alice> ?p ?o }',
    { baseIri: base }
);
```

### Using Prefixes

```javascript
const results = store.query(
    'SELECT * WHERE { ?s foaf:name ?name }',
    {
        prefixes: {
            'foaf': 'http://xmlns.com/foaf/0.1/',
            'ex': 'http://example.org/'
        }
    }
);

// Multiple prefixes
const results2 = store.query(
    'SELECT * WHERE { ?s a foaf:Person ; ex:status ?status }',
    {
        prefixes: {
            'foaf': 'http://xmlns.com/foaf/0.1/',
            'ex': 'http://example.org/',
            'rdf': 'http://www.w3.org/1999/02/22-rdf-syntax-ns#'
        }
    }
);
```

### Dataset Specification

Control which graphs are queried:

```javascript
// Add data to named graphs
const graph1 = oxigraph.namedNode('http://example.org/graph1');
const graph2 = oxigraph.namedNode('http://example.org/graph2');

store.add(oxigraph.quad(alice, name, aliceName, graph1));
store.add(oxigraph.quad(bob, name, bobName, graph2));

// Query only specific named graphs
const results = store.query(
    'SELECT * WHERE { ?s ?p ?o }',
    {
        namedGraphs: [graph1]  // Only query graph1
    }
);

// Use specific graphs as default graph
const results2 = store.query(
    'SELECT * WHERE { ?s ?p ?o }',  // Queries default graph
    {
        defaultGraph: [graph1, graph2]  // Use graph1 and graph2 as default
    }
);

// Union of all graphs as default graph
const results3 = store.query(
    'SELECT * WHERE { ?s ?p ?o }',
    {
        useDefaultGraphAsUnion: true  // Default graph is union of all graphs
    }
);
```

### Variable Substitutions

Pre-bind variables before query execution:

```javascript
const alice = oxigraph.namedNode('http://example.org/alice');

const results = store.query(
    `SELECT ?name WHERE { ?person foaf:name ?name }`,
    {
        prefixes: { foaf: 'http://xmlns.com/foaf/0.1/' },
        substitutions: {
            person: alice  // Pre-bind ?person to alice
        }
    }
);

// This is equivalent to:
const directQuery = `
    SELECT ?name WHERE {
        <http://example.org/alice> foaf:name ?name
    }
`;
```

## Result Serialization

Query results can be serialized to standard SPARQL formats:

### SELECT Results Serialization

```javascript
// Get results as JSON string
const jsonResults = store.query(
    'SELECT ?s ?p ?o WHERE { ?s ?p ?o }',
    { resultsFormat: 'json' }
);
console.log(jsonResults);
// '{"head":{"vars":["s","p","o"]},"results":{"bindings":[...]}}'

// Get results as XML string
const xmlResults = store.query(
    'SELECT ?s ?p ?o WHERE { ?s ?p ?o }',
    { resultsFormat: 'xml' }
);

// Get results as CSV
const csvResults = store.query(
    'SELECT ?s ?p ?o WHERE { ?s ?p ?o }',
    { resultsFormat: 'csv' }
);

// Get results as TSV
const tsvResults = store.query(
    'SELECT ?s ?p ?o WHERE { ?s ?p ?o }',
    { resultsFormat: 'tsv' }
);

// Media types also work
const jsonResults2 = store.query(
    'SELECT * WHERE { ?s ?p ?o }',
    { resultsFormat: 'application/sparql-results+json' }
);
```

### ASK Results Serialization

```javascript
// ASK as JSON
const askJson = store.query(
    'ASK { ?s ?p ?o }',
    { resultsFormat: 'json' }
);
console.log(askJson); // '{"boolean":true}'

// ASK as XML
const askXml = store.query(
    'ASK { ?s ?p ?o }',
    { resultsFormat: 'xml' }
);
```

### CONSTRUCT Results Serialization

```javascript
// CONSTRUCT as Turtle
const turtle = store.query(
    'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }',
    { resultsFormat: 'ttl' }
);

// CONSTRUCT as N-Triples
const ntriples = store.query(
    'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }',
    { resultsFormat: 'nt' }
);

// CONSTRUCT as RDF/XML
const rdfxml = store.query(
    'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }',
    { resultsFormat: 'rdf' }
);
```

## Asynchronous Queries

For long-running queries in browsers or to keep Node.js responsive:

```javascript
// Async query execution
async function runQuery() {
    const store = new oxigraph.Store();

    // Load data...
    store.load(largeDataset, { format: 'ttl' });

    try {
        // Use queryAsync instead of query
        const results = await store.queryAsync(`
            SELECT ?s ?p ?o WHERE {
                ?s ?p ?o .
            }
        `);

        // Results yielded to event loop every 1000 items
        for (const binding of results) {
            console.log(binding.get('s').value);
        }

    } catch (error) {
        console.error('Query failed:', error);
    }
}

runQuery();
```

### Async Queries in Browser

```javascript
import init, * as oxigraph from 'oxigraph/web';

async function browserQuery() {
    await init();

    const store = new oxigraph.Store();
    // ... load data ...

    // Keep UI responsive during long queries
    const results = await store.queryAsync(`
        SELECT ?item ?label WHERE {
            ?item rdfs:label ?label .
        }
    `);

    const listEl = document.getElementById('results');
    for (const binding of results) {
        const li = document.createElement('li');
        li.textContent = binding.get('label').value;
        listEl.appendChild(li);
    }
}
```

### Async with Result Serialization

```javascript
async function getSerializedResults() {
    const store = new oxigraph.Store();
    // ... load data ...

    const jsonResults = await store.queryAsync(
        'SELECT * WHERE { ?s ?p ?o }',
        { resultsFormat: 'json' }
    );

    return jsonResults;
}

getSerializedResults().then(json => {
    const parsed = JSON.parse(json);
    console.log('Variables:', parsed.head.vars);
    console.log('Bindings:', parsed.results.bindings.length);
});
```

## SPARQL UPDATE Operations

Modify the store using SPARQL UPDATE:

### INSERT DATA

```javascript
// Insert triples
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    INSERT DATA {
        ex:david foaf:name "David" ;
                 foaf:age 28 ;
                 foaf:knows ex:alice .
    }
`);

// Insert into named graph
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    INSERT DATA {
        GRAPH ex:graph1 {
            ex:eve foaf:name "Eve" .
        }
    }
`);
```

### DELETE DATA

```javascript
// Delete specific triples
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    DELETE DATA {
        ex:david foaf:age 28 .
    }
`);
```

### DELETE/INSERT with WHERE

```javascript
// Update data based on patterns
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    DELETE { ?person foaf:age ?oldAge }
    INSERT { ?person foaf:age ?newAge }
    WHERE {
        ?person foaf:age ?oldAge .
        BIND(?oldAge + 1 AS ?newAge)
    }
`);

// Conditional delete
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    DELETE {
        ?person foaf:knows ?friend .
    }
    WHERE {
        ?person foaf:age ?age .
        FILTER(?age < 18)
        ?person foaf:knows ?friend .
    }
`);
```

### Graph Management

```javascript
// Create empty named graph
store.update(`
    CREATE GRAPH <http://example.org/newGraph>
`);

// Drop graph
store.update(`
    DROP GRAPH <http://example.org/oldGraph>
`);

// Clear graph (remove all triples but keep graph)
store.update(`
    CLEAR GRAPH <http://example.org/graph1>
`);

// Copy graph
store.update(`
    COPY GRAPH <http://example.org/source>
    TO GRAPH <http://example.org/destination>
`);

// Move graph
store.update(`
    MOVE GRAPH <http://example.org/source>
    TO GRAPH <http://example.org/destination>
`);

// Add triples from one graph to another
store.update(`
    ADD GRAPH <http://example.org/source>
    TO GRAPH <http://example.org/destination>
`);
```

### Async Updates

```javascript
async function updateData() {
    const store = new oxigraph.Store();

    // Async update
    await store.updateAsync(`
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>
        PREFIX ex: <http://example.org/>

        INSERT DATA {
            ex:newPerson foaf:name "New Person" .
        }
    `);

    console.log('Update complete');
}

// With options
async function updateWithOptions() {
    await store.updateAsync(
        `INSERT DATA { <person> <name> "Test" }`,
        {
            baseIri: 'http://example.org/',
            prefixes: {
                foaf: 'http://xmlns.com/foaf/0.1/'
            }
        }
    );
}
```

## Complex Query Patterns

### OPTIONAL Patterns

```javascript
const query = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?name ?email WHERE {
        ?person foaf:name ?name .
        OPTIONAL { ?person foaf:mbox ?email }
    }
`;

const results = store.query(query);
for (const binding of results) {
    const name = binding.get('name').value;
    const email = binding.get('email');

    if (email) {
        console.log(`${name}: ${email.value}`);
    } else {
        console.log(`${name}: (no email)`);
    }
}
```

### UNION Patterns

```javascript
const query = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX schema: <http://schema.org/>

    SELECT ?person ?identifier WHERE {
        ?person a foaf:Person .
        {
            ?person foaf:mbox ?identifier
        } UNION {
            ?person schema:telephone ?identifier
        }
    }
`;
```

### Aggregation

```javascript
// Count, sum, average, etc.
const statsQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT
        (COUNT(?person) AS ?count)
        (AVG(?age) AS ?avgAge)
        (MIN(?age) AS ?minAge)
        (MAX(?age) AS ?maxAge)
    WHERE {
        ?person foaf:age ?age .
    }
`;

const stats = store.query(statsQuery)[0];
console.log('Count:', stats.get('count').value);
console.log('Average age:', stats.get('avgAge').value);
console.log('Min age:', stats.get('minAge').value);
console.log('Max age:', stats.get('maxAge').value);
```

### GROUP BY

```javascript
const groupQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    SELECT ?city (COUNT(?person) AS ?population) WHERE {
        ?person a foaf:Person ;
               ex:livesIn ?city .
    }
    GROUP BY ?city
    ORDER BY DESC(?population)
`;
```

### Subqueries

```javascript
const subquery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?person ?name ?friendCount WHERE {
        ?person foaf:name ?name .
        {
            SELECT ?person (COUNT(?friend) AS ?friendCount) WHERE {
                ?person foaf:knows ?friend .
            }
            GROUP BY ?person
        }
    }
    ORDER BY DESC(?friendCount)
`;
```

### Property Paths

```javascript
// Zero or more
const transitiveQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    SELECT ?person WHERE {
        ex:alice foaf:knows+ ?person .
    }
`;

// Alternative paths
const altPathQuery = `
    SELECT ?contact WHERE {
        ?person (foaf:mbox|foaf:phone) ?contact .
    }
`;

// Inverse paths
const inverseQuery = `
    SELECT ?knower WHERE {
        ex:alice ^foaf:knows ?knower .
    }
`;
```

### FILTER Functions

```javascript
const filterQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?name ?age WHERE {
        ?person foaf:name ?name ;
               foaf:age ?age .
        FILTER(
            ?age >= 18 &&
            ?age <= 65 &&
            REGEX(?name, "^A", "i")
        )
    }
`;

// String functions
const stringQuery = `
    SELECT ?name ?upper ?lower WHERE {
        ?person foaf:name ?name .
        BIND(UCASE(?name) AS ?upper)
        BIND(LCASE(?name) AS ?lower)
        FILTER(STRLEN(?name) > 3)
    }
`;
```

## Building a Semantic Web Application

Complete example of a knowledge base application:

```javascript
const oxigraph = require('oxigraph');

class KnowledgeBase {
    constructor() {
        this.store = new oxigraph.Store();
        this.prefixes = {
            foaf: 'http://xmlns.com/foaf/0.1/',
            schema: 'http://schema.org/',
            ex: 'http://example.org/',
            rdf: 'http://www.w3.org/1999/02/22-rdf-syntax-ns#',
            rdfs: 'http://www.w3.org/2000/01/rdf-schema#'
        };
    }

    // Add a person to the knowledge base
    addPerson(id, data) {
        const person = oxigraph.namedNode(`http://example.org/${id}`);

        let update = `
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            PREFIX schema: <http://schema.org/>
            PREFIX ex: <http://example.org/>

            INSERT DATA {
                ex:${id} a foaf:Person ;
                        foaf:name "${data.name}" .
        `;

        if (data.age) {
            update += `ex:${id} foaf:age ${data.age} .\n`;
        }

        if (data.email) {
            update += `ex:${id} foaf:mbox <mailto:${data.email}> .\n`;
        }

        if (data.interests) {
            for (const interest of data.interests) {
                update += `ex:${id} foaf:interest "${interest}" .\n`;
            }
        }

        update += '}';

        this.store.update(update);
    }

    // Add friendship
    addFriendship(person1Id, person2Id) {
        this.store.update(`
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            PREFIX ex: <http://example.org/>

            INSERT DATA {
                ex:${person1Id} foaf:knows ex:${person2Id} .
                ex:${person2Id} foaf:knows ex:${person1Id} .
            }
        `);
    }

    // Find person by name
    findByName(name) {
        const results = this.store.query(`
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>

            SELECT ?person ?age ?email WHERE {
                ?person foaf:name ?name ;
                       foaf:name "${name}" .
                OPTIONAL { ?person foaf:age ?age }
                OPTIONAL { ?person foaf:mbox ?email }
            }
        `);

        if (results.length === 0) return null;

        const binding = results[0];
        return {
            uri: binding.get('person').value,
            name: name,
            age: binding.get('age')?.value,
            email: binding.get('email')?.value
        };
    }

    // Get friends of a person
    getFriends(personId) {
        const results = this.store.query(`
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>
            PREFIX ex: <http://example.org/>

            SELECT ?friendName ?friendAge WHERE {
                ex:${personId} foaf:knows ?friend .
                ?friend foaf:name ?friendName .
                OPTIONAL { ?friend foaf:age ?friendAge }
            }
            ORDER BY ?friendName
        `);

        return results.map(binding => ({
            name: binding.get('friendName').value,
            age: binding.get('friendAge')?.value
        }));
    }

    // Find people with shared interests
    findByInterest(interest) {
        const results = this.store.query(`
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>

            SELECT ?name WHERE {
                ?person foaf:name ?name ;
                       foaf:interest ?interest .
                FILTER(CONTAINS(LCASE(?interest), LCASE("${interest}")))
            }
        `);

        return results.map(b => b.get('name').value);
    }

    // Get statistics
    getStats() {
        const result = this.store.query(`
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>

            SELECT
                (COUNT(DISTINCT ?person) AS ?peopleCount)
                (COUNT(DISTINCT ?friendship) AS ?friendshipCount)
                (AVG(?age) AS ?avgAge)
            WHERE {
                ?person a foaf:Person .
                OPTIONAL {
                    ?person foaf:age ?age
                }
                OPTIONAL {
                    ?person foaf:knows ?friend .
                    BIND(CONCAT(STR(?person), STR(?friend)) AS ?friendship)
                }
            }
        `)[0];

        return {
            people: parseInt(result.get('peopleCount').value),
            friendships: parseInt(result.get('friendshipCount').value) / 2,
            averageAge: parseFloat(result.get('avgAge')?.value || 0)
        };
    }

    // Export as Turtle
    export() {
        return this.store.dump({
            format: 'ttl',
            prefixes: this.prefixes
        });
    }

    // Import from Turtle
    import(turtleData) {
        this.store.load(turtleData, {
            format: 'ttl',
            baseIri: 'http://example.org/'
        });
    }
}

// Usage
const kb = new KnowledgeBase();

// Add people
kb.addPerson('alice', {
    name: 'Alice',
    age: 30,
    email: 'alice@example.org',
    interests: ['RDF', 'Semantic Web', 'JavaScript']
});

kb.addPerson('bob', {
    name: 'Bob',
    age: 25,
    email: 'bob@example.org',
    interests: ['SPARQL', 'Databases']
});

kb.addPerson('charlie', {
    name: 'Charlie',
    age: 35,
    interests: ['JavaScript', 'Web Development']
});

// Add friendships
kb.addFriendship('alice', 'bob');
kb.addFriendship('alice', 'charlie');

// Query
console.log('\nAlice:', kb.findByName('Alice'));
console.log('\nAlice\'s friends:', kb.getFriends('alice'));
console.log('\nPeople interested in JavaScript:', kb.findByInterest('JavaScript'));
console.log('\nStatistics:', kb.getStats());

// Export
console.log('\n--- Exported Turtle ---');
console.log(kb.export());
```

## Best Practices

### 1. Use Prefixes

```javascript
// Good: readable and maintainable
const results = store.query(query, {
    prefixes: {
        foaf: 'http://xmlns.com/foaf/0.1/',
        schema: 'http://schema.org/'
    }
});

// Bad: hard to read
const results = store.query(`
    SELECT * WHERE {
        ?s <http://xmlns.com/foaf/0.1/name> ?name .
    }
`);
```

### 2. Use Async for Large Queries

```javascript
// Good: keeps UI responsive
async function largeQuery() {
    const results = await store.queryAsync(largeQueryString);
    // Process results...
}

// Bad: blocks UI
const results = store.query(largeQueryString);
```

### 3. Handle Errors

```javascript
try {
    const results = store.query(userQuery);
    // Process results...
} catch (error) {
    console.error('Query failed:', error.message);
    // Show user-friendly error...
}
```

### 4. Use Pattern Matching for Simple Queries

```javascript
// Good: efficient for simple patterns
const matches = store.match(subject, predicate, null, null);

// Less efficient: SPARQL overhead for simple pattern
const results = store.query(`
    SELECT ?o WHERE {
        <${subject.value}> <${predicate.value}> ?o
    }
`);
```

### 5. Batch Updates

```javascript
// Good: single transaction
store.update(`
    INSERT DATA {
        ex:p1 foaf:name "Person 1" .
        ex:p2 foaf:name "Person 2" .
        ex:p3 foaf:name "Person 3" .
    }
`);

// Less efficient: multiple transactions
store.update('INSERT DATA { ex:p1 foaf:name "Person 1" }');
store.update('INSERT DATA { ex:p2 foaf:name "Person 2" }');
store.update('INSERT DATA { ex:p3 foaf:name "Person 3" }');
```

## Next Steps

- [Getting Started Guide](javascript-getting-started.md) - Installation and basic usage
- [RDF Data Model](javascript-rdf-model.md) - Working with RDF/JS terms and datasets
- Explore [GeoSPARQL](https://github.com/oxigraph/oxigraph#geosparql) for geospatial queries
- Learn about [RDF-star](https://www.w3.org/2021/12/rdf-star.html) for metadata on triples
