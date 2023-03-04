use anyhow::Context;
use sparesults::{
    QueryResultsFormat, QueryResultsParser, QueryResultsReader, QueryResultsSerializer,
};

pub fn fuzz_result_format(format: QueryResultsFormat, data: &[u8]) {
    let parser = QueryResultsParser::from_format(format);
    let serializer = QueryResultsSerializer::from_format(format);

    let Ok(reader) = parser.read_results(data) else {
        return;
    };
    match reader {
        QueryResultsReader::Solutions(solutions) => {
            let Ok(solutions) = solutions.collect::<Result<Vec<_>, _>>() else {
                return;
            };

            // We try to write again
            let mut writer = serializer
                .solutions_writer(
                    Vec::new(),
                    solutions
                        .get(0)
                        .map_or_else(Vec::new, |s| s.variables().to_vec()),
                )
                .unwrap();
            for solution in &solutions {
                writer.write(solution).unwrap();
            }
            let serialized = String::from_utf8(writer.finish().unwrap()).unwrap();

            // And to parse again
            if let QueryResultsReader::Solutions(roundtrip_solutions) = parser
                .read_results(serialized.as_bytes())
                .with_context(|| format!("Parsing {:?}", &serialized))
                .unwrap()
            {
                assert_eq!(
                    roundtrip_solutions
                        .collect::<Result<Vec<_>, _>>()
                        .with_context(|| format!("Parsing {:?}", &serialized))
                        .unwrap(),
                    solutions
                )
            }
        }
        QueryResultsReader::Boolean(value) => {
            // We try to write again
            let mut serialized = Vec::new();
            serializer
                .write_boolean_result(&mut serialized, value)
                .unwrap();

            // And to parse again
            if let QueryResultsReader::Boolean(roundtrip_value) =
                parser.read_results(serialized.as_slice()).unwrap()
            {
                assert_eq!(roundtrip_value, value)
            }
        }
    }
}
