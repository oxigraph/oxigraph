use crate::model::*;
use crate::{console_warn, format_err};
use js_sys::{Array, Function, Map, Promise, Reflect, try_iter};
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::*;
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsSerializer};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::{Store, Transaction};
#[cfg(feature = "geosparql")]
use spargeo::GEOSPARQL_EXTENSION_FUNCTIONS;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

// We skip_typescript on specific wasm_bindgen macros and provide custom TypeScript types for parts of this module in order to have narrower types
// instead of any and improve compatibility with RDF/JS Dataset interfaces (https://rdf.js.org/dataset-spec/).
//
// The Store type overlay hides deprecated parameters on methods like dump.
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
/**
 * A persistent RDF graph database that supports SPARQL queries and updates.
 *
 * Store provides an in-memory RDF quad store with full SPARQL 1.1 support.
 * It implements RDF/JS-compatible interfaces for seamless integration with the JavaScript RDF ecosystem.
 *
 * @see {@link https://www.w3.org/TR/rdf11-concepts/ | RDF 1.1 Concepts}
 * @see {@link https://www.w3.org/TR/sparql11-query/ | SPARQL 1.1 Query}
 * @see {@link https://rdf.js.org/dataset-spec/ | RDF/JS Dataset Specification}
 *
 * @example
 * ```typescript
 * // Create a new store
 * const store = new Store();
 *
 * // Add some quads
 * store.add(quad(
 *   namedNode('http://example.com/alice'),
 *   namedNode('http://xmlns.com/foaf/0.1/name'),
 *   literal('Alice'),
 *   defaultGraph()
 * ));
 *
 * // Query with SPARQL
 * const results = store.query('SELECT * WHERE { ?s ?p ?o }');
 * console.log(results.length); // 1
 * ```
 */
export class Store {
    /**
     * The number of quads in the store.
     * @readonly
     */
    readonly size: number;

    /**
     * Alias for size (Array-like compatibility).
     * @readonly
     */
    readonly length: number;

    /**
     * Creates a new RDF Store.
     *
     * @param quads - Optional iterable of quads to initialize the store
     *
     * @example
     * ```typescript
     * // Empty store
     * const store = new Store();
     *
     * // Initialize with quads
     * const store = new Store([
     *   quad(namedNode('http://example.com/s'), namedNode('http://example.com/p'), literal('o'))
     * ]);
     * ```
     */
    constructor(quads?: Iterable<Quad>);

    /**
     * Checks whether the store contains any quads.
     *
     * @returns true if the store is empty, false otherwise
     *
     * @example
     * ```typescript
     * const store = new Store();
     * console.log(store.isEmpty()); // true
     * store.add(quad(namedNode('http://example.com/s'), namedNode('http://example.com/p'), literal('o')));
     * console.log(store.isEmpty()); // false
     * ```
     */
    isEmpty(): boolean;

    /**
     * Adds a quad to the store.
     *
     * @param quad - The RDF quad to add
     *
     * @see {@link https://www.w3.org/TR/rdf11-concepts/#section-triples | RDF Triples}
     *
     * @example
     * ```typescript
     * store.add(quad(
     *   namedNode('http://example.com/alice'),
     *   namedNode('http://xmlns.com/foaf/0.1/knows'),
     *   namedNode('http://example.com/bob')
     * ));
     * ```
     */
    add(quad: Quad): void;

    /**
     * Removes a quad from the store.
     *
     * @param quad - The quad to remove
     *
     * @example
     * ```typescript
     * const q = quad(namedNode('http://example.com/s'), namedNode('http://example.com/p'), literal('o'));
     * store.add(q);
     * store.delete(q);
     * console.log(store.has(q)); // false
     * ```
     */
    delete(quad: Quad): void;

    /**
     * Serializes the store (or a specific graph) to a string in the specified RDF format.
     *
     * @param options - Serialization options
     * @param options.format - The RDF format (e.g., 'turtle', 'ntriples', 'rdfxml', 'jsonld')
     * @param options.fromGraphName - Graph to serialize (default: all graphs)
     * @param options.prefixes - Namespace prefix mappings for formats that support them
     * @param options.baseIri - Base IRI for relative URI resolution
     * @returns The serialized RDF data as a string
     *
     * @see {@link https://www.w3.org/TR/turtle/ | RDF 1.1 Turtle}
     * @see {@link https://www.w3.org/TR/n-triples/ | RDF 1.1 N-Triples}
     *
     * @example
     * ```typescript
     * const turtle = store.dump({
     *   format: 'turtle',
     *   prefixes: {
     *     'foaf': 'http://xmlns.com/foaf/0.1/',
     *     'ex': 'http://example.com/'
     *   }
     * });
     * console.log(turtle);
     * // @prefix foaf: <http://xmlns.com/foaf/0.1/> .
     * // @prefix ex: <http://example.com/> .
     * // ex:alice foaf:name "Alice" .
     * ```
     */
    dump(
        options: {
            format: string;
            fromGraphName?: BlankNode | DefaultGraph | NamedNode;
            prefixes?: Record<string, string>;
            baseIri?: NamedNode | string;
        }
    ): string;

    /**
     * Checks if the store contains a specific quad.
     *
     * @param quad - The quad to check for
     * @returns true if the quad is in the store, false otherwise
     *
     * @example
     * ```typescript
     * const q = quad(namedNode('http://example.com/s'), namedNode('http://example.com/p'), literal('o'));
     * console.log(store.has(q)); // false
     * store.add(q);
     * console.log(store.has(q)); // true
     * ```
     */
    has(quad: Quad): boolean;

    /**
     * Alias for has() (RDF/JS compatibility).
     *
     * @param quad - The quad to check for
     * @returns true if the quad is in the store, false otherwise
     */
    includes(quad: Quad): boolean;

    /**
     * Parses and loads RDF data into the store.
     *
     * @param data - The RDF data as a string
     * @param options - Parsing options
     * @param options.format - The RDF format (e.g., 'turtle', 'ntriples', 'rdfxml', 'jsonld')
     * @param options.baseIri - Base IRI for relative URI resolution
     * @param options.toGraphName - Target graph (default: default graph)
     * @param options.noTransaction - If true, use bulk loader for better performance
     * @param options.lenient - If true, continue parsing despite errors
     *
     * @see {@link https://www.w3.org/TR/turtle/ | RDF 1.1 Turtle}
     *
     * @example
     * ```typescript
     * const turtle = `
     *   @prefix foaf: <http://xmlns.com/foaf/0.1/> .
     *   @prefix ex: <http://example.com/> .
     *   ex:alice foaf:name "Alice" .
     * `;
     * store.load(turtle, { format: 'turtle' });
     * ```
     */
    load(
        data: string,
        options: {
            baseIri?: NamedNode | string;
            format: string;
            noTransaction?: boolean;
            toGraphName?: BlankNode | DefaultGraph | NamedNode;
            unchecked?: boolean;
            lenient?: boolean;
        }
    ): void;

    /**
     * Returns all quads matching the given pattern.
     * All parameters are optional - use null to match any term.
     *
     * @param subject - Subject to match (null matches any)
     * @param predicate - Predicate to match (null matches any)
     * @param object - Object to match (null matches any)
     * @param graph - Graph to match (null matches any)
     * @returns Array of matching quads
     *
     * @see {@link https://rdf.js.org/dataset-spec/#match | RDF/JS Dataset match}
     *
     * @example
     * ```typescript
     * // Find all quads with a specific subject
     * const quads = store.match(namedNode('http://example.com/alice'), null, null, null);
     *
     * // Find all foaf:name triples
     * const names = store.match(null, namedNode('http://xmlns.com/foaf/0.1/name'), null, null);
     *
     * // Find all quads in a specific graph
     * const graphQuads = store.match(null, null, null, namedNode('http://example.com/graph1'));
     * ```
     */
    match(subject?: Term | null, predicate?: Term | null, object?: Term | null, graph?: Term | null): Quad[];

