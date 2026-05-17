use wasm_bindgen::prelude::*;

mod io;
mod model;
mod reflect;
mod store;
mod utils;

// We skip_typescript on specific wasm_bindgen macros and provide custom TypeScript types for parts of this module to have narrower types
// instead of any and improve compatibility with RDF/JS Dataset interfaces (https://rdf.js.org/dataset-spec/).
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
import { BaseQuad, BlankNode, DataFactory, Literal, NamedNode, DefaultGraph, Term } from "@rdfjs/types";

interface Quad extends BaseQuad {
    subject: NamedNode | BlankNode;
    predicate: NamedNode;
    object: NamedNode | BlankNode | Literal | Quad;
    graph: NamedNode | BlankNode | DefaultGraph;
}

export class Store {
    readonly size: number;

    constructor(quads?: Iterable<Quad>);

    add(quad: Quad): void;

    delete(quad: Quad): void;

    dump(
        options: {
            format: string;
            from_graph_name?: BlankNode | DefaultGraph | NamedNode;
        }
    ): string;

    has(quad: Quad): boolean;

    load(
        input: string | UInt8Array | Iterable<string | UInt8Array>,
        options: {
            base_iri?: NamedNode | string;
            format: string;
            no_transaction?: boolean;
            to_graph_name?: BlankNode | DefaultGraph | NamedNode;
            unchecked?: boolean;
            lenient?: boolean;
        }
    ): void;

    match(subject?: Term | null, predicate?: Term | null, object?: Term | null, graph?: Term | null): Quad[];

    query(
        query: string,
        options?: {
            base_iri?: NamedNode | string;
            results_format?: string;
            default_graph?: BlankNode | DefaultGraph | NamedNode | Iterable<BlankNode | DefaultGraph | NamedNode>;
            named_graphs?: Iterable<BlankNode | NamedNode>;
            use_default_graph_as_union?: boolean;
        }
    ): boolean | Map<string, Term>[] | Quad[] | string;

    update(
        update: string,
        options?: {
            base_iri?: NamedNode | string;
        }
    ): void;
}

function parse(
    input: string | UInt8Array,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
        data_factory?: DataFactory;
    }
): Quad[];

function parse(
    input: Iterable<string | UInt8Array>,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
        data_factory?: DataFactory;
    }
): IterableIterator<Quad>;

function parse(
    input: AsyncIterable<string | UInt8Array>,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
        data_factory?: DataFactory;
    }
): AsyncIterableIterator<Quad>;
"###;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
