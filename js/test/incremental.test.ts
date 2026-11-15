import assert from "node:assert/strict";
import factory from "@rdfjs/data-model";
import { describe, it } from "vitest";
import { type Bindings, type Delta, DeltaKind, type Quad, Store } from "../pkg";

const p = factory.namedNode("urn:p");
const s = factory.namedNode("urn:s");
const first = factory.quad(s, p, factory.literal("first"));
const second = factory.quad(s, p, factory.literal("second"));
const select = "SELECT ?s ?o WHERE { ?s <urn:p> ?o }";

function rows(value: ReturnType<Store["query"]>): Bindings[] {
    assert(Array.isArray(value));
    for (const row of value) assert(row instanceof Map);
    return value as Bindings[];
}

function changes(
    value: ReturnType<ReturnType<Store["queryIncrementalDeltas"]>["deltas"]>,
): Delta<Bindings>[] {
    for (const change of value) {
        assert(typeof change !== "boolean");
        assert(change.value instanceof Map);
    }
    return value as Delta<Bindings>[];
}

describe("incremental queries", () => {
    it("returns owned snapshots while the same live query tracks writes", () => {
        const store = new Store();
        const query = store.queryIncrementalResults(select);
        assert.deepEqual(query.variables, ["s", "o"]);
        assert.deepEqual(query.results(), []);
        store.add(first);
        const previous = rows(query.results());
        previous[0]?.set("o", factory.literal("changed locally"));
        assert.equal(rows(query.results())[0]?.get("o")?.value, "first");
        store.add(second);
        assert.equal(rows(query.results()).length, 2);
        store.delete(first);
        const final = rows(query.results());
        query.close();
        assert.equal(final[0]?.get("o")?.value, "second");
        assert.equal(previous[0]?.get("o")?.value, "changed locally");
    });

    it("consumes initial additions, insertions, deletions and no-ops", () => {
        const store = new Store([first]);
        const query = store.queryIncrementalDeltas(select);
        assert.deepEqual(query.variables, ["s", "o"]);
        const initial = changes(query.deltas());
        assert.equal(initial[0]?.kind, DeltaKind.Addition);
        assert(Object.isFrozen(initial[0]));
        assert.deepEqual(query.deltas(), []);
        store.add(first);
        assert.deepEqual(query.deltas(), []);
        store.delete(first);
        const removed = changes(query.deltas());
        assert.equal(removed[0]?.kind, DeltaKind.Deletion);
        assert.equal(removed[0]?.value.get("o")?.value, "first");
        assert.equal(initial[0]?.value.get("o")?.value, "first");
        store.delete(first);
        assert.deepEqual(query.deltas(), []);
        query.close();
    });

    it("preserves SELECT multiplicity and independent consumers", () => {
        const store = new Store();
        const a = store.queryIncrementalDeltas("SELECT ?s WHERE { ?s <urn:p> ?o }");
        const b = store.queryIncrementalResults("SELECT ?s WHERE { ?s <urn:p> ?o }");
        const c = store.queryIncrementalDeltas("SELECT ?s WHERE { ?s <urn:p> ?o }");
        store.extend([first, second]);
        assert.equal(a.deltas().length, 2);
        assert.deepEqual(a.deltas(), []);
        assert.equal(c.deltas().length, 2);
        assert.equal(rows(b.results()).length, 2);
        store.delete(first);
        assert.equal(a.deltas().length, 1);
        assert.equal(rows(b.results()).length, 1);
        for (const handle of [a, b, c]) handle.close();
    });

    it("maintains joins after atomic commits", () => {
        const store = new Store();
        const text = "SELECT ?s ?v WHERE { ?s <urn:p> ?o . ?o <urn:q> ?v }";
        const query = store.queryIncrementalResults(text);
        const tx = store.transaction();
        const middle = factory.namedNode("urn:middle");
        tx.extend([
            factory.quad(s, p, middle),
            factory.quad(middle, factory.namedNode("urn:q"), factory.literal("joined")),
        ]);
        assert.deepEqual(query.results(), []);
        tx.commit();
        assert.deepEqual(query.results(), store.query(text));
        assert.equal(rows(query.results())[0]?.get("v")?.value, "joined");
        query.close();
    });

    it("returns initial false for ASK, followed only by changes", () => {
        const store = new Store();
        const query = store.queryIncrementalDeltas("ASK { ?s <urn:p> ?o }");
        const snapshots = store.queryIncrementalResults("ASK { ?s <urn:p> ?o }");
        assert.equal(query.variables, null);
        assert.deepEqual(query.deltas(), [false]);
        assert.equal(snapshots.results(), false);
        assert.deepEqual(query.deltas(), []);
        store.add(first);
        assert.deepEqual(query.deltas(), [true]);
        store.add(second);
        assert.deepEqual(query.deltas(), []);
        store.delete(first);
        assert.deepEqual(query.deltas(), []);
        store.delete(second);
        assert.deepEqual(query.deltas(), [false]);
        query.close();
        snapshots.close();
    });

    it("uses graph set semantics for CONSTRUCT and preserves blank-node identity", () => {
        const store = new Store();
        const query = store.queryIncrementalDeltas(
            "CONSTRUCT { ?s <urn:type> <urn:Item> } WHERE { ?s <urn:p> ?o }",
        );
        assert.equal(query.variables, null);
        store.extend([first, second]);
        assert.equal(query.deltas().length, 1);
        store.delete(first);
        assert.deepEqual(query.deltas(), []);
        store.delete(second);
        assert.equal(query.deltas().length, 1);
        query.close();
        const blank = store.queryIncrementalDeltas(
            "CONSTRUCT { _:b <urn:p> ?o } WHERE { ?s <urn:p> ?o }",
        );
        store.add(first);
        const added = blank.deltas() as Delta<Quad>[];
        assert.equal(added[0]?.value.subject.termType, "BlankNode");
        store.delete(first);
        const removed = blank.deltas() as Delta<Quad>[];
        assert(added[0]?.value.equals(removed[0]?.value));
        blank.close();
        const snapshots = store.queryIncrementalResults("CONSTRUCT WHERE { ?s ?p ?o }");
        store.add(first);
        const snapshot = snapshots.results() as Quad[];
        snapshots.close();
        assert(snapshot[0]?.equals(first));
    });

    it("shares ordinary graph selection and base IRI options", () => {
        const graph = factory.namedNode("urn:g");
        const store = new Store([first, factory.quad(s, p, factory.literal("named"), graph)]);
        for (const options of [
            { default_graph: graph },
            { use_default_graph_as_union: true },
            { default_graph: [factory.defaultGraph(), graph] },
        ]) {
            const query = store.queryIncrementalResults(select, options);
            assert.deepEqual(
                rows(query.results())
                    .map((row) => row.get("o")?.value)
                    .sort(),
                rows(store.query(select, options))
                    .map((row) => row.get("o")?.value)
                    .sort(),
            );
            query.close();
        }
        const named = store.queryIncrementalResults(
            "SELECT ?g WHERE { GRAPH ?g { ?s <urn:p> ?o } }",
            { named_graphs: [graph] },
        );
        assert.equal(rows(named.results())[0]?.get("g")?.value, "urn:g");
        named.close();
        const relative = store.queryIncrementalResults("SELECT ?o WHERE { ?s <p> ?o }", {
            base_iri: "http://example/",
        });
        store.add(
            factory.quad(s, factory.namedNode("http://example/p"), factory.literal("relative")),
        );
        assert.equal(rows(relative.results())[0]?.get("o")?.value, "relative");
        relative.close();
    });

    it("observes load and SPARQL update, and serializes snapshots", () => {
        const store = new Store();
        const query = store.queryIncrementalResults(select, { results_format: "json" });
        store.load('<urn:s> <urn:p> "loaded" .', { format: "nt" });
        assert.equal(JSON.parse(query.results() as string).results.bindings.length, 1);
        store.update(
            'DELETE DATA { <urn:s> <urn:p> "loaded" }; INSERT DATA { <urn:s> <urn:p> "updated" }',
        );
        assert.equal(JSON.parse(query.results() as string).results.bindings[0].o.value, "updated");
        query.close();
    });

    it("rejects unsupported plans and malformed options without silently requerying", () => {
        const store = new Store();
        for (const text of [
            "DESCRIBE ?s WHERE { ?s ?p ?o }",
            "SELECT ?s WHERE { ?s <urn:p> ?o FILTER(?o > 1) }",
            "SELECT ?s WHERE { ?s <urn:p> ?o } ORDER BY ?s",
        ]) {
            assert.throws(() => store.queryIncrementalResults(text), {
                name: "NotImplementedError",
            });
            assert.throws(() => store.queryIncrementalDeltas(text), {
                name: "NotImplementedError",
            });
        }
        assert.throws(() => store.queryIncrementalDeltas("bad query"));
        assert.throws(
            () => store.queryIncrementalDeltas(select, { results_format: "json" } as never),
            { name: "TypeError" },
        );
        assert.throws(() => store.queryIncrementalResults(select, { named_graphs: 1 } as never));
    });

    it("closes on read errors and tolerates repeated close", () => {
        const store = new Store();
        const query = store.queryIncrementalResults(select, { results_format: "unsupported" });
        assert.throws(() => query.results());
        assert.equal(query.closed, true);
        query.close();
        assert.throws(() => query.results(), /closed/);
        const deltas = store.queryIncrementalDeltas(select);
        deltas.close();
        deltas.close();
        assert.equal(deltas.closed, true);
        assert.throws(() => deltas.deltas(), /closed/);
    });

    it("keeps query state alive after freeing the Store wrapper", () => {
        const store = new Store([first]);
        const query = store.queryIncrementalResults(select);
        store.free();
        assert.equal(rows(query.results()).length, 1);
        query.close();
        query.free();
    });
});

