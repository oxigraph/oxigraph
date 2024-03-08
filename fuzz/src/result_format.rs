use anyhow::Context;
use sparesults::{
    FromReadQueryResultsReader, QueryResultsFormat, QueryResultsParser, QueryResultsSerializer,
};

pub fn fuzz_result_format(format: QueryResultsFormat, data: &[u8]) {
    let parser = QueryResultsParser::from_format(format);
    let serializer = QueryResultsSerializer::from_format(format);

    let Ok(reader) = parser.parse_read(data) else {
        return;
    };
    match reader {
        FromReadQueryResultsReader::Solutions(solutions) => {
            let Ok(solutions) = solutions.collect::<Result<Vec<_>, _>>() else {
                return;
            };

            // We try to write again
            let mut writer = serializer
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
            let serialized = String::from_utf8(writer.finish().unwrap()).unwrap();

            // And to parse again
            if let FromReadQueryResultsReader::Solutions(roundtrip_solutions) = parser
                .parse_read(serialized.as_bytes())
                .with_context(|| format!("Parsing {serialized:?}"))
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
        FromReadQueryResultsReader::Boolean(value) => {
            // We try to write again
            let mut serialized = Vec::new();
            serializer
                .serialize_boolean_to_write(&mut serialized, value)
                .unwrap();

            // And to parse again
            if let FromReadQueryResultsReader::Boolean(roundtrip_value) =
                parser.parse_read(serialized.as_slice()).unwrap()
            {
                assert_eq!(roundtrip_value, value)
            }
        }
    }
}
