#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph::model::dataset::{CanonicalizationAlgorithm, Dataset};
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;
use spargebra::SparqlParser;
#[cfg(feature = "rocksdb")]
use std::env::temp_dir;
use std::sync::OnceLock;

fuzz_target!(|data: sparql_smith::Update| {
    static DISK_STORE: OnceLock<Store> = OnceLock::new();
    let disk_store = DISK_STORE.get_or_init(|| {
        #[cfg(feature = "rocksdb")]
        {
            Store::open(temp_dir().join("oxigraph-fuzz-update")).unwrap()
        }
        #[cfg(not(feature = "rocksdb"))]
        {
            Store::new().unwrap()
        }
    });

    let update_str = data.to_string();
    if let Ok(update) = SparqlParser::new().parse_update(&update_str) {
        disk_store.clear().unwrap();
        let disk_with_opt = SparqlEvaluator::new()
            .for_update(update.clone())
            .on_store(disk_store)
            .execute();
        disk_store.validate().unwrap();
        let mut dataset_disk_with_opt = disk_store.iter().collect::<Result<Dataset, _>>().unwrap();
        dataset_disk_with_opt.canonicalize(CanonicalizationAlgorithm::Unstable);

        let memory_store = Store::new().unwrap();
        let memory_without_opt = SparqlEvaluator::new()
            .without_optimizations()
            .for_update(update.clone())
            .on_store(&memory_store)
            .execute();
        memory_store.validate().unwrap();
        let mut dataset_memory_without_opt =
            memory_store.iter().collect::<Result<Dataset, _>>().unwrap();
        dataset_memory_without_opt.canonicalize(CanonicalizationAlgorithm::Unstable);

        assert_eq!(
            disk_with_opt.is_ok(),
            memory_without_opt.is_ok(),
            "Worked and failed depending on using optimizations: {disk_with_opt:?} {memory_without_opt:?}"
        );
        assert_eq!(
            dataset_disk_with_opt, dataset_memory_without_opt,
            "With optimizations on disk:\n{dataset_disk_with_opt}\nWithout optimizations in memory:\n{dataset_memory_without_opt}"
        );

        // Parsing roundtrip
        let roundtrip = SparqlParser::new()
            .parse_update(&update.to_string())
            .unwrap();

        let memory_store = Store::new().unwrap();
        let memory_roundtrip = SparqlEvaluator::new()
            .without_optimizations()
            .for_update(roundtrip.clone())
            .on_store(&memory_store)
            .execute();
        memory_store.validate().unwrap();
        let mut dataset_memory_roundtrip =
            memory_store.iter().collect::<Result<Dataset, _>>().unwrap();
        dataset_memory_roundtrip.canonicalize(CanonicalizationAlgorithm::Unstable);

        assert_eq!(
            memory_without_opt.is_ok(),
            memory_roundtrip.is_ok(),
            "Worked and failed depending on roundtrip: {memory_without_opt:?} {memory_roundtrip:?}"
        );
        assert_eq!(
            dataset_memory_without_opt, dataset_memory_roundtrip,
            "With optimizations on disk:\n{dataset_memory_without_opt}\nWithout optimizations in memory:\n{dataset_memory_roundtrip}"
        );
    }
});
