import copy
import math
import pickle
import sys
import unittest

from pyoxigraph import (
    BaseDirection,
    BlankNode,
    DefaultGraph,
    Literal,
    NamedNode,
    Quad,
    Triple,
    Variable,
)

XSD_STRING = NamedNode("http://www.w3.org/2001/XMLSchema#string")
XSD_INTEGER = NamedNode("http://www.w3.org/2001/XMLSchema#integer")
XSD_DOUBLE = NamedNode("http://www.w3.org/2001/XMLSchema#double")
XSD_BOOLEAN = NamedNode("http://www.w3.org/2001/XMLSchema#boolean")
RDF_LANG_STRING = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString")
RDF_DIR_LANG_STRING = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#dirLangString")


def match_works(test: unittest.TestCase, matched_value: str, constraint: str) -> None:
    """Hack for Python < 3.10 compatibility"""
    if sys.version_info < (3, 10):
        return test.skipTest("match has been introduced by Python 3.10")
    found = True
    exec(
        f"""
match {matched_value}:
    case {constraint}:
        found = True
"""
    )
    test.assertTrue(found)
    return None


class TestNamedNode(unittest.TestCase):
    def test_constructor(self) -> None:
        self.assertEqual(NamedNode("http://foo").value, "http://foo")

    def test_string(self) -> None:
        self.assertEqual(str(NamedNode("http://foo")), "<http://foo>")

    def test_equal(self) -> None:
        self.assertEqual(NamedNode("http://foo"), NamedNode("http://foo"))
        self.assertNotEqual(NamedNode("http://foo"), NamedNode("http://bar"))

    def test_pickle(self) -> None:
        node = NamedNode("http://foo")
        self.assertEqual(pickle.loads(pickle.dumps(node)), node)
        self.assertEqual(copy.copy(node), node)
        self.assertEqual(copy.deepcopy(node), node)

    def test_basic_match(self) -> None:
        match_works(self, 'NamedNode("http://foo")', 'NamedNode("http://foo")')

    def test_wildcard_match(self) -> None:
        match_works(self, 'NamedNode("http://foo")', "NamedNode(x)")


class TestBlankNode(unittest.TestCase):
    def test_constructor(self) -> None:
        self.assertEqual(BlankNode("foo").value, "foo")
        self.assertNotEqual(BlankNode(), BlankNode())

    def test_string(self) -> None:
        self.assertEqual(str(BlankNode("foo")), "_:foo")

    def test_equal(self) -> None:
        self.assertEqual(BlankNode("foo"), BlankNode("foo"))
        self.assertNotEqual(BlankNode("foo"), BlankNode("bar"))
        self.assertNotEqual(BlankNode("foo"), NamedNode("http://foo"))
        self.assertNotEqual(NamedNode("http://foo"), BlankNode("foo"))

    def test_pickle(self) -> None:
        node = BlankNode("foo")
        self.assertEqual(pickle.loads(pickle.dumps(node)), node)
        self.assertEqual(copy.copy(node), node)
        self.assertEqual(copy.deepcopy(node), node)

        auto = BlankNode()
        self.assertEqual(pickle.loads(pickle.dumps(auto)), auto)
        self.assertEqual(copy.copy(auto), auto)
        self.assertEqual(copy.deepcopy(auto), auto)

    def test_basic_match(self) -> None:
        match_works(self, 'BlankNode("foo")', 'BlankNode("foo")')

    def test_wildcard_match(self) -> None:
        match_works(self, 'BlankNode("foo")', "BlankNode(x)")


