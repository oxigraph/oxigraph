# Getting Started with Oxigraph for JavaScript

This tutorial will guide you through installing and using Oxigraph in JavaScript environments, both in the browser and Node.js.

## Overview

Oxigraph for JavaScript is a WebAssembly-compiled graph database that implements the SPARQL standard. It provides:

- In-memory RDF storage with SPARQL 1.1 Query and Update support
- RDF/JS compatible API for interoperability with other JavaScript RDF libraries
- Support for multiple RDF serialization formats (Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD)
- Works in modern browsers and Node.js 18+

## Installation

Install Oxigraph via npm:

```bash
npm install oxigraph
```

## Browser Setup

Oxigraph works in modern browsers that support WebAssembly reference types and JavaScript `WeakRef`.

### Using ES Modules

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Oxigraph Browser Example</title>
</head>
<body>
    <h1>Oxigraph Browser Demo</h1>
    <div id="output"></div>

    <script type="module">
        import init, * as oxigraph from './node_modules/oxigraph/web.js';

        (async function() {
            // Initialize WebAssembly - this is required!
            await init();

            // Now you can use Oxigraph
            const store = new oxigraph.Store();
            console.log('Oxigraph initialized successfully!');

            document.getElementById('output').textContent =
                'Oxigraph loaded! Store is empty: ' + store.isEmpty();
        })();
    </script>
</body>
</html>
```

### Using with a Bundler (Webpack, Vite, etc.)

With modern bundlers like Webpack 5 or Vite, you can import Oxigraph directly:

```javascript
import init, * as oxigraph from 'oxigraph/web';

async function initializeApp() {
    await init();

    const store = new oxigraph.Store();
    // Use the store...
}

initializeApp();
```

## Node.js Setup

### CommonJS

```javascript
const oxigraph = require('oxigraph');

// Create a new store
const store = new oxigraph.Store();
console.log('Store created!');
```

### ES Modules

```javascript
import oxigraph from './node_modules/oxigraph/node.js';

// Or with package.json "type": "module"
import oxigraph from 'oxigraph/node';

const store = new oxigraph.Store();
console.log('Store created!');
```

## Your First Oxigraph Application

Let's create a simple knowledge base about people and their interests.

### Complete Node.js Example

```javascript
const oxigraph = require('oxigraph');

// Create a new in-memory store
const store = new oxigraph.Store();

// Create some RDF terms
const alice = oxigraph.namedNode('http://example.org/alice');
const bob = oxigraph.namedNode('http://example.org/bob');
const name = oxigraph.namedNode('http://xmlns.com/foaf/0.1/name');
const knows = oxigraph.namedNode('http://xmlns.com/foaf/0.1/knows');
const interest = oxigraph.namedNode('http://xmlns.com/foaf/0.1/interest');

// Add triples to the store
store.add(oxigraph.quad(
    alice,
    name,
    oxigraph.literal('Alice')
));

store.add(oxigraph.quad(
    bob,
    name,
    oxigraph.literal('Bob')
));

store.add(oxigraph.quad(
    alice,
    knows,
    bob
));

store.add(oxigraph.quad(
    alice,
    interest,
    oxigraph.namedNode('http://example.org/RDF')
));

store.add(oxigraph.quad(
    bob,
    interest,
    oxigraph.namedNode('http://example.org/SPARQL')
));

console.log(`Store contains ${store.size} triples`);

// Query the store with SPARQL
const query = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?person ?name ?interest WHERE {
        ?person foaf:name ?name .
        ?person foaf:interest ?interest .
    }
    ORDER BY ?name
`;

console.log('\nPeople and their interests:');
for (const binding of store.query(query)) {
    console.log(`- ${binding.get('name').value} is interested in ${binding.get('interest').value}`);
}

// Query who knows whom
const friendsQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?person1Name ?person2Name WHERE {
        ?person1 foaf:name ?person1Name .
        ?person1 foaf:knows ?person2 .
        ?person2 foaf:name ?person2Name .
    }
`;

console.log('\nFriendships:');
for (const binding of store.query(friendsQuery)) {
    console.log(`- ${binding.get('person1Name').value} knows ${binding.get('person2Name').value}`);
}
```

**Output:**
```
Store contains 5 triples

People and their interests:
- Alice is interested in http://example.org/RDF
- Bob is interested in http://example.org/SPARQL

