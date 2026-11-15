use wasm_bindgen::prelude::*;

mod incremental;
mod io;
mod model;
mod reflect;
mod store;
mod transaction;
mod utils;

// We skip_typescript on specific wasm_bindgen macros and provide custom TypeScript types for parts of this module to have narrower types
// instead of any and improve compatibility with RDF/JS Dataset interfaces (https://rdf.js.org/dataset-spec/).
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
import type { BaseQuad, BlankNode, DataFactory, Literal, NamedNode, DefaultGraph, Term } from "@rdfjs/types";

export interface Quad extends BaseQuad {
    subject: NamedNode | BlankNode;
    predicate: NamedNode;
    object: NamedNode | BlankNode | Literal | Quad;
    graph: NamedNode | BlankNode | DefaultGraph;
}
export type Bindings = Map<string, Term>;
export type QueryResult = boolean | Bindings[] | Quad[];
export interface QueryOptions {
    base_iri?: NamedNode | string;
    default_graph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
    named_graphs?: Iterable<BlankNode | NamedNode>;
    use_default_graph_as_union?: boolean;
}
export interface ResultsOptions extends QueryOptions { results_format?: string; }
export interface ParseOptions {
    base_iri?: NamedNode | string;
    format: string;
    to_graph_name?: BlankNode | DefaultGraph | NamedNode;
    lenient?: boolean;
    /** @deprecated Use lenient. */
    unchecked?: boolean;
    data_factory?: DataFactory;
}
export interface Delta<T> { readonly kind: DeltaKind; readonly value: T; }
export type QueryDeltas = Delta<Bindings>[] | boolean[] | Delta<Quad>[];

export class Store {
    readonly size: number;
    constructor(quads?: Iterable<BaseQuad>);
    add(quad: BaseQuad): void;
    delete(quad: BaseQuad): void;
    extend(quads: Iterable<BaseQuad>): void;
    has(quad: BaseQuad): boolean;
    match(subject?: Term | null, predicate?: Term | null, object?: Term | null, graph?: Term | null): Quad[];
    query(query: string, options?: ResultsOptions): QueryResult | string;
    queryIncrementalResults(query: string, options?: ResultsOptions): IncrementalQueryResults;
    queryIncrementalDeltas(query: string, options?: QueryOptions): IncrementalQueryDeltas;
    transaction(): Transaction;
    update(update: string, options?: { base_iri?: NamedNode | string }): void;
    load(input: string | Uint8Array | Iterable<string | Uint8Array>, options: Omit<ParseOptions, "data_factory"> & { no_transaction?: boolean }): void;
    dump(options: { format: string; from_graph_name?: BlankNode | DefaultGraph | NamedNode }): string;
    free(): void;
}
export class IncrementalQueryResults {
    private constructor();
    readonly variables: string[] | null;
    readonly closed: boolean;
    results(): QueryResult | string;
    close(): void;
    free(): void;
}
export class IncrementalQueryDeltas {
    private constructor();
    readonly variables: string[] | null;
    readonly closed: boolean;
    deltas(): QueryDeltas;
    close(): void;
    free(): void;
}
export class Transaction {
    private constructor();
    readonly closed: boolean;
    add(quad: BaseQuad): void;
    delete(quad: BaseQuad): void;
    extend(quads: Iterable<BaseQuad>): void;
    commit(): void;
    rollback(): void;
    free(): void;
}
export function parse(input: string | Uint8Array, options: ParseOptions): Quad[];
export function parse(input: Iterable<string | Uint8Array>, options: ParseOptions): IterableIterator<Quad>;
export function parse(input: AsyncIterable<string | Uint8Array>, options: ParseOptions): AsyncIterableIterator<Quad>;

"###;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
