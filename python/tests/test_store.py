import unittest
from abc import ABC, abstractmethod
from io import BytesIO

from pyoxigraph import *

foo = NamedNode("http://foo")
bar = NamedNode("http://bar")
baz = NamedNode("http://baz")
graph = NamedNode("http://graph")


class TestAbstractStore(unittest.TestCase, ABC):
    @abstractmethod
    def store(self):
        pass

    def test_add(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(len(store), 2)

    def test_remove(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        store.remove(Quad(foo, bar, baz))
        self.assertEqual(len(store), 1)

    def test_len(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(len(store), 2)

    def test_in(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertIn(Quad(foo, bar, baz), store)
        self.assertIn(Quad(foo, bar, baz, DefaultGraph()), store)
        self.assertIn(Quad(foo, bar, baz, graph), store)
        self.assertNotIn(Quad(foo, bar, baz, foo), store)

    def test_iter(self):
        store = self.store()
        store.add(Quad(foo, bar, baz, DefaultGraph()))
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(
            set(store),
            {Quad(foo, bar, baz, DefaultGraph()), Quad(foo, bar, baz, graph)},
        )

    def test_quads_for_pattern(self):
        store = self.store()
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

    def test_ask_query(self):
        store = self.store()
        store.add(Quad(foo, foo, foo))
        self.assertTrue(store.query("ASK { ?s ?s ?s }"))
        self.assertFalse(store.query("ASK { FILTER(false) }"))

    def test_construct_query(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
        self.assertIsInstance(results, QueryTriples)
        self.assertEqual(
            set(results), {Triple(foo, bar, baz)},
        )

    def test_select_query(self):
        store = self.store()
        store.add(Quad(foo, bar, baz))
        solutions = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }")
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
        s,o = solution
        self.assertEqual(s, foo)
        self.assertEqual(o, baz)

    def test_select_query_union_default_graph(self):
        store = self.store()
        store.add(Quad(foo, bar, baz, graph))
        self.assertEqual(len(list(store.query("SELECT ?s WHERE { ?s ?p ?o }"))), 0)
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

    def test_select_query_with_default_graph(self):
        store = self.store()
        graph_bnode = BlankNode("g")
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(foo, bar, foo))
        store.add(Quad(foo, bar, bar, graph_bnode))
        self.assertEqual(len(list(store.query("SELECT ?s WHERE { ?s ?p ?o }"))), 1)
        results = store.query("SELECT ?s WHERE { ?s ?p ?o }", default_graph=graph)
        self.assertEqual(len(list(results)), 1)
        results = store.query(
            "SELECT ?s WHERE { ?s ?p ?o }",
            default_graph=[DefaultGraph(), graph, graph_bnode],
        )
        self.assertEqual(len(list(results)), 3)

    def test_select_query_with_named_graph(self):
        store = self.store()
        graph_bnode = BlankNode("g")
        store.add(Quad(foo, bar, baz, graph))
        store.add(Quad(foo, bar, foo))
        store.add(Quad(foo, bar, bar, graph_bnode))
        store.add(Quad(foo, bar, bar, foo))
        results = store.query(
            "SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o } }",
            named_graphs=[graph, graph_bnode],
        )
        self.assertEqual(len(list(results)), 2)

    def test_update_insert_data(self):
        store = self.store()
        store.update('INSERT DATA { <http://foo> <http://foo> <http://foo> }')
        self.assertEqual(len(store), 1)

    def test_update_delete_data(self):
        store = self.store()
        store.add(Quad(foo, foo, foo))
        store.update('DELETE DATA { <http://foo> <http://foo> <http://foo> }')
        self.assertEqual(len(store), 0)

    def test_update_delete_where(self):
        store = self.store()
        store.add(Quad(foo, foo, foo))
        store.update('DELETE WHERE { ?v ?v ?v }')
        self.assertEqual(len(store), 0)

    def test_update_load(self):
        store = self.store()
        store.update('LOAD <https://www.w3.org/1999/02/22-rdf-syntax-ns>')
        self.assertGreater(len(store), 100)

    def test_load_ntriples_to_default_graph(self):
        store = self.store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> ."),
            mime_type="application/n-triples",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, DefaultGraph())})

    def test_load_ntriples_to_named_graph(self):
        store = self.store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> ."),
            mime_type="application/n-triples",
            to_graph=graph,
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_turtle_with_base_iri(self):
        store = self.store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <> ."),
            mime_type="text/turtle",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, DefaultGraph())})

    def test_load_nquads(self):
        store = self.store()
        store.load(
            BytesIO(b"<http://foo> <http://bar> <http://baz> <http://graph>."),
            mime_type="application/n-quads",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_load_trig_with_base_iri(self):
        store = self.store()
        store.load(
            BytesIO(b"<http://graph> { <http://foo> <http://bar> <> . }"),
            mime_type="application/trig",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {Quad(foo, bar, baz, graph)})

    def test_dump_ntriples(self):
        store = self.store()
        store.add(Quad(foo, bar, baz, graph))
        output = BytesIO()
        store.dump(output, "application/n-triples", from_graph=graph)
        self.assertEqual(
            output.getvalue(), b"<http://foo> <http://bar> <http://baz> .\n",
        )

    def test_dump_nquads(self):
        store = self.store()
        store.add(Quad(foo, bar, baz, graph))
        output = BytesIO()
        store.dump(output, "application/n-quads")
        self.assertEqual(
            output.getvalue(),
            b"<http://foo> <http://bar> <http://baz> <http://graph> .\n",
        )

    def test_write_in_read(self):
        store = self.store()
        store.add(Quad(foo, bar, bar))
        store.add(Quad(foo, bar, baz))
        for triple in store:
            store.add(Quad(triple.object, triple.predicate, triple.subject))
        self.assertEqual(len(store), 4)

    def test_add_graph(self):
        store = self.store()
        store.add_graph(graph)
        self.assertEqual(list(store.named_graphs()), [graph])

    def test_remove_graph(self):
        store = self.store()
        store.add(Quad(foo, bar, baz, graph))
        store.add_graph(NamedNode("http://graph2"))
        store.remove_graph(graph)
        store.remove_graph(NamedNode("http://graph2"))
        self.assertEqual(list(store.named_graphs()), [])
        self.assertEqual(list(store), [])


class TestMemoryStore(TestAbstractStore):
    def store(self):
        return MemoryStore()


class TestSledStore(TestAbstractStore):
    def store(self):
        return SledStore()


del TestAbstractStore  # We do not want to expose this class to the test runner

if __name__ == "__main__":
    unittest.main()
