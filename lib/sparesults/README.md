Sparesults
==========

[![Latest Version](https://img.shields.io/crates/v/sparesults.svg)](https://crates.io/crates/sparesults)
[![Released API docs](https://docs.rs/sparesults/badge.svg)](https://docs.rs/sparesults)
[![Crates.io downloads](https://img.shields.io/crates/d/sparesults)](https://crates.io/crates/sparesults)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Sparesults is a set of parsers and serializers for [SPARQL](https://www.w3.org/TR/sparql11-overview/) query results formats.

It supports [SPARQL Query Results XML Format (Second Edition)](http://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/2013/REC-sparql11-results-csv-tsv-20130321/).

Support for [SPARQL-star](https://w3c.github.io/rdf-star/cg-spec/#query-result-formats) is also available behind the `rdf-star` feature.

This crate is intended to be a building piece for SPARQL client and server implementations in Rust like [Oxigraph](https://oxigraph.org).

Usage example converting a JSON result file into a TSV result file:

```rust
use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader, QueryResultsSerializer};
use std::io::Result;

fn convert_json_to_tsv(json_file: &[u8]) -> Result<Vec<u8>> {
    let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
    let tsv_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Tsv);
    // We start to read the JSON file and see which kind of results it is
    match json_parser.read_results(json_file)? {
        QueryResultsReader::Boolean(value) => {
            // it's a boolean result, we copy it in TSV to the output buffer
            tsv_serializer.write_boolean_result(Vec::new(), value)
        },
        QueryResultsReader::Solutions(solutions_reader) => {
            // it's a set of solutions, we create a writer and we write to it while reading in streaming from the JSON file
            let mut solutions_writer = tsv_serializer.solutions_writer(Vec::new(), solutions_reader.variables().to_vec())?;
            for solution in solutions_reader {
                solutions_writer.write(&solution?)?;
            }
            solutions_writer.finish()
        }
    }
}

// Let's test with a boolean
assert_eq!(
    convert_json_to_tsv(b"{\"boolean\":true}".as_slice()).unwrap(),
    b"true"
);

// And with a set of solutions
assert_eq!(
    convert_json_to_tsv(b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}".as_slice()).unwrap(),
    b"?foo\t?bar\n\"test\"\t"
);
```

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
