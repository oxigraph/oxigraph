import unittest
from pyoxigraph import *

XSD_STRING = NamedNode("http://www.w3.org/2001/XMLSchema#string")
XSD_INTEGER = NamedNode("http://www.w3.org/2001/XMLSchema#integer")
RDF_LANG_STRING = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString")


class TestNamedNode(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(NamedNode("http://foo").value, "http://foo")

    def test_string(self):
        self.assertEqual(str(NamedNode("http://foo")), "<http://foo>")

    def test_equal(self):
        self.assertEqual(NamedNode("http://foo"), NamedNode("http://foo"))
        self.assertNotEqual(NamedNode("http://foo"), NamedNode("http://bar"))


class TestBlankNode(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(BlankNode("foo").value, "foo")
        self.assertNotEqual(BlankNode(), BlankNode())

    def test_string(self):
        self.assertEqual(str(BlankNode("foo")), "_:foo")

    def test_equal(self):
        self.assertEqual(BlankNode("foo"), BlankNode("foo"))
        self.assertNotEqual(BlankNode("foo"), BlankNode("bar"))
        self.assertNotEqual(BlankNode('foo'), NamedNode('http://foo'))
        self.assertNotEqual(NamedNode('http://foo'), BlankNode('foo'))


class TestLiteral(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(Literal("foo").value, "foo")
        self.assertEqual(Literal("foo").datatype, XSD_STRING)

        self.assertEqual(Literal("foo", language="en").value, "foo")
        self.assertEqual(Literal("foo", language="en").language, "en")
        self.assertEqual(Literal("foo", language="en").datatype, RDF_LANG_STRING)

        self.assertEqual(Literal("foo", datatype=XSD_INTEGER).value, "foo")
        self.assertEqual(Literal("foo", datatype=XSD_INTEGER).datatype, XSD_INTEGER)

    def test_string(self):
        self.assertEqual(str(Literal("foo")), '"foo"')
        self.assertEqual(str(Literal("foo", language="en")), '"foo"@en')
        self.assertEqual(
            str(Literal("foo", datatype=XSD_INTEGER)),
            '"foo"^^<http://www.w3.org/2001/XMLSchema#integer>',
        )

    def test_equals(self):
        self.assertEqual(Literal("foo", datatype=XSD_STRING), Literal("foo"))
        self.assertEqual(
            Literal("foo", language="en", datatype=RDF_LANG_STRING),
            Literal("foo", language="en"),
        )
        self.assertNotEqual(NamedNode('http://foo'), Literal('foo'))
        self.assertNotEqual(Literal('foo'), NamedNode('http://foo'))
        self.assertNotEqual(BlankNode('foo'), Literal('foo'))
        self.assertNotEqual(Literal('foo'), BlankNode('foo'))


class TestTriple(unittest.TestCase):
    def test_constructor(self):
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(t.subject, NamedNode("http://example.com/s"))
        self.assertEqual(t.predicate, NamedNode("http://example.com/p"))
        self.assertEqual(t.object, NamedNode("http://example.com/o"))

    def test_mapping(self):
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(t[0], NamedNode("http://example.com/s"))
        self.assertEqual(t[1], NamedNode("http://example.com/p"))
        self.assertEqual(t[2], NamedNode("http://example.com/o"))

    def test_destruct(self):
        (s, p, o) = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(s, NamedNode("http://example.com/s"))
        self.assertEqual(p, NamedNode("http://example.com/p"))
        self.assertEqual(o, NamedNode("http://example.com/o"))

    def test_string(self):
        self.assertEqual(
            str(
                Triple(
                    NamedNode("http://example.com/s"),
                    NamedNode("http://example.com/p"),
                    NamedNode("http://example.com/o"),
                )
            ),
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )


class TestQuad(unittest.TestCase):
    def test_constructor(self):
        t = Quad(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
            NamedNode("http://example.com/g"),
        )
        self.assertEqual(t.subject, NamedNode("http://example.com/s"))
        self.assertEqual(t.predicate, NamedNode("http://example.com/p"))
        self.assertEqual(t.object, NamedNode("http://example.com/o"))
        self.assertEqual(t.graph_name, NamedNode("http://example.com/g"))
        self.assertEqual(
            t.triple,
            Triple(
                NamedNode("http://example.com/s"),
                NamedNode("http://example.com/p"),
                NamedNode("http://example.com/o"),
            ),
        )
        self.assertEqual(
            Quad(
                NamedNode("http://example.com/s"),
                NamedNode("http://example.com/p"),
                NamedNode("http://example.com/o"),
            ),
            Quad(
                NamedNode("http://example.com/s"),
                NamedNode("http://example.com/p"),
                NamedNode("http://example.com/o"),
                DefaultGraph(),
            ),
        )

    def test_mapping(self):
        t = Quad(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
            NamedNode("http://example.com/g"),
        )
        self.assertEqual(t[0], NamedNode("http://example.com/s"))
        self.assertEqual(t[1], NamedNode("http://example.com/p"))
        self.assertEqual(t[2], NamedNode("http://example.com/o"))
        self.assertEqual(t[3], NamedNode("http://example.com/g"))

    def test_destruct(self):
        (s, p, o, g) = Quad(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
            NamedNode("http://example.com/g"),
        )
        self.assertEqual(s, NamedNode("http://example.com/s"))
        self.assertEqual(p, NamedNode("http://example.com/p"))
        self.assertEqual(o, NamedNode("http://example.com/o"))
        self.assertEqual(g, NamedNode("http://example.com/g"))

    def test_string(self):
        self.assertEqual(
            str(
                Triple(
                    NamedNode("http://example.com/s"),
                    NamedNode("http://example.com/p"),
                    NamedNode("http://example.com/o"),
                )
            ),
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )


class TestVariable(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(Variable("foo").value, "foo")

    def test_string(self):
        self.assertEqual(str(Variable("foo")), "?foo")

    def test_equal(self):
        self.assertEqual(Variable("foo"), Variable("foo"))
        self.assertNotEqual(Variable("foo"), Variable("bar"))


if __name__ == "__main__":
    unittest.main()
