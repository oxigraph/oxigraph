import unittest
from abc import ABC, abstractmethod

from oxigraph import *

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
        store.add((foo, bar, baz))
        store.add((foo, bar, baz, DefaultGraph()))
        store.add((foo, bar, baz, graph))
        self.assertEqual(len(store), 2)

    def test_remove(self):
        store = self.store()
        store.add((foo, bar, baz))
        store.add((foo, bar, baz, DefaultGraph()))
        store.add((foo, bar, baz, graph))
        store.remove((foo, bar, baz))
        self.assertEqual(len(store), 1)

    def test_len(self):
        store = self.store()
        store.add((foo, bar, baz))
        store.add((foo, bar, baz, graph))
        self.assertEqual(len(store), 2)

    def test_in(self):
        store = self.store()
        store.add((foo, bar, baz))
        store.add((foo, bar, baz, DefaultGraph()))
        store.add((foo, bar, baz, graph))
        self.assertTrue((foo, bar, baz) in store)
        self.assertTrue((foo, bar, baz, DefaultGraph()) in store)
        self.assertTrue((foo, bar, baz, graph) in store)
        self.assertTrue((foo, bar, baz, foo) not in store)

    def test_iter(self):
        store = self.store()
        store.add((foo, bar, baz, DefaultGraph()))
        store.add((foo, bar, baz, graph))
        self.assertEqual(
            set(store), {(foo, bar, baz, DefaultGraph()), (foo, bar, baz, graph)}
        )

    def test_match(self):
        store = self.store()
        store.add((foo, bar, baz, DefaultGraph()))
        store.add((foo, bar, baz, graph))
        self.assertEqual(
            set(store.match(None, None, None)),
            {(foo, bar, baz, DefaultGraph()), (foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.match(foo, None, None)),
            {(foo, bar, baz, DefaultGraph()), (foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.match(None, None, None, graph)), {(foo, bar, baz, graph)},
        )
        self.assertEqual(
            set(store.match(foo, None, None, DefaultGraph())),
            {(foo, bar, baz, DefaultGraph())},
        )

    def test_ask_query(self):
        store = self.store()
        store.add((foo, foo, foo))
        self.assertTrue(store.query("ASK { ?s ?s ?s }"))
        self.assertFalse(store.query("ASK { FILTER(false) }"))

    def test_construct_query(self):
        store = self.store()
        store.add((foo, bar, baz))
        self.assertEqual(
            set(store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")),
            {(foo, bar, baz)},
        )

    def test_select_query(self):
        store = self.store()
        store.add((foo, bar, baz))
        results = list(store.query("SELECT ?s WHERE { ?s ?p ?o }"))
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][0], foo)
        self.assertEqual(results[0]["s"], foo)

    def test_load_ntriples_to_default_graph(self):
        store = self.store()
        store.load(
            "<http://foo> <http://bar> <http://baz> .",
            mime_type="application/n-triples",
        )
        self.assertEqual(set(store), {(foo, bar, baz, DefaultGraph())})

    def test_load_ntriples_to_named_graph(self):
        store = self.store()
        store.load(
            "<http://foo> <http://bar> <http://baz> .",
            mime_type="application/n-triples",
            to_graph=graph,
        )
        self.assertEqual(set(store), {(foo, bar, baz, graph)})

    def test_load_turtle_with_base_iri(self):
        store = self.store()
        store.load(
            "<http://foo> <http://bar> <> .",
            mime_type="text/turtle",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {(foo, bar, baz, DefaultGraph())})

    def test_load_nquads(self):
        store = self.store()
        store.load(
            "<http://foo> <http://bar> <http://baz> <http://graph>.",
            mime_type="application/n-quads",
        )
        self.assertEqual(set(store), {(foo, bar, baz, graph)})

    def test_load_trig_with_base_iri(self):
        store = self.store()
        store.load(
            "<http://graph> { <http://foo> <http://bar> <> . }",
            mime_type="application/trig",
            base_iri="http://baz",
        )
        self.assertEqual(set(store), {(foo, bar, baz, graph)})


class TestMemoryStore(TestAbstractStore):
    def store(self):
        return MemoryStore()


class TestSledStore(TestAbstractStore):
    def store(self):
        return SledStore()


del TestAbstractStore  # We do not want to expose this class to the test runner

if __name__ == "__main__":
    unittest.main()
