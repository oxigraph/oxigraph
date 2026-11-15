from __future__ import annotations

import gc
import pickle
import unittest
from collections import Counter
from tempfile import TemporaryDirectory
from typing import Any

import pyincroxigraph
from pyincroxigraph import (
    BlankNode,
    DefaultGraph,
    Delta,
    DeltaKind,
    Literal,
    NamedNode,
    Quad,
    QueryBoolean,
    QueryBooleanDeltas,
    QuerySolution,
    QuerySolutionDeltas,
    QuerySolutions,
    QueryTripleDeltas,
    QueryTriples,
    RdfFormat,
    Store,
    Triple,
    Variable,
)

s = NamedNode("urn:s")
p = NamedNode("urn:p")
q = NamedNode("urn:q")
first = Quad(s, p, Literal(1))
second = Quad(s, p, Literal(2))
select = "SELECT ?s ?o WHERE { ?s <urn:p> ?o }"


def rows(result: QuerySolutions | QueryBoolean | QueryTriples) -> QuerySolutions | QueryTriples:
    assert isinstance(result, (QuerySolutions, QueryTriples))
    return result


class TestIncremental(unittest.TestCase):
    def test_self_contained_types(self) -> None:
        for cls in (NamedNode, BlankNode, Literal, Quad, Triple, DefaultGraph, Variable, Store, Delta, DeltaKind):
            self.assertEqual(cls.__module__, "pyincroxigraph")
            self.assertIs(getattr(pyincroxigraph, cls.__name__), cls)
        self.assertEqual(pickle.loads(pickle.dumps(s)), s)

    def test_select_snapshots_are_owned_and_repeatable(self) -> None:
        store = Store()
        query = store.query_incremental_results(select)
        self.assertEqual(query.variables, [Variable("s"), Variable("o")])
        self.assertEqual(list(rows(query.results())), [])
        store.add(first)
        previous = query.results()
        assert isinstance(previous, QuerySolutions)
        self.assertEqual(previous.variables, query.variables)
        store.add(second)
        current = rows(query.results())
        self.assertEqual([row["o"] for row in previous], [Literal(1)])
        self.assertEqual(
            Counter(tuple(row) for row in current), Counter(tuple(row) for row in rows(store.query(select)))
        )
        self.assertEqual(len(list(rows(query.results()))), 2)
        store.remove(first)
        final = rows(query.results())
        query.close()
        self.assertEqual([row["o"] for row in final], [Literal(2)])

    def test_select_deltas_initial_add_remove_and_noops(self) -> None:
        store = Store()
        store.add(first)
        with store.query_incremental_deltas(select) as query:
            changes = query.deltas()
            assert isinstance(changes, QuerySolutionDeltas)
            self.assertEqual(changes.variables, [Variable("s"), Variable("o")])
            initial = list(changes)
            self.assertEqual(len(initial), 1)
            self.assertEqual(initial[0].kind, DeltaKind.ADDITION)
            self.assertIsInstance(initial[0].value, QuerySolution)
            self.assertEqual(initial[0].value["o"], Literal(1))
            self.assertEqual(list(query.deltas()), [])
            store.add(first)
            self.assertEqual(list(query.deltas()), [])
            store.remove(first)
            removed = list(query.deltas())
            self.assertEqual(len(removed), 1)
            self.assertEqual(removed[0].kind, DeltaKind.DELETION)
            self.assertEqual(removed[0].value, initial[0].value)
            store.remove(first)
            self.assertEqual(list(query.deltas()), [])

    def test_select_preserves_duplicate_multiplicities(self) -> None:
        store = Store()
        with store.query_incremental_results("SELECT ?s WHERE { ?s <urn:p> ?o }") as snapshots:
            self.assertFalse(snapshots.closed)
            with store.query_incremental_deltas("SELECT ?s WHERE { ?s <urn:p> ?o }") as changes:
                self.assertEqual(list(changes.deltas()), [])
                store.extend([first, second])
                self.assertEqual([row["s"] for row in rows(snapshots.results())], [s, s])
                additions = list(changes.deltas())
                self.assertEqual(len(additions), 2)
                self.assertTrue(all(delta.kind == DeltaKind.ADDITION for delta in additions))
                self.assertEqual(additions[0].value, additions[1].value)
                store.remove(first)
                self.assertEqual(len(list(rows(snapshots.results()))), 1)
                deletions = list(changes.deltas())
                self.assertEqual(len(deletions), 1)
                self.assertEqual(deletions[0].kind, DeltaKind.DELETION)

    def test_join_matches_regular_query_after_commits(self) -> None:
        store = Store()
        query_text = "SELECT ?s ?o ?v WHERE { ?s <urn:p> ?o . ?o <urn:q> ?v }"
        with store.query_incremental_results(query_text) as snapshots:
            self.assertFalse(snapshots.closed)
            with store.query_incremental_deltas(query_text) as changes:
                counts: Counter[tuple[object, ...]] = Counter()
                updates = [
                    (True, Quad(s, p, q)),
                    (True, Quad(q, q, Literal(1))),
                    (True, Quad(q, q, Literal(2))),
                    (False, Quad(s, p, q)),
                ]
                for addition, quad in updates:
                    if addition:
                        store.add(quad)
                    else:
                        store.remove(quad)
                    for delta in changes.deltas():
                        row = tuple(delta.value)
                        counts[row] += 1 if delta.kind == DeltaKind.ADDITION else -1
                    expected = Counter(tuple(row) for row in rows(store.query(query_text)))
                    self.assertEqual(+counts, expected)
                    self.assertEqual(Counter(tuple(row) for row in rows(snapshots.results())), expected)

    def test_owned_batches_and_independent_handles(self) -> None:
        store = Store()
        left = store.query_incremental_deltas(select)
        right = store.query_incremental_deltas(select)
        store.add(first)
        batch = left.deltas()
        self.assertEqual(list(left.deltas()), [])
        left.close()
        self.assertEqual(len(list(batch)), 1)
        self.assertEqual(len(list(right.deltas())), 1)
        right.close()

    def test_static_values(self) -> None:
        store = Store()
        text = "SELECT ?s WHERE { VALUES ?s { <urn:a> <urn:b> } }"
        with store.query_incremental_results(text) as snapshots:
            self.assertEqual({row["s"] for row in rows(snapshots.results())}, {NamedNode("urn:a"), NamedNode("urn:b")})
            with store.query_incremental_deltas(text) as changes:
                self.assertEqual(len(list(changes.deltas())), 2)
                store.add(first)
                self.assertEqual(list(changes.deltas()), [])
                self.assertEqual(len(list(rows(snapshots.results()))), 2)

    def test_union_default_graph_tracks_duplicate_support(self) -> None:
        store = Store()
        graph = NamedNode("urn:g")
        in_graph = Quad(s, p, Literal(1), graph)
        with store.query_incremental_deltas(select, use_default_graph_as_union=True) as query:
            store.extend([first, in_graph])
            self.assertEqual(len(list(query.deltas())), 1)
            store.remove(first)
            self.assertEqual(list(query.deltas()), [])
            store.remove(in_graph)
            removed = list(query.deltas())
            self.assertEqual(len(removed), 1)
            self.assertEqual(removed[0].kind, DeltaKind.DELETION)

    def test_ask_snapshots_and_deltas(self) -> None:
        store = Store()
        with store.query_incremental_results("ASK { ?s <urn:p> ?o }") as snapshots:
            self.assertFalse(snapshots.closed)
            with store.query_incremental_deltas("ASK { ?s <urn:p> ?o }") as changes:
                self.assertIsNone(snapshots.variables)
                self.assertIsNone(changes.variables)
                self.assertIsInstance(snapshots.results(), QueryBoolean)
                self.assertFalse(snapshots.results())
                batch = changes.deltas()
                self.assertIsInstance(batch, QueryBooleanDeltas)
                self.assertEqual([bool(value) for value in batch], [False])
                self.assertEqual(list(changes.deltas()), [])
                store.add(first)
                self.assertTrue(snapshots.results())
                self.assertEqual([bool(value) for value in changes.deltas()], [True])
                store.add(second)
                store.remove(first)
                self.assertEqual(list(changes.deltas()), [])
                store.remove(second)
                self.assertFalse(snapshots.results())
                self.assertEqual([bool(value) for value in changes.deltas()], [False])

    def test_construct_set_semantics_and_owned_snapshot(self) -> None:
        store = Store()
        text = "CONSTRUCT { <urn:result> <urn:q> <urn:value> } WHERE { ?s <urn:p> ?o }"
        triple = Triple(NamedNode("urn:result"), q, NamedNode("urn:value"))
        with store.query_incremental_results(text) as snapshots:
            self.assertFalse(snapshots.closed)
            with store.query_incremental_deltas(text) as changes:
                self.assertIsNone(snapshots.variables)
                self.assertIsNone(changes.variables)
                self.assertEqual(list(rows(snapshots.results())), [])
                self.assertEqual(list(changes.deltas()), [])
                store.extend([first, second])
                previous = snapshots.results()
                assert isinstance(previous, QueryTriples)
                batch = changes.deltas()
                self.assertIsInstance(batch, QueryTripleDeltas)
                initial = list(batch)
                self.assertEqual(len(initial), 1)
                self.assertEqual(initial[0].kind, DeltaKind.ADDITION)
                self.assertEqual(initial[0].value, triple)
                store.remove(first)
                self.assertEqual(list(changes.deltas()), [])
                store.remove(second)
                self.assertEqual(list(rows(snapshots.results())), [])
                self.assertEqual(list(previous), [triple])
                removed = list(changes.deltas())
                self.assertEqual(len(removed), 1)
                self.assertEqual(removed[0].kind, DeltaKind.DELETION)
                self.assertEqual(removed[0].value, triple)

    def test_construct_blank_node_identity_survives_deletion(self) -> None:
        store = Store()
        store.add(first)
        with store.query_incremental_deltas("CONSTRUCT { _:b <urn:q> ?o } WHERE { ?s <urn:p> ?o }") as query:
            added = next(query.deltas()).value
            self.assertIsInstance(added, Triple)
            self.assertIsInstance(added.subject, BlankNode)
            store.remove(first)
            removed = next(query.deltas())
            self.assertEqual(removed.kind, DeltaKind.DELETION)
            self.assertEqual(removed.value, added)

    def test_options_match_regular_queries(self) -> None:
        store = Store()
        graph = NamedNode("urn:graph")
        store.extend([first, Quad(s, p, Literal(2), graph)])
        configurations: list[dict[str, Any]] = [
            {"default_graph": graph},
            {"default_graph": [DefaultGraph(), graph]},
            {"use_default_graph_as_union": True},
            {"substitutions": {Variable("o"): Literal(1)}},
        ]
        for options in configurations:
            with self.subTest(options=options), store.query_incremental_results(select, **options) as query:
                self.assertEqual(
                    Counter(tuple(row) for row in rows(query.results())),
                    Counter(tuple(row) for row in rows(store.query(select, **options))),
                )
        with store.query_incremental_deltas("SELECT ?s WHERE { ?s ex:p ?o }", prefixes={"ex": "urn:"}) as query:
            self.assertEqual(next(query.deltas()).value["s"], s)
        with store.query_incremental_deltas("SELECT ?s WHERE { ?s <p> ?o }", base_iri="http://example/") as query:
            self.assertEqual(list(query.deltas()), [])
            store.add(Quad(s, NamedNode("http://example/p"), Literal(1)))
            self.assertEqual(next(query.deltas()).value["s"], s)
        with store.query_incremental_results(
            "SELECT ?g WHERE { GRAPH ?g { ?s <urn:p> ?o } }", named_graphs=[graph]
        ) as query:
            self.assertEqual([row["g"] for row in rows(query.results())], [graph])

    def test_load_update_clear_and_named_graph_changes(self) -> None:
        store = Store()
        with store.query_incremental_results(select, use_default_graph_as_union=True) as query:
            store.load("<urn:s> <urn:p> 1 .", RdfFormat.TURTLE)
            self.assertEqual(len(list(rows(query.results()))), 1)
            store.update("INSERT DATA { GRAPH <urn:g> { <urn:s> <urn:p> 2 } }")
            self.assertEqual(len(list(rows(query.results()))), 2)
            store.clear_graph(NamedNode("urn:g"))
            self.assertEqual(len(list(rows(query.results()))), 1)
            store.clear()
            self.assertEqual(list(rows(query.results())), [])

    def test_snapshot_serialization(self) -> None:
        store = Store()
        store.add(first)
        with store.query_incremental_results("CONSTRUCT WHERE { ?s ?p ?o }") as query:
            snapshot = query.results()
            assert isinstance(snapshot, QueryTriples)
            query.close()
            self.assertEqual(
                snapshot.serialize(format=RdfFormat.N_TRIPLES),
                b'<urn:s> <urn:p> "1"^^<http://www.w3.org/2001/XMLSchema#integer> .\n',
            )

    def test_close_and_context_exception(self) -> None:
        store = Store()
        for factory, method in (
            (store.query_incremental_results, "results"),
            (store.query_incremental_deltas, "deltas"),
        ):
            query = factory(select)
            self.assertFalse(query.closed)
            with self.assertRaisesRegex(ValueError, "example"), query:
                raise ValueError("example")
            self.assertTrue(query.closed)
            query.close()
            with self.assertRaisesRegex(RuntimeError, "closed"):
                getattr(query, method)()
            with self.assertRaisesRegex(RuntimeError, "closed"):
                query.__enter__()

    def test_handle_keeps_store_alive(self) -> None:
        store = Store()
        store.add(first)
        query = store.query_incremental_results(select)
        del store
        gc.collect()
        self.assertEqual(len(list(rows(query.results()))), 1)
        query.close()

    def test_delta_is_immutable(self) -> None:
        store = Store()
        store.add(first)
        with store.query_incremental_deltas(select) as query:
            delta = next(query.deltas())
            self.assertIn("ADDITION", repr(delta))
            with self.assertRaises(AttributeError):
                delta.kind = DeltaKind.DELETION
            with self.assertRaises(AttributeError):
                delta.value = None
            self.assertNotEqual(DeltaKind.ADDITION, DeltaKind.DELETION)
            self.assertEqual(len({DeltaKind.ADDITION, DeltaKind.DELETION}), 2)

    def test_invalid_and_unsupported_queries(self) -> None:
        store = Store()
        for factory in (store.query_incremental_results, store.query_incremental_deltas):
            with self.assertRaises(SyntaxError):
                factory("invalid query")
            with self.assertRaises(ValueError):
                factory(select, default_graph=DefaultGraph(), use_default_graph_as_union=True)
            for text in (
                "DESCRIBE ?s WHERE { ?s ?p ?o }",
                "SELECT ?s WHERE { ?s <urn:p> ?o FILTER(?o > 1) }",
                "SELECT ?s WHERE { ?s <urn:p> ?o } ORDER BY ?s",
            ):
                with self.subTest(query=text), self.assertRaises(NotImplementedError):
                    factory(text)

    def test_disk_store(self) -> None:
        with TemporaryDirectory() as directory:
            store = Store(directory)
            with store.query_incremental_deltas(select) as changes:
                self.assertFalse(changes.closed)
                with store.query_incremental_results(select) as snapshots:
                    with store.transaction() as tx:
                        tx.extend([first, second])
                    self.assertEqual(len(list(changes.deltas())), 2)
                    self.assertEqual(len(list(rows(snapshots.results()))), 2)
            del store
            gc.collect()


