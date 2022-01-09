#![no_main]
use libfuzzer_sys::fuzz_target;
use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};

fuzz_target!(|data: &[u8]| {
    let parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    if let Ok(QueryResultsReader::Solutions(solutions)) = parser.read_results(data) {
        for _ in solutions {}
    }
});
