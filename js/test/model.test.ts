import assert from "node:assert";
import { webcrypto } from "node:crypto";
// @ts-expect-error
import dataModel from "@rdfjs/data-model";
import { describe, it, vi } from "vitest";
import oxigraph from "../pkg/oxigraph.js";

// thread_rng: Node.js ES modules are not directly supported, see https://docs.rs/getrandom#nodejs-es-module-support
vi.stubGlobal("crypto", webcrypto);

const ex = dataModel.namedNode("http://example.com");
const ex2 = dataModel.namedNode("http://example.com/2");

// TODO: add back when https://github.com/rdfjs-base/data-model/pull/52 is released
// runTests({ factory: oxigraph });

describe("DataModel", () => {
    describe("#toString()", () => {
        it("namedNode().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual(
                "<http://example.com>",
                oxigraph.namedNode("http://example.com").toString(),
            );
        });

        it("blankNode().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual("_:a", oxigraph.blankNode("a").toString());
        });

        it("literal().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual('"a\\"b"@en', oxigraph.literal('a"b', "en").toString());
        });

        it("defaultGraph().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual("DEFAULT", oxigraph.defaultGraph().toString());
        });

        it("variable().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual("?a", oxigraph.variable("a").toString());
        });

        it("quad().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual(
                "<http://example.com/s> <http://example.com/p> <<( <http://example.com/s1> <http://example.com/p1> <http://example.com/o1> )>> <http://example.com/g>",
                oxigraph
                    .quad(
                        oxigraph.namedNode("http://example.com/s"),
                        oxigraph.namedNode("http://example.com/p"),
                        oxigraph.quad(
                            oxigraph.namedNode("http://example.com/s1"),
                            oxigraph.namedNode("http://example.com/p1"),
                            oxigraph.namedNode("http://example.com/o1"),
                        ),
                        oxigraph.namedNode("http://example.com/g"),
                    )
                    .toString(),
            );
        });

        it("triple().toString() should return SPARQL compatible syntax", () => {
            assert.strictEqual(
                "<<( <http://example.com/s> <http://example.com/p> <http://example.com/o> )>>",
                oxigraph
                    .triple(
                        oxigraph.namedNode("http://example.com/s"),
                        oxigraph.namedNode("http://example.com/p"),
                        oxigraph.namedNode("http://example.com/o"),
                    )
                    .toString(),
            );
        });
    });

    describe("Triple", () => {
        it("should have correct termType", () => {
            const triple = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s"),
                oxigraph.namedNode("http://example.com/p"),
                oxigraph.namedNode("http://example.com/o"),
            );
            assert.strictEqual(triple.termType, "Triple");
        });

        it("should have empty value", () => {
            const triple = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s"),
                oxigraph.namedNode("http://example.com/p"),
                oxigraph.namedNode("http://example.com/o"),
            );
            assert.strictEqual(triple.value, "");
        });

        it("should have subject, predicate, and object properties", () => {
            const s = oxigraph.namedNode("http://example.com/s");
            const p = oxigraph.namedNode("http://example.com/p");
            const o = oxigraph.namedNode("http://example.com/o");
            const triple = oxigraph.triple(s, p, o);

            assert.ok(triple.subject.equals(s));
            assert.ok(triple.predicate.equals(p));
            assert.ok(triple.object.equals(o));
        });

        it("should support equals()", () => {
            const triple1 = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s"),
                oxigraph.namedNode("http://example.com/p"),
                oxigraph.namedNode("http://example.com/o"),
            );
            const triple2 = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s"),
                oxigraph.namedNode("http://example.com/p"),
                oxigraph.namedNode("http://example.com/o"),
            );
            const triple3 = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s2"),
                oxigraph.namedNode("http://example.com/p"),
                oxigraph.namedNode("http://example.com/o"),
            );

            assert.ok(triple1.equals(triple2));
            assert.ok(!triple1.equals(triple3));
        });

        it("should support nested triples (RDF-star)", () => {
            const innerTriple = oxigraph.triple(
                oxigraph.namedNode("http://example.com/s1"),
                oxigraph.namedNode("http://example.com/p1"),
                oxigraph.namedNode("http://example.com/o1"),
            );
            const outerTriple = oxigraph.triple(
                innerTriple,
                oxigraph.namedNode("http://example.com/p2"),
                oxigraph.namedNode("http://example.com/o2"),
            );

            assert.strictEqual(outerTriple.termType, "Triple");
            assert.ok(outerTriple.subject.equals(innerTriple));
        });
    });

    describe("Dataset", () => {
        describe("#constructor()", () => {
            it("should create empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                assert.strictEqual(0, dataset.size);
            });

            it("should create dataset with initial quads", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                ]);
                assert.strictEqual(2, dataset.size);
            });
        });

        describe("#add()", () => {
            it("should add quad to dataset", () => {
                const dataset = new oxigraph.Dataset();
                dataset.add(dataModel.quad(ex, ex, ex));
                assert.strictEqual(1, dataset.size);
                assert(dataset.has(dataModel.quad(ex, ex, ex)));
            });

            it("should not add duplicate quad", () => {
                const dataset = new oxigraph.Dataset();
                dataset.add(dataModel.quad(ex, ex, ex));
                dataset.add(dataModel.quad(ex, ex, ex));
                assert.strictEqual(1, dataset.size);
            });
        });

        describe("#delete()", () => {
            it("should delete existing quad and return true", () => {
                const dataset = new oxigraph.Dataset([dataModel.quad(ex, ex, ex)]);
                const result = dataset.delete(dataModel.quad(ex, ex, ex));
                assert.strictEqual(true, result);
                assert.strictEqual(0, dataset.size);
            });

            it("should return false when deleting non-existing quad", () => {
                const dataset = new oxigraph.Dataset();
                const result = dataset.delete(dataModel.quad(ex, ex, ex));
                assert.strictEqual(false, result);
            });
        });

        describe("#discard()", () => {
            it("should silently discard non-existing quad", () => {
                const dataset = new oxigraph.Dataset();
                dataset.discard(dataModel.quad(ex, ex, ex)); // Should not throw
                assert.strictEqual(0, dataset.size);
            });

            it("should discard existing quad", () => {
                const dataset = new oxigraph.Dataset([dataModel.quad(ex, ex, ex)]);
                dataset.discard(dataModel.quad(ex, ex, ex));
                assert.strictEqual(0, dataset.size);
                assert(!dataset.has(dataModel.quad(ex, ex, ex)));
            });

            it("should not throw on empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                dataset.discard(dataModel.quad(ex, ex, ex));
                dataset.discard(dataModel.quad(ex2, ex2, ex2));
                assert.strictEqual(0, dataset.size);
            });
        });

        describe("#has()", () => {
            it("should return true for existing quad", () => {
                const dataset = new oxigraph.Dataset([dataModel.quad(ex, ex, ex)]);
                assert(dataset.has(dataModel.quad(ex, ex, ex)));
            });

            it("should return false for non-existing quad", () => {
                const dataset = new oxigraph.Dataset();
                assert(!dataset.has(dataModel.quad(ex, ex, ex)));
            });
        });

        describe("#match()", () => {
            it("should return all quads with no pattern", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                ]);
                const results = dataset.match();
                assert.strictEqual(2, results.length);
            });

            it("should filter by subject", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex2, ex, ex),
                ]);
                const results = dataset.match(ex);
                assert.strictEqual(1, results.length);
            });

            it("should filter by predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex2, ex),
                ]);
                const results = dataset.match(null, ex2);
                assert.strictEqual(1, results.length);
            });

            it("should filter by object", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                ]);
                const results = dataset.match(null, null, ex2);
                assert.strictEqual(1, results.length);
            });

            it("should filter by graph", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex, ex),
                    dataModel.quad(ex, ex, ex, ex2),
                ]);
                const results = dataset.match(null, null, null, ex2);
                assert.strictEqual(1, results.length);
            });
        });

        describe("#clear()", () => {
            it("should clear all quads", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                ]);
                dataset.clear();
                assert.strictEqual(0, dataset.size);
            });
        });

        describe("#quadsForSubject()", () => {
            it("should return quads matching subject", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                    dataModel.quad(ex2, ex, ex),
                ]);
                const results = dataset.quadsForSubject(ex);
                assert.strictEqual(2, results.length);
            });
        });

        describe("#quadsForPredicate()", () => {
            it("should return quads matching predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex2, ex),
                ]);
                const results = dataset.quadsForPredicate(ex2);
                assert.strictEqual(1, results.length);
            });
        });

        describe("#quadsForObject()", () => {
            it("should return quads matching object", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2),
                ]);
                const results = dataset.quadsForObject(ex2);
                assert.strictEqual(1, results.length);
            });
        });

        describe("#quadsForGraphName()", () => {
            it("should return quads matching graph", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex, ex),
                    dataModel.quad(ex, ex, ex, ex2),
                ]);
                const results = dataset.quadsForGraphName(ex2);
                assert.strictEqual(1, results.length);
            });

            it("should return quads in default graph", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, ex),
                    dataModel.quad(ex, ex, ex2, ex),
                ]);
                const results = dataset.quadsForGraphName(dataModel.defaultGraph());
                assert.strictEqual(1, results.length);
            });
        });

        describe("#[Symbol.iterator]()", () => {
            it("should make Dataset iterable with for...of", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                const quads: oxigraph.Quad[] = [];
                for (const quad of dataset) {
                    quads.push(quad);
                }
                assert.strictEqual(3, quads.length);
            });

            it("should work with spread operator", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                ]);
                const quads = [...dataset];
                assert.strictEqual(2, quads.length);
            });

            it("should work with Array.from()", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                ]);
                const quads = Array.from(dataset);
                assert.strictEqual(1, quads.length);
            });
        });
    });
});
