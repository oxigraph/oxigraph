#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn rdf11_n_triples_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-n-triples/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf12_n_triples_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax/manifest.ttl",
        &[
            // TODO: RDF 1.2
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-01",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-02",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-03",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-bnode-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-nested-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-bad-09",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-bad-reified-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-bad-reified-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-star-bad-reified-3",
        ],
    )
}

#[test]
fn rdf12_n_quads_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-quads/syntax/manifest.ttl",
        &[],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn rdf12_n_triples_c14n_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/c14n/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf11_n_quads_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-n-quads/manifest.ttl",
        &[],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn rdf11_turtle_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-turtle/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf12_turtle_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax/manifest.ttl",
        &[
            // TODO: RDF 1.2
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#turtle-star-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#turtle-star-4",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#turtle-star-inside-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#turtle-star-inside-4",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#turtle-star-ann-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-star-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-star-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-star-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-star-bnode-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-star-nested-1",
        ],
    )
}

#[test]
fn rdf12_turtle_eval_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval/manifest.ttl",
        &[
            // TODO RDF 1.2
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-4",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-bnode-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-bnode-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-3",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-4",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-5",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-6",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-7",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-annotation-8",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-quoted-annotation-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-quoted-annotation-2",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval#turtle-star-quoted-annotation-3",
        ],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn rdf11_trig_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-trig/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf12_trig_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-trig/syntax/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf12_trig_eval_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-trig/eval/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf11_xml_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-xml/manifest.ttl",
        &[],
    )
}

#[test]
fn n3_parser_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/N3/tests/N3Tests/manifest-parser.ttl",
        &[],
    )
}
#[test]
fn n3_extended_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/N3/tests/N3Tests/manifest-extended.ttl",
        &[],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn n3_turtle_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/N3/tests/TurtleTests/manifest.ttl",
        &[],
    )
}
