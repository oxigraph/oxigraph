pub mod rs {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref RESULT_SET: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet")
                .unwrap();
        pub static ref RESULT_VARIABLE: NamedNode = NamedNode::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable"
        )
        .unwrap();
        pub static ref SOLUTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution")
                .unwrap();
        pub static ref BINDING: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding")
                .unwrap();
        pub static ref VALUE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value")
                .unwrap();
        pub static ref VARIABLE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable")
                .unwrap();
        pub static ref INDEX: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index")
                .unwrap();
        pub static ref BOOLEAN: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean")
                .unwrap();
    }
}

pub mod mf {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref INCLUDE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include")
                .unwrap();
        pub static ref ENTRIES: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries")
                .unwrap();
        pub static ref NAME: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name")
                .unwrap();
        pub static ref ACTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action")
                .unwrap();
        pub static ref RESULT: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result")
                .unwrap();
    }
}

pub mod qt {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref QUERY: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query")
                .unwrap();
        pub static ref DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data").unwrap();
        pub static ref GRAPH_DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData")
                .unwrap();
        pub static ref SERVICE_DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#serviceData")
                .unwrap();
        pub static ref ENDPOINT: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#endpoint")
                .unwrap();
    }
}