class TestTransactions(unittest.TestCase):
    def test_commit_and_visibility(self) -> None:
        store = Store()
        with store.query_incremental_deltas(select) as query:
            with store.transaction() as tx:
                tx.add(first)
                tx.extend([second])
                tx.remove(first)
                self.assertEqual(len(store), 0)
                self.assertEqual(list(query.deltas()), [])
            self.assertTrue(tx.closed)
            changes = list(query.deltas())
            self.assertEqual(len(changes), 1)
            self.assertEqual(changes[0].kind, DeltaKind.ADDITION)
            self.assertEqual(changes[0].value["o"], Literal(2))
            self.assertEqual(list(store), [second])

    def test_exception_rolls_back(self) -> None:
        store = Store()
        with self.assertRaisesRegex(ValueError, "rollback"), store.transaction() as tx:
            tx.add(first)
            raise ValueError("rollback")
        self.assertTrue(tx.closed)
        self.assertEqual(len(store), 0)

    def test_explicit_commit_rollback_and_drop(self) -> None:
        store = Store()
        tx = store.transaction()
        tx.add(first)
        tx.commit()
        self.assertTrue(tx.closed)
        self.assertIn(first, store)
        with self.assertRaisesRegex(RuntimeError, "closed"):
            tx.commit()
        with self.assertRaisesRegex(RuntimeError, "closed"):
            tx.add(second)
        with self.assertRaisesRegex(RuntimeError, "closed"):
            tx.remove(first)
        with self.assertRaisesRegex(RuntimeError, "closed"):
            tx.extend([])
        with self.assertRaisesRegex(RuntimeError, "closed"):
            tx.__enter__()
        tx.rollback()
        tx = store.transaction()
        tx.add(second)
        tx.rollback()
        tx.rollback()
        self.assertNotIn(second, store)
        tx = store.transaction()
        tx.add(second)
        del tx
        gc.collect()
        self.assertNotIn(second, store)

    def test_nested_and_manual_context_completion(self) -> None:
        store = Store()
        with store.transaction() as tx:
            with self.assertRaisesRegex(RuntimeError, "already entered"):
                tx.__enter__()
            tx.add(first)
            tx.commit()
        self.assertIn(first, store)
        with store.transaction() as tx:
            tx.remove(first)
            tx.rollback()
        self.assertIn(first, store)

    def test_extend_rejects_partial_invalid_input(self) -> None:
        store = Store()
        with store.transaction() as tx:
            with self.assertRaises(TypeError):
                tx.extend([first, "invalid"])  # ty: ignore[invalid-argument-type]
            tx.add(second)
        self.assertEqual(list(store), [second])

    def test_operation_order_and_net_noop(self) -> None:
        store = Store()
        with store.query_incremental_deltas(select) as query:
            with store.transaction() as tx:
                tx.add(first)
                tx.remove(first)
            self.assertEqual(list(query.deltas()), [])
            with store.transaction() as tx:
                tx.remove(first)
                tx.add(first)
                tx.add(first)
            self.assertEqual(len(list(query.deltas())), 1)


if __name__ == "__main__":
    unittest.main()