class TestLiteral(unittest.TestCase):
    def test_constructor(self) -> None:
        self.assertEqual(Literal("foo").value, "foo")
        self.assertEqual(Literal("foo").datatype, XSD_STRING)

        self.assertEqual(Literal("foo", language="en").value, "foo")
        self.assertEqual(Literal("foo", language="en").language, "en")
        self.assertEqual(Literal("foo", language="en").datatype, RDF_LANG_STRING)

        self.assertEqual(Literal("foo", language="en", direction=BaseDirection.LTR).value, "foo")
        self.assertEqual(Literal("foo", language="en", direction=BaseDirection.LTR).language, "en")
        self.assertEqual(Literal("foo", language="en", direction=BaseDirection.LTR).direction, BaseDirection.LTR)
        self.assertEqual(Literal("foo", language="en", direction=BaseDirection.LTR).datatype, RDF_DIR_LANG_STRING)

        self.assertEqual(Literal("foo", datatype=XSD_INTEGER).value, "foo")
        self.assertEqual(Literal("foo", datatype=XSD_INTEGER).datatype, XSD_INTEGER)

        self.assertEqual(Literal(True), Literal("true", datatype=XSD_BOOLEAN))  # noqa: FBT003
        self.assertEqual(Literal(False), Literal("false", datatype=XSD_BOOLEAN))  # noqa: FBT003

        self.assertEqual(Literal(0), Literal("0", datatype=XSD_INTEGER))
        self.assertEqual(Literal(1), Literal("1", datatype=XSD_INTEGER))

        self.assertEqual(Literal(0.1), Literal("0.1", datatype=XSD_DOUBLE))
        self.assertEqual(Literal(math.inf), Literal("INF", datatype=XSD_DOUBLE))
        self.assertEqual(Literal(-math.inf), Literal("-INF", datatype=XSD_DOUBLE))
        self.assertEqual(Literal(-math.nan), Literal("NaN", datatype=XSD_DOUBLE))

    def test_string(self) -> None:
        self.assertEqual(str(Literal("foo")), '"foo"')
        self.assertEqual(str(Literal("foo", language="en")), '"foo"@en')
        self.assertEqual(str(Literal("foo", language="en", direction=BaseDirection.RTL)), '"foo"@en--rtl')
        self.assertEqual(
            str(Literal("foo", datatype=XSD_INTEGER)),
            '"foo"^^<http://www.w3.org/2001/XMLSchema#integer>',
        )

    def test_equals(self) -> None:
        self.assertEqual(Literal("foo", datatype=XSD_STRING), Literal("foo"))
        self.assertEqual(
            Literal("foo", language="en", datatype=RDF_LANG_STRING),
            Literal("foo", language="en"),
        )
        self.assertEqual(
            Literal("foo", language="en", direction=BaseDirection.RTL, datatype=RDF_DIR_LANG_STRING),
            Literal("foo", language="en", direction=BaseDirection.RTL),
        )
        self.assertNotEqual(NamedNode("http://foo"), Literal("foo"))
        self.assertNotEqual(Literal("foo"), NamedNode("http://foo"))
        self.assertNotEqual(BlankNode("foo"), Literal("foo"))
        self.assertNotEqual(Literal("foo"), BlankNode("foo"))

    def test_pickle(self) -> None:
        simple = Literal("foo")
        self.assertEqual(pickle.loads(pickle.dumps(simple)), simple)
        self.assertEqual(copy.copy(simple), simple)
        self.assertEqual(copy.deepcopy(simple), simple)

        lang_tagged = Literal("foo", language="en")
        self.assertEqual(pickle.loads(pickle.dumps(lang_tagged)), lang_tagged)
        self.assertEqual(copy.copy(lang_tagged), lang_tagged)
        self.assertEqual(copy.deepcopy(lang_tagged), lang_tagged)

        dir_lang_tagged = Literal("foo", language="en", direction=BaseDirection.RTL)
        self.assertEqual(pickle.loads(pickle.dumps(dir_lang_tagged)), dir_lang_tagged)
        self.assertEqual(copy.copy(dir_lang_tagged), dir_lang_tagged)
        self.assertEqual(copy.deepcopy(dir_lang_tagged), dir_lang_tagged)

        number = Literal("1", datatype=XSD_INTEGER)
        self.assertEqual(pickle.loads(pickle.dumps(number)), number)
        self.assertEqual(copy.copy(number), number)
        self.assertEqual(copy.deepcopy(number), number)

    def test_basic_match(self) -> None:
        match_works(self, 'Literal("foo", language="en")', 'Literal("foo", language="en")')
        match_works(
            self,
            'Literal("foo", language="en", direction=BaseDirection.RTL)',
            'Literal("foo", language="en", direction=BaseDirection.RTL)',
        )
        match_works(
            self,
            'Literal("1", datatype=XSD_INTEGER)',
            'Literal("1", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))',
        )

    def test_wildcard_match(self) -> None:
        match_works(self, 'Literal("foo", language="en")', "Literal(v, language=l)")
        match_works(
            self, 'Literal("foo", language="en", direction=BaseDirection.RTL)', "Literal(v, language=l, direction=d)"
        )
        match_works(self, 'Literal("1", datatype=XSD_INTEGER)', "Literal(v, datatype=d)")


