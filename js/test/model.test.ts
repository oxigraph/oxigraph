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

        describe("#forEach()", () => {
            it("should iterate over all quads", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                const values: string[] = [];
                dataset.forEach((quad) => {
                    values.push(quad.object.value);
                });
                assert.strictEqual(3, values.length);
                assert(values.includes("1"));
                assert(values.includes("2"));
                assert(values.includes("3"));
            });

            it("should work with empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                let count = 0;
                dataset.forEach(() => {
                    count++;
                });
                assert.strictEqual(0, count);
            });
        });

        describe("#filter()", () => {
            it("should filter quads based on predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                const filtered = dataset.filter((quad) => quad.object.value === "2");
                assert.strictEqual(1, filtered.length);
                assert.strictEqual("2", filtered[0].object.value);
            });

            it("should return empty array when no quads match", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                ]);
                const filtered = dataset.filter((quad) => quad.object.value === "99");
                assert.strictEqual(0, filtered.length);
            });

            it("should filter based on quad properties", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("hello")),
                    dataModel.quad(ex2, ex, dataModel.literal("world")),
                ]);
                const filtered = dataset.filter((quad) => quad.subject.value === ex.value);
                assert.strictEqual(1, filtered.length);
                assert.strictEqual(ex.value, filtered[0].subject.value);
            });
        });

        describe("#some()", () => {
            it("should return true if any quad matches predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                assert.strictEqual(true, dataset.some((quad) => quad.object.value === "2"));
            });

            it("should return false if no quads match", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                ]);
                assert.strictEqual(false, dataset.some((quad) => quad.object.value === "99"));
            });

            it("should return false for empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                assert.strictEqual(false, dataset.some(() => true));
            });
        });

        describe("#every()", () => {
            it("should return true if all quads match predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                ]);
                assert.strictEqual(true, dataset.every((quad) => quad.subject.equals(ex)));
            });

            it("should return false if any quad doesn't match", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex2, ex, dataModel.literal("2")),
                ]);
                assert.strictEqual(false, dataset.every((quad) => quad.subject.equals(ex)));
            });

            it("should return true for empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                assert.strictEqual(true, dataset.every(() => false));
            });
        });

        describe("#find()", () => {
            it("should return first quad matching predicate", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                const found = dataset.find((quad) => quad.object.value === "2");
                assert(found !== undefined);
                assert.strictEqual("2", found.object.value);
            });

            it("should return undefined if no quad matches", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                ]);
                const found = dataset.find((quad) => quad.object.value === "99");
                assert.strictEqual(undefined, found);
            });

            it("should return undefined for empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                const found = dataset.find(() => true);
                assert.strictEqual(undefined, found);
            });
        });

        describe("#toArray()", () => {
            it("should return array of all quads", () => {
                const dataset = new oxigraph.Dataset([
                    dataModel.quad(ex, ex, dataModel.literal("1")),
                    dataModel.quad(ex, ex, dataModel.literal("2")),
                    dataModel.quad(ex, ex, dataModel.literal("3")),
                ]);
                const array = dataset.toArray();
                assert.strictEqual(3, array.length);
                assert(Array.isArray(array));
            });

            it("should return empty array for empty dataset", () => {
                const dataset = new oxigraph.Dataset();
                const array = dataset.toArray();
                assert.strictEqual(0, array.length);
                assert(Array.isArray(array));
            });

            it("should work with large datasets", () => {
                const quads = [];
                for (let i = 0; i < 100; i++) {
                    quads.push(dataModel.quad(ex, ex, dataModel.literal(String(i))));
                }
                const dataset = new oxigraph.Dataset(quads);
                const array = dataset.toArray();
                assert.strictEqual(100, array.length);
            });
        });
    });

    describe("RDF Terms - valueOf(), toJSON(), and static from()", () => {
        describe("NamedNode", () => {
            it("valueOf() should return string value", () => {
                const node = oxigraph.namedNode("http://example.com");
                assert.strictEqual("http://example.com", node.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const node = oxigraph.namedNode("http://example.com");
                const json = node.toJSON();
                assert.strictEqual("NamedNode", json.termType);
                assert.strictEqual("http://example.com", json.value);
            });

            it("static from() should create NamedNode from string", () => {
                const node = oxigraph.NamedNode.from("http://example.com");
                assert.strictEqual("NamedNode", node.termType);
                assert.strictEqual("http://example.com", node.value);
            });
        });

        describe("BlankNode", () => {
            it("valueOf() should return string value", () => {
                const node = oxigraph.blankNode("b1");
                assert.strictEqual("b1", node.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const node = oxigraph.blankNode("b1");
                const json = node.toJSON();
                assert.strictEqual("BlankNode", json.termType);
                assert.strictEqual("b1", json.value);
            });

            it("static from() should create BlankNode with value", () => {
                const node = oxigraph.BlankNode.from("b1");
                assert.strictEqual("BlankNode", node.termType);
                assert.strictEqual("b1", node.value);
            });

            it("static from() should create BlankNode with auto-generated ID", () => {
                const node = oxigraph.BlankNode.from();
                assert.strictEqual("BlankNode", node.termType);
                assert(node.value.length > 0);
            });
        });

        describe("Literal", () => {
            it("valueOf() should return string value", () => {
                const lit = oxigraph.literal("hello");
                assert.strictEqual("hello", lit.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const lit = oxigraph.literal("hello", "en");
                const json = lit.toJSON();
                assert.strictEqual("Literal", json.termType);
                assert.strictEqual("hello", json.value);
                assert.strictEqual("en", json.language);
                assert.strictEqual("NamedNode", json.datatype.termType);
            });

            it("static from() should create simple literal", () => {
                const lit = oxigraph.Literal.from("hello");
                assert.strictEqual("Literal", lit.termType);
                assert.strictEqual("hello", lit.value);
            });

            it("static from() should create language-tagged literal", () => {
                const lit = oxigraph.Literal.from("hello", "en");
                assert.strictEqual("Literal", lit.termType);
                assert.strictEqual("hello", lit.value);
                assert.strictEqual("en", lit.language);
            });

            it("static from() should create typed literal", () => {
                const xsdString = oxigraph.namedNode("http://www.w3.org/2001/XMLSchema#string");
                const lit = oxigraph.Literal.from("hello", xsdString);
                assert.strictEqual("Literal", lit.termType);
                assert.strictEqual("hello", lit.value);
                assert.strictEqual(xsdString.value, lit.datatype.value);
            });
        });

        describe("DefaultGraph", () => {
            it("valueOf() should return empty string", () => {
                const dg = oxigraph.defaultGraph();
                assert.strictEqual("", dg.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const dg = oxigraph.defaultGraph();
                const json = dg.toJSON();
                assert.strictEqual("DefaultGraph", json.termType);
                assert.strictEqual("", json.value);
            });
        });

        describe("Variable", () => {
            it("valueOf() should return variable name", () => {
                const v = oxigraph.variable("x");
                assert.strictEqual("x", v.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const v = oxigraph.variable("x");
                const json = v.toJSON();
                assert.strictEqual("Variable", json.termType);
                assert.strictEqual("x", json.value);
            });
        });

        describe("Triple", () => {
            it("valueOf() should return empty string", () => {
                const triple = oxigraph.triple(
                    oxigraph.namedNode("http://example.com/s"),
                    oxigraph.namedNode("http://example.com/p"),
                    oxigraph.namedNode("http://example.com/o")
                );
                assert.strictEqual("", triple.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const triple = oxigraph.triple(
                    oxigraph.namedNode("http://example.com/s"),
                    oxigraph.namedNode("http://example.com/p"),
                    oxigraph.namedNode("http://example.com/o")
                );
                const json = triple.toJSON();
                assert.strictEqual("Triple", json.termType);
                assert.strictEqual("", json.value);
                assert(json.subject !== undefined);
                assert(json.predicate !== undefined);
                assert(json.object !== undefined);
            });

            it("static from() should create Triple", () => {
                const s = oxigraph.namedNode("http://example.com/s");
                const p = oxigraph.namedNode("http://example.com/p");
                const o = oxigraph.namedNode("http://example.com/o");
                const triple = oxigraph.Triple.from(s, p, o);
                assert.strictEqual("Triple", triple.termType);
                assert(triple.subject.equals(s));
                assert(triple.predicate.equals(p));
                assert(triple.object.equals(o));
            });
        });

        describe("Quad", () => {
            it("valueOf() should return empty string", () => {
                const quad = oxigraph.quad(
                    oxigraph.namedNode("http://example.com/s"),
                    oxigraph.namedNode("http://example.com/p"),
                    oxigraph.namedNode("http://example.com/o")
                );
                assert.strictEqual("", quad.valueOf());
            });

            it("toJSON() should return RDF/JS compatible object", () => {
                const quad = oxigraph.quad(
                    oxigraph.namedNode("http://example.com/s"),
                    oxigraph.namedNode("http://example.com/p"),
                    oxigraph.namedNode("http://example.com/o"),
                    oxigraph.namedNode("http://example.com/g")
                );
                const json = quad.toJSON();
                assert.strictEqual("Quad", json.termType);
                assert.strictEqual("", json.value);
                assert(json.subject !== undefined);
                assert(json.predicate !== undefined);
                assert(json.object !== undefined);
                assert(json.graph !== undefined);
            });

            it("static from() should create Quad with default graph", () => {
                const s = oxigraph.namedNode("http://example.com/s");
                const p = oxigraph.namedNode("http://example.com/p");
                const o = oxigraph.namedNode("http://example.com/o");
                const quad = oxigraph.Quad.from(s, p, o);
                assert.strictEqual("Quad", quad.termType);
                assert(quad.subject.equals(s));
                assert(quad.predicate.equals(p));
                assert(quad.object.equals(o));
                assert.strictEqual("DefaultGraph", quad.graph.termType);
            });

            it("static from() should create Quad with named graph", () => {
                const s = oxigraph.namedNode("http://example.com/s");
                const p = oxigraph.namedNode("http://example.com/p");
                const o = oxigraph.namedNode("http://example.com/o");
                const g = oxigraph.namedNode("http://example.com/g");
                const quad = oxigraph.Quad.from(s, p, o, g);
                assert.strictEqual("Quad", quad.termType);
                assert(quad.subject.equals(s));
                assert(quad.predicate.equals(p));
                assert(quad.object.equals(o));
                assert(quad.graph.equals(g));
            });
        });
    });

    describe("DataFactory", () => {
        it("should expose all factory methods", () => {
            assert(typeof oxigraph.DataFactory.namedNode === "function");
            assert(typeof oxigraph.DataFactory.blankNode === "function");
            assert(typeof oxigraph.DataFactory.literal === "function");
            assert(typeof oxigraph.DataFactory.variable === "function");
            assert(typeof oxigraph.DataFactory.defaultGraph === "function");
            assert(typeof oxigraph.DataFactory.triple === "function");
            assert(typeof oxigraph.DataFactory.quad === "function");
            assert(typeof oxigraph.DataFactory.fromTerm === "function");
            assert(typeof oxigraph.DataFactory.fromQuad === "function");
        });

        it("should create NamedNode via DataFactory", () => {
            const node = oxigraph.DataFactory.namedNode("http://example.com");
            assert.strictEqual(node.termType, "NamedNode");
            assert.strictEqual(node.value, "http://example.com");
        });

        it("should create BlankNode via DataFactory", () => {
            const node = oxigraph.DataFactory.blankNode("b1");
            assert.strictEqual(node.termType, "BlankNode");
            assert.strictEqual(node.value, "b1");
        });

        it("should create Literal via DataFactory", () => {
            const lit = oxigraph.DataFactory.literal("hello", "en");
            assert.strictEqual(lit.termType, "Literal");
            assert.strictEqual(lit.value, "hello");
            assert.strictEqual(lit.language, "en");
        });

        it("should create Variable via DataFactory", () => {
            const variable = oxigraph.DataFactory.variable("x");
            assert.strictEqual(variable.termType, "Variable");
            assert.strictEqual(variable.value, "x");
        });

        it("should create DefaultGraph via DataFactory", () => {
            const graph = oxigraph.DataFactory.defaultGraph();
            assert.strictEqual(graph.termType, "DefaultGraph");
            assert.strictEqual(graph.value, "");
        });

        it("should create Triple via DataFactory", () => {
            const s = oxigraph.DataFactory.namedNode("http://example.com/s");
            const p = oxigraph.DataFactory.namedNode("http://example.com/p");
            const o = oxigraph.DataFactory.namedNode("http://example.com/o");
            const triple = oxigraph.DataFactory.triple(s, p, o);
            assert.strictEqual(triple.termType, "Triple");
        });

        it("should create Quad via DataFactory", () => {
            const s = oxigraph.DataFactory.namedNode("http://example.com/s");
            const p = oxigraph.DataFactory.namedNode("http://example.com/p");
            const o = oxigraph.DataFactory.literal("hello");
            const quad = oxigraph.DataFactory.quad(s, p, o);
            assert.strictEqual(quad.termType, "Quad");
            assert.strictEqual(quad.subject, s);
            assert.strictEqual(quad.predicate, p);
            assert.strictEqual(quad.object, o);
        });

        it("should convert terms via DataFactory.fromTerm", () => {
            const original = dataModel.namedNode("http://example.com");
            const converted = oxigraph.DataFactory.fromTerm(original);
            assert(converted);
            assert.strictEqual(converted.termType, "NamedNode");
            assert.strictEqual(converted.value, "http://example.com");
        });

        it("should convert quads via DataFactory.fromQuad", () => {
            const original = dataModel.quad(ex, ex, ex);
            const converted = oxigraph.DataFactory.fromQuad(original);
            assert(converted);
            assert.strictEqual(converted.termType, "Quad");
        });
    });
});