Friendships:
- Alice knows Bob
```

### Complete Browser Example

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Oxigraph Knowledge Base</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
        }
        .result {
            background: #f0f0f0;
            padding: 10px;
            margin: 10px 0;
            border-radius: 5px;
        }
    </style>
</head>
<body>
    <h1>Oxigraph Knowledge Base</h1>
    <div id="status">Loading...</div>
    <div id="results"></div>

    <script type="module">
        import init, * as oxigraph from './node_modules/oxigraph/web.js';

        (async function() {
            // Initialize WebAssembly
            await init();

            const statusEl = document.getElementById('status');
            const resultsEl = document.getElementById('results');

            statusEl.textContent = 'Oxigraph loaded! Building knowledge base...';

            // Create store and add data
            const store = new oxigraph.Store();

            const alice = oxigraph.namedNode('http://example.org/alice');
            const bob = oxigraph.namedNode('http://example.org/bob');
            const name = oxigraph.namedNode('http://xmlns.com/foaf/0.1/name');
            const knows = oxigraph.namedNode('http://xmlns.com/foaf/0.1/knows');
            const interest = oxigraph.namedNode('http://xmlns.com/foaf/0.1/interest');

            store.add(oxigraph.quad(alice, name, oxigraph.literal('Alice')));
            store.add(oxigraph.quad(bob, name, oxigraph.literal('Bob')));
            store.add(oxigraph.quad(alice, knows, bob));
            store.add(oxigraph.quad(alice, interest, oxigraph.namedNode('http://example.org/RDF')));
            store.add(oxigraph.quad(bob, interest, oxigraph.namedNode('http://example.org/SPARQL')));

            statusEl.textContent = `Knowledge base ready! (${store.size} triples)`;

            // Query and display results
            const query = `
                PREFIX foaf: <http://xmlns.com/foaf/0.1/>

                SELECT ?person ?name ?interest WHERE {
                    ?person foaf:name ?name .
                    ?person foaf:interest ?interest .
                }
                ORDER BY ?name
            `;

            let html = '<h2>People and Interests</h2>';
            for (const binding of store.query(query)) {
                html += `
                    <div class="result">
                        <strong>${binding.get('name').value}</strong>
                        is interested in
                        <code>${binding.get('interest').value}</code>
                    </div>
                `;
            }

            resultsEl.innerHTML = html;
        })();
    </script>
</body>
</html>
```

## Adding Triples

There are several ways to add triples to a store:

### Individual Quads

```javascript
const store = new oxigraph.Store();

// Create a quad and add it
const quad = oxigraph.quad(
    oxigraph.namedNode('http://example.org/subject'),
    oxigraph.namedNode('http://example.org/predicate'),
    oxigraph.literal('object value')
);

store.add(quad);
```

### Named Graphs

```javascript
// Add a quad to a named graph
const graphName = oxigraph.namedNode('http://example.org/graph1');

store.add(oxigraph.quad(
    oxigraph.namedNode('http://example.org/subject'),
    oxigraph.namedNode('http://example.org/predicate'),
    oxigraph.literal('value'),
    graphName  // fourth parameter is the graph
));
```

### Initialize with Quads

```javascript
// Create a store with initial data
const store = new oxigraph.Store([
    oxigraph.quad(subject1, predicate1, object1),
    oxigraph.quad(subject2, predicate2, object2)
]);
```

### Loading from RDF Files

```javascript
const turtleData = `
    @prefix foaf: <http://xmlns.com/foaf/0.1/> .
    @prefix ex: <http://example.org/> .

    ex:alice foaf:name "Alice" ;
             foaf:knows ex:bob .

    ex:bob foaf:name "Bob" .
`;

store.load(turtleData, {
    format: 'text/turtle',  // or 'ttl'
    baseIri: 'http://example.org/'
});

console.log(`Store now has ${store.size} triples`);
```

## Simple SPARQL Queries

### SELECT Queries

```javascript
// Get all triples
const allQuery = 'SELECT * WHERE { ?s ?p ?o }';
const results = store.query(allQuery);

for (const binding of results) {
    console.log('Subject:', binding.get('s').value);
    console.log('Predicate:', binding.get('p').value);
    console.log('Object:', binding.get('o').value);
    console.log('---');
}
```

### ASK Queries

```javascript
// Check if a pattern exists
const askQuery = `
    ASK {
        <http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> ?person
    }
`;

const exists = store.query(askQuery);
console.log('Alice knows someone:', exists); // true or false
```

### CONSTRUCT Queries

```javascript
// Create new triples from existing data
const constructQuery = `
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    CONSTRUCT {
        ?person a foaf:Person .
    }
    WHERE {
        ?person foaf:name ?name .
    }
