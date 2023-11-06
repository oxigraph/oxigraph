#[allow(unused_imports)]
use oxigraph::model::{Literal, NamedNode};
use oxigraph::sparql::dataset::HDTDatasetView;
use oxigraph::sparql::{evaluate_hdt_query, EvaluationError, Query, QueryOptions, QueryResults};
use oxigraph_testsuite::sparql_evaluator::{
    are_query_results_isomorphic, load_sparql_query_result, StaticQueryResults,
};
use std::fs;
use std::rc::Rc;

#[allow(dead_code)]
fn hdt_query(hdt_path: &str, sparql_query: &str) -> Result<QueryResults, EvaluationError> {
    // Open the HDT file.
    let dataset = Rc::new(HDTDatasetView::new(hdt_path));
    let sparql_query = sparql_query;

    // SPARQL query
    let (results, _explain) = evaluate_hdt_query(
        Rc::clone(&dataset),
        sparql_query,
        QueryOptions::default(),
        false,
    )
    .expect("failed to evaluate SPARQL query");

    return results;
}

#[allow(dead_code)]
fn rdf_test_runner(query_path: &str, data_path: &str, result_path: &str) -> bool {
    // The test SPARQL query
    let rq = fs::read_to_string(&query_path).expect("Failed to read test query from file");

    let query = Query::parse(&rq, None).expect("Failed to parse the test query string");

    // The test data in HDT format
    let data = Rc::new(HDTDatasetView::new(&data_path));

    // The expected results in XML format
    // let f = File::open(result_path).expect("Failed to open the expected results from file");
    // let f = BufReader::new(f);
    // let ref_results = QueryResults::read(f, QueryResultsFormat::Xml);

    // Execute the query
    let (results, _explain) =
        evaluate_hdt_query(Rc::clone(&data), query, QueryOptions::default(), false)
            .expect("Failed to evaluate SPARQL query");

    // Compare the XML results

    // XML result serializations may differ for the same semantics
    // due to whitespace and the order of nodes.

    // Results may differ when a SELECT * wildcard is used since
    // the order of the bindings is undefined.

    // let static_ref_results =
    //     StaticQueryResults::from_query_results(ref_results.unwrap(), false).unwrap();

    // Load the SPARQL query results, automatically identifying the source format.
    let static_ref_results = load_sparql_query_result(&result_path)
        .expect("Failed to load the reference results from file");

    let static_results = StaticQueryResults::from_query_results(results.unwrap(), false)
        .expect("Failed to transorm the calculated results to a static result");

    // Debug failures by rerunning the query and printing out the
    // results.

    // let query2 = Query::parse(&rq, None).unwrap();
    // let (results_dbg, _explain) = evaluate_hdt_query(
    //     Rc::clone(&data),
    //     query2,
    //     QueryOptions::default(),
    //     false,
    // )
    //     .expect("Failed to evaluate SPARQL query");
    // if let QueryResults::Solutions(solutions) = results_dbg.unwrap() {
    //     for row in solutions {
    //         dbg!(&row);
    //     }
    // }

    return are_query_results_isomorphic(&static_ref_results, &static_results);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdt_sparql_select_o_literal_by_s_uri() {
        let ex = Literal::new_simple_literal("SPARQL Tutorial");

        if let QueryResults::Solutions(mut solutions) = hdt_query(
            "tests/resources/test.hdt",
            "SELECT ?o WHERE { <http://example.org/book/book1> ?p ?o }",
        )
        .unwrap()
        {
            assert_eq!(
                solutions.next().unwrap().unwrap().get("o"),
                Some(&ex.into())
            );
        }
    }

    #[test]
    fn hdt_sparql_select_s_uri_by_p_uri() {
        let ex = NamedNode::new("http://example.org/book/book1").unwrap();

        if let QueryResults::Solutions(mut solutions) = hdt_query(
            "tests/resources/test.hdt",
            "SELECT ?s WHERE { ?s <http://purl.org/dc/elements/1.1/title> ?o }",
        )
        .unwrap()
        {
            assert_eq!(
                solutions.next().unwrap().unwrap().get("s"),
                Some(&ex.into())
            );
        }
    }

    #[test]
    fn hdt_sparql_select_spo_by_s_uri_and_o_literal() {
        let ex_s = NamedNode::new("http://example.org/book/book1").unwrap();
        let ex_p = NamedNode::new("http://purl.org/dc/elements/1.1/title").unwrap();
        let ex_o = Literal::new_simple_literal("SPARQL Tutorial");

        if let QueryResults::Solutions(mut solutions) = hdt_query(
            "tests/resources/test.hdt",
            "SELECT ?s ?p ?o WHERE { <http://example.org/book/book1> ?p ?o . ?s ?p \"SPARQL Tutorial\" }"
        ).unwrap() {
            let row = solutions.next().unwrap().unwrap();
            assert_eq!(row.get("s"), Some(&ex_s.into()));
            assert_eq!(row.get("p"), Some(&ex_p.into()));
            assert_eq!(row.get("o"), Some(&ex_o.into()));
        }
    }

    // Create W3C SPARQL 1.0 Basic test functions.
    macro_rules! rdf_sparql10_basic_test {
        ($(($name:ident, $query:literal, $data:literal, $result:literal)),*) => {
            $(
                #[test]
                fn $name() {
                    assert!(rdf_test_runner(
                        concat!("../testsuite/rdf-tests/sparql/sparql10/basic/", $query),
                        concat!("tests/resources/rdf-tests/sparql/sparql10/basic/", $data),
                        concat!("https://w3c.github.io/rdf-tests/sparql/sparql10/basic/", $result)
                    ));
                }
            )*
        }
    }

    // Create W3C SPARQL 1.0 DAWG Triple Pattern test functions.
    macro_rules! rdf_sparql10_triple_match_test {
        ($(($name:ident, $query:literal, $data:literal, $result:literal)),*) => {
            $(
                #[test]
                fn $name() {
                    assert!(rdf_test_runner(
                        concat!("../testsuite/rdf-tests/sparql/sparql10/triple-match/", $query),
                        concat!("tests/resources/rdf-tests/sparql/sparql10/triple-match/", $data),
                        concat!("https://w3c.github.io/rdf-tests/sparql/sparql10/triple-match/", $result)
                    ));
                }
            )*
        }
    }

    // Create test functions for the combinations of input, data, and
    // output from the W3C SPARQL 1.0 Basic test suite. Note that this
    // implementation fails to stay automatically up-to-date with
    // changes in the upstream W3C SPARQL test suite since the list is
    // hard-coded. Processing the manifest.ttl would enable
    // synchronization with the upsteam suite.
    rdf_sparql10_basic_test! {
        (base_prefix_1, "base-prefix-1.rq", "data-1.hdt", "base-prefix-1.srx"),
        (base_prefix_2, "base-prefix-2.rq", "data-1.hdt", "base-prefix-2.srx"),
        (base_prefix_3, "base-prefix-3.rq", "data-1.hdt", "base-prefix-3.srx"),
        (base_prefix_4, "base-prefix-4.rq", "data-1.hdt", "base-prefix-4.srx"),
        (base_prefix_5, "base-prefix-5.rq", "data-1.hdt", "base-prefix-5.srx"),

        (list_1, "list-1.rq", "data-2.hdt", "list-1.srx"),
        (list_2, "list-2.rq", "data-2.hdt", "list-2.srx"),
        (list_3, "list-3.rq", "data-2.hdt", "list-3.srx"),
        (list_4, "list-4.rq", "data-2.hdt", "list-4.srx"),

        (quotes_1, "quotes-1.rq", "data-3.hdt", "quotes-1.srx"),
        (quotes_2, "quotes-2.rq", "data-3.hdt", "quotes-2.srx"),

        // HDT Java (https://github.com/rdfhdt/hdt-java) creates the
        // data-3.hdt from the data-3.ttl correctly. HDT C++ does not
        // per https://github.com/rdfhdt/hdt-cpp/issues/219.
        (quotes_3, "quotes-3.rq", "data-3.hdt", "quotes-3.srx"),

        (quotes_4, "quotes-4.rq", "data-3.hdt", "quotes-4.srx"),

        (term_1, "term-1.rq", "data-4.hdt", "term-1.srx"),
        (term_2, "term-2.rq", "data-4.hdt", "term-2.srx"),
        (term_3, "term-3.rq", "data-4.hdt", "term-3.srx"),
        (term_4, "term-4.rq", "data-4.hdt", "term-4.srx"),
        (term_5, "term-5.rq", "data-4.hdt", "term-5.srx"),
        (term_6, "term-6.rq", "data-4.hdt", "term-6.srx"),
        (term_7, "term-7.rq", "data-4.hdt", "term-7.srx"),
        (term_8, "term-8.rq", "data-4.hdt", "term-8.srx"),
        (term_9, "term-9.rq", "data-4.hdt", "term-9.srx"),

        (var_1, "var-1.rq", "data-5.hdt", "var-1.srx"),
        (var_2, "var-2.rq", "data-5.hdt", "var-2.srx"),

        (bgp_no_match, "bgp-no-match.rq", "data-7.hdt", "bgp-no-match.srx"),
        (spoo_1, "spoo-1.rq", "data-6.hdt", "spoo-1.srx"),

        (prefix_name_1, "prefix-name-1.rq", "data-6.hdt", "prefix-name-1.srx")
    }

    rdf_sparql10_triple_match_test! {
        (dawg_triple_pattern_001, "dawg-tp-01.rq", "data-01.hdt", "result-tp-01.ttl"),
        (dawg_triple_pattern_002, "dawg-tp-02.rq", "data-01.hdt", "result-tp-02.ttl"),
        (dawg_triple_pattern_003, "dawg-tp-03.rq", "data-02.hdt", "result-tp-03.ttl"),
        (dawg_triple_pattern_004, "dawg-tp-04.rq", "dawg-data-01.hdt", "result-tp-04.ttl")
    }
}
