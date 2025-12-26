# JavaScript Integration Patterns

Comprehensive integration examples for Oxigraph in JavaScript/TypeScript applications, covering Node.js backends, frontend frameworks, performance optimization, and production deployment.

## Table of Contents

1. [Express.js REST API](#expressjs-rest-api)
2. [Express.js Middleware](#expressjs-middleware)
3. [React Integration](#react-integration)
4. [Vue.js Integration](#vuejs-integration)
5. [Worker Threads](#worker-threads-for-performance)
6. [TypeScript Usage](#typescript-usage)
7. [Bundle Optimization](#bundle-optimization)
8. [GraphQL Integration](#graphql-integration)

## Express.js REST API

Complete REST API using Express.js with proper error handling and CORS.

### package.json

```json
{
  "name": "oxigraph-express-api",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "start": "node src/server.js",
    "dev": "nodemon src/server.js"
  },
  "dependencies": {
    "oxigraph": "^0.4.0",
    "express": "^4.18.2",
    "cors": "^2.8.5",
    "morgan": "^1.10.0",
    "helmet": "^7.1.0",
    "dotenv": "^16.3.1"
  },
  "devDependencies": {
    "nodemon": "^3.0.1"
  }
}
```

### src/server.js

```javascript
import express from 'express';
import cors from 'cors';
import morgan from 'morgan';
import helmet from 'helmet';
import dotenv from 'dotenv';
import oxigraph from 'oxigraph';

dotenv.config();

const app = express();
const PORT = process.env.PORT || 3000;

// Initialize Oxigraph store
const store = new oxigraph.Store();

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json());
app.use(morgan('combined'));

// Error handler
class ApiError extends Error {
    constructor(message, statusCode = 500) {
        super(message);
        this.statusCode = statusCode;
    }
}

const asyncHandler = (fn) => (req, res, next) => {
    Promise.resolve(fn(req, res, next)).catch(next);
};

// Helper functions
function termToJSON(term) {
    if (term.termType === 'NamedNode') {
        return {
            type: 'NamedNode',
            value: term.value
        };
    } else if (term.termType === 'Literal') {
        const result = {
            type: 'Literal',
            value: term.value
        };
        if (term.language) {
            result.language = term.language;
        } else if (term.datatype && term.datatype.value !== 'http://www.w3.org/2001/XMLSchema#string') {
            result.datatype = term.datatype.value;
        }
        return result;
    } else if (term.termType === 'BlankNode') {
        return {
            type: 'BlankNode',
            value: term.value
        };
    } else if (term.termType === 'Quad') {
        return {
            type: 'Quad',
            subject: termToJSON(term.subject),
            predicate: termToJSON(term.predicate),
            object: termToJSON(term.object)
        };
    }
    return { type: 'Unknown', value: String(term) };
}

function jsonToTerm(data) {
    switch (data.type) {
        case 'NamedNode':
            return oxigraph.namedNode(data.value);
        case 'Literal':
            if (data.language) {
                return oxigraph.literal(data.value, data.language);
            } else if (data.datatype) {
                return oxigraph.literal(data.value, oxigraph.namedNode(data.datatype));
            }
            return oxigraph.literal(data.value);
        case 'BlankNode':
            return oxigraph.blankNode(data.value);
        default:
            throw new ApiError(`Unsupported term type: ${data.type}`, 400);
    }
}

// Routes

app.get('/', (req, res) => {
    res.json({
        service: 'Oxigraph SPARQL API',
        version: '1.0.0',
        endpoints: {
            health: '/health',
            query: '/query',
            update: '/update',
            triples: '/triples',
            load: '/load',
            export: '/export',
            stats: '/stats'
        }
    });
});

app.get('/health', (req, res) => {
    res.json({
        status: 'healthy',
        store_size: store.size
    });
});

app.post('/query', asyncHandler(async (req, res) => {
    const { query, options = {} } = req.body;

    if (!query) {
        throw new ApiError('Missing query parameter', 400);
    }

    console.log(`Executing query: ${query.substring(0, 100)}...`);

    try {
        const results = store.query(query, options);
        const output = [];

        for (const result of results) {
            if (result instanceof Map) {
                // SELECT results
                const binding = {};
                for (const [variable, term] of result.entries()) {
                    binding[variable] = termToJSON(term);
                }
                output.push(binding);
            } else if (result.subject) {
                // CONSTRUCT results
                output.push({
                    subject: termToJSON(result.subject),
                    predicate: termToJSON(result.predicate),
                    object: termToJSON(result.object)
                });
            } else {
                // ASK results
                output.push({ result: Boolean(result) });
            }
        }

        res.json({
            results: output,
            count: output.length
        });
    } catch (error) {
        throw new ApiError(`Query execution failed: ${error.message}`, 400);
    }
}));

app.post('/update', asyncHandler(async (req, res) => {
    const { update, options = {} } = req.body;

    if (!update) {
        throw new ApiError('Missing update parameter', 400);
    }

    console.log(`Executing update: ${update.substring(0, 100)}...`);

    try {
        store.update(update, options);
        res.json({ message: 'Update executed successfully' });
    } catch (error) {
        throw new ApiError(`Update failed: ${error.message}`, 400);
    }
}));

app.post('/triples', asyncHandler(async (req, res) => {
    const { subject, predicate, object } = req.body;

    if (!subject || !predicate || !object) {
        throw new ApiError('Missing required fields: subject, predicate, object', 400);
    }

    try {
        const s = jsonToTerm(subject);
        const p = jsonToTerm(predicate);
        const o = jsonToTerm(object);

        const quad = oxigraph.quad(s, p, o);
        store.add(quad);

        console.log(`Added triple: ${s.value} ${p.value} ${o.value}`);

        res.status(201).json({ message: 'Triple added successfully' });
    } catch (error) {
        throw new ApiError(`Failed to add triple: ${error.message}`, 400);
    }
}));

app.get('/triples', asyncHandler(async (req, res) => {
    const limit = parseInt(req.query.limit) || 100;
    const offset = parseInt(req.query.offset) || 0;

    const triples = [];
    let index = 0;

    for (const quad of store) {
        if (index < offset) {
            index++;
            continue;
        }
        if (triples.length >= limit) {
            break;
        }

        triples.push({
            subject: termToJSON(quad.subject),
            predicate: termToJSON(quad.predicate),
            object: termToJSON(quad.object),
            graph: quad.graph ? termToJSON(quad.graph) : null
        });

        index++;
    }

    res.json({
        triples,
        count: triples.length,
        limit,
        offset
    });
}));

app.get('/triples/:subjectIri', asyncHandler(async (req, res) => {
    const subjectIri = decodeURIComponent(req.params.subjectIri);

    console.log(`Getting triples for subject: ${subjectIri}`);

    const subject = oxigraph.namedNode(subjectIri);
    const triples = [];

    for (const quad of store.match(subject, null, null, null)) {
        triples.push({
            predicate: termToJSON(quad.predicate),
            object: termToJSON(quad.object)
        });
    }

    res.json({
        subject: subjectIri,
        triples,
        count: triples.length
    });
}));

app.post('/load', asyncHandler(async (req, res) => {
    const { content, format, baseIri } = req.body;

    if (!content || !format) {
        throw new ApiError('Missing content or format parameter', 400);
    }

    const formatMap = {
        'turtle': 'text/turtle',
        'ttl': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nt': 'application/n-triples',
        'rdfxml': 'application/rdf+xml',
        'nquads': 'application/n-quads',
        'nq': 'application/n-quads',
        'trig': 'application/trig',
        'jsonld': 'application/ld+json'
    };

    const mimeType = formatMap[format.toLowerCase()] || format;

    console.log(`Loading data with format: ${mimeType}`);

    try {
        const options = { format: mimeType };
        if (baseIri) {
            options.baseIri = baseIri;
        }

        store.load(content, options);

        res.status(201).json({
            message: 'Data loaded successfully',
            store_size: store.size
        });
    } catch (error) {
        throw new ApiError(`Failed to load data: ${error.message}`, 400);
    }
}));

app.get('/export', asyncHandler(async (req, res) => {
    const format = req.query.format || 'turtle';

    const formatMap = {
        'turtle': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nquads': 'application/n-quads',
        'trig': 'application/trig'
    };

    const mimeType = formatMap[format.toLowerCase()] || 'text/turtle';

    console.log(`Exporting data as ${mimeType}`);

    try {
        const data = store.dump({ format: mimeType });
        res.type(mimeType).send(data);
    } catch (error) {
        throw new ApiError(`Failed to export data: ${error.message}`, 500);
    }
}));

app.get('/stats', asyncHandler(async (req, res) => {
    const subjects = new Set();
    const predicates = new Set();
    const objects = new Set();

    for (const quad of store) {
        subjects.add(quad.subject.value);
        predicates.add(quad.predicate.value);
        objects.add(quad.object.value);
    }

    res.json({
        total_quads: store.size,
        unique_subjects: subjects.size,
        unique_predicates: predicates.size,
        unique_objects: objects.size
    });
}));

// Error handling middleware
app.use((err, req, res, next) => {
    console.error('Error:', err);

    if (err instanceof ApiError) {
        return res.status(err.statusCode).json({
            error: err.message
        });
    }

    res.status(500).json({
        error: 'Internal server error'
    });
});

// 404 handler
app.use((req, res) => {
    res.status(404).json({ error: 'Not found' });
});

// Start server
app.listen(PORT, () => {
    console.log(`Oxigraph API server listening on port ${PORT}`);
});
```

### Testing

```bash
npm install
npm start

# Test endpoints
curl http://localhost:3000/health

# Add triple
curl -X POST http://localhost:3000/triples \
  -H "Content-Type: application/json" \
  -d '{
    "subject": {"type": "NamedNode", "value": "http://example.org/alice"},
    "predicate": {"type": "NamedNode", "value": "http://schema.org/name"},
    "object": {"type": "Literal", "value": "Alice"}
  }'

# Query
curl -X POST http://localhost:3000/query \
  -H "Content-Type: application/json" \
  -d '{"query": "SELECT * WHERE { ?s ?p ?o } LIMIT 10"}'
```

## Express.js Middleware

Reusable middleware for adding SPARQL capabilities to Express apps.

### src/middleware/oxigraph.js

```javascript
import oxigraph from 'oxigraph';

export function createOxigraphMiddleware(options = {}) {
    const store = options.store || new oxigraph.Store();

    return {
        // Middleware to attach store to req object
        attachStore: (req, res, next) => {
            req.oxigraph = store;
            next();
        },

        // SPARQL query endpoint middleware
        sparqlQuery: () => async (req, res, next) => {
            try {
                const query = req.method === 'POST' ? req.body.query : req.query.query;

                if (!query) {
                    return res.status(400).json({ error: 'Missing query parameter' });
                }

                const results = [];
                for (const result of store.query(query)) {
                    if (result instanceof Map) {
                        const binding = {};
                        for (const [key, value] of result.entries()) {
                            binding[key] = value.value;
                        }
                        results.push(binding);
                    }
                }

                res.json({ results });
            } catch (error) {
                next(error);
            }
        },

        // Store accessor
        getStore: () => store
    };
}

// Usage example
/*
import express from 'express';
import { createOxigraphMiddleware } from './middleware/oxigraph.js';

const app = express();
const { attachStore, sparqlQuery } = createOxigraphMiddleware();

app.use(attachStore);
app.post('/sparql', sparqlQuery());
*/
```

## React Integration

React hooks and components for working with RDF data.

### package.json

```json
{
  "name": "oxigraph-react-app",
  "version": "0.1.0",
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "oxigraph": "^0.4.0"
  }
}
```

### src/hooks/useOxigraph.js

```javascript
import { useState, useEffect, useCallback, useRef } from 'react';
import oxigraph from 'oxigraph';

export function useOxigraph() {
    const storeRef = useRef(null);

    if (!storeRef.current) {
        storeRef.current = new oxigraph.Store();
    }

    return storeRef.current;
}

export function useSparqlQuery(store, query, dependencies = []) {
    const [results, setResults] = useState([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState(null);

    const executeQuery = useCallback(() => {
        if (!query) return;

        setLoading(true);
        setError(null);

        try {
            const queryResults = [];
            for (const result of store.query(query)) {
                if (result instanceof Map) {
                    const binding = {};
                    for (const [key, value] of result.entries()) {
                        binding[key] = value.value;
                    }
                    queryResults.push(binding);
                } else {
                    queryResults.push(result);
                }
            }
            setResults(queryResults);
        } catch (err) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    }, [store, query, ...dependencies]);

    useEffect(() => {
        executeQuery();
    }, [executeQuery]);

    return { results, loading, error, refetch: executeQuery };
}

export function useRdfData(store) {
    const [data, setData] = useState([]);

    const loadData = useCallback((content, format = 'text/turtle') => {
        try {
            store.load(content, { format });
            setData(Array.from(store));
        } catch (error) {
            console.error('Failed to load RDF data:', error);
        }
    }, [store]);

    const addTriple = useCallback((subject, predicate, object) => {
        try {
            const s = oxigraph.namedNode(subject);
            const p = oxigraph.namedNode(predicate);
            const o = typeof object === 'string' && object.startsWith('http')
                ? oxigraph.namedNode(object)
                : oxigraph.literal(object);

            store.add(oxigraph.quad(s, p, o));
            setData(Array.from(store));
        } catch (error) {
            console.error('Failed to add triple:', error);
        }
    }, [store]);

    useEffect(() => {
        setData(Array.from(store));
    }, [store]);

    return { data, loadData, addTriple };
}
```

### src/components/SparqlQueryForm.jsx

```javascript
import React, { useState } from 'react';
import { useSparqlQuery, useOxigraph } from '../hooks/useOxigraph';

export function SparqlQueryForm() {
    const store = useOxigraph();
    const [queryText, setQueryText] = useState('SELECT * WHERE { ?s ?p ?o } LIMIT 10');
    const [currentQuery, setCurrentQuery] = useState('');

    const { results, loading, error } = useSparqlQuery(store, currentQuery);

    const handleSubmit = (e) => {
        e.preventDefault();
        setCurrentQuery(queryText);
    };

    return (
        <div className="sparql-query-form">
            <h2>SPARQL Query</h2>

            <form onSubmit={handleSubmit}>
                <textarea
                    value={queryText}
                    onChange={(e) => setQueryText(e.target.value)}
                    rows={6}
                    style={{ width: '100%', fontFamily: 'monospace' }}
                />
                <button type="submit" disabled={loading}>
                    {loading ? 'Executing...' : 'Execute Query'}
                </button>
            </form>

            {error && (
                <div className="error" style={{ color: 'red' }}>
                    Error: {error}
                </div>
            )}

            {results.length > 0 && (
                <div className="results">
                    <h3>Results ({results.length})</h3>
                    <table border="1" cellPadding="5">
                        <thead>
                            <tr>
                                {Object.keys(results[0]).map((key) => (
                                    <th key={key}>{key}</th>
                                ))}
                            </tr>
                        </thead>
                        <tbody>
                            {results.map((result, idx) => (
                                <tr key={idx}>
                                    {Object.values(result).map((value, i) => (
                                        <td key={i}>{String(value)}</td>
                                    ))}
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    );
}
```

### src/components/RdfDataLoader.jsx

```javascript
import React, { useState } from 'react';
import { useOxigraph, useRdfData } from '../hooks/useOxigraph';

export function RdfDataLoader() {
    const store = useOxigraph();
    const { data, loadData } = useRdfData(store);
    const [input, setInput] = useState('');
    const [format, setFormat] = useState('text/turtle');

    const handleLoad = () => {
        loadData(input, format);
        setInput('');
    };

    return (
        <div className="rdf-data-loader">
            <h2>Load RDF Data</h2>

            <select value={format} onChange={(e) => setFormat(e.target.value)}>
                <option value="text/turtle">Turtle</option>
                <option value="application/n-triples">N-Triples</option>
                <option value="application/rdf+xml">RDF/XML</option>
                <option value="application/ld+json">JSON-LD</option>
            </select>

            <textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                placeholder="Paste RDF data here..."
                rows={6}
                style={{ width: '100%', fontFamily: 'monospace' }}
            />

            <button onClick={handleLoad}>Load Data</button>

            <div className="store-info">
                <p>Store contains {data.length} quads</p>
            </div>
        </div>
    );
}
```

### src/App.jsx

```javascript
import React from 'react';
import { SparqlQueryForm } from './components/SparqlQueryForm';
import { RdfDataLoader } from './components/RdfDataLoader';

function App() {
    return (
        <div className="App">
            <h1>Oxigraph React Demo</h1>
            <RdfDataLoader />
            <hr />
            <SparqlQueryForm />
        </div>
    );
}

export default App;
```

## Vue.js Integration

Vue 3 composition API integration with Oxigraph.

### src/composables/useOxigraph.js

```javascript
import { ref, computed, onMounted } from 'vue';
import oxigraph from 'oxigraph';

let storeInstance = null;

export function useOxigraph() {
    if (!storeInstance) {
        storeInstance = new oxigraph.Store();
    }
    return storeInstance;
}

export function useSparqlQuery(initialQuery = '') {
    const store = useOxigraph();
    const query = ref(initialQuery);
    const results = ref([]);
    const loading = ref(false);
    const error = ref(null);

    const execute = async () => {
        if (!query.value) return;

        loading.value = true;
        error.value = null;

        try {
            const queryResults = [];
            for (const result of store.query(query.value)) {
                if (result instanceof Map) {
                    const binding = {};
                    for (const [key, value] of result.entries()) {
                        binding[key] = value.value;
                    }
                    queryResults.push(binding);
                }
            }
            results.value = queryResults;
        } catch (err) {
            error.value = err.message;
        } finally {
            loading.value = false;
        }
    };

    return {
        query,
        results,
        loading,
        error,
        execute
    };
}

export function useRdfStore() {
    const store = useOxigraph();
    const quads = ref([]);
    const size = computed(() => store.size);

    const loadData = (content, format = 'text/turtle') => {
        try {
            store.load(content, { format });
            quads.value = Array.from(store);
        } catch (error) {
            console.error('Failed to load data:', error);
            throw error;
        }
    };

    const addQuad = (subject, predicate, object) => {
        const s = oxigraph.namedNode(subject);
        const p = oxigraph.namedNode(predicate);
        const o = typeof object === 'string' && object.startsWith('http')
            ? oxigraph.namedNode(object)
            : oxigraph.literal(object);

        store.add(oxigraph.quad(s, p, o));
        quads.value = Array.from(store);
    };

    const clear = () => {
        // Create new store instance
        storeInstance = new oxigraph.Store();
        quads.value = [];
    };

    onMounted(() => {
        quads.value = Array.from(store);
    });

    return {
        store,
        quads,
        size,
        loadData,
        addQuad,
        clear
    };
}
```

### src/components/SparqlQuery.vue

```vue
<template>
  <div class="sparql-query">
    <h2>SPARQL Query</h2>

    <textarea
      v-model="query"
      rows="6"
      placeholder="Enter SPARQL query..."
      class="query-input"
    />

    <button @click="execute" :disabled="loading">
      {{ loading ? 'Executing...' : 'Execute' }}
    </button>

    <div v-if="error" class="error">
      Error: {{ error }}
    </div>

    <div v-if="results.length > 0" class="results">
      <h3>Results ({{ results.length }})</h3>
      <table>
        <thead>
          <tr>
            <th v-for="key in Object.keys(results[0])" :key="key">
              {{ key }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(result, idx) in results" :key="idx">
            <td v-for="(value, key) in result" :key="key">
              {{ value }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { useSparqlQuery } from '../composables/useOxigraph';

const { query, results, loading, error, execute } = useSparqlQuery(
  'SELECT * WHERE { ?s ?p ?o } LIMIT 10'
);
</script>

<style scoped>
.query-input {
  width: 100%;
  font-family: monospace;
  padding: 8px;
}

.error {
  color: red;
  margin: 10px 0;
}

table {
  border-collapse: collapse;
  width: 100%;
  margin-top: 10px;
}

th, td {
  border: 1px solid #ddd;
  padding: 8px;
  text-align: left;
}
</style>
```

## Worker Threads for Performance

Use worker threads for heavy SPARQL queries without blocking the main thread.

### src/worker/sparql-worker.js

```javascript
import { parentPort } from 'worker_threads';
import oxigraph from 'oxigraph';

// Initialize store in worker
const store = new oxigraph.Store();

parentPort.on('message', (message) => {
    const { type, data, id } = message;

    try {
        switch (type) {
            case 'load':
                store.load(data.content, { format: data.format });
                parentPort.postMessage({
                    id,
                    type: 'load_success',
                    data: { size: store.size }
                });
                break;

            case 'query':
                const results = [];
                for (const result of store.query(data.query)) {
                    if (result instanceof Map) {
                        const binding = {};
                        for (const [key, value] of result.entries()) {
                            binding[key] = value.value;
                        }
                        results.push(binding);
                    }
                }
                parentPort.postMessage({
                    id,
                    type: 'query_success',
                    data: { results }
                });
                break;

            case 'update':
                store.update(data.update);
                parentPort.postMessage({
                    id,
                    type: 'update_success',
                    data: { size: store.size }
                });
                break;

            default:
                throw new Error(`Unknown message type: ${type}`);
        }
    } catch (error) {
        parentPort.postMessage({
            id,
            type: 'error',
            data: { message: error.message }
        });
    }
});
```

### src/utils/sparql-worker-pool.js

```javascript
import { Worker } from 'worker_threads';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export class SparqlWorkerPool {
    constructor(size = 4) {
        this.workers = [];
        this.currentWorker = 0;
        this.messageId = 0;
        this.pendingMessages = new Map();

        for (let i = 0; i < size; i++) {
            this.createWorker();
        }
    }

    createWorker() {
        const worker = new Worker(path.join(__dirname, '../worker/sparql-worker.js'));

        worker.on('message', (message) => {
            const { id } = message;
            const pending = this.pendingMessages.get(id);

            if (pending) {
                if (message.type === 'error') {
                    pending.reject(new Error(message.data.message));
                } else {
                    pending.resolve(message.data);
                }
                this.pendingMessages.delete(id);
            }
        });

        worker.on('error', (error) => {
            console.error('Worker error:', error);
        });

        this.workers.push(worker);
    }

    getWorker() {
        const worker = this.workers[this.currentWorker];
        this.currentWorker = (this.currentWorker + 1) % this.workers.length;
        return worker;
    }

    async sendMessage(type, data) {
        const id = this.messageId++;
        const worker = this.getWorker();

        return new Promise((resolve, reject) => {
            this.pendingMessages.set(id, { resolve, reject });
            worker.postMessage({ type, data, id });

            // Timeout after 30 seconds
            setTimeout(() => {
                if (this.pendingMessages.has(id)) {
                    this.pendingMessages.delete(id);
                    reject(new Error('Query timeout'));
                }
            }, 30000);
        });
    }

    async query(query) {
        return this.sendMessage('query', { query });
    }

    async load(content, format) {
        return this.sendMessage('load', { content, format });
    }

    async update(update) {
        return this.sendMessage('update', { update });
    }

    destroy() {
        for (const worker of this.workers) {
            worker.terminate();
        }
        this.workers = [];
    }
}

// Usage example
/*
const pool = new SparqlWorkerPool(4);

const results = await pool.query('SELECT * WHERE { ?s ?p ?o } LIMIT 100');
console.log(results);

pool.destroy();
*/
```

## TypeScript Usage

Type-safe Oxigraph integration with TypeScript.

### src/types/oxigraph.d.ts

```typescript
declare module 'oxigraph' {
    export class Store {
        constructor();
        readonly size: number;

        add(quad: Quad): void;
        delete(quad: Quad): void;
        has(quad: Quad): boolean;
        match(
            subject?: Term | null,
            predicate?: Term | null,
            object?: Term | null,
            graph?: Term | null
        ): IterableIterator<Quad>;

        query(query: string, options?: QueryOptions): QueryResults;
        update(update: string, options?: UpdateOptions): void;

        load(data: string, options: LoadOptions): void;
        dump(options: DumpOptions): string;

        [Symbol.iterator](): IterableIterator<Quad>;
    }

    export interface Term {
        readonly termType: 'NamedNode' | 'BlankNode' | 'Literal' | 'DefaultGraph' | 'Quad';
        readonly value: string;
        equals(other: Term | null | undefined): boolean;
    }

    export interface NamedNode extends Term {
        readonly termType: 'NamedNode';
    }

    export interface BlankNode extends Term {
        readonly termType: 'BlankNode';
    }

    export interface Literal extends Term {
        readonly termType: 'Literal';
        readonly language: string;
        readonly datatype: NamedNode;
    }

    export interface DefaultGraph extends Term {
        readonly termType: 'DefaultGraph';
    }

    export interface Quad {
        readonly termType: 'Quad';
        readonly subject: Term;
        readonly predicate: Term;
        readonly object: Term;
        readonly graph: Term;
        equals(other: Quad | null | undefined): boolean;
    }

    export type QueryResults = IterableIterator<Map<string, Term>> | IterableIterator<Quad> | boolean;

    export interface QueryOptions {
        baseIri?: string;
        useDefaultGraphAsUnion?: boolean;
        defaultGraph?: Term[];
        namedGraphs?: Term[];
    }

    export interface UpdateOptions {
        baseIri?: string;
    }

    export interface LoadOptions {
        format: string;
        baseIri?: string;
        toGraphName?: Term;
    }

    export interface DumpOptions {
        format: string;
        fromGraphName?: Term;
    }

    export function namedNode(iri: string): NamedNode;
    export function blankNode(value?: string): BlankNode;
    export function literal(value: string, languageOrDatatype?: string | NamedNode): Literal;
    export function defaultGraph(): DefaultGraph;
    export function quad(
        subject: Term,
        predicate: Term,
        object: Term,
        graph?: Term
    ): Quad;
    export function triple(subject: Term, predicate: Term, object: Term): Quad;
}
```

### src/services/RdfService.ts

```typescript
import oxigraph, { Store, NamedNode, Literal, Quad, Term } from 'oxigraph';

export interface TripleData {
    subject: string;
    predicate: string;
    object: string | number | boolean;
    objectType?: 'iri' | 'literal' | 'typed';
    datatype?: string;
}

export interface QueryResult {
    [key: string]: string;
}

export class RdfService {
    private store: Store;

    constructor() {
        this.store = new oxigraph.Store();
    }

    getStoreSize(): number {
        return this.store.size;
    }

    addTriple(data: TripleData): void {
        const subject = oxigraph.namedNode(data.subject);
        const predicate = oxigraph.namedNode(data.predicate);

        let object: Term;
        if (data.objectType === 'iri') {
            object = oxigraph.namedNode(String(data.object));
        } else if (data.objectType === 'typed' && data.datatype) {
            object = oxigraph.literal(
                String(data.object),
                oxigraph.namedNode(data.datatype)
            );
        } else {
            object = oxigraph.literal(String(data.object));
        }

        const quad = oxigraph.quad(subject, predicate, object);
        this.store.add(quad);
    }

    query(sparql: string): QueryResult[] {
        const results: QueryResult[] = [];
        const queryResults = this.store.query(sparql);

        if (Symbol.iterator in Object(queryResults)) {
            for (const result of queryResults as IterableIterator<Map<string, Term>>) {
                if (result instanceof Map) {
                    const binding: QueryResult = {};
                    for (const [key, value] of result.entries()) {
                        binding[key] = value.value;
                    }
                    results.push(binding);
                }
            }
        }

        return results;
    }

    loadTurtle(content: string, baseIri?: string): void {
        this.store.load(content, {
            format: 'text/turtle',
            baseIri: baseIri
        });
    }

    exportTurtle(): string {
        return this.store.dump({ format: 'text/turtle' });
    }

    clear(): void {
        this.store = new oxigraph.Store();
    }
}
```

## Bundle Optimization

Optimize bundle size for web applications.

### vite.config.js

```javascript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
    plugins: [react()],
    optimizeDeps: {
        include: ['oxigraph']
    },
    build: {
        rollupOptions: {
            output: {
                manualChunks: {
                    'oxigraph': ['oxigraph']
                }
            }
        }
    }
});
```

### Lazy loading

```javascript
// Lazy load Oxigraph only when needed
const OxigraphApp = lazy(async () => {
    const oxigraph = await import('oxigraph');
    return import('./components/OxigraphApp');
});

function App() {
    return (
        <Suspense fallback={<div>Loading...</div>}>
            <OxigraphApp />
        </Suspense>
    );
}
```

## GraphQL Integration

Integrate Oxigraph with GraphQL using Apollo Server.

### package.json additions

```json
{
  "dependencies": {
    "@apollo/server": "^4.9.0",
    "graphql": "^16.8.0"
  }
}
```

### src/graphql/schema.js

```javascript
export const typeDefs = `#graphql
  type Query {
    sparql(query: String!): SparqlResult!
    triple(subject: String!): [Triple!]!
    stats: StoreStats!
  }

  type Mutation {
    addTriple(subject: String!, predicate: String!, object: String!): Triple!
    loadData(content: String!, format: String!): LoadResult!
  }

  type SparqlResult {
    results: [JSONObject!]!
    count: Int!
  }

  type Triple {
    subject: String!
    predicate: String!
    object: String!
  }

  type StoreStats {
    totalQuads: Int!
    uniqueSubjects: Int!
    uniquePredicates: Int!
  }

  type LoadResult {
    success: Boolean!
    storeSize: Int!
  }

  scalar JSONObject
`;
```

### src/graphql/resolvers.js

```javascript
import oxigraph from 'oxigraph';

const store = new oxigraph.Store();

export const resolvers = {
    Query: {
        sparql: (_, { query }) => {
            const results = [];
            for (const result of store.query(query)) {
                if (result instanceof Map) {
                    const binding = {};
                    for (const [key, value] of result.entries()) {
                        binding[key] = value.value;
                    }
                    results.push(binding);
                }
            }
            return { results, count: results.length };
        },

        triple: (_, { subject }) => {
            const triples = [];
            const subjectNode = oxigraph.namedNode(subject);

            for (const quad of store.match(subjectNode, null, null, null)) {
                triples.push({
                    subject: quad.subject.value,
                    predicate: quad.predicate.value,
                    object: quad.object.value
                });
            }

            return triples;
        },

        stats: () => {
            const subjects = new Set();
            const predicates = new Set();

            for (const quad of store) {
                subjects.add(quad.subject.value);
                predicates.add(quad.predicate.value);
            }

            return {
                totalQuads: store.size,
                uniqueSubjects: subjects.size,
                uniquePredicates: predicates.size
            };
        }
    },

    Mutation: {
        addTriple: (_, { subject, predicate, object }) => {
            const s = oxigraph.namedNode(subject);
            const p = oxigraph.namedNode(predicate);
            const o = oxigraph.literal(object);

            const quad = oxigraph.quad(s, p, o);
            store.add(quad);

            return { subject, predicate, object };
        },

        loadData: (_, { content, format }) => {
            store.load(content, { format });
            return {
                success: true,
                storeSize: store.size
            };
        }
    }
};
```

### src/graphql/server.js

```javascript
import { ApolloServer } from '@apollo/server';
import { startStandaloneServer } from '@apollo/server/standalone';
import { typeDefs } from './schema.js';
import { resolvers } from './resolvers.js';

const server = new ApolloServer({
    typeDefs,
    resolvers,
});

const { url } = await startStandaloneServer(server, {
    listen: { port: 4000 },
});

console.log(`🚀 GraphQL server ready at: ${url}`);
```

---

These patterns provide production-ready integration examples for using Oxigraph in JavaScript/TypeScript applications across various frameworks and use cases!
