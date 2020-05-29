#[cfg(target_arch = "wasm32")]
mod test {
    use oxigraph::model::*;
    use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult};
    use oxigraph::{MemoryStore, Result};
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn simple() {
        let store = MemoryStore::default();

        // insertion
        let ex = NamedNode::parse("http://example.com").unwrap();
        let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
        store.insert(&quad).unwrap();
        // quad filter
        let results: Result<Vec<Quad>> = store.quads_for_pattern(None, None, None, None).collect();
        assert_eq!(vec![quad], results.unwrap());

        // SPARQL query
        let prepared_query = store
            .prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())
            .unwrap();
        let results = prepared_query.exec().unwrap();
        if let QueryResult::Bindings(mut solutions) = results {
            assert_eq!(
                solutions.next().unwrap().unwrap().get("s"),
                Some(&ex.into())
            );
        }
    }

    #[wasm_bindgen_test]
    fn now() {
        let store = MemoryStore::default();
        let prepared_query = store
            .prepare_query(
                "SELECT (YEAR(NOW()) AS ?y) WHERE {}",
                QueryOptions::default(),
            )
            .unwrap();
        let results = prepared_query.exec().unwrap();
        if let QueryResult::Bindings(mut solutions) = results {
            if let Some(Term::Literal(l)) = solutions.next().unwrap().unwrap().get(0) {
                let year = i64::from_str(l.value()).unwrap();
                assert!(2020 <= year && year <= 2100);
            }
        }
    }
}