    /**
     * Executes a SPARQL 1.1 query synchronously.
     *
     * @param query - The SPARQL query string
     * @param options - Query options
     * @param options.baseIri - Base IRI for relative URI resolution
     * @param options.prefixes - Namespace prefix mappings
     * @param options.resultsFormat - Serialization format for results (e.g., 'json', 'xml', 'csv')
     * @param options.defaultGraph - Default graph(s) for the query
     * @param options.namedGraphs - Named graphs accessible in the query
     * @param options.useDefaultGraphAsUnion - Treat default graph as union of all graphs
     * @param options.substitutions - Variable bindings to substitute in the query
     * @returns Query results: boolean for ASK, Map array for SELECT, Quad array for CONSTRUCT/DESCRIBE, or string if resultsFormat specified
     *
     * @see {@link https://www.w3.org/TR/sparql11-query/ | SPARQL 1.1 Query Language}
     *
     * @example
     * ```typescript
     * // SELECT query
     * const results = store.query('SELECT * WHERE { ?s ?p ?o }');
     * for (const result of results) {
     *   console.log(result.get('s'), result.get('p'), result.get('o'));
     * }
     *
     * // ASK query
     * const exists = store.query('ASK { ?s a <http://example.com/Person> }');
     * console.log(exists); // true or false
     *
     * // CONSTRUCT query
     * const graph = store.query('CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }');
     *
     * // With prefixes
     * const results = store.query('SELECT * WHERE { ?s foaf:name ?name }', {
     *   prefixes: { foaf: 'http://xmlns.com/foaf/0.1/' }
     * });
     * ```
     */
    query(
        query: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
            resultsFormat?: string;
            defaultGraph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
            namedGraphs?: Iterable<BlankNode | NamedNode>;
            useDefaultGraphAsUnion?: boolean;
            substitutions?: Record<string, Term>;
        }
    ): boolean | Map<string, Term>[] | Quad[] | string;

    /**
     * Executes a SPARQL 1.1 query asynchronously.
     * Yields to the event loop every 1000 results to keep UI responsive.
     *
     * @param query - The SPARQL query string
     * @param options - Query options (same as query())
     * @returns Promise resolving to query results
     *
     * @see {@link https://www.w3.org/TR/sparql11-query/ | SPARQL 1.1 Query Language}
     *
     * @example
     * ```typescript
     * const results = await store.queryAsync('SELECT * WHERE { ?s ?p ?o }');
     * for (const result of results) {
     *   console.log(result.get('s'));
     * }
     * ```
     */
    queryAsync(
        query: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
            resultsFormat?: string;
            defaultGraph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
            namedGraphs?: Iterable<BlankNode | NamedNode>;
            useDefaultGraphAsUnion?: boolean;
            substitutions?: Record<string, Term>;
        }
    ): Promise<boolean | Map<string, Term>[] | Quad[] | string>;

    /**
     * Executes a SPARQL 1.1 update operation synchronously.
     *
     * @param update - The SPARQL update string
     * @param options - Update options
     * @param options.baseIri - Base IRI for relative URI resolution
     * @param options.prefixes - Namespace prefix mappings
     *
     * @see {@link https://www.w3.org/TR/sparql11-update/ | SPARQL 1.1 Update}
     *
     * @example
     * ```typescript
     * // Insert data
     * store.update('INSERT DATA { <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" }');
     *
     * // Delete data
     * store.update('DELETE DATA { <http://example.com/alice> <http://xmlns.com/foaf/0.1/name> "Alice" }');
     *
     * // Delete/Insert pattern
     * store.update(`
     *   DELETE { ?s foaf:name ?oldName }
     *   INSERT { ?s foaf:name "Updated Name" }
     *   WHERE { ?s foaf:name ?oldName }
     * `, {
     *   prefixes: { foaf: 'http://xmlns.com/foaf/0.1/' }
     * });
     * ```
     */
    update(
        update: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
        }
    ): void;

    /**
     * Executes a SPARQL 1.1 update operation asynchronously.
     *
     * @param update - The SPARQL update string
     * @param options - Update options (same as update())
     * @returns Promise that resolves when update completes
     *
     * @see {@link https://www.w3.org/TR/sparql11-update/ | SPARQL 1.1 Update}
     */
    updateAsync(
        update: string,
        options?: {
            baseIri?: NamedNode | string;
            prefixes?: Record<string, string>;
        }
    ): Promise<void>;

    /**
     * Adds multiple quads to the store.
     *
     * @param quads - Iterable of quads to add
     *
     * @example
     * ```typescript
     * store.extend([
     *   quad(namedNode('http://example.com/s1'), namedNode('http://example.com/p'), literal('o1')),
     *   quad(namedNode('http://example.com/s2'), namedNode('http://example.com/p'), literal('o2'))
     * ]);
     * ```
     */
    extend(quads: Iterable<Quad>): void;

    /**
     * Optimized bulk loading of RDF data.
     * Bypasses transactions for better performance on large datasets.
     *
     * @param data - The RDF data as a string
     * @param options - Parsing options
     * @param options.format - The RDF format
     * @param options.baseIri - Base IRI for relative URI resolution
     * @param options.toGraphName - Target graph
     * @param options.lenient - Continue parsing despite errors
     *
     * @example
     * ```typescript
     * const largeDataset = '...'; // Large Turtle file
     * store.bulkLoad(largeDataset, { format: 'turtle' });
     * ```
     */
    bulkLoad(
        data: string,
        options: {
            baseIri?: NamedNode | string;
            format: string;
            toGraphName?: BlankNode | DefaultGraph | NamedNode;
            lenient?: boolean;
        }
    ): void;

    /**
     * Returns all named graphs in the store.
     *
     * @returns Array of named graph identifiers (NamedNode or BlankNode)
     *
     * @example
     * ```typescript
     * const graphs = store.namedGraphs();
     * for (const graph of graphs) {
     *   console.log(graph.value);
     * }
     * ```
     */
    namedGraphs(): (BlankNode | NamedNode)[];

    /**
     * Checks if a named graph exists in the store.
     *
     * @param graph_name - The graph name to check
     * @returns true if the graph exists, false otherwise
     *
     * @example
     * ```typescript
     * const graphName = namedNode('http://example.com/graph1');
     * console.log(store.containsNamedGraph(graphName)); // false
     * store.addGraph(graphName);
     * console.log(store.containsNamedGraph(graphName)); // true
     * ```
     */
    containsNamedGraph(graph_name: BlankNode | DefaultGraph | NamedNode): boolean;

    /**
     * Creates a new named graph in the store.
     *
     * @param graph_name - The graph name
     *
     * @example
     * ```typescript
     * store.addGraph(namedNode('http://example.com/graph1'));
     * ```
     */
    addGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    /**
     * Removes all quads from a graph but keeps the graph.
     *
     * @param graph_name - The graph to clear
     *
     * @example
     * ```typescript
     * store.clearGraph(namedNode('http://example.com/graph1'));
     * ```
     */
    clearGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    /**
     * Removes a graph and all its quads from the store.
     *
     * @param graph_name - The graph to remove
     *
     * @example
     * ```typescript
     * store.removeGraph(namedNode('http://example.com/graph1'));
     * ```
     */
    removeGraph(graph_name: BlankNode | DefaultGraph | NamedNode): void;

    /**
     * Removes all quads from the store.
     *
     * @example
     * ```typescript
     * store.clear();
     * console.log(store.size); // 0
     * ```
     */
    clear(): void;

    /**
     * Executes a callback for each quad in the store.
     * Note: Crosses WASM boundary for each quad - consider using match() or SPARQL for better performance.
     *
     * @param callback - Function to call for each quad
     * @param thisArg - Value to use as 'this' when executing callback
     *
     * @example
     * ```typescript
     * store.forEach(quad => {
     *   console.log(quad.subject.value, quad.predicate.value, quad.object.value);
     * });
     * ```
     */
    forEach(callback: (quad: Quad) => void, thisArg?: any): void;

    /**
     * Returns an array of quads matching the predicate.
     *
     * @param predicate - Function to test each quad
     * @param thisArg - Value to use as 'this' when executing predicate
     * @returns Array of quads that pass the test
     *
     * @example
     * ```typescript
     * const literals = store.filter(quad => quad.object.termType === 'Literal');
     * ```
     */
    filter(predicate: (quad: Quad) => boolean, thisArg?: any): Quad[];

    /**
     * Tests whether at least one quad passes the predicate test.
     *
     * @param predicate - Function to test each quad
     * @param thisArg - Value to use as 'this' when executing predicate
     * @returns true if any quad passes the test
     *
     * @example
     * ```typescript
     * const hasLiterals = store.some(quad => quad.object.termType === 'Literal');
     * ```
     */
    some(predicate: (quad: Quad) => boolean, thisArg?: any): boolean;

    /**
     * Tests whether all quads pass the predicate test.
     *
     * @param predicate - Function to test each quad
     * @param thisArg - Value to use as 'this' when executing predicate
     * @returns true if all quads pass the test
     *
     * @example
     * ```typescript
     * const allHaveSubjects = store.every(quad => quad.subject !== null);
     * ```
     */
    every(predicate: (quad: Quad) => boolean, thisArg?: any): boolean;

    /**
     * Returns the first quad that passes the predicate test.
     *
     * @param predicate - Function to test each quad
     * @param thisArg - Value to use as 'this' when executing predicate
     * @returns The first matching quad, or undefined
     *
     * @example
     * ```typescript
     * const firstLiteral = store.find(quad => quad.object.termType === 'Literal');
     * ```
     */
    find(predicate: (quad: Quad) => boolean, thisArg?: any): Quad | undefined;

    /**
     * Returns the quad at the specified index.
     * Supports negative indices (counting from the end).
     *
     * @param index - The index (negative indices count from end)
     * @returns The quad at the index, or undefined
     *
     * @example
     * ```typescript
     * const first = store.at(0);
     * const last = store.at(-1);
     * ```
     */
    at(index: number): Quad | undefined;

    /**
     * Returns a shallow copy of a portion of the store.
     *
     * @param start - Start index (inclusive)
     * @param end - End index (exclusive)
     * @returns Array of quads in the slice
     *
     * @example
     * ```typescript
     * const firstTen = store.slice(0, 10);
     * ```
     */
    slice(start?: number, end?: number): Quad[];

    /**
     * Concatenates the store with other quads/iterables.
     *
     * @param others - Quads or iterables to concatenate
     * @returns New array with concatenated quads
     *
     * @example
     * ```typescript
     * const combined = store.concat([quad(...)], otherStore);
     * ```
     */
    concat(...others: (Quad | Iterable<Quad>)[]): Quad[];

    /**
     * Returns the first index of the quad in the store.
     *
     * @param quad - The quad to search for
     * @returns The index, or -1 if not found
     *
     * @example
     * ```typescript
     * const index = store.indexOf(myQuad);
     * ```
     */
    indexOf(quad: Quad): number;

    /**
     * Returns the index of the first quad that passes the predicate test.
     *
     * @param predicate - Function to test each quad
     * @param thisArg - Value to use as 'this' when executing predicate
     * @returns The index, or -1 if not found
     *
     * @example
     * ```typescript
     * const index = store.findIndex(quad => quad.object.termType === 'Literal');
     * ```
     */
    findIndex(predicate: (quad: Quad) => boolean, thisArg?: any): number;

    /**
     * Joins all quads into a string.
     *
     * @param separator - String to separate quads (default: ',')
     * @returns String representation of all quads
     *
     * @example
     * ```typescript
     * const str = store.join('\n');
     * ```
     */
    join(separator?: string): string;

    /**
     * Creates an array with the results of calling a function on every quad.
     *
     * @param callback - Function that produces an element of the new array
     * @param thisArg - Value to use as 'this' when executing callback
     * @returns Array of mapped values
     *
     * @example
     * ```typescript
     * const subjects = store.map(quad => quad.subject);
     * ```
     */
    map<T>(callback: (quad: Quad) => T, thisArg?: any): T[];

    /**
     * Reduces the store to a single value.
     *
     * @param callback - Function to execute on each quad
     * @param initialValue - Initial value for the accumulator
     * @returns The final accumulated value
     *
     * @example
     * ```typescript
     * const count = store.reduce((acc, quad) => acc + 1, 0);
     * ```
     */
    reduce<T>(callback: (accumulator: T, quad: Quad, index: number) => T, initialValue: T): T;

    /**
     * Returns an iterator of [index, quad] pairs.
     *
     * @returns Iterator of entries
     *
     * @example
     * ```typescript
     * for (const [index, quad] of store.entries()) {
     *   console.log(index, quad);
     * }
     * ```
     */
    entries(): IterableIterator<[number, Quad]>;

    /**
     * Returns an iterator of indices.
     *
     * @returns Iterator of keys
     *
     * @example
     * ```typescript
     * for (const index of store.keys()) {
     *   console.log(index);
     * }
     * ```
     */
    keys(): IterableIterator<number>;

    /**
     * Returns an iterator of quads.
     *
     * @returns Iterator of values
     *
     * @example
     * ```typescript
     * for (const quad of store.values()) {
     *   console.log(quad);
     * }
     * ```
     */
    values(): IterableIterator<Quad>;

    /**
     * Returns an iterator of quads (makes Store iterable).
     *
     * @returns Iterator of quads
     *
     * @example
     * ```typescript
     * for (const quad of store) {
     *   console.log(quad);
     * }
     * ```
     */
    [Symbol.iterator](): Iterator<Quad>;

    /**
     * Begins a new transaction for batch modifications.
     * Transactions can improve performance when making many changes.
     *
     * @returns A transaction object
     *
     * @example
     * ```typescript
     * const tx = store.beginTransaction();
     * tx.add(quad(...));
     * tx.add(quad(...));
     * tx.delete(quad(...));
     * tx.commit();
     * ```
     */
    beginTransaction(): StoreTransaction;

    /**
     * Convenience method to load Turtle data.
     * Equivalent to load(data, { format: 'turtle', ...options })
     *
     * @param data - Turtle data as a string
     * @param options - Parsing options
     * @param options.baseIri - Base IRI for relative URI resolution
     * @param options.toGraphName - Target graph
     * @param options.lenient - Continue parsing despite errors
     *
     * @see {@link https://www.w3.org/TR/turtle/ | RDF 1.1 Turtle}
     *
     * @example
     * ```typescript
     * store.loadTurtle(`
     *   @prefix ex: <http://example.com/> .
     *   ex:alice a ex:Person .
     * `);
     * ```
     */
    loadTurtle(data: string, options?: {
        baseIri?: NamedNode | string;
        toGraphName?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
    }): void;

    /**
     * Convenience method to load N-Triples data.
     * Equivalent to load(data, { format: 'ntriples', ...options })
     *
     * @param data - N-Triples data as a string
     * @param options - Parsing options
     * @param options.toGraphName - Target graph
     * @param options.lenient - Continue parsing despite errors
     *
     * @see {@link https://www.w3.org/TR/n-triples/ | RDF 1.1 N-Triples}
     *
     * @example
     * ```typescript
     * store.loadNTriples(`
     *   <http://example.com/alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.com/Person> .
     * `);
     * ```
     */
    loadNTriples(data: string, options?: {
        toGraphName?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
    }): void;

    /**
     * Convenience method to serialize to Turtle format.
     * Equivalent to dump({ format: 'turtle', ...options })
     *
     * @param options - Serialization options
     * @param options.fromGraphName - Graph to serialize
     * @param options.prefixes - Namespace prefix mappings
     * @param options.baseIri - Base IRI for relative URIs
     * @returns Turtle-formatted string
     *
     * @see {@link https://www.w3.org/TR/turtle/ | RDF 1.1 Turtle}
     *
     * @example
     * ```typescript
     * const turtle = store.toTurtle({
     *   prefixes: {
     *     ex: 'http://example.com/',
     *     foaf: 'http://xmlns.com/foaf/0.1/'
     *   }
     * });
     * ```
     */
    toTurtle(options?: {
        fromGraphName?: BlankNode | DefaultGraph | NamedNode;
        prefixes?: Record<string, string>;
        baseIri?: NamedNode | string;
    }): string;

    /**
     * Convenience method to serialize to N-Triples format.
     * Equivalent to dump({ format: 'ntriples', ...options })
     *
     * @param options - Serialization options
     * @param options.fromGraphName - Graph to serialize
     * @returns N-Triples-formatted string
     *
     * @see {@link https://www.w3.org/TR/n-triples/ | RDF 1.1 N-Triples}
     *
     * @example
     * ```typescript
     * const ntriples = store.toNTriples();
     * ```
     */
    toNTriples(options?: {
        fromGraphName?: BlankNode | DefaultGraph | NamedNode;
    }): string;
}

