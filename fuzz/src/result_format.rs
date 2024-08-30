use anyhow::Context;
use sparesults::{
    QueryResultsFormat, QueryResultsParser, QueryResultsSerializer, SliceQueryResultsParserOutput,
};

pub fn fuzz_result_format(format: QueryResultsFormat, data: &[u8]) {
    let Ok(reader) = QueryResultsParser::from_format(format).for_slice(data) else {
        return;
    };
    match reader {
        SliceQueryResultsParserOutput::Solutions(solutions) => {
            let Ok(solutions) = solutions.collect::<Result<Vec<_>, _>>() else {
                return;
            };

            // We try to write again
            let mut serializer = QueryResultsSerializer::from_format(format)
                .serialize_solutions_to_writer(
                    Vec::new(),
                    solutions
                        .first()
                        .map_or_else(Vec::new, |s| s.variables().to_vec()),
                )
                .unwrap();
            for solution in &solutions {
                serializer.serialize(solution).unwrap();
            }
            let serialized = serializer.finish().unwrap();

            // And to parse again
            if let SliceQueryResultsParserOutput::Solutions(roundtrip_solutions) =
                QueryResultsParser::from_format(format)
                    .for_slice(&serialized)
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
        SliceQueryResultsParserOutput::Boolean(value) => {
            // We try to write again
            let mut serialized = Vec::new();
            QueryResultsSerializer::from_format(format)
                .serialize_boolean_to_writer(&mut serialized, value)
                .unwrap();

            // And to parse again
            if let SliceQueryResultsParserOutput::Boolean(roundtrip_value) =
                QueryResultsParser::from_format(format)
                    .for_slice(&serialized)
                    .unwrap()
            {
                assert_eq!(roundtrip_value, value)
            }
        }
    }
}
