//! Utilities to read and write RDF results formats  using [sparesults](https://crates.io/crates/sparesults).
//!
//! It supports [SPARQL Query Results XML Format (Second Edition)](https://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).
//!
//! Usage example converting a JSON result file into a TSV result file:
//!
//! ```
//! use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput, QueryResultsSerializer};
//! use std::io::Result;
//!
//! fn convert_json_to_tsv(json_file: &[u8]) -> Result<Vec<u8>> {
//!     let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
//!     let tsv_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Tsv);
//!     // We start to read the JSON file and see which kind of results it is
//!     match json_parser.for_reader(json_file)? {
//!         ReaderQueryResultsParserOutput::Boolean(value) => {
//!             // it's a boolean result, we copy it in TSV to the output buffer
//!             tsv_serializer.serialize_boolean_to_writer(Vec::new(), value)
//!         }
//!         ReaderQueryResultsParserOutput::Solutions(solutions_reader) => {
//!             // it's a set of solutions, we create a writer and we write to it while reading in streaming from the JSON file
//!             let mut tsv_solutions_serializer = tsv_serializer.serialize_solutions_to_writer(Vec::new(), solutions_reader.variables().to_vec())?;
//!             for solution in solutions_reader {
//!                 tsv_solutions_serializer.serialize(&solution?)?;
//!             }
//!             tsv_solutions_serializer.finish()
//!         }
//!     }
//! }
//!
//! // Let's test with a boolean
//! assert_eq!(
//!     convert_json_to_tsv(br#"{"boolean":true}"#.as_slice()).unwrap(),
//!     b"true"
//! );
//!
//! // And with a set of solutions
//! assert_eq!(
//!     convert_json_to_tsv(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice()).unwrap(),
//!     b"?foo\t?bar\n\"test\"\t\n"
//! );
//! ```

pub use sparesults::*;