/**
 * A transaction for batch modifications to a Store.
 * Provides better performance for multiple add/delete operations.
 *
 * @example
 * ```typescript
 * const tx = store.beginTransaction();
 * try {
 *   tx.add(quad(namedNode('http://example.com/s1'), namedNode('http://example.com/p'), literal('o1')));
 *   tx.add(quad(namedNode('http://example.com/s2'), namedNode('http://example.com/p'), literal('o2')));
 *   tx.delete(quad(namedNode('http://example.com/old'), namedNode('http://example.com/p'), literal('old')));
 *   tx.commit();
 * } catch (e) {
 *   // Transaction will be rolled back if not committed
 *   console.error('Transaction failed:', e);
 * }
 * ```
 */
export class StoreTransaction {
    /**
     * Adds a quad to the transaction.
     * Changes are not visible until commit() is called.
     *
     * @param quad - The quad to add
     */
    add(quad: Quad): void;

    /**
     * Removes a quad in the transaction.
     * Changes are not visible until commit() is called.
     *
     * @param quad - The quad to delete
     */
    delete(quad: Quad): void;

    /**
     * Commits the transaction, making all changes visible.
     * After commit, the transaction cannot be reused.
     */
    commit(): void;
}
"###;

#[wasm_bindgen(js_name = Store, skip_typescript)]
pub struct JsStore {
    pub(crate) store: Store,
}

