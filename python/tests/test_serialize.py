import unittest
import io

from pyoxigraph import *


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

        assert (
            output.getvalue() == b'<http://example.com> <http://example.com/p> "1" .\n'
        )
