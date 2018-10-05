import unittest
from rudf import *

XSD_STRING = NamedNode('http://www.w3.org/2001/XMLSchema#string')
XSD_INTEGER = NamedNode('http://www.w3.org/2001/XMLSchema#integer')
RDF_LANG_STRING = NamedNode('http://www.w3.org/1999/02/22-rdf-syntax-ns#langString')


class TestNamedNode(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(NamedNode('http://foo').value(), 'http://foo/')


class TestBlankNode(unittest.TestCase):
    def test_constructor(self):
        self.assertNotEqual(BlankNode(), BlankNode())


class TestLiteral(unittest.TestCase):
    def test_constructor(self):
        self.assertEqual(Literal('foo').value(), 'foo')
        self.assertEqual(Literal('foo').datatype(), XSD_STRING)

        self.assertEqual(Literal('foo', 'en').value(), 'foo')
        self.assertEqual(Literal('foo', 'en').language(), 'en')
        self.assertEqual(Literal('foo', 'en').datatype(), RDF_LANG_STRING)

        self.assertEqual(Literal('foo', datatype=XSD_INTEGER).value(), 'foo')
        self.assertEqual(Literal('foo', datatype=XSD_INTEGER).datatype(), XSD_INTEGER)


if __name__ == '__main__':
    unittest.main()