class TestBaseDirection(unittest.TestCase):
    def test_constructor(self) -> None:
        self.assertEqual(BaseDirection("ltr"), BaseDirection.LTR)
        self.assertEqual(BaseDirection("rtl"), BaseDirection.RTL)

    def test_string(self) -> None:
        self.assertEqual(str(BaseDirection("ltr")), "ltr")

    def test_equal(self) -> None:
        self.assertEqual(BaseDirection("rtl"), BaseDirection("rtl"))
        self.assertNotEqual(BaseDirection("rtl"), BaseDirection("ltr"))

    def test_pickle(self) -> None:
        v = BaseDirection("rtl")
        self.assertEqual(pickle.loads(pickle.dumps(v)), v)
        self.assertEqual(copy.copy(v), v)
        self.assertEqual(copy.deepcopy(v), v)

    def test_basic_match(self) -> None:
        match_works(self, 'BaseDirection("rtl")', 'BaseDirection("rtl")')

    def test_wildcard_match(self) -> None:
        match_works(self, 'BaseDirection("rtl")', "BaseDirection(x)")


class TestTriple(unittest.TestCase):
    def test_constructor(self) -> None:
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(t.subject, NamedNode("http://example.com/s"))
        self.assertEqual(t.predicate, NamedNode("http://example.com/p"))
        self.assertEqual(t.object, NamedNode("http://example.com/o"))

    def test_rdf_star_constructor(self) -> None:
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            Triple(
                NamedNode("http://example.com/os"),
                NamedNode("http://example.com/op"),
                NamedNode("http://example.com/oo"),
            ),
        )
        self.assertEqual(t.subject, NamedNode("http://example.com/s"))
        self.assertEqual(t.predicate, NamedNode("http://example.com/p"))
        self.assertEqual(
            t.object,
            Triple(
                NamedNode("http://example.com/os"),
                NamedNode("http://example.com/op"),
                NamedNode("http://example.com/oo"),
            ),
        )

    def test_mapping(self) -> None:
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(t[0], NamedNode("http://example.com/s"))
        self.assertEqual(t[1], NamedNode("http://example.com/p"))
        self.assertEqual(t[2], NamedNode("http://example.com/o"))

    def test_destruct(self) -> None:
        (s, p, o) = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(s, NamedNode("http://example.com/s"))
        self.assertEqual(p, NamedNode("http://example.com/p"))
        self.assertEqual(o, NamedNode("http://example.com/o"))

    def test_string(self) -> None:
        self.assertEqual(
            str(
                Triple(
                    NamedNode("http://example.com/s"),
                    NamedNode("http://example.com/p"),
                    NamedNode("http://example.com/o"),
                )
            ),
            "<http://example.com/s> <http://example.com/p> <http://example.com/o>",
        )

    def test_equals(self) -> None:
        t = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(
            t,
            Triple(
                NamedNode("http://example.com/s"),
                NamedNode("http://example.com/p"),
                NamedNode("http://example.com/o"),
            ),
        )
        self.assertNotEqual(
            t,
            Triple(
                NamedNode("http://example.com/s"),
                NamedNode("http://example.com/p"),
                NamedNode("http://example.com/s"),
            ),
        )
        self.assertNotEqual(t, NamedNode("http://foo"))
        self.assertNotEqual(t, BlankNode("foo"))
        self.assertNotEqual(t, Literal("foo"))
        self.assertNotEqual(t, DefaultGraph())
        self.assertNotEqual(NamedNode("http://foo"), t)
        self.assertNotEqual(BlankNode("foo"), t)
        self.assertNotEqual(Literal("foo"), t)
        self.assertNotEqual(DefaultGraph(), t)

    def test_pickle(self) -> None:
        triple = Triple(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
        )
        self.assertEqual(pickle.loads(pickle.dumps(triple)), triple)
        self.assertEqual(copy.copy(triple), triple)
        self.assertEqual(copy.deepcopy(triple), triple)

    def test_match(self) -> None:
        match_works(
            self,
            'Triple(NamedNode("http://example.com/s"), NamedNode("http://example.com/p"), '
            'NamedNode("http://example.com/o"))',
            'Triple(NamedNode("http://example.com/s"), NamedNode(p), o)',
        )


