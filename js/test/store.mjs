/* global describe, it */

import assert from "assert";
import dataModel from "@rdfjs/data-model";
import { Store } from "../pkg/oxigraph.js";

const ex = dataModel.namedNode("http://example.com");
const triple = dataModel.quad(
    dataModel.blankNode("s"),
    dataModel.namedNode("http://example.com/p"),
    dataModel.literal("o"),
);

describe("Store", () => {
    describe("#add()", () => {
        it("an added quad should be in the store", () => {
            const store = new Store();
            store.add(dataModel.quad(ex, ex, triple));
            assert(store.has(dataModel.quad(ex, ex, triple)));
        });
    });

    describe("#delete()", () => {
        it("an removed quad should not be in the store anymore", () => {
            const store = new Store([dataModel.quad(triple, ex, ex)]);
            assert(store.has(dataModel.quad(triple, ex, ex)));
            store.delete(dataModel.quad(triple, ex, ex));
            assert(!store.has(dataModel.quad(triple, ex, ex)));
        });
    });

    describe("#has()", () => {
        it("an added quad should be in the store", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            assert(store.has(dataModel.quad(ex, ex, ex)));
        });
    });

    describe("#size()", () => {
        it("A store with one quad should have 1 for size", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            assert.strictEqual(1, store.size);
        });
    });

    describe("#match_quads()", () => {
        it("blank pattern should return all quads", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.match();
            assert.strictEqual(1, results.length);
            assert(dataModel.quad(ex, ex, ex).equals(results[0]));
        });
    });

    describe("#query()", () => {
        it("ASK true", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            assert.strictEqual(true, store.query("ASK { ?s ?s ?s }"));
        });

        it("ASK false", () => {
            const store = new Store();
            assert.strictEqual(false, store.query("ASK { FILTER(false)}"));
        });

        it("CONSTRUCT", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
            assert.strictEqual(1, results.length);
            assert(dataModel.quad(ex, ex, ex).equals(results[0]));
        });

        it("SELECT", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("SELECT ?s WHERE { ?s ?p ?o }");
            assert.strictEqual(1, results.length);
            assert(ex.equals(results[0].get("s")));
        });

        it("SELECT with NOW()", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query(
                "SELECT * WHERE { FILTER(2022 <= YEAR(NOW()) && YEAR(NOW()) <= 2100) }",
            );
            assert.strictEqual(1, results.length);
        });

        it("SELECT with RAND()", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("SELECT (RAND() AS ?y) WHERE {}");
            assert.strictEqual(1, results.length);
        });

        it("SELECT with base IRI", () => {
            const store = new Store();
            const results = store.query("SELECT * WHERE { BIND(<t> AS ?t) }", {
                base_iri: "http://example.com/",
            });
            assert.strictEqual(1, results.length);
        });

        it("SELECT with union graph", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            const results = store.query("SELECT * WHERE { ?s ?p ?o }", {
                use_default_graph_as_union: true,
            });
            assert.strictEqual(1, results.length);
        });
    });

    describe("#update()", () => {
        it("INSERT DATA", () => {
            const store = new Store();
            store.update(
                "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
            );
            assert.strictEqual(1, store.size);
        });

        it("DELETE DATA", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            store.update(
                "DELETE DATA { <http://example.com> <http://example.com> <http://example.com> }",
            );
            assert.strictEqual(0, store.size);
        });

        it("DELETE WHERE", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            store.update("DELETE WHERE { ?v ?v ?v }");
            assert.strictEqual(0, store.size);
        });
    });

    describe("#load()", () => {
        it("load NTriples in the default graph", () => {
            const store = new Store();
            store.load(
                "<http://example.com> <http://example.com> <http://example.com> .",
                "application/n-triples",
            );
            assert(store.has(dataModel.quad(ex, ex, ex)));
        });

        it("load NTriples in an other graph", () => {
            const store = new Store();
            store.load(
                "<http://example.com> <http://example.com> <http://example.com> .",
                "application/n-triples",
                null,
                ex,
            );
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load Turtle with a base IRI", () => {
            const store = new Store();
            store.load(
                "<http://example.com> <http://example.com> <> .",
                "text/turtle",
                "http://example.com",
            );
            assert(store.has(dataModel.quad(ex, ex, ex)));
        });

        it("load NQuads", () => {
            const store = new Store();
            store.load(
                "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .",
                "application/n-quads",
            );
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load TriG with a base IRI", () => {
            const store = new Store();
            store.load(
                "GRAPH <> { <http://example.com> <http://example.com> <> }",
                "application/trig",
                "http://example.com",
            );
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });
    });

    describe("#dump()", () => {
        it("dump dataset content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n",
                store.dump("application/n-quads"),
            );
        });

        it("dump named graph content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> .\n",
                store.dump("application/n-triples", ex),
            );
        });

        it("dump default graph content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual("", store.dump("application/n-triples", dataModel.defaultGraph()));
        });
    });
});