`;

const newTriples = store.query(constructQuery);

// Result is an array of Quads
console.log(`Generated ${newTriples.length} new triples`);

// Create a new store with the results
const resultStore = new oxigraph.Store(newTriples);
```

## SPARQL Updates

Modify data using SPARQL UPDATE:

```javascript
// Insert new data
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    INSERT DATA {
        ex:charlie foaf:name "Charlie" ;
                   foaf:knows ex:alice .
    }
`);

// Delete data
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>

    DELETE DATA {
        ex:charlie foaf:knows ex:alice .
    }
`);

// Delete and insert (update)
store.update(`
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    DELETE { ?person foaf:name ?oldName }
    INSERT { ?person foaf:name "Alice Smith" }
    WHERE {
        ?person foaf:name ?oldName .
        FILTER(?oldName = "Alice")
    }
`);
```

## Pattern Matching

Use the `match()` method for efficient pattern-based queries:

```javascript
// Get all triples with a specific subject
const subject = oxigraph.namedNode('http://example.org/alice');
const matches = store.match(subject, null, null, null);

console.log(`Found ${matches.length} triples about Alice`);

// Get all triples in the default graph
const defaultGraph = oxigraph.defaultGraph();
const defaultGraphTriples = store.match(null, null, null, defaultGraph);

// Get all uses of a specific predicate
const foafName = oxigraph.namedNode('http://xmlns.com/foaf/0.1/name');
const nameTriples = store.match(null, foafName, null, null);

for (const quad of nameTriples) {
    console.log(`${quad.subject.value} has name: ${quad.object.value}`);
}
```

## Checking Store Contents

```javascript
// Check if store is empty
if (store.isEmpty()) {
    console.log('Store is empty');
}

// Get number of triples
console.log(`Store contains ${store.size} triples`);

// Check if a specific quad exists
const quad = oxigraph.quad(subject, predicate, object);
if (store.has(quad)) {
    console.log('Quad exists in store');
}
```

## Error Handling

Always handle errors when working with Oxigraph:

```javascript
try {
    // Invalid SPARQL query
    const results = store.query('INVALID QUERY');
} catch (error) {
    console.error('Query error:', error.message);
}

try {
    // Invalid Turtle syntax
    store.load('invalid turtle', { format: 'ttl' });
} catch (error) {
    console.error('Parse error:', error.message);
}

try {
    // Invalid IRI
    const node = oxigraph.namedNode('not a valid iri');
} catch (error) {
    console.error('IRI error:', error.message);
}
```

## Next Steps

Now that you've learned the basics, explore:

- [Working with the RDF Data Model](javascript-rdf-model.md) - Deep dive into RDF/JS terms and Dataset
- [Advanced SPARQL Queries](javascript-sparql.md) - Complex queries, async operations, and semantic web apps

## Complete Working Example

Here's a complete example you can run in Node.js:

```javascript
#!/usr/bin/env node
const oxigraph = require('oxigraph');

function main() {
    // Create store
    const store = new oxigraph.Store();

    // Load some data
    const data = `
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .
        @prefix ex: <http://example.org/> .

        ex:alice foaf:name "Alice" ;
                 foaf:age 30 ;
                 foaf:knows ex:bob, ex:charlie .

        ex:bob foaf:name "Bob" ;
               foaf:age 25 .

        ex:charlie foaf:name "Charlie" ;
                   foaf:age 35 .
    `;

    try {
        store.load(data, {
            format: 'ttl',
            baseIri: 'http://example.org/'
        });

        console.log(`Loaded ${store.size} triples\n`);

        // Query for people and their ages
        const query = `
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>

            SELECT ?name ?age WHERE {
                ?person foaf:name ?name ;
                       foaf:age ?age .
            }
            ORDER BY DESC(?age)
        `;

        console.log('People by age (descending):');
        for (const result of store.query(query)) {
            console.log(`  ${result.get('name').value}: ${result.get('age').value}`);
        }

        // Count friendships
        const countQuery = `
            PREFIX foaf: <http://xmlns.com/foaf/0.1/>

            SELECT (COUNT(?friend) as ?count) WHERE {
                ?person foaf:knows ?friend .
            }
        `;

        const countResult = store.query(countQuery)[0];
        console.log(`\nTotal friendships: ${countResult.get('count').value}`);

    } catch (error) {
        console.error('Error:', error.message);
        process.exit(1);
    }
}

main();
```

Save this as `example.js` and run with `node example.js`.
