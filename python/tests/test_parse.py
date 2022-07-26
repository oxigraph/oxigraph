import unittest
import io

from pyoxigraph import *


class TestParse(unittest.TestCase):
    def test_parse(self):
        input = io.BytesIO(b'<foo> <p> "1" .')
        result = list(parse(input, "text/turtle", base_iri="http://example.com/"))

        assert result == [
            Triple(
                NamedNode("http://example.com/foo"),
                NamedNode("http://example.com/p"),
                Literal(
                    "1", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#string")
                ),
            )
        ]