#[wasm_bindgen(js_class = Store)]
impl JsStore {
    #[wasm_bindgen(constructor)]
    pub fn new(quads: &JsValue) -> Result<JsStore, JsValue> {
        console_error_panic_hook::set_once();

        let store = Self {
            store: Store::new().map_err(JsError::from)?,
        };
        if !quads.is_undefined() && !quads.is_null() {
            if let Some(quads) = try_iter(quads)? {
                for quad in quads {
                    store.add(&quad?)?;
                }
            }
        }
        Ok(store)
    }

    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .insert(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?;
        Ok(())
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        self.store
            .remove(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?;
        Ok(())
    }

    pub fn has(&self, quad: &JsValue) -> Result<bool, JsValue> {
        Ok(self
            .store
            .contains(&FROM_JS.with(|c| c.to_quad(quad))?)
            .map_err(JsError::from)?)
    }

    pub fn includes(&self, quad: &JsValue) -> Result<bool, JsValue> {
        self.has(quad)
    }

    #[wasm_bindgen(getter=size)]
    pub fn size(&self) -> Result<usize, JsError> {
        Ok(self.store.len()?)
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> Result<usize, JsError> {
        Ok(self.store.len()?)
    }

    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> Result<bool, JsError> {
        Ok(self.store.is_empty()?)
    }

    #[wasm_bindgen(js_name = match)]
    pub fn match_quads(
        &self,
        subject: &JsValue,
        predicate: &JsValue,
        object: &JsValue,
        graph_name: &JsValue,
    ) -> Result<Box<[JsValue]>, JsValue> {
        Ok(self
            .store
            .quads_for_pattern(
                if let Some(subject) = FROM_JS.with(|c| c.to_optional_term(subject))? {
                    Some(subject.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&NamedOrBlankNode>::into),
                if let Some(predicate) = FROM_JS.with(|c| c.to_optional_term(predicate))? {
                    Some(NamedNode::try_from(predicate)?)
                } else {
                    None
                }
                .as_ref()
                .map(<&NamedNode>::into),
                if let Some(object) = FROM_JS.with(|c| c.to_optional_term(object))? {
                    Some(object.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&Term>::into),
                if let Some(graph_name) = FROM_JS.with(|c| c.to_optional_term(graph_name))? {
                    Some(graph_name.try_into()?)
                } else {
                    None
                }
                .as_ref()
                .map(<&GraphName>::into),
            )
            .map(|v| v.map(|v| JsQuad::from(v).into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
            .into_boxed_slice())
    }

    pub fn query(&self, query: &str, options: &JsValue) -> Result<JsValue, JsValue> {
        // Parsing options
        let mut base_iri = None;
        let mut prefixes = None;
        let mut use_default_graph_as_union = false;
        let mut results_format = None;
        let mut default_graph = None;
        let mut named_graphs = None;
        let mut substitutions = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;

            let js_prefixes = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                prefixes = Some(extract_prefixes(&js_prefixes)?);
            }

            let js_default_graph = Reflect::get(options, &JsValue::from_str("defaultGraph"))?;
            default_graph = if js_default_graph.is_undefined() || js_default_graph.is_null() {
                None
            } else if let Some(iter) = try_iter(&js_default_graph)? {
                Some(
                    iter.map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                        .collect::<Result<Vec<GraphName>, _>>()?,
                )
            } else {
                Some(vec![
                    FROM_JS.with(|c| c.to_term(&js_default_graph))?.try_into()?,
                ])
            };

            let js_named_graphs = Reflect::get(options, &JsValue::from_str("namedGraphs"))?;
            named_graphs = if js_named_graphs.is_null() || js_named_graphs.is_undefined() {
                None
            } else {
                Some(
                    try_iter(&Reflect::get(options, &JsValue::from_str("namedGraphs"))?)?
                        .ok_or_else(|| format_err!("namedGraphs option must be iterable"))?
                        .map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                        .collect::<Result<Vec<NamedOrBlankNode>, _>>()?,
                )
            };

            use_default_graph_as_union =
                Reflect::get(options, &JsValue::from_str("useDefaultGraphAsUnion"))?.is_truthy();

            let js_results_format = Reflect::get(options, &JsValue::from_str("resultsFormat"))?;
            if !js_results_format.is_undefined() && !js_results_format.is_null() {
                results_format = Some(
                    js_results_format
                        .as_string()
                        .ok_or_else(|| format_err!("resultsFormat option must be a string"))?,
                );
            }

            let js_substitutions = Reflect::get(options, &JsValue::from_str("substitutions"))?;
            if !js_substitutions.is_undefined() && !js_substitutions.is_null() {
                substitutions = Some(extract_substitutions(&js_substitutions)?);
            }
        }

        let mut evaluator = SparqlEvaluator::new();
        #[cfg(feature = "geosparql")]
        for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
            evaluator = evaluator.with_custom_function(name.into(), implementation)
        }
        if let Some(base_iri) = base_iri {
            evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                evaluator = evaluator
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }

        let mut prepared_query = evaluator.parse_query(&query).map_err(JsError::from)?;
        if use_default_graph_as_union {
            prepared_query.dataset_mut().set_default_graph_as_union();
        }
        if let Some(default_graph) = default_graph {
            prepared_query
                .dataset_mut()
                .set_default_graph(default_graph);
        }
        if let Some(named_graphs) = named_graphs {
            prepared_query
                .dataset_mut()
                .set_available_named_graphs(named_graphs);
        }
        if let Some(substitutions) = substitutions {
            for (variable, term) in substitutions {
                prepared_query = prepared_query.substitute_variable(variable, term);
            }
        }

        let results = prepared_query
            .on_store(&self.store)
            .execute()
            .map_err(JsError::from)?;
        Ok(match results {
            QueryResults::Solutions(solutions) => {
                if let Some(results_format) = results_format {
                    let mut serializer =
                        QueryResultsSerializer::from_format(query_results_format(&results_format)?)
                            .serialize_solutions_to_writer(Vec::new(), solutions.variables().into())
                            .map_err(JsError::from)?;
                    for solution in solutions {
                        serializer
                            .serialize(&solution.map_err(JsError::from)?)
                            .map_err(JsError::from)?;
                    }
                    JsValue::from_str(
                        &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                            .map_err(JsError::from)?,
                    )
                } else {
                    let results = Array::new();
                    for solution in solutions {
                        let solution = solution.map_err(JsError::from)?;
                        let result = Map::new();
                        for (variable, value) in solution.iter() {
                            result.set(
                                &variable.as_str().into(),
                                &JsTerm::from(value.clone()).into(),
                            );
                        }
                        results.push(&result.into());
                    }
                    results.into()
                }
            }
            QueryResults::Graph(triples) => {
                if let Some(results_format) = results_format {
                    let mut serializer = RdfSerializer::from_format(rdf_format(&results_format)?)
                        .for_writer(Vec::new());
                    for triple in triples {
                        serializer
                            .serialize_triple(&triple.map_err(JsError::from)?)
                            .map_err(JsError::from)?;
                    }
                    JsValue::from_str(
                        &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                            .map_err(JsError::from)?,
                    )
                } else {
                    let results = Array::new();
                    for triple in triples {
                        results.push(
                            &JsQuad::from(
                                triple
                                    .map_err(JsError::from)?
                                    .in_graph(GraphName::DefaultGraph),
                            )
                            .into(),
                        );
                    }
                    results.into()
                }
            }
            QueryResults::Boolean(b) => {
                if let Some(results_format) = results_format {
                    JsValue::from_str(
                        &String::from_utf8(
                            QueryResultsSerializer::from_format(query_results_format(
                                &results_format,
                            )?)
                            .serialize_boolean_to_writer(Vec::new(), b)
                            .map_err(JsError::from)?,
                        )
                        .map_err(JsError::from)?,
                    )
                } else {
                    b.into()
                }
            }
        })
    }

    #[wasm_bindgen(js_name = queryAsync)]
    pub fn query_async(&self, query: String, options: JsValue) -> Promise {
        let store = self.store.clone();
        future_to_promise(async move {
            // Parsing options
            let mut base_iri = None;
            let mut prefixes = None;
            let mut use_default_graph_as_union = false;
            let mut results_format = None;
            let mut default_graph = None;
            let mut named_graphs = None;
            let mut substitutions = None;
            if !options.is_undefined() {
                base_iri =
                    convert_base_iri(&Reflect::get(&options, &JsValue::from_str("baseIri"))?)?;

                let js_prefixes = Reflect::get(&options, &JsValue::from_str("prefixes"))?;
                if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                    prefixes = Some(extract_prefixes(&js_prefixes)?);
                }

                let js_default_graph = Reflect::get(&options, &JsValue::from_str("defaultGraph"))?;
                default_graph = if js_default_graph.is_undefined() || js_default_graph.is_null() {
                    None
                } else if let Some(iter) = try_iter(&js_default_graph)? {
                    Some(
                        iter.map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                            .collect::<Result<Vec<GraphName>, _>>()?,
                    )
                } else {
                    Some(vec![
                        FROM_JS.with(|c| c.to_term(&js_default_graph))?.try_into()?,
                    ])
                };

                let js_named_graphs = Reflect::get(&options, &JsValue::from_str("namedGraphs"))?;
                named_graphs = if js_named_graphs.is_null() || js_named_graphs.is_undefined() {
                    None
                } else {
                    Some(
                        try_iter(&Reflect::get(&options, &JsValue::from_str("namedGraphs"))?)?
                            .ok_or_else(|| format_err!("namedGraphs option must be iterable"))?
                            .map(|term| FROM_JS.with(|c| c.to_term(&term?))?.try_into())
                            .collect::<Result<Vec<NamedOrBlankNode>, _>>()?,
                    )
                };

                use_default_graph_as_union =
                    Reflect::get(&options, &JsValue::from_str("useDefaultGraphAsUnion"))?
                        .is_truthy();

                let js_results_format =
                    Reflect::get(&options, &JsValue::from_str("resultsFormat"))?;
                if !js_results_format.is_undefined() && !js_results_format.is_null() {
                    results_format = Some(
                        js_results_format
                            .as_string()
                            .ok_or_else(|| format_err!("resultsFormat option must be a string"))?,
                    );
                }

                let js_substitutions = Reflect::get(&options, &JsValue::from_str("substitutions"))?;
                if !js_substitutions.is_undefined() && !js_substitutions.is_null() {
                    substitutions = Some(extract_substitutions(&js_substitutions)?);
                }
            }

            let mut evaluator = SparqlEvaluator::new();
            #[cfg(feature = "geosparql")]
            for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
                evaluator = evaluator.with_custom_function(name.into(), implementation)
            }
            if let Some(base_iri) = base_iri {
                evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
            }
            if let Some(prefixes) = prefixes {
                for (prefix_name, prefix_iri) in prefixes {
                    evaluator = evaluator
                        .with_prefix(&prefix_name, &prefix_iri)
                        .map_err(JsError::from)?;
                }
            }

            let mut prepared_query = evaluator.parse_query(&query).map_err(JsError::from)?;
            if use_default_graph_as_union {
                prepared_query.dataset_mut().set_default_graph_as_union();
            }
            if let Some(default_graph) = default_graph {
                prepared_query
                    .dataset_mut()
                    .set_default_graph(default_graph);
            }
            if let Some(named_graphs) = named_graphs {
                prepared_query
                    .dataset_mut()
                    .set_available_named_graphs(named_graphs);
            }
            if let Some(substitutions) = substitutions {
                for (variable, term) in substitutions {
                    prepared_query = prepared_query.substitute_variable(variable, term);
                }
            }

            let results = prepared_query
                .on_store(&store)
                .execute()
                .map_err(JsError::from)?;
            Ok(match results {
                QueryResults::Solutions(solutions) => {
                    if let Some(results_format) = results_format {
                        let mut serializer = QueryResultsSerializer::from_format(
                            query_results_format(&results_format)?,
                        )
                        .serialize_solutions_to_writer(Vec::new(), solutions.variables().into())
                        .map_err(JsError::from)?;
                        for solution in solutions {
                            serializer
                                .serialize(&solution.map_err(JsError::from)?)
                                .map_err(JsError::from)?;
                        }
                        JsValue::from_str(
                            &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                                .map_err(JsError::from)?,
                        )
                    } else {
                        let results = Array::new();
                        let mut count = 0;
                        for solution in solutions {
                            let solution = solution.map_err(JsError::from)?;
                            let result = Map::new();
                            for (variable, value) in solution.iter() {
                                result.set(
                                    &variable.as_str().into(),
                                    &JsTerm::from(value.clone()).into(),
                                );
                            }
                            results.push(&result.into());

                            // Yield to the event loop every 1000 solutions to keep the UI responsive
                            count += 1;
                            if count % 1000 == 0 {
                                let promise = Promise::resolve(&JsValue::undefined());
                                wasm_bindgen_futures::JsFuture::from(promise).await?;
                            }
                        }
                        results.into()
                    }
                }
                QueryResults::Graph(triples) => {
                    if let Some(results_format) = results_format {
                        let mut serializer =
                            RdfSerializer::from_format(rdf_format(&results_format)?)
                                .for_writer(Vec::new());
                        for triple in triples {
                            serializer
                                .serialize_triple(&triple.map_err(JsError::from)?)
                                .map_err(JsError::from)?;
                        }
                        JsValue::from_str(
                            &String::from_utf8(serializer.finish().map_err(JsError::from)?)
                                .map_err(JsError::from)?,
                        )
                    } else {
                        let results = Array::new();
                        let mut count = 0;
                        for triple in triples {
                            results.push(
                                &JsQuad::from(
                                    triple
                                        .map_err(JsError::from)?
                                        .in_graph(GraphName::DefaultGraph),
                                )
                                .into(),
                            );

                            // Yield to the event loop every 1000 triples to keep the UI responsive
                            count += 1;
                            if count % 1000 == 0 {
                                let promise = Promise::resolve(&JsValue::undefined());
                                wasm_bindgen_futures::JsFuture::from(promise).await?;
                            }
                        }
                        results.into()
                    }
                }
                QueryResults::Boolean(b) => {
                    if let Some(results_format) = results_format {
                        JsValue::from_str(
                            &String::from_utf8(
                                QueryResultsSerializer::from_format(query_results_format(
                                    &results_format,
                                )?)
                                .serialize_boolean_to_writer(Vec::new(), b)
                                .map_err(JsError::from)?,
                            )
                            .map_err(JsError::from)?,
                        )
                    } else {
                        b.into()
                    }
                }
            })
        })
    }

    pub fn update(&self, update: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut base_iri = None;
        let mut prefixes = None;
        if !options.is_undefined() {
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;

            let js_prefixes = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                prefixes = Some(extract_prefixes(&js_prefixes)?);
            }
        }

        let mut evaluator = SparqlEvaluator::new();
        #[cfg(feature = "geosparql")]
        for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
            evaluator = evaluator.with_custom_function(name.into(), implementation)
        }
        if let Some(base_iri) = base_iri {
            evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                evaluator = evaluator
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }

