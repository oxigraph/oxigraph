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
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-langdir-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/syntax#ntriples-langdir-2",
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
        &[
            // TODO: RDF 1.2
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-quads/syntax#nquads-base-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-quads/syntax#nquads-base-2",
        ],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn rdf12_n_triples_c14n_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/c14n/manifest.ttl",
        &["https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-n-triples/c14n#dirlangtagged_string"],
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
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-base-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-turtle/syntax#nt-ttl-base-2",
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
        &[
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-trig/syntax#trig-base-1",
            "https://w3c.github.io/rdf-tests/rdf/rdf12/rdf-trig/syntax#trig-base-2",
        ],
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

#[test]
fn jsonld_to_rdf_streaming_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld",
        &[
            // @context in @context
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tv006",
            // @set
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te047",
            // float exp notation
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0022",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0035",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te031",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te061",
            // @container
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te004",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te015",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te016",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te023",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te027",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te030",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te035",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te036",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te040",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te044",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te050",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter35",
            // @iri
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te005",
            // @id alias
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te006",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te051",
            // null in context
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te003",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te018",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te032",
            // @reverse
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0031",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te078",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te037",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te039",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te042",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te043",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te049",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te063",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te064",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te065",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te066",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te074",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0119",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter14",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter15",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter17",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter25",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter33",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter34",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter36",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter50",
            // @index
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter31",
            // relative IRI resolution discrepancies
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0122",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0123",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te062",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te091",
            // expandContext option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te077",
            // normative option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi12",
            // processingMode option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc029",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te075",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tep02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter21",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter42",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ttn01",
            // produceGeneralizedRdf option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0118",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te068",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te075",
            // rdfDirection option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi12",
            // specVersion option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0118",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0124",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#t0125",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc001",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc002",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc003",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc004",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc005",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc006",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc007",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc008",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc009",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc010",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc011",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc012",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc013",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc014",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc015",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc016",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc017",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc018",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc019",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc020",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc021",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc022",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc023",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc024",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc025",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc026",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc027",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc028",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc029",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc030",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc031",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc032",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc033",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc034",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tc035",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tdi12",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te014",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te026",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te038",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te071",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te079",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te080",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te081",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te082",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te083",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te084",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te085",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te086",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te087",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te092",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te093",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te094",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te095",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te096",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te097",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te098",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te099",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te100",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te101",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te102",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te103",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te104",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te105",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te106",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te107",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te108",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te110",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te111",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te112",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te114",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te115",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te116",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te117",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te118",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te119",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te120",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te121",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te122",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te123",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te126",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te127",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te128",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te129",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#te130",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter53",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tec01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tec02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tem01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ten06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tep02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tep03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter21",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter24",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter32",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter42",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter43",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter44",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter48",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ter49",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tin09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs12",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs13",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs14",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs15",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs16",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs17",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs18",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs19",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs20",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs21",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs22",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs23",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tli10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm001",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm002",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm003",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm004",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm005",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm006",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm007",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm008",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm009",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm010",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm011",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm012",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm013",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm014",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm015",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm016",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm017",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm018",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm019",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tm020",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn001",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn002",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn003",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn004",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn005",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn006",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn007",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tn008",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tp001",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tp002",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tp003",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tp004",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpi11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr12",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr13",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr14",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr15",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr16",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr17",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr18",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr19",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr20",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr21",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr22",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr23",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr24",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr25",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr26",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr27",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr28",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr29",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr30",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr31",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr32",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr33",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr34",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr35",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr36",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr37",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr38",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr39",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tpr40",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#trt01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso12",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tso13",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ttn01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#ttn02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#twf07",
            // useJCS option
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs01",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs02",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs03",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs04",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs05",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs06",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs07",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs08",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs09",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs10",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs11",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs12",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs13",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs14",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs15",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs16",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs17",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs18",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs19",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs20",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs21",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs22",
            "https://w3c.github.io/json-ld-streaming/tests/stream-toRdf-manifest.jsonld#tjs23",
        ],
    )
}
