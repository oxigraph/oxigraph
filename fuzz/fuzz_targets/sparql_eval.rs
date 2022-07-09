#![no_main]
use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;
use oxigraph::io::DatasetFormat;
use oxigraph::sparql::Query;
use oxigraph::store::Store;

lazy_static! {
    static ref STORE: Store = {
        let store = Store::new().unwrap();
        store
            .load_dataset(
                sparql_smith::DATA_TRIG.as_bytes(),
                DatasetFormat::TriG,
                None,
            )
            .unwrap();
        store
    };
}

fuzz_target!(|data: sparql_smith::Query| {
    if let Ok(q) = Query::parse(&data.to_string(), None) {
        STORE.query(q).unwrap();
    }
});