        Ok(evaluator
            .parse_update(update)
            .map_err(JsError::from)?
            .on_store(&self.store)
            .execute()
            .map_err(JsError::from)?)
    }

    #[wasm_bindgen(js_name = updateAsync)]
    pub fn update_async(&self, update: String, options: JsValue) -> Promise {
        let store = self.store.clone();
        future_to_promise(async move {
            // Parsing options
            let mut base_iri = None;
            let mut prefixes = None;
            if !options.is_undefined() {
                base_iri =
                    convert_base_iri(&Reflect::get(&options, &JsValue::from_str("baseIri"))?)?;

                let js_prefixes = Reflect::get(&options, &JsValue::from_str("prefixes"))?;
                if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
                    prefixes = Some(extract_prefixes(&js_prefixes)?);
                }
            }

            let mut evaluator = SparqlEvaluator::new();
            #[cfg(feature = "geosparql")]
            for (name, implementation) in GEOSPARQL_EXTENSION_FUNCTIONS {
                evaluator = evaluator.with_custom_function(name.into(), implementation)
            }
            if let Some(base_iri) = base_iri {
                evaluator = evaluator.with_base_iri(base_iri).map_err(JsError::from)?;
            }
            if let Some(prefixes) = prefixes {
                for (prefix_name, prefix_iri) in prefixes {
                    evaluator = evaluator
                        .with_prefix(&prefix_name, &prefix_iri)
                        .map_err(JsError::from)?;
                }
            }

            evaluator
                .parse_update(&update)
                .map_err(JsError::from)?
                .on_store(&store)
                .execute()
                .map_err(JsError::from)?;

            Ok(JsValue::undefined())
        })
    }

    pub fn load(
        &self,
        data: &str,
        options: &JsValue,
        base_iri: &JsValue,
        to_graph_name: &JsValue,
    ) -> Result<(), JsValue> {
        // Parsing options
        let mut format = None;
        let mut parsed_base_iri = None;
        let mut parsed_to_graph_name = None;
        let mut unchecked = false;
        let mut lenient = false;
        let mut no_transaction = false;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!(
                "The format should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt'}})"
            );
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            parsed_base_iri =
                convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
            let to_graph_name_js = Reflect::get(options, &JsValue::from_str("toGraphName"))?;
            parsed_to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
            unchecked = Reflect::get(options, &JsValue::from_str("unchecked"))?.is_truthy();
            lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
            no_transaction =
                Reflect::get(options, &JsValue::from_str("noTransaction"))?.is_truthy();
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.load(my_content, {{format: 'nt'}}"))?;
        if let Some(base_iri) = convert_base_iri(base_iri)? {
            console_warn!(
                "The baseIri should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', baseIri: 'http//example.com'}})"
            );
            parsed_base_iri = Some(base_iri);
        }
        if let Some(to_graph_name) = FROM_JS.with(|c| c.to_optional_term(to_graph_name))? {
            console_warn!(
                "The target graph name should be passed to Store.load in an option dictionary like store.load(my_content, {{format: 'nt', toGraphName: 'http//example.com'}})"
            );
            parsed_to_graph_name = Some(to_graph_name);
        }

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = parsed_to_graph_name {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = parsed_base_iri {
            parser = parser.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if unchecked {
            console_warn!(
                "The `unchecked` option in Store.load is deprecated, please use `lenient` instead"
            );
            parser = parser.lenient();
        } else if lenient {
            parser = parser.lenient();
        }
        if no_transaction {
            let mut loader = self.store.bulk_loader();
            loader
                .load_from_slice(parser, data.as_bytes())
                .map_err(JsError::from)?;
            loader.commit().map_err(JsError::from)?;
        } else {
            self.store
                .load_from_slice(parser, data)
                .map_err(JsError::from)?;
        }
        Ok(())
    }

    pub fn dump(&self, options: &JsValue, from_graph_name: &JsValue) -> Result<String, JsValue> {
        // Serialization options
        let mut format = None;
        let mut parsed_from_graph_name = None;
        let mut prefixes = None;
        let mut base_iri = None;
        if let Some(format_str) = options.as_string() {
            // Backward compatibility with format as a string
            console_warn!(
                "The format should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt'}})"
            );
            format = Some(rdf_format(&format_str)?);
        } else if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            let from_graph_name_js = Reflect::get(options, &JsValue::from_str("fromGraphName"))?;
            parsed_from_graph_name = FROM_JS.with(|c| c.to_optional_term(&from_graph_name_js))?;

            // Parse prefixes option
            let prefixes_js = Reflect::get(options, &JsValue::from_str("prefixes"))?;
            if !prefixes_js.is_undefined() && !prefixes_js.is_null() {
                let mut prefixes_map = Vec::new();

                // Handle both plain objects and Maps
                if let Some(entries) = try_iter(&prefixes_js)? {
                    // It's a Map or other iterable
                    for entry in entries {
                        let entry = entry?;
                        let key = Reflect::get(&entry, &0.into())?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix key must be a string"))?;
                        let value = Reflect::get(&entry, &1.into())?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix IRI must be a string"))?;
                        prefixes_map.push((key, value));
                    }
                } else {
                    // It's a plain object
                    let keys = js_sys::Object::keys(&js_sys::Object::from(prefixes_js.clone()));
                    for i in 0..keys.length() {
                        let key = keys
                            .get(i)
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix key must be a string"))?;
                        let value = Reflect::get(&prefixes_js, &JsValue::from_str(&key))?
                            .as_string()
                            .ok_or_else(|| format_err!("Prefix IRI must be a string"))?;
                        prefixes_map.push((key, value));
                    }
                }
                prefixes = Some(prefixes_map);
            }

            // Parse base_iri option
            base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like store.dump({{format: 'nt'}}"))?;
        if let Some(from_graph_name) = FROM_JS.with(|c| c.to_optional_term(from_graph_name))? {
            console_warn!(
                "The source graph name should be passed to Store.dump in an option dictionary like store.dump({{format: 'nt', fromGraphName: 'http//example.com'}})"
            );
            parsed_from_graph_name = Some(from_graph_name);
        }

        // Create serializer with prefixes and base_iri
        let mut serializer = RdfSerializer::from_format(format);
        if let Some(prefixes) = prefixes {
            for (prefix_name, prefix_iri) in prefixes {
                serializer = serializer
                    .with_prefix(&prefix_name, &prefix_iri)
                    .map_err(JsError::from)?;
            }
        }
        if let Some(base_iri) = base_iri {
            serializer = serializer.with_base_iri(base_iri).map_err(JsError::from)?;
        }

        let buffer = if let Some(from_graph_name) = parsed_from_graph_name {
            self.store.dump_graph_to_writer(
                &GraphName::try_from(from_graph_name)?,
                serializer,
                Vec::new(),
            )
        } else {
            self.store.dump_to_writer(serializer, Vec::new())
        }
        .map_err(JsError::from)?;
        Ok(String::from_utf8(buffer).map_err(JsError::from)?)
    }

    pub fn extend(&self, quads: &JsValue) -> Result<(), JsValue> {
        let quads = if let Some(quads) = try_iter(quads)? {
            quads
                .map(|q| FROM_JS.with(|c| c.to_quad(&q?)))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            return Err(format_err!("quads argument must be iterable"));
        };
        self.store.extend(quads).map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = bulkLoad)]
    pub fn bulk_load(&self, data: &str, options: &JsValue) -> Result<(), JsValue> {
        // Parsing options
        let mut format = None;
        let mut parsed_base_iri = None;
        let mut parsed_to_graph_name = None;
        let mut lenient = false;
        if !options.is_undefined() && !options.is_null() {
            if let Some(format_str) =
                Reflect::get(options, &JsValue::from_str("format"))?.as_string()
            {
                format = Some(rdf_format(&format_str)?);
            }
            parsed_base_iri =
                convert_base_iri(&Reflect::get(options, &JsValue::from_str("baseIri"))?)?;
            let to_graph_name_js = Reflect::get(options, &JsValue::from_str("toGraphName"))?;
            parsed_to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
            lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
        }
        let format = format
            .ok_or_else(|| format_err!("The format option should be provided like store.bulk_load(my_content, {{format: 'nt'}}"))?;

        let mut parser = RdfParser::from_format(format);
        if let Some(to_graph_name) = parsed_to_graph_name {
            parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
        }
        if let Some(base_iri) = parsed_base_iri {
            parser = parser.with_base_iri(base_iri).map_err(JsError::from)?;
        }
        if lenient {
            parser = parser.lenient();
        }

        let mut loader = self.store.bulk_loader();
        loader
            .load_from_slice(parser, data.as_bytes())
            .map_err(JsError::from)?;
        loader.commit().map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = namedGraphs)]
    pub fn named_graphs(&self) -> Result<Box<[JsValue]>, JsValue> {
        Ok(self
            .store
            .named_graphs()
            .map(|g| {
                g.map(|g| {
                    JsTerm::from(match g {
                        NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n),
                        NamedOrBlankNode::BlankNode(n) => Term::BlankNode(n),
                    })
                    .into()
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
            .into_boxed_slice())
    }

    #[wasm_bindgen(js_name = containsNamedGraph)]
    pub fn contains_named_graph(&self, graph_name: &JsValue) -> Result<bool, JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        let result = match &graph_name_ref {
            GraphName::DefaultGraph => Ok(true),
            GraphName::NamedNode(g) => self.store.contains_named_graph(g),
            GraphName::BlankNode(g) => self.store.contains_named_graph(g),
        };
        Ok(result.map_err(JsError::from)?)
    }

    #[wasm_bindgen(js_name = addGraph)]
    pub fn add_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        match &graph_name_ref {
            GraphName::DefaultGraph => Ok(()),
            GraphName::NamedNode(g) => self.store.insert_named_graph(g),
            GraphName::BlankNode(g) => self.store.insert_named_graph(g),
        }
        .map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = clearGraph)]
    pub fn clear_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        self.store
            .clear_graph(&graph_name_ref)
            .map_err(JsError::from)?;
        Ok(())
    }

    #[wasm_bindgen(js_name = removeGraph)]
    pub fn remove_graph(&self, graph_name: &JsValue) -> Result<(), JsValue> {
        let graph_name = FROM_JS.with(|c| c.to_term(graph_name))?;
        let graph_name_ref = GraphName::try_from(graph_name)?;
        match &graph_name_ref {
            GraphName::DefaultGraph => self.store.clear_graph(GraphNameRef::DefaultGraph),
            GraphName::NamedNode(g) => self.store.remove_named_graph(g),
            GraphName::BlankNode(g) => self.store.remove_named_graph(g),
        }
        .map_err(JsError::from)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), JsValue> {
        self.store.clear().map_err(JsError::from)?;
        Ok(())
    }

    /// JavaScript-idiomatic collection methods
    /// Note: These methods iterate over all quads and call JavaScript callbacks,
    /// which crosses the JS/WASM boundary for each quad. For large datasets,
    /// consider using SPARQL queries or the match() method for better performance.

    #[wasm_bindgen(js_name = forEach)]
    pub fn for_each(&self, callback: &Function, this_arg: &JsValue) -> Result<(), JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            callback.call1(&this, &js_quad)?;
        }
        Ok(())
    }

    pub fn filter(
        &self,
        predicate: &Function,
        this_arg: &JsValue,
    ) -> Result<Box<[JsValue]>, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        let mut results = Vec::new();
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                results.push(js_quad);
            }
        }
        Ok(results.into_boxed_slice())
    }

    pub fn some(&self, predicate: &Function, this_arg: &JsValue) -> Result<bool, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn every(&self, predicate: &Function, this_arg: &JsValue) -> Result<bool, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if !matches.is_truthy() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn find(&self, predicate: &Function, this_arg: &JsValue) -> Result<JsValue, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let matches = predicate.call1(&this, &js_quad)?;
            if matches.is_truthy() {
                return Ok(js_quad);
            }
        }
        Ok(JsValue::UNDEFINED)
    }

    pub fn at(&self, index: i32) -> Result<JsValue, JsValue> {
        let quads: Vec<Quad> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;

        let len = quads.len() as i32;
        let idx = if index < 0 { len + index } else { index };

        if idx < 0 || idx >= len {
            Ok(JsValue::UNDEFINED)
        } else {
            Ok(JsQuad::from(quads[idx as usize].clone()).into())
        }
    }

    pub fn slice(&self, start: Option<i32>, end: Option<i32>) -> Result<Box<[JsValue]>, JsValue> {
        let quads: Vec<JsValue> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|q| q.map(|quad| JsQuad::from(quad).into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;

        let len = quads.len() as i32;
        let start = start.unwrap_or(0);
        let end = end.unwrap_or(len);

        // Handle negative indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len)
        } as usize;
        let end = if end < 0 {
            (len + end).max(0)
        } else {
            end.min(len)
        } as usize;

        Ok(quads[start..end.max(start)].to_vec().into_boxed_slice())
    }

    pub fn concat(&self, others: &JsValue) -> Result<Box<[JsValue]>, JsValue> {
        let mut results: Vec<JsValue> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|q| q.map(|quad| JsQuad::from(quad).into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;

        // Handle additional iterables passed
        if !others.is_undefined() && !others.is_null() {
            if let Some(iter) = try_iter(others)? {
                for item in iter {
                    let item = item?;
                    // If item is iterable (another store/dataset/array), flatten it
                    if let Some(inner_iter) = try_iter(&item)? {
                        for inner_item in inner_iter {
                            results.push(inner_item?);
                        }
                    } else {
                        results.push(item);
                    }
                }
            }
        }
        Ok(results.into_boxed_slice())
    }

    #[wasm_bindgen(js_name = indexOf)]
    pub fn index_of(&self, quad: &JsValue) -> Result<i32, JsValue> {
        let target = FROM_JS.with(|c| c.to_quad(quad))?;
        for (index, q) in self
            .store
            .quads_for_pattern(None, None, None, None)
            .enumerate()
        {
            let q = q.map_err(JsError::from)?;
            if q == target {
                return Ok(index as i32);
            }
        }
        Ok(-1)
    }

    #[wasm_bindgen(js_name = findIndex)]
    pub fn find_index(&self, predicate: &Function, this_arg: &JsValue) -> Result<i32, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        for (index, quad) in self
            .store
            .quads_for_pattern(None, None, None, None)
            .enumerate()
        {
            let quad = quad.map_err(JsError::from)?;
            let js_quad = JsQuad::from(quad).into();
            if predicate.call1(&this, &js_quad)?.is_truthy() {
                return Ok(index as i32);
            }
        }
        Ok(-1)
    }
    pub fn join(&self, separator: Option<String>) -> Result<String, JsValue> {
        let sep = separator.unwrap_or_else(|| ",".to_string());
        let strings: Vec<String> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|q| q.map(|quad| quad.to_string()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;
        Ok(strings.join(&sep))
    }

    pub fn map(&self, callback: &Function, this_arg: &JsValue) -> Result<Box<[JsValue]>, JsValue> {
        let this = if this_arg.is_undefined() {
            JsValue::NULL
        } else {
            this_arg.clone()
        };
        let mut results = Vec::new();
        for quad in self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
        {
            let js_quad = JsQuad::from(quad).into();
            let mapped = callback.call1(&this, &js_quad)?;
            results.push(mapped);
        }
        Ok(results.into_boxed_slice())
    }

    pub fn reduce(&self, callback: &Function, initial_value: &JsValue) -> Result<JsValue, JsValue> {
        let quads: Vec<Quad> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;

        let mut accumulator = initial_value.clone();
        for (index, quad) in quads.iter().enumerate() {
            let js_quad = JsQuad::from(quad.clone()).into();
            accumulator = callback.call3(
                &JsValue::UNDEFINED,
                &accumulator,
                &js_quad,
                &JsValue::from(index as u32),
            )?;
        }
        Ok(accumulator)
    }

    pub fn entries(&self) -> Result<JsValue, JsValue> {
        let quads: Vec<_> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .enumerate()
            .map(|(i, q)| {
                q.map(|quad| {
                    let arr = Array::new();
                    arr.push(&JsValue::from(i as u32));
                    arr.push(&JsQuad::from(quad).into());
                    arr.into()
                })
            })
            .collect::<Result<Vec<JsValue>, _>>()
            .map_err(JsError::from)?;
        Ok(Array::from_iter(quads).values().into())
    }

    pub fn keys(&self) -> Result<JsValue, JsValue> {
        let count = self.store.len().map_err(JsError::from)? as u32;
        let keys: Vec<JsValue> = (0..count).map(JsValue::from).collect();
        Ok(Array::from_iter(keys).values().into())
    }

    pub fn values(&self) -> Result<JsValue, JsValue> {
        let quads: Vec<JsValue> = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|q| q.map(|quad| JsQuad::from(quad).into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?;
        Ok(Array::from_iter(quads).values().into())
    }

    // Symbol.iterator implementation - must be manually wired up in JavaScript
    // as wasm-bindgen doesn't support computed property names
    #[wasm_bindgen(skip_typescript)]
    pub fn __iterator(&self) -> Result<JsValue, JsValue> {
        let quads = self
            .store
            .quads_for_pattern(None, None, None, None)
            .map(|v| v.map(|v| JsQuad::from(v).into()))
            .collect::<Result<Vec<JsValue>, _>>()
            .map_err(JsError::from)?;
        Ok(Array::from_iter(quads).values().into())
    }

    #[wasm_bindgen(js_name = beginTransaction)]
    pub fn begin_transaction(&self) -> Result<JsTransaction, JsValue> {
        Ok(JsTransaction::new(self.store.clone())?)
    }

    /// Convenience method to load Turtle data
    #[wasm_bindgen(js_name = loadTurtle)]
    pub fn load_turtle(&self, data: &str, options: &JsValue) -> Result<(), JsValue> {
        let opts = js_sys::Object::new();
        Reflect::set(&opts, &JsValue::from_str("format"), &JsValue::from_str("ttl"))?;

        // Copy over user-provided options (baseIri, toGraphName, lenient)
        if !options.is_undefined() && !options.is_null() {
            if let Ok(base_iri) = Reflect::get(options, &JsValue::from_str("baseIri")) {
                if !base_iri.is_undefined() && !base_iri.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("baseIri"), &base_iri)?;
                }
            }
            if let Ok(to_graph) = Reflect::get(options, &JsValue::from_str("toGraphName")) {
                if !to_graph.is_undefined() && !to_graph.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("toGraphName"), &to_graph)?;
                }
            }
            if let Ok(lenient) = Reflect::get(options, &JsValue::from_str("lenient")) {
                if lenient.is_truthy() {
                    Reflect::set(&opts, &JsValue::from_str("lenient"), &JsValue::TRUE)?;
                }
            }
        }

        self.load(data, &opts.into(), &JsValue::UNDEFINED, &JsValue::UNDEFINED)
    }

    /// Convenience method to load N-Triples data
    #[wasm_bindgen(js_name = loadNTriples)]
    pub fn load_ntriples(&self, data: &str, options: &JsValue) -> Result<(), JsValue> {
        let opts = js_sys::Object::new();
        Reflect::set(&opts, &JsValue::from_str("format"), &JsValue::from_str("nt"))?;

        // Copy over user-provided options (toGraphName, lenient)
        if !options.is_undefined() && !options.is_null() {
            if let Ok(to_graph) = Reflect::get(options, &JsValue::from_str("toGraphName")) {
                if !to_graph.is_undefined() && !to_graph.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("toGraphName"), &to_graph)?;
                }
            }
            if let Ok(lenient) = Reflect::get(options, &JsValue::from_str("lenient")) {
                if lenient.is_truthy() {
                    Reflect::set(&opts, &JsValue::from_str("lenient"), &JsValue::TRUE)?;
                }
            }
        }

        self.load(data, &opts.into(), &JsValue::UNDEFINED, &JsValue::UNDEFINED)
    }

    /// Convenience method to serialize all quads to Turtle
    #[wasm_bindgen(js_name = toTurtle)]
    pub fn to_turtle(&self, options: &JsValue) -> Result<String, JsValue> {
        let opts = js_sys::Object::new();
        Reflect::set(&opts, &JsValue::from_str("format"), &JsValue::from_str("ttl"))?;

        // Copy over user-provided options (fromGraphName, prefixes, baseIri)
        if !options.is_undefined() && !options.is_null() {
            if let Ok(from_graph) = Reflect::get(options, &JsValue::from_str("fromGraphName")) {
                if !from_graph.is_undefined() && !from_graph.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("fromGraphName"), &from_graph)?;
                }
            }
            if let Ok(prefixes) = Reflect::get(options, &JsValue::from_str("prefixes")) {
                if !prefixes.is_undefined() && !prefixes.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("prefixes"), &prefixes)?;
                }
            }
            if let Ok(base_iri) = Reflect::get(options, &JsValue::from_str("baseIri")) {
                if !base_iri.is_undefined() && !base_iri.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("baseIri"), &base_iri)?;
                }
            }
        }

        self.dump(&opts.into(), &JsValue::UNDEFINED)
    }

    /// Convenience method to serialize all quads to N-Triples
    #[wasm_bindgen(js_name = toNTriples)]
    pub fn to_ntriples(&self, options: &JsValue) -> Result<String, JsValue> {
        let opts = js_sys::Object::new();
        Reflect::set(&opts, &JsValue::from_str("format"), &JsValue::from_str("nt"))?;

        // Copy over user-provided options (fromGraphName)
        if !options.is_undefined() && !options.is_null() {
            if let Ok(from_graph) = Reflect::get(options, &JsValue::from_str("fromGraphName")) {
                if !from_graph.is_undefined() && !from_graph.is_null() {
                    Reflect::set(&opts, &JsValue::from_str("fromGraphName"), &from_graph)?;
                }
            }
        }

        self.dump(&opts.into(), &JsValue::UNDEFINED)
    }
}

