#[cfg(test)]
mod tests {
    use oxrdf::Quad;
    #[cfg(feature = "rdf-12")]
    use oxrdf::TermRef;
    use oxttl::{NQuadsParser, NTriplesParser, TurtleParseError};
    use std::io::{self, Read};

    #[test]
    fn nquads_borrowed_callback_matches_owned_parser() {
        let input = concat!(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
            "_:subject <http://example.com/p> _:object <http://example.com/g> .\n",
            "<http://example.com/s> <http://example.com/p> \"simple\" _:graph .\n",
            "<http://example.com/s> <http://example.com/p> \"language\"@EN-us .\n",
            "<http://example.com/s> <http://example.com/p> \"42\"^^<http://www.w3.org/2001/XMLSchema#integer> .\n",
            "<http://example.com/\\u0073> <http://example.com/p> \"line\\nκαφέ \\u2615\" . # comment\n",
        );
        let expected = NQuadsParser::new()
            .for_reader(input.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let mut actual = Vec::new();
        NQuadsParser::new()
            .parse_reader(input.as_bytes(), |quad| {
                actual.push(quad.into_owned());
                Ok::<_, TurtleParseError>(())
            })
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn ntriples_borrowed_callback_matches_owned_parser() {
        let input = concat!(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\r\n",
            "_:subject <http://example.com/p> _:object .\n",
            "<http://example.com/s> <http://example.com/p> \"literal\"@FR .",
        );
        let expected = NTriplesParser::new()
            .for_reader(input.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let mut actual = Vec::new();
        NTriplesParser::new()
            .parse_reader(input.as_bytes(), |triple| {
                actual.push(triple.into_owned());
                Ok::<_, TurtleParseError>(())
            })
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[derive(Debug)]
    enum CallbackError {
        Parse,
        Stop(&'static str),
    }

    impl From<TurtleParseError> for CallbackError {
        fn from(_error: TurtleParseError) -> Self {
            Self::Parse
        }
    }

    #[test]
    fn callback_error_stops_parsing_unchanged() {
        let input = concat!(
            "<http://example.com/s1> <http://example.com/p> <http://example.com/o> .\n",
            "<http://example.com/s2> <http://example.com/p> <http://example.com/o> .\n",
        );
        let mut calls = 0;
        let error = NTriplesParser::new()
            .parse_reader(input.as_bytes(), |_| {
                calls += 1;
                Err(CallbackError::Stop("sentinel"))
            })
            .unwrap_err();

        let value = match error {
            CallbackError::Stop(value) => value,
            CallbackError::Parse => "parser error",
        };
        assert_eq!(value, "sentinel");
        assert_eq!(calls, 1);
    }

    #[test]
    fn late_parse_error_keeps_streaming_prefix_visible() {
        let input = concat!(
            "<http://example.com/s1> <http://example.com/p> <http://example.com/o> .\n",
            "<http://example.com/s2> <http://example.com/p> <http://example.com/o>\n",
        );
        let mut calls = 0;
        let error = NTriplesParser::new()
            .parse_reader(input.as_bytes(), |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 1);
        assert!(error.to_string().contains("followed by a dot"));
    }

    #[test]
    fn invalid_statement_is_not_exposed_to_callback() {
        let input = "<http://example.com/s> <http://example.com/p> <http://example.com/o>\n";
        let owned_error = NTriplesParser::new()
            .for_reader(input.as_bytes())
            .find_map(Result::err)
            .unwrap();
        let mut calls = 0;
        let borrowed_error = NTriplesParser::new()
            .parse_reader(input.as_bytes(), |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 0);
        assert_eq!(borrowed_error.to_string(), owned_error.to_string());
    }

    #[test]
    fn extra_token_after_dot_is_not_exposed_to_callback() {
        let input =
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> . trailing\n";
        let mut calls = 0;
        let error = NTriplesParser::new()
            .parse_reader(input.as_bytes(), |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 0);
        assert!(error.to_string().contains("Only a single triple"));
    }

    struct ErrorAfterData {
        data: Option<&'static [u8]>,
    }

    impl Read for ErrorAfterData {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if let Some(data) = self.data.take() {
                buf[..data.len()].copy_from_slice(data);
                Ok(data.len())
            } else {
                Err(io::Error::other("reader failed"))
            }
        }
    }

    #[test]
    fn reader_error_is_returned_after_completed_callbacks() {
        let reader = ErrorAfterData {
            data: Some(b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n"),
        };
        let mut calls = 0;
        let error = NTriplesParser::new()
            .parse_reader(reader, |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 1);
        assert!(matches!(error, TurtleParseError::Io(_)));
        assert!(error.to_string().contains("reader failed"));
    }

    #[test]
    fn reader_error_before_statement_boundary_is_fail_closed() {
        let reader = ErrorAfterData {
            data: Some(b"<http://example.com/s> <http://example.com/p> <http://example.com/o> ."),
        };
        let mut calls = 0;
        let error = NTriplesParser::new()
            .parse_reader(reader, |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 0);
        assert!(matches!(error, TurtleParseError::Io(_)));
    }

    struct TinyChunks<'a> {
        data: &'a [u8],
    }

    impl Read for TinyChunks<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let Some((byte, remaining)) = self.data.split_first() else {
                return Ok(0);
            };
            buf[0] = *byte;
            self.data = remaining;
            Ok(1)
        }
    }

    #[test]
    #[expect(
        clippy::non_ascii_literal,
        reason = "this test exercises raw Unicode parser input"
    )]
    fn tiny_reader_chunks_preserve_escapes_and_unicode() {
        let input = "<http://example.com/\\u0073> <http://example.com/p> \"καφέ \\u2615\" .\n";
        let expected = NTriplesParser::new()
            .for_reader(input.as_bytes())
            .next()
            .unwrap()
            .unwrap();
        let mut actual = None;
        NTriplesParser::new()
            .parse_reader(
                TinyChunks {
                    data: input.as_bytes(),
                },
                |triple| {
                    actual = Some(triple.into_owned());
                    Ok::<_, TurtleParseError>(())
                },
            )
            .unwrap();

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn maximum_buffer_size_remains_enforced() {
        let input = format!(
            "<http://example.com/s> <http://example.com/p> \"{}\" .\n",
            "a".repeat(5000)
        );
        let mut calls = 0;
        let error = NTriplesParser::new()
            .with_max_buffer_size(4096)
            .parse_reader(input.as_bytes(), |_| {
                calls += 1;
                Ok::<_, TurtleParseError>(())
            })
            .unwrap_err();

        assert_eq!(calls, 0);
        assert!(matches!(error, TurtleParseError::Io(_)));
    }

    #[test]
    fn syntax_errors_match_the_owned_parser() {
        for input in [
            "<http://example.com/s>\n",
            "<http://example.com/s> <http://example.com/p>\n",
            "<http://example.com/s> <http://example.com/p> \"value\"^^\n",
            "<relative> <http://example.com/p> <http://example.com/o> .\n",
            "<http://example.com/s> <http://example.com/p> \"bad\\q\" .\n",
            "<http://example.com/s> <http://example.com/p> <http://example.com/o>\n",
        ] {
            let owned = NTriplesParser::new()
                .for_reader(input.as_bytes())
                .find_map(Result::err)
                .unwrap();
            let borrowed = NTriplesParser::new()
                .parse_reader(input.as_bytes(), |_| Ok::<_, TurtleParseError>(()))
                .unwrap_err();
            assert_eq!(borrowed.to_string(), owned.to_string(), "input: {input:?}");
        }
    }

    #[test]
    fn token_debug_output_remains_stable() {
        let input = "<http://example.com/s> <http://example.com/p> <http://example.com/o> . <http://example.com/extra>\n";
        let error = NTriplesParser::new()
            .for_reader(input.as_bytes())
            .find_map(Result::err)
            .unwrap();
        let message = error.to_string();

        assert!(message.contains("IriRef(\"http://example.com/extra\")"));
        assert!(!message.contains("Borrowed"));
        assert!(!message.contains("Owned"));
    }

    #[cfg(feature = "rdf-12")]
    #[test]
    fn rdf_12_directional_literal_is_borrowed() {
        use oxrdf::BaseDirection;

        let input = "<http://example.com/s> <http://example.com/p> \"value\"@EN--rtl .\n";
        let mut direction = None;
        NTriplesParser::new()
            .parse_reader(input.as_bytes(), |triple| {
                if let TermRef::Literal(literal) = triple.object {
                    direction = literal.direction();
                }
                Ok::<_, TurtleParseError>(())
            })
            .unwrap();

        assert_eq!(direction, Some(BaseDirection::Rtl));
    }

    #[cfg(feature = "rdf-12")]
    #[test]
    fn rdf_12_triple_terms_match_owned_parser() {
        let input = "<http://example.com/s> <http://example.com/p> <<( <http://example.com/qs> <http://example.com/qp> <<( <http://example.com/ns> <http://example.com/np> \"nested\"@EN )>> )>> .\n";
        let owned = NTriplesParser::new()
            .for_reader(input.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let mut borrowed = Vec::new();
        NTriplesParser::new()
            .parse_reader(input.as_bytes(), |triple| {
                assert!(
                    matches!(
                        triple.object,
                        TermRef::Triple(triple_term) if triple_term.object.is_triple()
                    ),
                    "expected nested RDF 1.2 triple terms"
                );
                borrowed.push(triple.into_owned());
                Ok::<_, TurtleParseError>(())
            })
            .unwrap();

        assert_eq!(borrowed, owned);

        let input = format!(
            "{} <http://example.com/g> .\n",
            input.trim_end().trim_end_matches(" .")
        );
        let owned = NQuadsParser::new()
            .for_reader(input.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut borrowed = Vec::new();
        NQuadsParser::new()
            .parse_reader(input.as_bytes(), |quad| {
                borrowed.push(quad.into_owned());
                Ok::<_, TurtleParseError>(())
            })
            .unwrap();
        assert_eq!(borrowed, owned);
    }

    #[cfg(feature = "rdf-12")]
    #[test]
    fn malformed_rdf_12_triple_terms_fail_closed_like_owned_parser() {
        for input in [
            "<http://example.com/s> <http://example.com/p> <<( <http://example.com/qs> <http://example.com/qp> <http://example.com/qo> .\n",
            "<http://example.com/s> <http://example.com/p> <<( <http://example.com/qs> <http://example.com/qp> )>> .\n",
            "<http://example.com/s> <http://example.com/p> <<( <http://example.com/qs> _:predicate <http://example.com/qo> )>> .\n",
            "<http://example.com/s> <http://example.com/p> <<( <http://example.com/qs> <http://example.com/qp> <http://example.com/qo>\n)>> .\n",
        ] {
            let owned = NTriplesParser::new()
                .for_reader(input.as_bytes())
                .find_map(Result::err)
                .unwrap();
            let mut calls = 0;
            let borrowed = NTriplesParser::new()
                .parse_reader(input.as_bytes(), |_| {
                    calls += 1;
                    Ok::<_, TurtleParseError>(())
                })
                .unwrap_err();

            assert_eq!(calls, 0, "input: {input:?}");
            assert_eq!(borrowed.to_string(), owned.to_string(), "input: {input:?}");
        }
    }

    #[test]
    fn owned_quad_type_remains_usable() {
        let quad: Quad = NQuadsParser::new()
            .for_reader(
                &b"<http://example.com/s> <http://example.com/p> <http://example.com/o> ."[..],
            )
            .next()
            .unwrap()
            .unwrap();
        assert!(quad.graph_name.is_default_graph());
    }
}