describe("write batches", () => {
    it("commits buffered writes atomically and drops net no-ops", () => {
        const store = new Store();
        const query = store.queryIncrementalDeltas(select);
        assert.deepEqual(query.deltas(), []);
        const tx = store.transaction();
        tx.add(first);
        assert.equal(store.size, 0);
        assert.deepEqual(query.deltas(), []);
        tx.commit();
        assert.equal(tx.closed, true);
        assert.equal(store.size, 1);
        assert.equal(query.deltas().length, 1);
        const noop = store.transaction();
        noop.add(second);
        noop.delete(second);
        noop.commit();
        assert.deepEqual(query.deltas(), []);
        assert.throws(() => tx.add(second), /closed/);
        assert.throws(() => tx.commit(), /closed/);
        tx.rollback();
        query.close();
    });

    it("discards rolled-back and freed transactions", () => {
        const store = new Store();
        const tx = store.transaction();
        tx.add(first);
        tx.rollback();
        tx.rollback();
        assert.equal(tx.closed, true);
        const dropped = store.transaction();
        dropped.add(second);
        dropped.free();
        assert.equal(store.size, 0);
    });

    it("validates an entire extend call before changing a batch or store", () => {
        const store = new Store();
        const tx = store.transaction();
        tx.add(first);
        assert.throws(() => tx.extend([second, {}] as never));
        tx.commit();
        assert(store.has(first));
        assert(!store.has(second));
        assert.throws(() => store.extend([second, {}] as never));
        assert(!store.has(second));
        assert.throws(() => store.extend(12 as never));
    });

    it("accepts structural RDF/JS quads without a package-specific class", () => {
        const store = new Store();
        store.add({
            subject: { termType: "NamedNode", value: "urn:s" },
            predicate: { termType: "NamedNode", value: "urn:p" },
            object: {
                termType: "Literal",
                value: "plain",
                language: "",
                datatype: {
                    termType: "NamedNode",
                    value: "http://www.w3.org/2001/XMLSchema#string",
                },
            },
            graph: { termType: "DefaultGraph", value: "" },
        } as Quad);
        assert(store.has(factory.quad(s, p, factory.literal("plain"))));
    });
});
