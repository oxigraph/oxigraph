import assert from "node:assert";
import { webcrypto } from "node:crypto";
// @ts-expect-error
import dataModel from "@rdfjs/data-model";
import { describe, it, vi } from "vitest";
import {
    type Quad,
    Store,
    type Term,
    parse,
    serialize,
    RdfFormat,
    QueryResultsFormat,
    parseQueryResults,
    serializeQuerySolutions,
    serializeQueryBoolean,
} from "../pkg/oxigraph.js";

// thread_rng: Node.js ES modules are not directly supported, see https://docs.rs/getrandom#nodejs-es-module-support
vi.stubGlobal("crypto", webcrypto);

const ex = dataModel.namedNode("http://example.com");
const ex2 = dataModel.namedNode("http://example.com/2");
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
        it("a removed quad should not be in the store anymore", () => {
            const store = new Store([dataModel.quad(ex, ex, triple, ex)]);
            assert(store.has(dataModel.quad(ex, ex, triple, ex)));
            store.delete(dataModel.quad(ex, ex, triple, ex));
            assert(!store.has(dataModel.quad(ex, ex, triple, ex)));
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
            const results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }") as Quad[];
            assert.strictEqual(1, results.length);
            assert(dataModel.quad(ex, ex, ex).equals(results[0]));
        });

        it("SELECT", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("SELECT ?s WHERE { ?s ?p ?o }") as Map<string, Term>[];
            assert.strictEqual(1, results.length);
            assert(ex.equals(results[0]?.get("s")));
        });

        it("SELECT with NOW()", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query(
                "SELECT * WHERE { FILTER(2022 <= YEAR(NOW()) && YEAR(NOW()) <= 2100) }",
            ) as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with RAND()", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("SELECT (RAND() AS ?y) WHERE {}") as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with base IRI", () => {
            const store = new Store();
            const results = store.query("SELECT * WHERE { BIND(<t> AS ?t) }", {
                base_iri: "http://example.com/",
            }) as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with union graph", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            const results = store.query("SELECT * WHERE { ?s ?p ?o }", {
                use_default_graph_as_union: true,
            }) as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with explicit default graph", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            const results = store.query("SELECT * WHERE { ?s ?p ?o }", {
                default_graph: ex,
            }) as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with explicit default graph list", () => {
            const store = new Store([dataModel.quad(ex, ex, ex), dataModel.quad(ex, ex, ex, ex)]);
            const results = store.query("SELECT * WHERE { ?s ?p ?o }", {
                default_graph: [dataModel.defaultGraph(), ex],
            }) as Map<string, Term>[];
            assert.strictEqual(2, results.length);
        });

        it("SELECT with explicit named graphs list", () => {
            const store = new Store([
                dataModel.quad(ex, ex, ex, ex),
                dataModel.quad(ex, ex, ex, ex2),
            ]);
            const results = store.query("SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }", {
                named_graphs: [ex],
            }) as Map<string, Term>[];
            assert.strictEqual(1, results.length);
        });

        it("SELECT with results format", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }", {
                results_format: "json",
            });
            assert.strictEqual(
                '{"head":{"vars":["s","p","o"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.com"},"p":{"type":"uri","value":"http://example.com"},"o":{"type":"uri","value":"http://example.com"}}]}}',
                results,
            );
        });

        it("CONSTRUCT with results format", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("CONSTRUCT WHERE { ?s ?p ?o }", {
                results_format: "text/turtle",
            });
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> .\n",
                results,
            );
        });

        it("ASK with results format", () => {
            const store = new Store([dataModel.quad(ex, ex, ex)]);
            const results = store.query("ASK { ?s ?p ?o }", {
                results_format: "csv",
            });
            assert.strictEqual("true", results);
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
            store.load("<http://example.com> <http://example.com> <http://example.com> .", {
                format: "application/n-triples",
            });
            assert(store.has(dataModel.quad(ex, ex, ex)));
        });

        it("load NTriples in an other graph", () => {
            const store = new Store();
            store.load("<http://example.com> <http://example.com> <http://example.com> .", {
                format: "application/n-triples",
                to_graph_name: ex,
            });
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load NTriples in an other graph with options", () => {
            const store = new Store();
            store.load("<http://example.com> <http://example.com> <http://example.com> .", {
                format: "application/n-triples",
                to_graph_name: ex,
            });
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load Turtle with a base IRI", () => {
            const store = new Store();
            store.load("<http://example.com> <http://example.com> <> .", {
                base_iri: "http://example.com",
                format: "text/turtle",
            });
            assert(store.has(dataModel.quad(ex, ex, ex)));
        });

        it("load NQuads", () => {
            const store = new Store();
            store.load(
                "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .",
                { format: "application/n-quads" },
            );
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load TriG with a base IRI", () => {
            const store = new Store();
            store.load("GRAPH <> { <http://example.com> <http://example.com> <> }", {
                format: "application/trig",
                base_iri: "http://example.com",
            });
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });

        it("load TriG with options", () => {
            const store = new Store();
            store.load("GRAPH <> { <http://example.com> <http://example.com> <> }", {
                format: "application/trig",
                base_iri: "http://example.com",
                unchecked: true,
                no_transaction: true,
            });
            assert(store.has(dataModel.quad(ex, ex, ex, ex)));
        });
    });

    describe("#dump()", () => {
        it("dump dataset content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n",
                store.dump({ format: "application/n-quads" }),
            );
        });

        it("dump named graph content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> .\n",
                store.dump({ format: "application/n-triples", from_graph_name: ex }),
            );
        });

        it("dump named graph content with options", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "<http://example.com> <http://example.com> <http://example.com> .\n",
                store.dump({ format: "application/n-triples", from_graph_name: ex }),
            );
        });

        it("dump default graph content", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(
                "",
                store.dump({
                    format: "application/n-triples",
                    from_graph_name: dataModel.defaultGraph(),
                }),
            );
        });
    });

    describe("#extend()", () => {
        it("should add multiple quads at once", () => {
            const store = new Store();
            store.extend([
                dataModel.quad(ex, ex, ex),
                dataModel.quad(ex, ex, ex2),
            ]);
            assert.strictEqual(2, store.size);
        });
    });

    describe("#named_graphs()", () => {
        it("should return all named graphs", () => {
            const store = new Store([
                dataModel.quad(ex, ex, ex, ex),
                dataModel.quad(ex, ex, ex, ex2),
            ]);
            const graphs = store.named_graphs();
            assert.strictEqual(2, graphs.length);
        });
    });

    describe("#contains_named_graph()", () => {
        it("should return true for existing graph", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            assert.strictEqual(true, store.contains_named_graph(ex));
        });

        it("should return false for non-existing graph", () => {
            const store = new Store();
            assert.strictEqual(false, store.contains_named_graph(ex));
        });

        it("should return true for default graph", () => {
            const store = new Store();
            assert.strictEqual(true, store.contains_named_graph(dataModel.defaultGraph()));
        });
    });

    describe("#add_graph()", () => {
        it("should add an empty named graph", () => {
            const store = new Store();
            store.add_graph(ex);
            assert.strictEqual(true, store.contains_named_graph(ex));
            assert.strictEqual(0, store.size);
        });
    });

    describe("#clear_graph()", () => {
        it("should clear quads from a graph without removing it", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            store.clear_graph(ex);
            assert.strictEqual(0, store.size);
            assert.strictEqual(true, store.contains_named_graph(ex));
        });
    });

    describe("#remove_graph()", () => {
        it("should remove a named graph entirely", () => {
            const store = new Store([dataModel.quad(ex, ex, ex, ex)]);
            store.remove_graph(ex);
            assert.strictEqual(0, store.size);
            assert.strictEqual(false, store.contains_named_graph(ex));
        });
    });

    describe("#clear()", () => {
        it("should clear the entire store", () => {
            const store = new Store([
                dataModel.quad(ex, ex, ex),
                dataModel.quad(ex, ex, ex, ex),
            ]);
            store.clear();
            assert.strictEqual(0, store.size);
        });
    });
});

