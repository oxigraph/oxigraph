import assert from "node:assert";
import { webcrypto } from "node:crypto";
import { describe, it, vi } from "vitest";
import oxigraph from "../pkg/oxigraph.js";
import { DataFactory } from "@rdfjs/data-model";

// thread_rng: Node.js ES modules are not directly supported, see https://docs.rs/getrandom#nodejs-es-module-support
vi.stubGlobal("crypto", webcrypto);

describe("RDF/JS Interoperability", () => {
    describe("fromTerm()", () => {
        it("should convert plain NamedNode object to Oxigraph NamedNode", () => {
            const plainTerm = {
                termType: "NamedNode",
                value: "http://example.com/subject",
            };
            const term = oxigraph.fromTerm(plainTerm);
            assert.strictEqual(term.termType, "NamedNode");
            assert.strictEqual(term.value, "http://example.com/subject");
        });

        it("should convert plain BlankNode object to Oxigraph BlankNode", () => {
            const plainTerm = {
                termType: "BlankNode",
                value: "b1",
            };
            const term = oxigraph.fromTerm(plainTerm);
            assert.strictEqual(term.termType, "BlankNode");
            assert.strictEqual(term.value, "b1");
        });

        it("should convert plain Literal object to Oxigraph Literal", () => {
            const plainTerm = {
                termType: "Literal",
                value: "hello",
                language: "en",
                datatype: {
                    termType: "NamedNode",
                    value: "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString",
                },
            };
            const term = oxigraph.fromTerm(plainTerm);
            assert.strictEqual(term.termType, "Literal");
            assert.strictEqual(term.value, "hello");
            assert.strictEqual(term.language, "en");
        });

        it("should convert @rdfjs/data-model terms to Oxigraph terms", () => {
            const rdfJsTerm = DataFactory.namedNode("http://example.com/node");
            const oxigraphTerm = oxigraph.fromTerm(rdfJsTerm);
            assert.strictEqual(oxigraphTerm.termType, "NamedNode");
            assert.strictEqual(oxigraphTerm.value, "http://example.com/node");
        });

        it("should handle null input", () => {
            const result = oxigraph.fromTerm(null);
            assert.strictEqual(result, null);
        });
    });

    describe("fromQuad()", () => {
        it("should convert plain Quad object to Oxigraph Quad", () => {
            const plainQuad = {
                subject: { termType: "NamedNode", value: "http://example.com/s" },
                predicate: { termType: "NamedNode", value: "http://example.com/p" },
                object: {
                    termType: "Literal",
                    value: "hello",
                    datatype: {
                        termType: "NamedNode",
                        value: "http://www.w3.org/2001/XMLSchema#string",
                    },
                },
                graph: { termType: "DefaultGraph", value: "" },
            };

            const quad = oxigraph.fromQuad(plainQuad);
            assert.strictEqual(quad.subject.termType, "NamedNode");
            assert.strictEqual(quad.subject.value, "http://example.com/s");
            assert.strictEqual(quad.predicate.value, "http://example.com/p");
            assert.strictEqual(quad.object.value, "hello");
            assert.strictEqual(quad.graph.termType, "DefaultGraph");
        });

        it("should convert @rdfjs/data-model quads to Oxigraph quads", () => {
            const rdfJsQuad = DataFactory.quad(
                DataFactory.namedNode("http://example.com/s"),
                DataFactory.namedNode("http://example.com/p"),
                DataFactory.literal("object value"),
                DataFactory.namedNode("http://example.com/g"),
            );

            const oxigraphQuad = oxigraph.fromQuad(rdfJsQuad);
            assert.strictEqual(oxigraphQuad.subject.value, "http://example.com/s");
            assert.strictEqual(oxigraphQuad.predicate.value, "http://example.com/p");
            assert.strictEqual(oxigraphQuad.object.value, "object value");
            assert.strictEqual(oxigraphQuad.graph.value, "http://example.com/g");
        });

        it("should handle null input", () => {
            const result = oxigraph.fromQuad(null);
            assert.strictEqual(result, null);
        });
    });

    describe("Bidirectional conversion", () => {
        it("should allow round-trip conversion between RDF/JS and Oxigraph", () => {
            // Create a term using Oxigraph
            const oxigraphOriginal = oxigraph.namedNode("http://example.com/test");

            // Convert to plain object (this happens automatically with RDF/JS libraries)
            const plainObject = {
                termType: oxigraphOriginal.termType,
                value: oxigraphOriginal.value,
            };

            // Convert back to Oxigraph
            const oxigraphConverted = oxigraph.fromTerm(plainObject);

            assert.strictEqual(
                oxigraphOriginal.value,
                oxigraphConverted.value,
            );
            assert.strictEqual(
                oxigraphOriginal.termType,
                oxigraphConverted.termType,
            );
        });
    });
});
