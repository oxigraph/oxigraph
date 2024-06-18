use anyhow::Context;
use sparesults::{
    FromSliceQueryResultsReader, QueryResultsFormat, QueryResultsParser, QueryResultsSerializer,
};

pub fn fuzz_result_format(format: QueryResultsFormat, data: &[u8]) {
    let Ok(reader) = QueryResultsParser::from_format(format).parse_slice(data) else {
        return;
    };
    match reader {
        FromSliceQueryResultsReader::Solutions(solutions) => {
            let Ok(solutions) = solutions.collect::<Result<Vec<_>, _>>() else {
                return;
            };

            // We try to write again
            let mut writer = QueryResultsSerializer::from_format(format)
                .serialize_solutions_to_write(
                    Vec::new(),
                    solutions
                        .first()
                        .map_or_else(Vec::new, |s| s.variables().to_vec()),
                )
                .unwrap();
            for solution in &solutions {
                writer.write(solution).unwrap();
            }
            let serialized = writer.finish().unwrap();

            // And to parse again
            if let FromSliceQueryResultsReader::Solutions(roundtrip_solutions) =
                QueryResultsParser::from_format(format)
                    .parse_slice(&serialized)
                    .with_context(|| format!("Parsing {:?}", String::from_utf8_lossy(&serialized)))
                    .unwrap()
            {
                assert_eq!(
                    roundtrip_solutions
                        .collect::<Result<Vec<_>, _>>()
                        .with_context(|| format!("Parsing {serialized:?}"))
                        .unwrap(),
                    solutions
                )
            }
        }
        FromSliceQueryResultsReader::Boolean(value) => {
            // We try to write again
            let mut serialized = Vec::new();
            QueryResultsSerializer::from_format(format)
                .serialize_boolean_to_write(&mut serialized, value)
                .unwrap();

            // And to parse again
            if let FromSliceQueryResultsReader::Boolean(roundtrip_value) =
                QueryResultsParser::from_format(format)
                    .parse_slice(&serialized)
                    .unwrap()
            {
                assert_eq!(roundtrip_value, value)
            }
        }
    }
}
