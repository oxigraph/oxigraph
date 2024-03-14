Oxigraph Rust crates
====================

Oxigraph is implemented in Rust.
It is composed on a main library, [`oxigraph`](./oxigraph) and a set of smaller crates used by the `oxigraph` crate:
* [`oxrdf`](./oxrdf), datastructures encoding RDF basic concepts (the `model` module of the `oxigraph` crate).
* [`oxrdfio`](./oxrdfio), a unified parser and serializer API for RDF formats (the `io` module of the `oxigraph` crate). It itself relies on:
    * [`oxttl`](./oxttl), N-Triple, N-Quad, Turtle, TriG and N3 parsing and serialization.
    * [`oxrdfxml`](./oxrdfxml), RDF/XML parsing and serialization.
* [`spargebra`](./spargebra), a SPARQL parser.
* [`sparesults`](./sparesults), parsers and serializers for SPARQL result formats (the `sparql::results` module of the `oxigraph` crate).
* [`sparopt`](./sparesults), a SPARQL optimizer.
* [`oxsdatatypes`](./oxsdatatypes), an implementation of some XML Schema datatypes.
