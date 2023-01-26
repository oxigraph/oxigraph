import unittest
from io import StringIO, BytesIO, UnsupportedOperation
from tempfile import NamedTemporaryFile, TemporaryFile

from pyoxigraph import *


EXAMPLE_TRIPLE = Triple(
    NamedNode("http://example.com/foo"), NamedNode("http://example.com/p"), Literal("1")
)
EXAMPLE_QUAD = Quad(
    NamedNode("http://example.com/foo"),
    NamedNode("http://example.com/p"),
    Literal("1"),
    NamedNode("http://example.com/g"),
)


class TestParse(unittest.TestCase):
    def test_parse_file(self) -> None:
        with NamedTemporaryFile() as fp:
            fp.write(b'<foo> <p> "1" .')
            fp.flush()
            self.assertEqual(
                list(parse(fp.name, "text/turtle", base_iri="http://example.com/")),
                [EXAMPLE_TRIPLE],
            )

    def test_parse_not_existing_file(self) -> None:
        with self.assertRaises(IOError) as _:
            parse("/tmp/not-existing-oxigraph-file.ttl", "text/turtle")

    def test_parse_str_io(self) -> None:
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

    def test_parse_bytes_io(self) -> None:
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

    def test_parse_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _:
            with TemporaryFile("wb") as fp:
                list(parse(fp, mime_type="application/n-triples"))

    def test_parse_quad(self) -> None:
        self.assertEqual(
            list(
                parse(
                    StringIO('<g> { <foo> <p> "1" }'),
                    "application/trig",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_QUAD],
        )


class TestSerialize(unittest.TestCase):
    def test_serialize_to_bytes_io(self) -> None:
        output = BytesIO()
        serialize([EXAMPLE_TRIPLE], output, "text/turtle")
        self.assertEqual(
            output.getvalue(),
            b'<http://example.com/foo> <http://example.com/p> "1" .\n',
        )

    def test_serialize_to_file(self) -> None:
        with NamedTemporaryFile() as fp:
            serialize([EXAMPLE_TRIPLE], fp.name, "text/turtle")
            self.assertEqual(
                fp.read(), b'<http://example.com/foo> <http://example.com/p> "1" .\n'
            )

    def test_serialize_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _:
            with TemporaryFile("rb") as fp:
                serialize([EXAMPLE_TRIPLE], fp, "text/turtle")

    def test_serialize_quad(self) -> None:
        output = BytesIO()
        serialize([EXAMPLE_QUAD], output, "application/trig")
        self.assertEqual(
            output.getvalue(),
            b'<http://example.com/g> { <http://example.com/foo> <http://example.com/p> "1" }\n',
        )
