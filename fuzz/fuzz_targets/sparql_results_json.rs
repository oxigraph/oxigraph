#![no_main]
use libfuzzer_sys::fuzz_target;
use oxigraph_fuzz::result_format::fuzz_result_format;
use sparesults::QueryResultsFormat;

fuzz_target!(|data: &[u8]| fuzz_result_format(QueryResultsFormat::Json, data));
