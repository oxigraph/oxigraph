Oxigraph is an RDF and SPARQL database implementation organized as a Rust workspace.

## Specifications

When behavior is unclear, consult the specification relevant to the crate being modified:

- `oxrdf`: https://www.w3.org/TR/rdf12-concepts/ and https://www.w3.org/TR/rdf-canon/
- `oxttl`: https://www.w3.org/TR/rdf12-turtle/, https://www.w3.org/TR/rdf12-trig/, https://www.w3.org/TR/rdf12-n-triples/, and https://www.w3.org/TR/rdf12-n-quads/
- `oxrdfxml`: https://www.w3.org/TR/rdf12-xml/
- `oxjsonld`: https://www.w3.org/TR/json-ld11-api/
- `spargebra`: https://www.w3.org/TR/sparql12-query/ and https://www.w3.org/TR/sparql12-update/
- `sparopt`: https://www.w3.org/TR/sparql12-query/ and https://www.w3.org/TR/sparql12-update/
- `spareval`: https://www.w3.org/TR/sparql12-query/, https://www.w3.org/TR/sparql12-update/, and https://www.w3.org/TR/sparql12-federated-query/
- `sparesults`: https://www.w3.org/TR/sparql12-results-json/, https://www.w3.org/TR/sparql12-results-csv-tsv/, and https://www.w3.org/TR/sparql12-results-xml/
- `oxigraph-cli`: https://www.w3.org/TR/sparql12-protocol/ and https://www.w3.org/TR/sparql12-graph-store-protocol/
- `spargeo`: https://docs.ogc.org/is/22-047r1/22-047r1.html

## Testing

Oxigraph primarily relies on file-based test suites. Run the complete suite with:

```shell
cargo test -p oxigraph-testsuite
```

When feasible, prefer adding a test case under `testsuite/oxigraph-tests/` over adding a plain Rust unit test. Use these directories:

- `oxttl` and `oxrdfxml`: `parser`, `parser-error`, `parser-lenient` or `parser-recovery`
- `oxjsonld`: `jsonld`
- `spargeo`: `geosparql`
- `spargebra` and `spareval`: `sparql`
- `sparopt`: `sparql-optimization`
- `sparesults`: `sparql-results`

## Fuzz testing

When modifying one of the crates listed below, run each of its relevant fuzz targets for one minute with:
```shell
cargo fuzz run <target> --sanitizer none -- -max_total_time=60
```
The fuzz targets are:
- `oxttl`: `nquads`, `trig`, and `n3`
- `oxrdfxml`: `rdf_xml`
- `oxjsonld`: `jsonld`
- `spargebra`: `sparql_query` and `sparql_update`
- `spareval`: `sparql_query_eval` and `sparql_update_eval`
- `sparesults`: `sparql_results_json`, `sparql_results_tsv`, and `sparql_results_xml`
