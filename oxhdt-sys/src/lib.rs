#[allow(unused_imports)]
use oxigraph::model::{NamedNode, Literal};
use oxigraph::sparql::EvaluationError;
use oxigraph::sparql::QueryOptions;
use oxigraph::sparql::QueryResults;
use oxigraph::sparql::dataset::HDTDatasetView;
use oxigraph::sparql::evaluate_hdt_query;
use std::rc::Rc;

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
}
