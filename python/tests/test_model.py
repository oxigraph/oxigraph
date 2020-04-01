import unittest
from oxigraph import *

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
        # TODO self.assertNotEqual(BlankNode('foo'), NamedNode('http://foo'))
        # TODO self.assertNotEqual(NamedNode('http://foo'), BlankNode('foo'))


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
        # TODO self.assertNotEqual(NamedNode('http://foo'), Literal('foo'))
        # TODO self.assertNotEqual(Literal('foo'), NamedNode('http://foo'))
        # TODO self.assertNotEqual(BlankNode('foo'), Literal('foo'))
        # TODO self.assertNotEqual(Literal('foo'), BlankNode('foo'))


if __name__ == "__main__":
    unittest.main()