describe("RdfFormat", () => {
    it("should have static format properties", () => {
        assert.strictEqual("text/turtle", RdfFormat.TURTLE.media_type);
        assert.strictEqual("application/n-triples", RdfFormat.N_TRIPLES.media_type);
        assert.strictEqual("application/n-quads", RdfFormat.N_QUADS.media_type);
        assert.strictEqual("application/trig", RdfFormat.TRIG.media_type);
    });

    it("should support from_media_type lookup", () => {
        const format = RdfFormat.from_media_type("text/turtle");
        assert.notStrictEqual(null, format);
        assert.strictEqual("ttl", format?.file_extension);
    });

    it("should support from_extension lookup", () => {
        const format = RdfFormat.from_extension("nt");
        assert.notStrictEqual(null, format);
        assert.strictEqual("application/n-triples", format?.media_type);
    });

    it("should indicate dataset support", () => {
        assert.strictEqual(false, RdfFormat.TURTLE.supports_datasets);
        assert.strictEqual(true, RdfFormat.N_QUADS.supports_datasets);
        assert.strictEqual(true, RdfFormat.TRIG.supports_datasets);
    });
});

describe("QueryResultsFormat", () => {
    it("should have static format properties", () => {
        assert.strictEqual("application/sparql-results+json", QueryResultsFormat.JSON.media_type);
        assert.strictEqual("application/sparql-results+xml", QueryResultsFormat.XML.media_type);
    });

    it("should support from_media_type lookup", () => {
        const format = QueryResultsFormat.from_media_type("application/sparql-results+json");
        assert.notStrictEqual(null, format);
    });

    it("should support from_extension lookup", () => {
        const format = QueryResultsFormat.from_extension("srj");
        assert.notStrictEqual(null, format);
    });
});

