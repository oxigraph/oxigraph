import unittest
from io import StringIO, BytesIO, RawIOBase
from tempfile import NamedTemporaryFile

from pyoxigraph import *


EXAMPLE_TRIPLE = Triple(
    NamedNode("http://example.com/foo"), NamedNode("http://example.com/p"), Literal("1")
)


class TestParse(unittest.TestCase):
    def test_parse_file(self):
        with NamedTemporaryFile() as fp:
            fp.write(b'<foo> <p> "1" .')
            fp.flush()
            self.assertEqual(
                list(parse(fp.name, "text/turtle", base_iri="http://example.com/")),
                [EXAMPLE_TRIPLE],
            )

    def test_parse_not_existing_file(self):
        with self.assertRaises(IOError) as _:
            parse("/tmp/not-existing-oxigraph-file.ttl", "text/turtle")

    def test_parse_str_io(self):
        self.assertEqual(
            list(
                parse(
                    StringIO('<foo> <p> "1" .'),
                    "text/turtle",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_TRIPLE],
        )

    def test_parse_bytes_io(self):
        self.assertEqual(
            list(
                parse(
                    BytesIO(b'<foo> <p> "1" .'),
                    "text/turtle",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_TRIPLE],
        )

    def test_parse_io_error(self):
        class BadIO(RawIOBase):
            pass

        with self.assertRaises(NotImplementedError) as _:
            list(parse(BadIO(), mime_type="application/n-triples"))


class TestSerialize(unittest.TestCase):
    def test_serialize_to_bytes_io(self):
        output = BytesIO()
        serialize([EXAMPLE_TRIPLE], output, "text/turtle")
        self.assertEqual(
            output.getvalue(),
            b'<http://example.com/foo> <http://example.com/p> "1" .\n',
        )

    def test_serialize_to_file(self):
        with NamedTemporaryFile() as fp:
            serialize([EXAMPLE_TRIPLE], fp.name, "text/turtle")
            self.assertEqual(
                fp.read(), b'<http://example.com/foo> <http://example.com/p> "1" .\n'
            )
