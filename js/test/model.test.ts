import assert from "node:assert";
import { webcrypto } from "node:crypto";
import { describe, it, vi } from "vitest";
import oxigraph from "../pkg/oxigraph.js";

// thread_rng: Node.js ES modules are not directly supported, see https://docs.rs/getrandom#nodejs-es-module-support
vi.stubGlobal("crypto", webcrypto);

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
});