describe("parse()", () => {
    it("should parse N-Triples", () => {
        const quads = parse(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
            RdfFormat.N_TRIPLES,
            {},
        );
        assert.strictEqual(1, quads.length);
    });

    it("should parse Turtle with base IRI", () => {
        const quads = parse(
            "<s> <p> <o> .",
            RdfFormat.TURTLE,
            { base_iri: "http://example.com/" },
        );
        assert.strictEqual(1, quads.length);
        assert.strictEqual("http://example.com/s", quads[0].subject.value);
    });

    it("should parse N-Quads", () => {
        const quads = parse(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .",
            RdfFormat.N_QUADS,
            {},
        );
        assert.strictEqual(1, quads.length);
        assert.strictEqual("http://example.com/g", quads[0].graph.value);
    });
});

describe("serialize()", () => {
    it("should serialize to N-Triples", () => {
        const store = new Store([dataModel.quad(ex, ex, ex)]);
        const quads = store.match();
        const result = serialize(quads, RdfFormat.N_TRIPLES, {});
        assert(result.includes("<http://example.com>"));
    });

    it("should serialize to Turtle with prefixes", () => {
        const store = new Store([dataModel.quad(ex, ex, ex)]);
        const quads = store.match();
        const result = serialize(quads, RdfFormat.TURTLE, {
            prefixes: { ex: "http://example.com/" },
        });
        assert(result.length > 0);
    });
});

describe("parseQueryResults()", () => {
    it("should parse JSON SELECT results", () => {
        const json = '{"head":{"vars":["s"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.com"}}]}}';
        const results = parseQueryResults(json, "json") as Map<string, Term>[];
        assert.strictEqual(1, results.length);
        assert.strictEqual("http://example.com", results[0].get("s")?.value);
    });

    it("should parse JSON ASK results", () => {
        const json = '{"head":{},"boolean":true}';
        const result = parseQueryResults(json, "json");
        assert.strictEqual(true, result);
    });

    it("should parse XML SELECT results", () => {
        const xml = `<?xml version="1.0"?>
<sparql xmlns="http://www.w3.org/2005/sparql-results#">
  <head><variable name="s"/></head>
  <results>
    <result><binding name="s"><uri>http://example.com</uri></binding></result>
  </results>
</sparql>`;
        const results = parseQueryResults(xml, "xml") as Map<string, Term>[];
        assert.strictEqual(1, results.length);
    });
});

describe("serializeQueryBoolean()", () => {
    it("should serialize true to JSON", () => {
        const result = serializeQueryBoolean(true, "json");
        assert(result.includes('"boolean":true'));
    });

    it("should serialize false to JSON", () => {
        const result = serializeQueryBoolean(false, "json");
        assert(result.includes('"boolean":false'));
    });
});

describe("serializeQuerySolutions()", () => {
    it("should serialize solutions to JSON", () => {
        const solutions = [new Map([["s", dataModel.namedNode("http://example.com")]])];
        const result = serializeQuerySolutions(solutions, ["s"], "json");
        assert(result.includes('"vars":["s"]'));
        assert(result.includes('"value":"http://example.com"'));
    });
});
