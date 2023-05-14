use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn ntriples_w3c_testsuite() -> Result<()> {
    check_testsuite("http://w3c.github.io/rdf-tests/ntriples/manifest.ttl", &[])
}

#[test]
fn nquads_w3c_testsuite() -> Result<()> {
    check_testsuite("http://w3c.github.io/rdf-tests/nquads/manifest.ttl", &[])
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn turtle_w3c_testsuite() -> Result<()> {
    check_testsuite("http://w3c.github.io/rdf-tests/turtle/manifest.ttl", &[])
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn trig_w3c_testsuite() -> Result<()> {
    check_testsuite("http://w3c.github.io/rdf-tests/trig/manifest.ttl", &[])
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

#[test]
fn rdf_xml_w3c_testsuite() -> Result<()> {
    check_testsuite("http://www.w3.org/2013/RDFXMLTests/manifest.ttl", &[])
}

#[test]
fn ntriples_star_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-star/tests/nt/syntax/manifest.ttl",
        &[],
    )
}

#[test]
fn turtle_star_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-star/tests/turtle/syntax/manifest.ttl",
        &[],
    )
}

#[test]
fn turtle_star_eval_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-star/tests/turtle/eval/manifest.ttl",
        &[],
    )
}

#[test]
fn trig_star_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-star/tests/trig/syntax/manifest.ttl",
        &[],
    )
}

#[test]
fn trig_star_eval_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-star/tests/trig/eval/manifest.ttl",
        &[],
    )
}
