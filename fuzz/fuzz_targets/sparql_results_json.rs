#![no_main]
use libfuzzer_sys::fuzz_target;
use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};

fuzz_target!(|data: &[u8]| {
    let parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
    if let Ok(QueryResultsReader::Solutions(solutions)) = parser.read_results(data) {
        for s in solutions {
            if s.is_err() {
                // TODO: avoid infinite loop of errors
                break;
            }
        }
    }
});
