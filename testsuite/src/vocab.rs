pub mod rs {
    use oxigraph::model::NamedNode;

    pub const RESULT_SET: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet",
    );
    pub const RESULT_VARIABLE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable",
    );
    pub const SOLUTION: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution",
    );
    pub const BINDING: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding",
    );
    pub const VALUE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#value",
    );
    pub const VARIABLE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable",
    );
    pub const INDEX: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#index",
    );
    pub const BOOLEAN: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean",
    );
}

pub mod mf {
    use oxigraph::model::NamedNode;

    pub const INCLUDE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include",
    );
    pub const ENTRIES: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries",
    );
    pub const MANIFEST: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#Manifest",
    );
    pub const NAME: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name",
    );
    pub const ACTION: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action",
    );
    pub const ASSUMED_TEST_BASE: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#assumedTestBase",
    );
    pub const RESULT: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result",
    );
}

pub mod rdft {
    use oxigraph::model::NamedNode;

    pub const APPROVAL: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/rdftest#approval");
    pub const REJECTED: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/rdftest#Rejected");
}

pub mod qt {
    use oxigraph::model::NamedNode;

    pub const QUERY: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#query",
    );
    pub const DATA: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#data",
    );
    pub const GRAPH_DATA: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData",
    );
    pub const SERVICE_DATA: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#serviceData",
    );
    pub const ENDPOINT: NamedNode = NamedNode::new_const_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#endpoint",
    );
}

pub mod ut {
    use oxigraph::model::NamedNode;
    pub const DATA: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/2009/sparql/tests/test-update#data");
    pub const GRAPH_DATA: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/2009/sparql/tests/test-update#graphData");
    pub const GRAPH: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/2009/sparql/tests/test-update#graph");
    pub const REQUEST: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/2009/sparql/tests/test-update#request");
}

pub mod jld {
    use oxigraph::model::NamedNode;

    pub const BASE: NamedNode =
        NamedNode::new_const_unchecked("https://w3c.github.io/json-ld-api/tests/vocab#base");
    pub const NEGATIVE_EVALUATION_TEST: NamedNode = NamedNode::new_const_unchecked(
        "https://w3c.github.io/json-ld-api/tests/vocab#NegativeEvaluationTest",
    );
    pub const OPTION: NamedNode =
        NamedNode::new_const_unchecked("https://w3c.github.io/json-ld-api/tests/vocab#option");
    pub const POSITIVE_EVALUATION_TEST: NamedNode = NamedNode::new_const_unchecked(
        "https://w3c.github.io/json-ld-api/tests/vocab#PositiveEvaluationTest",
    );
    pub const POSITIVE_SYNTAX_TEST: NamedNode = NamedNode::new_const_unchecked(
        "https://w3c.github.io/json-ld-api/tests/vocab#PositiveSyntaxTest",
    );
    pub const PROCESSING_MODE: NamedNode = NamedNode::new_const_unchecked(
        "https://w3c.github.io/json-ld-api/tests/vocab#processingMode",
    );
    pub const STREAM_TEST: NamedNode =
        NamedNode::new_const_unchecked("https://w3c.github.io/json-ld-api/tests/vocab#StreamTest");
}

pub mod rdfc {
    use oxigraph::model::NamedNode;

    pub const HASH_ALGORITHM: NamedNode =
        NamedNode::new_const_unchecked("https://w3c.github.io/rdf-canon/tests/vocab#hashAlgorithm");
}
