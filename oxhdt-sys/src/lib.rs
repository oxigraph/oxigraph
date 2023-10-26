#[allow(unused_imports)]
use oxigraph::model::{NamedNode, Literal};
use oxigraph::sparql::EvaluationError;
use oxigraph::sparql::QueryOptions;
use oxigraph::sparql::Query;
use oxigraph::sparql::QueryResults;
use oxigraph::sparql::dataset::HDTDatasetView;
use oxigraph::sparql::evaluate_hdt_query;
use oxigraph::sparql::results::QueryResultsFormat;
use std::rc::Rc;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use oxigraph_testsuite::sparql_evaluator::{are_query_results_isomorphic, StaticQueryResults};

#[allow(dead_code)]
fn hdt_query(hdt_path: &str, sparql_query: &str)  -> Result<QueryResults, EvaluationError> {
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
    let rq = fs::read_to_string(&query_path)
        .expect("Failed to read test query from file");
    let query = Query::parse(&rq, None).unwrap();
    
    // The test data in HDT format
    let data = Rc::new(HDTDatasetView::new(&data_path));
    
    // The expected results in XML format
    let f = File::open(result_path).unwrap();
    let f = BufReader::new(f);
    let ref_results = QueryResults::read(f, QueryResultsFormat::Xml);
    
    // Execute the query
    let (results, _explain) = evaluate_hdt_query(
        Rc::clone(&data),
        query,
        QueryOptions::default(),
        false,
    )
        .expect("failed to evaluate SPARQL query");
    
    // Compare the XML results
    
    // XML result serializations may differ for the same semantics
    // due to whitespace and the order of nodes.
    
    // Results may differ when a SELECT * wildcard is used since
    // the order of the bindings is undefined.
    
    let static_ref_results = StaticQueryResults::from_query_results(ref_results.unwrap(), true).unwrap();
    let static_results = StaticQueryResults::from_query_results(results.unwrap(), true).unwrap();
    
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
            "SELECT ?o WHERE { <http://example.org/book/book1> ?p ?o }"
        ).unwrap() {
            assert_eq!(solutions.next().unwrap().unwrap().get("o"), Some(&ex.into()));
        }
    }

    #[test]
    fn hdt_sparql_select_s_uri_by_p_uri() {
        let ex = NamedNode::new("http://example.org/book/book1").unwrap();
        
        if let QueryResults::Solutions(mut solutions) = hdt_query(
            "tests/resources/test.hdt",
            "SELECT ?s WHERE { ?s <http://purl.org/dc/elements/1.1/title> ?o }"
        ).unwrap() {
            assert_eq!(solutions.next().unwrap().unwrap().get("s"), Some(&ex.into()));
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

    // ```
    // :base-prefix-1 rdf:type mf:QueryEvaluationTest ;
    // mf:name    "Basic - Prefix/Base 1" ;
    // dawgt:approval dawgt:Approved ;
    // dawgt:approvedBy <http://lists.w3.org/Archives/Public/public-rdf-dawg/2007JulSep/att-0060/2007-08-07-dawg-minutes.html> ;
    // mf:action
    //     [ qt:query  <base-prefix-1.rq> ;
    //       qt:data   <data-1.ttl> ] ;
    // mf:result  <base-prefix-1.srx> ;
    // .
    // ```
    /// Basic - Prefix/Base 1
    #[test]
    fn base_prefix_1() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-1.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-1.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-1.srx",
        ));
    }

    // ```
    // :base-prefix-2 rdf:type mf:QueryEvaluationTest ;
    // mf:name    "Basic - Prefix/Base 2" ;
    // dawgt:approval dawgt:Approved ;
    // dawgt:approvedBy <http://lists.w3.org/Archives/Public/public-rdf-dawg/2007JulSep/att-0060/2007-08-07-dawg-minutes.html> ;
    // mf:action
    //      [ qt:query  <base-prefix-2.rq> ;
    //        qt:data   <data-1.ttl> ] ;
    // mf:result  <base-prefix-2.srx>
    // .
    // ```
    /// Basic - Prefix/Base 2
    #[test]
    fn base_prefix_2() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-2.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-1.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-2.srx",
        ));
    }

    /// Basic - Prefix/Base 3
    #[test]
    fn base_prefix_3() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-3.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-1.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-3.srx",
        ));
    }

    /// Basic - Prefix/Base 4
    #[test]
    fn base_prefix_4() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-4.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-1.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-4.srx",
        ));
    }

    /// Basic - Prefix/Base 5
    #[test]
    fn base_prefix_5() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-5.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-1.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/base-prefix-5.srx",
        ));
    }

    /// Basic - List 1
    #[test]
    fn list_1() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-1.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-2.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-1.srx",
        ));
    }

    /// Basic - List 2
    #[test]
    fn list_2() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-2.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-2.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-2.srx",
        ));
    }

    /// Basic - List 3
    #[test]
    fn list_3() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-3.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-2.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-3.srx",
        ));
    }

    /// Basic - List 4
    #[test]
    fn list_4() {
        assert!(rdf_test_runner(
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-4.rq",
            "tests/resources/rdf-tests/sparql/sparql10/basic/data-2.hdt",
            "../testsuite/rdf-tests/sparql/sparql10/basic/list-4.srx",
        ));
    }
}