class TestDefaultGraph(unittest.TestCase):
    def test_equal(self) -> None:
        self.assertEqual(DefaultGraph(), DefaultGraph())
        self.assertNotEqual(DefaultGraph(), NamedNode("http://bar"))

    def test_pickle(self) -> None:
        self.assertEqual(pickle.loads(pickle.dumps(DefaultGraph())), DefaultGraph())
        self.assertEqual(copy.copy(DefaultGraph()), DefaultGraph())
        self.assertEqual(copy.deepcopy(DefaultGraph()), DefaultGraph())

    def test_match(self) -> None:
        match_works(self, "DefaultGraph()", "DefaultGraph()")


class TestQuad(unittest.TestCase):
    def test_constructor(self) -> None:
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

    def test_mapping(self) -> None:
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

    def test_destruct(self) -> None:
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

    def test_string(self) -> None:
        self.assertEqual(
            str(
                Triple(
                    NamedNode("http://example.com/s"),
                    NamedNode("http://example.com/p"),
                    NamedNode("http://example.com/o"),
                )
            ),
            "<http://example.com/s> <http://example.com/p> <http://example.com/o>",
        )

    def test_pickle(self) -> None:
        quad = Quad(
            NamedNode("http://example.com/s"),
            NamedNode("http://example.com/p"),
            NamedNode("http://example.com/o"),
            NamedNode("http://example.com/g"),
        )
        self.assertEqual(pickle.loads(pickle.dumps(quad)), quad)
        self.assertEqual(copy.copy(quad), quad)
        self.assertEqual(copy.deepcopy(quad), quad)

    def test_match(self) -> None:
        match_works(
            self,
            'Quad(NamedNode("http://example.com/s"), NamedNode("http://example.com/p"), '
            'NamedNode("http://example.com/o"), NamedNode("http://example.com/g"))',
            'Quad(NamedNode("http://example.com/s"), NamedNode(p), o, NamedNode("http://example.com/g"))',
        )


class TestVariable(unittest.TestCase):
    def test_constructor(self) -> None:
        self.assertEqual(Variable("foo").value, "foo")

    def test_string(self) -> None:
        self.assertEqual(str(Variable("foo")), "?foo")

    def test_equal(self) -> None:
        self.assertEqual(Variable("foo"), Variable("foo"))
        self.assertNotEqual(Variable("foo"), Variable("bar"))

    def test_pickle(self) -> None:
        v = Variable("foo")
        self.assertEqual(pickle.loads(pickle.dumps(v)), v)
        self.assertEqual(copy.copy(v), v)
        self.assertEqual(copy.deepcopy(v), v)

    def test_basic_match(self) -> None:
        match_works(self, 'Variable("foo")', 'Variable("foo")')

    def test_wildcard_match(self) -> None:
        match_works(self, 'Variable("foo")', "Variable(x)")


if __name__ == "__main__":
    unittest.main()