fn rdf_format(format: &str) -> Result<RdfFormat, JsValue> {
    if format.contains('/') {
        RdfFormat::from_media_type(format)
            .ok_or_else(|| format_err!("Not supported RDF format media type: {}", format))
    } else {
        RdfFormat::from_extension(format)
            .ok_or_else(|| format_err!("Not supported RDF format extension: {}", format))
    }
}

fn query_results_format(format: &str) -> Result<QueryResultsFormat, JsValue> {
    if format.contains('/') {
        QueryResultsFormat::from_media_type(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format media type: {}",
                format
            )
        })
    } else {
        QueryResultsFormat::from_extension(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format extension: {}",
                format
            )
        })
    }
}

fn convert_base_iri(value: &JsValue) -> Result<Option<String>, JsValue> {
    if value.is_null() || value.is_undefined() {
        Ok(None)
    } else if let Some(value) = value.as_string() {
        Ok(Some(value))
    } else if let JsTerm::NamedNode(value) = FROM_JS.with(|c| c.to_term(value))? {
        Ok(Some(value.value()))
    } else {
        Err(format_err!(
            "If provided, the base IRI must be a NamedNode or a string"
        ))
    }
}

fn extract_prefixes(prefixes_obj: &JsValue) -> Result<Vec<(String, String)>, JsValue> {
    let mut prefixes = Vec::new();
    let obj = js_sys::Object::try_from(prefixes_obj)
        .ok_or_else(|| format_err!("prefixes option must be an object"))?;
    let entries = js_sys::Object::entries(&obj);
    for i in 0..entries.length() {
        let entry = entries.get(i);
        let pair = Array::from(&entry);
        let prefix_name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| format_err!("prefix name must be a string"))?;
        let prefix_iri = pair
            .get(1)
            .as_string()
            .ok_or_else(|| format_err!("prefix IRI must be a string"))?;
        prefixes.push((prefix_name, prefix_iri));
    }
    Ok(prefixes)
}

