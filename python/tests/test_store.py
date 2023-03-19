import unittest
from io import BytesIO, UnsupportedOperation
from pathlib import Path
from tempfile import NamedTemporaryFile, TemporaryDirectory, TemporaryFile
from typing import Any

from pyoxigraph import (
    BlankNode,
    DefaultGraph,
    NamedNode,
    Quad,
    QuerySolution,
    QuerySolutions,
    QueryTriples,
    Store,
    Triple,
    Variable,
)

foo = NamedNode("http://foo")
bar = NamedNode("http://bar")
baz = NamedNode("http://baz")
triple = Triple(foo, foo, foo)
graph = NamedNode("http://graph")


class TestStore(unittest.TestCase):
    def test_add(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(triple, bar, baz))
        store.add(Quad(foo, bar, triple))
        self.assertEqual(len(store), 4)

    def test_extend(self) -> None:
        store = Store()
        store.extend(
            (
                Quad(foo, bar, baz),
                Quad(foo, bar, baz, graph),
                Quad(foo, bar, baz, DefaultGraph()),
            )
        )
        self.assertEqual(len(store), 2)

    def test_bulk_extend(self) -> None:
        store = Store()
        store.bulk_extend(
            (
                Quad(foo, bar, baz),
                Quad(foo, bar, baz, graph),
                Quad(foo, bar, baz, DefaultGraph()),
            )
        )
        self.assertEqual(len(store), 2)

    def test_remove(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        store.remove(Quad(foo, bar, baz))
        self.assertEqual(len(store), 1)

    def test_len(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(len(store), 2)

    def test_in(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertIn(Quad(foo, bar, baz), store)
        self.assertIn(Quad(foo, bar, baz, DefaultGraph()), store)
        self.assertIn(Quad(foo, bar, baz, graph), store)
        self.assertNotIn(Quad(foo, bar, baz, foo), store)

    def test_iter(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(
            set(store),
            {Quad(foo, bar, baz, DefaultGraph()), Quad(foo, bar, baz, graph)},
        )

    def test_quads_for_pattern(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(
            set(store.quads_for_pattern(None, None, None)),
            {Quad(foo, bar, baz, DefaultGraph()), Quad(foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.quads_for_pattern(foo, None, None)),
            {Quad(foo, bar, baz, DefaultGraph()), Quad(foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.quads_for_pattern(None, None, None, graph)),
            {Quad(foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.quads_for_pattern(foo, None, None, DefaultGraph())),
            {Quad(foo, bar, baz, DefaultGraph())},
        )

    def test_ask_query(self) -> None:
        store = Store()
        store.add(Quad(foo, foo, foo))
        self.assertTrue(store.query("ASK { ?s ?s ?s }"))
        self.assertFalse(store.query("ASK { FILTER(false) }"))

    def test_construct_query(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        results: Any = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
        self.assertIsInstance(results, QueryTriples)
        self.assertEqual(
            set(results),
            {Triple(foo, bar, baz)},
        )

    def test_select_query(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz))
        solutions: Any = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }")
        self.assertIsInstance(solutions, QuerySolutions)
        self.assertEqual(solutions.variables, [Variable("s"), Variable("o")])
        solution = next(solutions)
        self.assertIsInstance(solution, QuerySolution)
        self.assertEqual(solution[0], foo)
        self.assertEqual(solution[1], baz)
        self.assertEqual(solution["s"], foo)
        self.assertEqual(solution["o"], baz)
        self.assertEqual(solution[Variable("s")], foo)
        self.assertEqual(solution[Variable("o")], baz)
        s, o = solution
        self.assertEqual(s, foo)
        self.assertEqual(o, baz)

    def test_select_query_union_default_graph(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        results: Any = store.query("SELECT ?s WHERE { ?s ?p ?o }")
        self.assertEqual(len(list(results)), 0)
        results = store.query(
            "SELECT ?s WHERE { ?s ?p ?o }", use_default_graph_as_union=True
        )
        self.assertEqual(len(list(results)), 1)
        results = store.query(
            "SELECT ?s WHERE { ?s ?p ?o }",
            use_default_graph_as_union=True,
            named_graphs=[graph],
        )
        self.assertEqual(len(list(results)), 1)

    def test_select_query_with_default_graph(self) -> None:
        store = Store()
        graph_bnode = BlankNode("g")
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(foo, bar, foo))
        store.add(Quad(foo, bar, bar, graph_bnode))
        results: Any = store.query("SELECT ?s WHERE { ?s ?p ?o }")
        self.assertEqual(len(list(results)), 1)
        results = store.query("SELECT ?s WHERE { ?s ?p ?o }", default_graph=graph)
        self.assertEqual(len(list(results)), 1)
        results = store.query(
            "SELECT ?s WHERE { ?s ?p ?o }",
            default_graph=[DefaultGraph(), graph, graph_bnode],
        )
        self.assertEqual(len(list(results)), 3)

    def test_select_query_with_named_graph(self) -> None:
        store = Store()
        graph_bnode = BlankNode("g")
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(foo, bar, foo))
        store.add(Quad(foo, bar, bar, graph_bnode))
        store.add(Quad(foo, bar, bar, foo))
        results: Any = store.query(
            "SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o } }",
            named_graphs=[graph, graph_bnode],
        )
        self.assertEqual(len(list(results)), 2)

    def test_update_insert_data(self) -> None:
        store = Store()
        store.update("INSERT DATA { <http://foo> <http://foo> <http://foo> }")
        self.assertEqual(len(store), 1)

    def test_update_delete_data(self) -> None:
        store = Store()
        store.add(Quad(foo, foo, foo))
        store.update("DELETE DATA { <http://foo> <http://foo> <http://foo> }")
        self.assertEqual(len(store), 0)

    def test_update_delete_where(self) -> None:
        store = Store()
        store.add(Quad(foo, foo, foo))
        store.update("DELETE WHERE { ?v ?v ?v }")
        self.assertEqual(len(store), 0)

    def test_update_load(self) -> None:
        store = Store()
        store.update("LOAD <https://www.w3.org/1999/02/22-rdf-syntax-ns>")
        self.assertGreater(len(store), 100)

    def test_update_star(self) -> None:
        store = Store()
        store.update(
            "PREFIX : <http://www.example.org/> INSERT DATA { :alice :claims << :bob :age 23 >> }"
        )
        results: Any = store.query(
            "PREFIX : <http://www.example.org/> SELECT ?p ?a WHERE { ?p :claims << :bob :age ?a >> }"
        )
        self.assertEqual(len(list(results)), 1)

    def test_load_ntriples_to_default_graph(self) -> None:
        store = Store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> ."),
            mime_type="application/n-triples",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, DefaultGraph())})

    def test_load_ntriples_to_named_graph(self) -> None:
        store = Store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> ."),
            mime_type="application/n-triples",
            to_graph=graph,
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_turtle_with_base_iri(self) -> None:
        store = Store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <> ."),
            mime_type="text/turtle",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, DefaultGraph())})

    def test_load_nquads(self) -> None:
        store = Store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> <http://graph>."),
            mime_type="application/n-quads",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_trig_with_base_iri(self) -> None:
        store = Store()
        store.load(
            BytesIO(b"<http://graph> { <http://foo> <http://bar> <> . }"),
            mime_type="application/trig",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_file(self) -> None:
        with NamedTemporaryFile(delete=False) as fp:
            file_name = fp.name
            fp.write(b"<http://foo> <http://bar> <http://baz> <http://graph>.")
        store = Store()
        store.load(file_name, mime_type="application/n-quads")
        Path(file_name).unlink()
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_with_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _, TemporaryFile("wb") as fp:
            Store().load(fp, mime_type="application/n-triples")

    def test_dump_ntriples(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        output = BytesIO()
        store.dump(output, "application/n-triples", from_graph=graph)
        self.assertEqual(
            output.getvalue(),
            b"<http://foo> <http://bar> <http://baz> .\n",
        )

    def test_dump_nquads(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        output = BytesIO()
        store.dump(output, "application/n-quads")
        self.assertEqual(
            output.getvalue(),
            b"<http://foo> <http://bar> <http://baz> <http://graph> .\n",
        )

    def test_dump_trig(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(foo, bar, baz))
        output = BytesIO()
        store.dump(output, "application/trig")
        self.assertEqual(
            output.getvalue(),
            b"<http://foo> <http://bar> <http://baz> .\n<http://graph> { <http://foo> <http://bar> <http://baz> }\n",
        )

    def test_dump_file(self) -> None:
        with NamedTemporaryFile(delete=False) as fp:
            file_name = fp.name
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        store.dump(file_name, "application/n-quads")
        self.assertEqual(
            Path(file_name).read_text(),
            "<http://foo> <http://bar> <http://baz> <http://graph> .\n",
        )

    def test_dump_with_io_error(self) -> None:
        with self.assertRaises(OSError) as _, TemporaryFile("rb") as fp:
            Store().dump(fp, mime_type="application/rdf+xml")

    def test_write_in_read(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, bar))
        store.add(Quad(foo, bar, baz))
        for triple in store:
            store.add(Quad(triple.object, triple.predicate, triple.subject))
        self.assertEqual(len(store), 4)

    def test_add_graph(self) -> None:
        store = Store()
        store.add_graph(graph)
        self.assertEqual(list(store.named_graphs()), [graph])

    def test_remove_graph(self) -> None:
        store = Store()
        store.add(Quad(foo, bar, baz, graph))
        store.add_graph(NamedNode("http://graph2"))
        store.remove_graph(graph)
        store.remove_graph(NamedNode("http://graph2"))
        self.assertEqual(list(store.named_graphs()), [])
        self.assertEqual(list(store), [])

    def test_read_only(self) -> None:
        quad = Quad(foo, bar, baz, graph)
        with TemporaryDirectory() as dir:
            store = Store(dir)
            store.add(quad)
            del store
            store = Store.read_only(dir)
            self.assertEqual(list(store), [quad])

    def test_secondary(self) -> None:
        quad = Quad(foo, bar, baz, graph)
        with TemporaryDirectory() as dir:
            store = Store(dir)
            store.add(quad)
            store.flush()

            secondary_store = Store.secondary(dir)
            self.assertEqual(list(secondary_store), [quad])

            store.remove(quad)
            store.flush()
            self.assertEqual(list(secondary_store), [])
            del secondary_store
            del store


if __name__ == "__main__":
    unittest.main()
