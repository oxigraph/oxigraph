import sys
import unittest
from io import BytesIO, StringIO, UnsupportedOperation
from tempfile import NamedTemporaryFile, TemporaryFile

from pyoxigraph import (
    Literal,
    NamedNode,
    Quad,
    QueryBoolean,
    QuerySolutions,
    parse,
    parse_query_results,
    serialize,
)

EXAMPLE_TRIPLE = Quad(
    NamedNode("http://example.com/foo"),
    NamedNode("http://example.com/p"),
    Literal("éù"),
)
EXAMPLE_QUAD = Quad(
    NamedNode("http://example.com/foo"),
    NamedNode("http://example.com/p"),
    Literal("1"),
    NamedNode("http://example.com/g"),
)


class TestParse(unittest.TestCase):
    def test_parse_file(self) -> None:
        with NamedTemporaryFile(suffix=".ttl") as fp:
            fp.write('<foo> <p> "éù" .'.encode())
            fp.flush()
            self.assertEqual(
                list(parse(fp.name, base_iri="http://example.com/")),
                [EXAMPLE_TRIPLE],
            )

    def test_parse_not_existing_file(self) -> None:
        with self.assertRaises(IOError) as _:
            parse("/tmp/not-existing-oxigraph-file.ttl", "text/turtle")

    def test_parse_str_io(self) -> None:
        self.assertEqual(
            list(
                parse(
                    StringIO('<foo> <p> "éù" .'),
                    "text/turtle",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_TRIPLE],
        )

    def test_parse_long_str_io(self) -> None:
        self.assertEqual(
            list(
                parse(
                    StringIO('<foo> <p> "éù" .\n' * 1024),
                    "text/turtle",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_TRIPLE] * 1024,
        )

    def test_parse_bytes_io(self) -> None:
        self.assertEqual(
            list(
                parse(
                    BytesIO('<foo> <p> "éù" .'.encode()),
                    "text/turtle",
                    base_iri="http://example.com/",
                )
            ),
            [EXAMPLE_TRIPLE],
        )

    def test_parse_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _, TemporaryFile("wb") as fp:
            list(parse(fp, "nt"))

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

    def test_parse_syntax_error(self) -> None:
        with NamedTemporaryFile() as fp:
            fp.write(b"@base <http://example.com/> .\n")
            fp.write(b'<foo> "p" "1"')
            fp.flush()
            with self.assertRaises(SyntaxError) as ctx:
                list(parse(fp.name, "text/turtle"))
            self.assertEqual(ctx.exception.filename, fp.name)
            self.assertEqual(ctx.exception.lineno, 2)
            self.assertEqual(ctx.exception.offset, 7)
            if sys.version_info >= (3, 10):
                self.assertEqual(ctx.exception.end_lineno, 2)
                self.assertEqual(ctx.exception.end_offset, 10)

    def test_parse_without_named_graphs(self) -> None:
        with self.assertRaises(SyntaxError) as _:
            list(
                parse(
                    StringIO('<g> { <foo> <p> "1" }'),
                    "application/trig",
                    base_iri="http://example.com/",
                    without_named_graphs=True,
                )
            )

    def test_parse_rename_blank_nodes(self) -> None:
        self.assertNotEqual(
            list(
                parse(
                    StringIO('_:s <http://example.com/p> "o" .'),
                    "application/n-triples",
                    rename_blank_nodes=True,
                )
            ),
            list(
                parse(
                    StringIO('_:s <http://example.com/p> "o" .'),
                    "application/n-triples",
                    rename_blank_nodes=True,
                )
            ),
        )


class TestSerialize(unittest.TestCase):
    def test_serialize_to_bytes(self) -> None:
        self.assertEqual(
            (serialize([EXAMPLE_TRIPLE.triple], None, "text/turtle") or b"").decode(),
            '<http://example.com/foo> <http://example.com/p> "éù" .\n',
        )

    def test_serialize_to_bytes_io(self) -> None:
        output = BytesIO()
        serialize([EXAMPLE_TRIPLE.triple], output, "text/turtle")
        self.assertEqual(
            output.getvalue().decode(),
            '<http://example.com/foo> <http://example.com/p> "éù" .\n',
        )

    def test_serialize_to_file(self) -> None:
        with NamedTemporaryFile(suffix=".ttl") as fp:
            serialize([EXAMPLE_TRIPLE], fp.name)
            self.assertEqual(
                fp.read().decode(),
                '<http://example.com/foo> <http://example.com/p> "éù" .\n',
            )

    def test_serialize_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _, TemporaryFile("rb") as fp:
            serialize([EXAMPLE_TRIPLE], fp, "text/turtle")

    def test_serialize_quad(self) -> None:
        output = BytesIO()
        serialize([EXAMPLE_QUAD], output, "application/trig")
        self.assertEqual(
            output.getvalue(),
            b'<http://example.com/g> {\n\t<http://example.com/foo> <http://example.com/p> "1" .\n}\n',
        )


class TestParseQuerySolutions(unittest.TestCase):
    def test_parse_file(self) -> None:
        with NamedTemporaryFile(suffix=".tsv") as fp:
            fp.write(
                b'?s\t?p\t?o\n<http://example.com/s>\t<http://example.com/s>\t"1"\n'
            )
            fp.flush()
            r = parse_query_results(fp.name)
            self.assertIsInstance(r, QuerySolutions)
            results = list(r)  # type: ignore[arg-type]
            self.assertEqual(results[0]["s"], NamedNode("http://example.com/s"))
            self.assertEqual(results[0][2], Literal("1"))

    def test_parse_not_existing_file(self) -> None:
        with self.assertRaises(IOError) as _:
            parse_query_results(
                "/tmp/not-existing-oxigraph-file.ttl", "application/json"
            )

    def test_parse_str_io(self) -> None:
        result = parse_query_results(StringIO("true"), "tsv")
        self.assertIsInstance(result, QueryBoolean)
        self.assertTrue(result)

    def test_parse_bytes_io(self) -> None:
        result = parse_query_results(BytesIO(b"false"), "tsv")
        self.assertIsInstance(result, QueryBoolean)
        self.assertFalse(result)

    def test_parse_io_error(self) -> None:
        with self.assertRaises(UnsupportedOperation) as _, TemporaryFile("wb") as fp:
            parse_query_results(fp, "srx")

    def test_parse_syntax_error_json(self) -> None:
        with NamedTemporaryFile() as fp:
            fp.write(b"{]")
            fp.flush()
            with self.assertRaises(SyntaxError) as ctx:
                list(parse_query_results(fp.name, "srj"))  # type: ignore[arg-type]
            self.assertEqual(ctx.exception.filename, fp.name)
            self.assertEqual(ctx.exception.lineno, 1)
            self.assertEqual(ctx.exception.offset, 2)
            if sys.version_info >= (3, 10):
                self.assertEqual(ctx.exception.end_lineno, 1)
                self.assertEqual(ctx.exception.end_offset, 3)

    def test_parse_syntax_error_tsv(self) -> None:
        with NamedTemporaryFile() as fp:
            fp.write(b"?a\t?test\n")
            fp.write(b"1\t<foo >\n")
            fp.flush()
            with self.assertRaises(SyntaxError) as ctx:
                list(parse_query_results(fp.name, "tsv"))  # type: ignore[arg-type]
            self.assertEqual(ctx.exception.filename, fp.name)
            self.assertEqual(ctx.exception.lineno, 2)
            self.assertEqual(ctx.exception.offset, 3)
            if sys.version_info >= (3, 10):
                self.assertEqual(ctx.exception.end_lineno, 2)
                self.assertEqual(ctx.exception.end_offset, 9)