fn extract_substitutions(substitutions_obj: &JsValue) -> Result<Vec<(Variable, Term)>, JsValue> {
    let mut substitutions = Vec::new();
    let obj = js_sys::Object::try_from(substitutions_obj)
        .ok_or_else(|| format_err!("substitutions option must be an object"))?;
    let entries = js_sys::Object::entries(&obj);
    for i in 0..entries.length() {
        let entry = entries.get(i);
        let pair = Array::from(&entry);
        let variable_name = pair
            .get(0)
            .as_string()
            .ok_or_else(|| format_err!("variable name must be a string"))?;
        let variable = Variable::new(variable_name).map_err(JsError::from)?;
        let term = Term::try_from(FROM_JS.with(|c| c.to_term(&pair.get(1)))?)?;
        substitutions.push((variable, term));
    }
    Ok(substitutions)
}

#[wasm_bindgen(js_name = StoreTransaction, skip_typescript)]
pub struct JsTransaction {
    // We store the Store to keep it alive for the transaction's lifetime
    // The transaction borrows from this store, but we use RefCell to work around
    // the single ownership requirement of wasm_bindgen
    store: Store,
    inner: RefCell<Option<Transaction<'static>>>,
}

impl JsTransaction {
    fn new(store: Store) -> Result<Self, JsValue> {
        // SAFETY: We transmute the lifetime of the transaction to 'static.
        // This is safe because:
        // 1. The transaction is created from `store` which we own
        // 2. The transaction is stored alongside the store in the same struct
        // 3. The transaction will be dropped before the store (Rust drop order guarantees)
        // 4. We prevent access after commit/drop via the Option wrapper
        let transaction = unsafe {
            let transaction = store.start_transaction().map_err(JsError::from)?;
            std::mem::transmute::<Transaction<'_>, Transaction<'static>>(transaction)
        };
        Ok(Self {
            store,
            inner: RefCell::new(Some(transaction)),
        })
    }
}

#[wasm_bindgen(js_class = StoreTransaction)]
impl JsTransaction {
    pub fn add(&self, quad: &JsValue) -> Result<(), JsValue> {
        let mut inner = self.inner.borrow_mut();
        let transaction = inner
            .as_mut()
            .ok_or_else(|| format_err!("Transaction has already been committed or rolled back"))?;
        transaction.insert(FROM_JS.with(|c| c.to_quad(quad))?.as_ref());
        Ok(())
    }

    pub fn delete(&self, quad: &JsValue) -> Result<(), JsValue> {
        let mut inner = self.inner.borrow_mut();
        let transaction = inner
            .as_mut()
            .ok_or_else(|| format_err!("Transaction has already been committed or rolled back"))?;
        transaction.remove(FROM_JS.with(|c| c.to_quad(quad))?.as_ref());
        Ok(())
    }

    pub fn commit(&self) -> Result<(), JsValue> {
        let mut inner = self.inner.borrow_mut();
        let transaction = inner
            .take()
            .ok_or_else(|| format_err!("Transaction has already been committed or rolled back"))?;
        transaction.commit().map_err(JsError::from)?;
        Ok(())
    }
}
