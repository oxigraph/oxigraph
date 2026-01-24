#![cfg(feature = "datafusion")]

use bzip2::read::MultiBzDecoder;
use oxhttp::model::{Request, Uri};
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;
use oxrdfio::{RdfFormat, RdfParser};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

#[test]
fn test_datafusion() -> Result<(), Box<dyn std::error::Error>> {
    let data = read_bz2_data(&format!(
        "https://zenodo.org/records/12663333/files/dataset-5000.nt.bz2"
    ));
    let store = Store::new()?;
    let mut loader = store.bulk_loader();
    loader
        .load_from_slice(RdfParser::from_format(RdfFormat::NTriples).lenient(), &data)
        .unwrap();
    loader.commit().unwrap();
    store.optimize().unwrap();

    for op in csv::Reader::from_reader(
        read_bz2_data(&format!(
            "https://zenodo.org/records/12663333/files/explore-5000.csv.bz2"
        ))
        .as_slice(),
    )
    .records()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
    .into_iter()
    .rev()
    .take(30)
    {
        let result = SparqlEvaluator::new()
            .parse_query(&op[2])?
            .datafusion_explain(&store)?;
        fs::write(format!("query-{}.txt", &op[0]), result)?;
    }
    Ok(())
}

fn read_bz2_data(url: &str) -> Vec<u8> {
    let url = Uri::from_str(url).unwrap();
    let file_name = url.path().split('/').next_back().unwrap().to_owned();
    if !Path::new(&file_name).exists() {
        let client = oxhttp::Client::new().with_redirection_limit(5);
        let request = Request::builder().uri(&url).body(()).unwrap();
        let response = client.request(request).unwrap();
        assert!(response.status().is_success(), "{url}");
        std::io::copy(
            &mut response.into_body(),
            &mut File::create(&file_name).unwrap(),
        )
        .unwrap();
    }
    let mut buf = Vec::new();
    MultiBzDecoder::new(File::open(&file_name).unwrap())
        .read_to_end(&mut buf)
        .unwrap();
    buf
}
