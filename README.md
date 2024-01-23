# Oxigraph-handlegraph

[Oxigraph](https://github.com/oxigraph/oxigraph) is a graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

This is a fork of oxigraph that includes [rs-handlegraph](https://github.com/chfi/rs-handlegraph).
The purpose is to make pangenomic GFA-files accessible with SPARQL queries.

Oxigraph implements the following specifications:

- [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
- [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
- [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).

When cloning this codebase, don't forget to clone the submodules using
`git clone --recursive https://github.com/oxigraph/oxigraph.git` to clone the repository including submodules or
`git submodule update --init` to add the submodules to the already cloned repository.


## Help

Feel free to ask [heringerp](https://github.com/heringerp) for help.
[Bug reports](https://github.com/heringerp/oxigraph/issues) are also very welcome.


## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
