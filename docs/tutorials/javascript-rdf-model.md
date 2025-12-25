# Working with the RDF Data Model in JavaScript

This tutorial covers Oxigraph's RDF/JS compatible data model, showing you how to create and manipulate RDF terms, quads, and datasets.

## Overview

Oxigraph implements the [RDF/JS DataModel specification](https://rdf.js.org/data-model-spec/), which provides a standard way to represent RDF data in JavaScript. This makes Oxigraph interoperable with other RDF/JS libraries.

## RDF/JS DataFactory Methods

Oxigraph provides factory methods for creating RDF terms:

```javascript
const oxigraph = require('oxigraph');

// Named nodes (IRIs)
const subject = oxigraph.namedNode('http://example.org/alice');

// Blank nodes
const blankNode = oxigraph.blankNode();  // Auto-generated ID
const namedBlank = oxigraph.blankNode('b1');  // Specific ID

// Literals
const simpleLiteral = oxigraph.literal('Hello');
const stringLiteral = oxigraph.literal('Hello', oxigraph.namedNode('http://www.w3.org/2001/XMLSchema#string'));
const integerLiteral = oxigraph.literal('42', oxigraph.namedNode('http://www.w3.org/2001/XMLSchema#integer'));
const langLiteral = oxigraph.literal('Hello', 'en');

// Default graph
const defaultGraph = oxigraph.defaultGraph();

// Variables (for SPARQL)
const variable = oxigraph.variable('x');

// Triples (RDF-star)
const triple = oxigraph.triple(subject, predicate, object);

// Quads
const quad = oxigraph.quad(subject, predicate, object);  // In default graph
const quadInGraph = oxigraph.quad(subject, predicate, object, graphName);
```

## RDF Terms

### NamedNode

Represents an IRI (Internationalized Resource Identifier):

```javascript
const person = oxigraph.namedNode('http://xmlns.com/foaf/0.1/Person');

console.log(person.termType);  // 'NamedNode'
console.log(person.value);     // 'http://xmlns.com/foaf/0.1/Person'
console.log(person.toString()); // '<http://xmlns.com/foaf/0.1/Person>'

// RDF/JS compliance
console.log(person.toJSON());
// { termType: 'NamedNode', value: 'http://xmlns.com/foaf/0.1/Person' }
```

### BlankNode

Represents an anonymous resource:

```javascript
// Auto-generated blank node
const blank1 = oxigraph.blankNode();
console.log(blank1.termType);  // 'BlankNode'
console.log(blank1.value);     // Something like 'b0'

// Named blank node
const blank2 = oxigraph.blankNode('myBlank');
console.log(blank2.value);     // 'myBlank'
console.log(blank2.toString()); // '_:myBlank'

// Each auto-generated blank node is unique
const b1 = oxigraph.blankNode();
const b2 = oxigraph.blankNode();
console.log(b1.equals(b2));    // false
```

### Literal

Represents literal values with optional language tags or datatypes:

```javascript
// Simple literal (xsd:string)
const simple = oxigraph.literal('Hello World');
console.log(simple.value);     // 'Hello World'
console.log(simple.datatype.value); // 'http://www.w3.org/2001/XMLSchema#string'
console.log(simple.language);  // ''

// Language-tagged literal
const english = oxigraph.literal('Hello', 'en');
console.log(english.value);    // 'Hello'
console.log(english.language); // 'en'
console.log(english.datatype.value); // 'http://www.w3.org/1999/02/22-rdf-syntax-ns#langString'
console.log(english.toString()); // '"Hello"@en'

// Typed literal
const xsd = oxigraph.namedNode('http://www.w3.org/2001/XMLSchema#');
const integer = oxigraph.literal('42', oxigraph.namedNode(xsd.value + 'integer'));
console.log(integer.value);    // '42'
console.log(integer.datatype.value); // 'http://www.w3.org/2001/XMLSchema#integer'

// Date literal
const date = oxigraph.literal('2024-01-01', oxigraph.namedNode(xsd.value + 'date'));
console.log(date.toString()); // '"2024-01-01"^^<http://www.w3.org/2001/XMLSchema#date>'

// Boolean literal
const bool = oxigraph.literal('true', oxigraph.namedNode(xsd.value + 'boolean'));
```

### Working with Literal Direction (RDF 1.2)

If compiled with RDF 1.2 support, directional language tags are available:

```javascript
// Directional language-tagged literal (requires rdf-12 feature)
const rtlLiteral = oxigraph.literal('مرحبا', { language: 'ar', direction: 'rtl' });
console.log(rtlLiteral.language);   // 'ar'
console.log(rtlLiteral.direction);  // 'rtl'

const ltrLiteral = oxigraph.literal('Hello', { language: 'en', direction: 'ltr' });
console.log(ltrLiteral.direction);  // 'ltr'
```

### DefaultGraph

Represents the default graph:

```javascript
const defaultGraph = oxigraph.defaultGraph();
console.log(defaultGraph.termType); // 'DefaultGraph'
console.log(defaultGraph.value);    // ''
console.log(defaultGraph.toString()); // 'DEFAULT'
```

### Variable

Represents a SPARQL variable:

```javascript
const variable = oxigraph.variable('name');
console.log(variable.termType);  // 'Variable'
console.log(variable.value);     // 'name'
console.log(variable.toString()); // '?name'
```

### Comparing Terms

All terms have an `equals()` method:

```javascript
const node1 = oxigraph.namedNode('http://example.org/alice');
const node2 = oxigraph.namedNode('http://example.org/alice');
const node3 = oxigraph.namedNode('http://example.org/bob');

console.log(node1.equals(node2)); // true
console.log(node1.equals(node3)); // false

const lit1 = oxigraph.literal('Hello', 'en');
const lit2 = oxigraph.literal('Hello', 'en');
const lit3 = oxigraph.literal('Hello', 'fr');

console.log(lit1.equals(lit2)); // true
console.log(lit1.equals(lit3)); // false
```

## Quads and Triples

### Creating Quads

```javascript
const subject = oxigraph.namedNode('http://example.org/alice');
const predicate = oxigraph.namedNode('http://xmlns.com/foaf/0.1/name');
const object = oxigraph.literal('Alice');

// Quad in default graph
const quad1 = oxigraph.quad(subject, predicate, object);
console.log(quad1.subject.value);   // 'http://example.org/alice'
console.log(quad1.predicate.value); // 'http://xmlns.com/foaf/0.1/name'
console.log(quad1.object.value);    // 'Alice'
console.log(quad1.graph.termType);  // 'DefaultGraph'

// Quad in named graph
const graphName = oxigraph.namedNode('http://example.org/graph1');
const quad2 = oxigraph.quad(subject, predicate, object, graphName);
console.log(quad2.graph.value);     // 'http://example.org/graph1'

// Quad properties are read-only
console.log(quad1.termType);  // 'Quad'
console.log(quad1.value);     // ''
```

### RDF-star: Triples and Quoted Triples

RDF-star allows triples to be subjects or objects of other triples:

```javascript
// Create a triple
const innerTriple = oxigraph.triple(
    oxigraph.namedNode('http://example.org/alice'),
    oxigraph.namedNode('http://xmlns.com/foaf/0.1/name'),
    oxigraph.literal('Alice')
);

// Use the triple as a subject
const metaQuad = oxigraph.quad(
    innerTriple,  // Triple as subject
    oxigraph.namedNode('http://example.org/certainty'),
    oxigraph.literal('0.9', oxigraph.namedNode('http://www.w3.org/2001/XMLSchema#decimal'))
);

console.log(metaQuad.subject.termType); // 'Triple'

// Use the triple as an object
const sourceQuad = oxigraph.quad(
    oxigraph.namedNode('http://example.org/source1'),
    oxigraph.namedNode('http://example.org/states'),
    innerTriple  // Triple as object
);
```

### Converting Between Quads and Triples

```javascript
// A triple is just a quad without a named graph
const triple = oxigraph.triple(subject, predicate, object);
const quad = oxigraph.quad(subject, predicate, object);

// Quads can be printed in N-Quads format
console.log(quad.toString());
// <http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
```

## Dataset: In-Memory RDF Collection

The `Dataset` class provides an in-memory collection of quads with array-like methods:

### Creating a Dataset

```javascript
// Empty dataset
const dataset = new oxigraph.Dataset();

// Dataset with initial quads
const dataset2 = new oxigraph.Dataset([
    oxigraph.quad(subject1, predicate1, object1),
    oxigraph.quad(subject2, predicate2, object2)
]);
```

### Adding and Removing Quads

```javascript
const dataset = new oxigraph.Dataset();

// Add a quad
const quad = oxigraph.quad(
    oxigraph.namedNode('http://example.org/alice'),
    oxigraph.namedNode('http://xmlns.com/foaf/0.1/name'),
    oxigraph.literal('Alice')
);
dataset.add(quad);

console.log(dataset.size); // 1

// Check if quad exists
console.log(dataset.has(quad));      // true
console.log(dataset.includes(quad)); // true (alias for has)

// Delete a quad (returns true if deleted)
const deleted = dataset.delete(quad);
console.log(deleted);      // true
console.log(dataset.size); // 0

// Discard a quad (returns nothing)
dataset.add(quad);
dataset.discard(quad);
console.log(dataset.size); // 0

// Clear all quads
dataset.clear();
```

### Pattern Matching

```javascript
const dataset = new oxigraph.Dataset();

// Add some data
const alice = oxigraph.namedNode('http://example.org/alice');
const bob = oxigraph.namedNode('http://example.org/bob');
const name = oxigraph.namedNode('http://xmlns.com/foaf/0.1/name');
const knows = oxigraph.namedNode('http://xmlns.com/foaf/0.1/knows');

dataset.add(oxigraph.quad(alice, name, oxigraph.literal('Alice')));
dataset.add(oxigraph.quad(bob, name, oxigraph.literal('Bob')));
dataset.add(oxigraph.quad(alice, knows, bob));

// Match all quads
const allQuads = dataset.match();
console.log(`Total: ${allQuads.length} quads`);

// Match quads with specific subject
const aliceQuads = dataset.match(alice, null, null, null);
console.log(`Alice quads: ${aliceQuads.length}`);

// Match quads with specific predicate
const nameQuads = dataset.match(null, name, null, null);
console.log(`Name quads: ${nameQuads.length}`);

// Match quads with specific subject and predicate
const aliceNameQuads = dataset.match(alice, name, null, null);
console.log(aliceNameQuads[0].object.value); // 'Alice'

// Match in specific graph
const graph1 = oxigraph.namedNode('http://example.org/graph1');
const graphQuads = dataset.match(null, null, null, graph1);
```

### Optimized Quad Lookups

```javascript
const dataset = new oxigraph.Dataset();
// ... add data ...

// Get all quads with a specific subject (optimized)
const subjectQuads = dataset.quadsForSubject(alice);

// Get all quads with a specific predicate (optimized)
const predicateQuads = dataset.quadsForPredicate(name);

// Get all quads with a specific object (optimized)
const objectQuads = dataset.quadsForObject(oxigraph.literal('Alice'));

// Get all quads in a specific graph (optimized)
const graphQuads = dataset.quadsForGraphName(oxigraph.defaultGraph());
```

### Iterating Over Datasets

```javascript
const dataset = new oxigraph.Dataset();
// ... add quads ...

// Using for...of (Symbol.iterator)
for (const quad of dataset) {
    console.log(`${quad.subject.value} -> ${quad.object.value}`);
}

// Using forEach
dataset.forEach((quad) => {
    console.log(quad.toString());
});

// With thisArg
const context = { count: 0 };
dataset.forEach(function(quad) {
    this.count++;
}, context);
console.log(`Counted ${context.count} quads`);
```

### Array-like Methods

Dataset supports many JavaScript array methods:

```javascript
const dataset = new oxigraph.Dataset();
// ... add quads ...

// Filter quads
const filtered = dataset.filter((quad) => {
    return quad.predicate.value.includes('foaf');
});
console.log(`Filtered to ${filtered.size} quads`);

// Map over quads
const subjects = dataset.map((quad) => quad.subject);
console.log(`Subjects: ${subjects.map(s => s.value).join(', ')}`);

// Find a quad
const found = dataset.find((quad) => {
    return quad.object.value === 'Alice';
});
if (found) {
    console.log(`Found: ${found.toString()}`);
}

// Check if some quads match a condition
const hasLiterals = dataset.some((quad) => {
    return quad.object.termType === 'Literal';
});

// Check if every quad matches a condition
const allInDefaultGraph = dataset.every((quad) => {
    return quad.graph.termType === 'DefaultGraph';
});

// Reduce to a value
const subjectCount = dataset.reduce((acc, quad) => {
    acc.add(quad.subject.value);
    return acc;
}, new Set()).size;
console.log(`Unique subjects: ${subjectCount}`);

// Get quad at index
const first = dataset.at(0);
const last = dataset.at(-1);

// Slice dataset
const firstTwo = dataset.slice(0, 2);

// Convert to array
const array = dataset.toArray();
```

## Interoperability with RDF/JS Libraries

Oxigraph's RDF/JS compliance allows it to work with other RDF libraries:

### Converting from Other RDF/JS Libraries

```javascript
const oxigraph = require('oxigraph');

// If you have terms from another RDF/JS library, convert them:
const externalQuad = {
    termType: 'Quad',
    subject: { termType: 'NamedNode', value: 'http://example.org/alice' },
    predicate: { termType: 'NamedNode', value: 'http://xmlns.com/foaf/0.1/name' },
    object: { termType: 'Literal', value: 'Alice', language: '', datatype: {
        termType: 'NamedNode',
        value: 'http://www.w3.org/2001/XMLSchema#string'
    }},
    graph: { termType: 'DefaultGraph', value: '' }
};

// Convert to Oxigraph quad
const oxiQuad = oxigraph.fromQuad(externalQuad);
console.log(oxiQuad.subject.value); // 'http://example.org/alice'

// Convert individual terms
const externalTerm = { termType: 'NamedNode', value: 'http://example.org/test' };
const oxiTerm = oxigraph.fromTerm(externalTerm);
console.log(oxiTerm.termType); // 'NamedNode'
```

### Using with N3.js

```javascript
const oxigraph = require('oxigraph');
const N3 = require('n3');

// Parse with N3.js
const parser = new N3.Parser();
const quads = parser.parse(`
    @prefix foaf: <http://xmlns.com/foaf/0.1/> .
    <http://example.org/alice> foaf:name "Alice" .
`);

// Convert to Oxigraph and add to store
const store = new oxigraph.Store();
for (const n3quad of quads) {
    const oxiQuad = oxigraph.fromQuad(n3quad);
    store.add(oxiQuad);
}

console.log(`Loaded ${store.size} quads`);
```

## Practical Example: Building a Knowledge Graph

```javascript
const oxigraph = require('oxigraph');

// Vocabularies
const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const RDFS = 'http://www.w3.org/2000/01/rdf-schema#';
const FOAF = 'http://xmlns.com/foaf/0.1/';
const SCHEMA = 'http://schema.org/';
const EX = 'http://example.org/';

// Helper to create terms
function n(suffix) {
    return oxigraph.namedNode(EX + suffix);
}

function foaf(suffix) {
    return oxigraph.namedNode(FOAF + suffix);
}

function schema(suffix) {
    return oxigraph.namedNode(SCHEMA + suffix);
}

function rdf(suffix) {
    return oxigraph.namedNode(RDF + suffix);
}

function lit(value, langOrType) {
    if (typeof langOrType === 'string' && langOrType.length === 2) {
        return oxigraph.literal(value, langOrType);
    } else if (langOrType) {
        return oxigraph.literal(value, langOrType);
    }
    return oxigraph.literal(value);
}

// Build a knowledge graph
const dataset = new oxigraph.Dataset();

// Define people
const alice = n('alice');
const bob = n('bob');
const charlie = n('charlie');

// Add type information
dataset.add(oxigraph.quad(alice, rdf('type'), foaf('Person')));
dataset.add(oxigraph.quad(bob, rdf('type'), foaf('Person')));
dataset.add(oxigraph.quad(charlie, rdf('type'), foaf('Person')));

// Add names
dataset.add(oxigraph.quad(alice, foaf('name'), lit('Alice')));
dataset.add(oxigraph.quad(bob, foaf('name'), lit('Bob')));
dataset.add(oxigraph.quad(charlie, foaf('name'), lit('Charlie')));

// Add multilingual names
dataset.add(oxigraph.quad(alice, schema('name'), lit('Alice', 'en')));
dataset.add(oxigraph.quad(alice, schema('name'), lit('アリス', 'ja')));

// Add ages
const xsdInteger = oxigraph.namedNode('http://www.w3.org/2001/XMLSchema#integer');
dataset.add(oxigraph.quad(alice, foaf('age'), lit('30', xsdInteger)));
dataset.add(oxigraph.quad(bob, foaf('age'), lit('25', xsdInteger)));
dataset.add(oxigraph.quad(charlie, foaf('age'), lit('35', xsdInteger)));

// Add relationships
dataset.add(oxigraph.quad(alice, foaf('knows'), bob));
dataset.add(oxigraph.quad(alice, foaf('knows'), charlie));
dataset.add(oxigraph.quad(bob, foaf('knows'), charlie));

console.log(`Knowledge graph contains ${dataset.size} quads\n`);

// Query the knowledge graph
console.log('People and their ages:');
const ageQuads = dataset.match(null, foaf('age'), null, null);
for (const quad of ageQuads) {
    const nameQuad = dataset.match(quad.subject, foaf('name'), null, null)[0];
    console.log(`  ${nameQuad.object.value}: ${quad.object.value} years old`);
}

console.log('\nFriendships:');
const friendships = dataset.match(null, foaf('knows'), null, null);
for (const quad of friendships) {
    const person1Name = dataset.match(quad.subject, foaf('name'), null, null)[0].object.value;
    const person2Name = dataset.match(quad.object, foaf('name'), null, null)[0].object.value;
    console.log(`  ${person1Name} knows ${person2Name}`);
}

// Clone the dataset
const backup = dataset.clone();
console.log(`\nBackup dataset has ${backup.size} quads`);

// Convert to string (N-Quads format)
console.log('\nDataset as N-Quads:');
console.log(dataset.toString());
```

## Dataset Canonicalization

Canonicalize a dataset using RDF Dataset Canonicalization (RDFC-1.0):

```javascript
const { Dataset, CanonicalizationAlgorithm } = require('oxigraph');

const dataset = new Dataset();
// ... add quads with blank nodes ...

// Canonicalize using RDFC-1.0 (the W3C standard algorithm)
dataset.canonicalize(CanonicalizationAlgorithm.RDFC_1_0);

// The blank node IDs are now canonical and deterministic
console.log(dataset.toString());
```

Available canonicalization algorithms:

```javascript
// RDFC-1.0 with SHA-256 (the W3C standard, alias for RDFC_1_0_SHA_256)
CanonicalizationAlgorithm.RDFC_1_0

// RDFC-1.0 with SHA-256 (default)
CanonicalizationAlgorithm.RDFC_1_0_SHA_256

// RDFC-1.0 with SHA-384
CanonicalizationAlgorithm.RDFC_1_0_SHA_384

// Oxigraph's optimized algorithm (warning: may change between versions)
CanonicalizationAlgorithm.UNSTABLE
```

## Next Steps

- [Getting Started Guide](javascript-getting-started.md) - Installation and basic usage
- [Advanced SPARQL](javascript-sparql.md) - Complex queries and semantic applications
