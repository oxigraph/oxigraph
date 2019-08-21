/// [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphSyntax {
    /// [N-Triples](https://www.w3.org/TR/n-triples/)
    NTriples,
    /// [Turtle](https://www.w3.org/TR/turtle/)
    Turtle,
    /// [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/)
    RdfXml,
}

/// [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum DatasetSyntax {
    /// [N-Quads](https://www.w3.org/TR/n-quads/)
    NQuads,
    /// [TriG](https://www.w3.org/TR/trig/)
    TriG,
}
