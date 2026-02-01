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

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_n_triples_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax/manifest.ttl",
        &[],
    )
}
#[cfg(all(feature = "rdf-12", not(windows)))] // Tests don't like git auto "\r\n" on Windows
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

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_n_quads_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-quads/syntax/manifest.ttl",
        &[],
    )
}

#[cfg(all(feature = "rdf-12", not(windows)))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn rdf12_n_quads_c14n_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-quads/c14n/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf11_turtle_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-turtle/manifest.ttl",
        &[],
    )
}

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_turtle_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax/manifest.ttl",
        &[],
    )
}

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_turtle_eval_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/eval/manifest.ttl",
        &[],
    )
}

#[test]
fn rdf11_trig_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf11/rdf-trig/manifest.ttl",
        &[],
    )
}

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_trig_syntax_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-trig/syntax/manifest.ttl",
        &[],
    )
}

#[cfg(feature = "rdf-12")]
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

#[cfg(feature = "rdf-12")]
#[test]
fn rdf12_xml_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-xml/manifest.ttl",
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

#[test]
fn jsonld_to_rdf_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/json-ld-api/tests/toRdf-manifest.jsonld",
        &[
            // relative IRI resolution discrepancies
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#t0122",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#t0123",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te062",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te091",
            // Weird @base IRI support
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tli12",
            // expandContext
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te077",
            // produceGeneralizedRdf
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#t0118",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te075",
            // we always emit base direction when targeting RDF 1.2
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi02",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi04",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi05",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi06",
            // non-normative - rdfDirection
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi09",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi10",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi11",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tdi12",
            // Scoped contexts somehow propagate to elements inside of containers?
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tc013",
            // useJCS
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#tjs12",
            // specVersion json-ld-1.0
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te026",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te071",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te115",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#te116",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#ter02",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#ter03",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#ter24",
            "https://w3c.github.io/json-ld-api/tests/toRdf-manifest#ter32",
        ],
    )
}

#[test]
fn jsonld_to_rdf_streaming_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld",
        &[
            // We do not allow root @graph followed with other keys
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tv017",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tv019",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tv021",
            // relative IRI resolution discrepancies
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#t0122",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#t0123",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te062",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te091",
            // expandContext option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te077",
            // normative option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi12",
            // produceGeneralizedRdf option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#t0118",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te068",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te075",
            // rdfDirection option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi12",
            // we always emit base direction when targeting RDF 1.2
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi02",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi04",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi05",
            #[cfg(feature = "rdf-12")]
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tdi06",
            // specVersion json-ld-1.0
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te026",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te071",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te115",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te116",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#ter02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#ter03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#ter24",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#ter32",
            // Scoped contexts somehow propagate to elements inside of containers?
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tc013",
            // useJCS
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tjs12",
            // something is before @type
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te038",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#te014",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tin06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest#tn008",
        ],
    )
}
