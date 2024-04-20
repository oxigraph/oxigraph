#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph::model::dataset::{CanonicalizationAlgorithm, Dataset};
use oxigraph::sparql::{QueryOptions, Update};
use oxigraph::store::Store;
use std::sync::OnceLock;

fuzz_target!(|data: sparql_smith::Update| {
    static STORE: OnceLock<Store> = OnceLock::new();
    let store = STORE.get_or_init(|| Store::new().unwrap());

    let update_str = data.to_string();
    if let Ok(update) = Update::parse(&update_str, None) {
        let options = QueryOptions::default();

        store.clear().unwrap();
        let with_opt = store.update_opt(update.clone(), options.clone());
        let mut dataset_with_opt = store.iter().collect::<Result<Dataset, _>>().unwrap();
        dataset_with_opt.canonicalize(CanonicalizationAlgorithm::Unstable);

        store.clear().unwrap();
        let without_opt = store.update_opt(update, options.without_optimizations());
        let mut dataset_without_opt = store.iter().collect::<Result<Dataset, _>>().unwrap();
        dataset_without_opt.canonicalize(CanonicalizationAlgorithm::Unstable);

        assert_eq!(
            with_opt.is_ok(),
            without_opt.is_ok(),
            "Worked and failed depending on using optimizations"
        );
        assert_eq!(
            dataset_with_opt, dataset_without_opt,
            "With opts:\n{dataset_with_opt}\nWithout opts:\n{dataset_without_opt}"
        );
    }
});
