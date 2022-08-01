import unittest
import io

from pyoxigraph import *


class TestParse(unittest.TestCase):
    def test_parse(self):
        input = io.BytesIO(b'<foo> <p> "1" .')
        result = list(parse(input, "text/turtle", base_iri="http://example.com/"))

        self.assertEqual(
            result,
            [
                Triple(
                    NamedNode("http://example.com/foo"),
                    NamedNode("http://example.com/p"),
                    Literal(
                        "1",
                        datatype=NamedNode("http://www.w3.org/2001/XMLSchema#string"),
                    ),
                )
            ],
        )


class TestSerialize(unittest.TestCase):
    def test_serialize(self):
        output = io.BytesIO()
        serialize(
            [
                Triple(
                    NamedNode("http://example.com"),
                    NamedNode("http://example.com/p"),
                    Literal("1"),
                )
            ],
            output,
            "text/turtle",
        )

        self.assertEqual(
            output.getvalue(), b'<http://example.com> <http://example.com/p> "1" .\n'
        )
