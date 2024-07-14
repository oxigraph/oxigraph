import assert from "node:assert";
// @ts-expect-error
import runTests from "@rdfjs/data-model/test/index.js";
import { describe, it } from "vitest";
import oxigraph from "../pkg/oxigraph.js";

runTests({ factory: oxigraph });

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
                "<http://example.com/s> <http://example.com/p> <<<http://example.com/s1> <http://example.com/p1> <http://example.com/o1>>> <http://example.com/g>",
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
    });
});
