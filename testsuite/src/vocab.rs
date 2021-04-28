pub mod rs {
    use oxigraph::model::NamedNodeRef;

    pub const RESULT_SET: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet",
    );
    pub const RESULT_VARIABLE: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable",
    );
    pub const SOLUTION: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution",
    );
    pub const BINDING: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding",
    );
    pub const VALUE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value");
    pub const VARIABLE: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable",
    );
    pub const INDEX: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index");
    pub const BOOLEAN: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean",
    );
}

pub mod mf {
    use oxigraph::model::NamedNodeRef;

    pub const INCLUDE: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include",
    );
    pub const ENTRIES: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries",
    );
    pub const MANIFEST: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#Manifest",
    );
    pub const NAME: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name",
    );
    pub const ACTION: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action",
    );
    pub const RESULT: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result",
    );
}

pub mod qt {
    use oxigraph::model::NamedNodeRef;

    pub const QUERY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query");
    pub const DATA: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data");
    pub const GRAPH_DATA: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData",
    );
    pub const SERVICE_DATA: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#serviceData",
    );
    pub const ENDPOINT: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-query#endpoint",
    );
}

pub mod ut {
    use oxigraph::model::NamedNodeRef;
    pub const DATA: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2009/sparql/tests/test-update#data");
    pub const GRAPH_DATA: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2009/sparql/tests/test-update#graphData");
    pub const GRAPH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2009/sparql/tests/test-update#graph");
    pub const REQUEST: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2009/sparql/tests/test-update#request");
}
