use oxigraph::io::{RdfFormat, RdfSerializer};
use oxigraph::model::vocab::rdf;
use oxigraph::model::{BlankNode, NamedNode, OxString, Triple};
use oxigraph::sparql::results::QueryResultsFormat;
#[cfg(feature = "geosparql")]
use spargeo::GEOSPARQL_EXTENSION_FUNCTIONS;

mod sd {
    use oxigraph::model::NamedNode;

    pub const SERVICE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql-service-description#Service");

    pub const DEFAULT_ENTAILMENT_REGIME: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#defaultEntailmentRegime",
    );
    pub const ENDPOINT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql-service-description#endpoint");
    #[cfg(feature = "geosparql")]
    pub const EXTENSION_FUNCTION: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#extensionFunction",
    );
    pub const FEATURE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql-service-description#feature");
    pub const RESULT_FORMAT: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#resultFormat",
    );
    pub const SUPPORTED_LANGUAGE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#supportedLanguage",
    );

    pub const EMPTY_GRAPHS: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#EmptyGraphs",
    );
    #[cfg(any(
        feature = "native-tls",
        feature = "rustls-native",
        feature = "rustls-webpki"
    ))]
    pub const BASIC_FEDERATED_QUERY: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#BasicFederatedQuery",
    );
    pub const SPARQL_10_QUERY: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#SPARQL10Query",
    );
    pub const SPARQL_11_QUERY: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#SPARQL11Query",
    );
    pub const SPARQL_11_UPDATE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#SPARQL11Update",
    );
    pub const UNION_DEFAULT_GRAPH: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/ns/sparql-service-description#UnionDefaultGraph",
    );
}

#[derive(Clone, Copy)]
pub struct EndpointKind {
    pub query: bool,
    pub update: bool,
}

pub fn generate_service_description(
    format: RdfFormat,
    kind: EndpointKind,
    union_default_graph: bool,
    endpoint_base_url: OxString,
) -> Vec<u8> {
    let mut serializer = RdfSerializer::from_format(format)
        .with_prefix("sd", "http://www.w3.org/ns/sparql-service-description#")
        .unwrap()
        .for_writer(Vec::new());
    for t in
        generate_service_description_graph(format, kind, union_default_graph, endpoint_base_url)
    {
        serializer.serialize_triple(&t).unwrap();
    }
    serializer.finish().unwrap()
}

fn generate_service_description_graph(
    format: RdfFormat,
    kind: EndpointKind,
    union_default_graph: bool,
    endpoint_base_url: OxString,
) -> Vec<Triple> {
    let mut graph = Vec::new();
    let root = BlankNode::default();
    graph.push(Triple::new(root.clone(), rdf::TYPE, sd::SERVICE));
    graph.push(Triple::new(
        root.clone(),
        sd::ENDPOINT,
        NamedNode::new_unchecked(match format {
            RdfFormat::Turtle
            | RdfFormat::TriG
            | RdfFormat::N3
            | RdfFormat::JsonLd { .. }
            | RdfFormat::RdfXml => {
                // The document base URL is also the endpoint URL, so we can just use it
                OxString::default()
            }
            RdfFormat::NTriples | RdfFormat::NQuads | _ => {
                // We need to return an absolute URL, we use the request target url
                endpoint_base_url
            }
        }),
    ));
    if kind.query {
        graph.push(Triple::new(
            root.clone(),
            sd::SUPPORTED_LANGUAGE,
            sd::SPARQL_10_QUERY,
        ));
        graph.push(Triple::new(
            root.clone(),
            sd::SUPPORTED_LANGUAGE,
            sd::SPARQL_11_QUERY,
        ));
    }
    if kind.update {
        graph.push(Triple::new(
            root.clone(),
            sd::SUPPORTED_LANGUAGE,
            sd::SPARQL_11_UPDATE,
        ));
    }
    if kind.query {
        for format in [
            QueryResultsFormat::Json,
            QueryResultsFormat::Xml,
            QueryResultsFormat::Csv,
            QueryResultsFormat::Tsv,
        ] {
            graph.push(Triple::new(
                root.clone(),
                sd::RESULT_FORMAT,
                NamedNode::new_const_unchecked(format.iri()),
            ));
        }
        for format in [
            RdfFormat::NTriples,
            RdfFormat::NQuads,
            RdfFormat::Turtle,
            RdfFormat::TriG,
            RdfFormat::N3,
            RdfFormat::RdfXml,
        ] {
            graph.push(Triple::new(
                root.clone(),
                sd::RESULT_FORMAT,
                NamedNode::new_const_unchecked(format.iri()),
            ));
        }
    }
    #[cfg(any(
        feature = "native-tls",
        feature = "rustls-native",
        feature = "rustls-webpki"
    ))]
    if kind.query {
        graph.push(Triple::new(
            root.clone(),
            sd::FEATURE,
            sd::BASIC_FEDERATED_QUERY,
        ));
    }
    graph.push(Triple::new(root.clone(), sd::FEATURE, sd::EMPTY_GRAPHS));
    if union_default_graph {
        graph.push(Triple::new(
            root.clone(),
            sd::FEATURE,
            sd::UNION_DEFAULT_GRAPH,
        ));
    }
    graph.push(Triple::new(
        root.clone(),
        sd::DEFAULT_ENTAILMENT_REGIME,
        NamedNode::new_const_unchecked("http://www.w3.org/ns/entailment/Simple"),
    ));
    #[cfg(feature = "geosparql")]
    for (function_name, _) in GEOSPARQL_EXTENSION_FUNCTIONS {
        graph.push(Triple::new(
            root.clone(),
            sd::EXTENSION_FUNCTION,
            function_name,
        ));
    }
    graph
}
