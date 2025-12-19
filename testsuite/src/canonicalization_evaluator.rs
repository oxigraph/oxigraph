use crate::evaluator::TestEvaluator;
use crate::files::{load_dataset, read_file, read_file_to_string};
use crate::manifest::Test;
use crate::report::format_diff;
use crate::vocab::rdfc;
use anyhow::{Context, Result, anyhow, bail, ensure};
use json_event_parser::{JsonEvent, ReaderJsonParser};
use oxigraph::io::RdfFormat;
use oxrdf::dataset::CanonicalizationAlgorithm;
use oxrdf::graph::CanonicalizationHashAlgorithm;
use oxrdf::{Dataset, Term};
use std::collections::BTreeMap;

pub fn register_canonicalization_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10EvalTest",
        evaluate_canonicalization_eval_test,
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10NegativeEvalTest",
        |_| Ok(()), // TODO: not a proper implementation
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10MapTest",
        evaluate_canonicalization_map_test,
    );
}

fn evaluate_canonicalization_eval_test(test: &Test) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let mut dataset =
        load_dataset(action, RdfFormat::NQuads, false, false).context("Parse error")?;
    dataset.canonicalize(CanonicalizationAlgorithm::Rdfc10 {
        hash_algorithm: hash_algorithm(test)?,
    });
    let actual = canonical_nquads(&dataset);

    let results = test.result.as_ref().context("No tests result found")?;
    let expected = read_file_to_string(results)
        .with_context(|| format!("Read error on file {results}"))?
        .replace('\r', ""); // Windows compatibility

    ensure!(
        expected == actual,
        "The two files are not equal. Diff:\n{}",
        format_diff(&expected, &actual, "c14n")
    );
    Ok(())
}

fn canonical_nquads(dataset: &Dataset) -> String {
    let mut nquads = Vec::new();
    for q in dataset {
        nquads.push(format!("{q} .\n"));
    }
    nquads.sort();
    nquads.join("")
}

fn hash_algorithm(test: &Test) -> Result<CanonicalizationHashAlgorithm> {
    Ok(
        if let Some(Term::Literal(hash_algorithm)) =
            test.option.get(&rdfc::HASH_ALGORITHM.into_owned())
        {
            match hash_algorithm.value() {
                "SHA256" => CanonicalizationHashAlgorithm::Sha256,
                "SHA384" => CanonicalizationHashAlgorithm::Sha384,
                v => bail!("Unknown hash algorithm: {v}"),
            }
        } else {
            CanonicalizationHashAlgorithm::Sha256
        },
    )
}

fn evaluate_canonicalization_map_test(test: &Test) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let dataset = load_dataset(action, RdfFormat::NQuads, false, false).context("Parse error")?;
    let actual = dataset
        .canonicalize_blank_nodes(CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: hash_algorithm(test)?,
        })
        .into_iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.as_str().to_owned()))
        .collect::<BTreeMap<_, _>>();

    let results = test.result.as_ref().context("No tests result found")?;
    let expected =
        read_blank_node_map(results).with_context(|| format!("Read error on file {results}"))?;

    ensure!(
        expected == actual,
        "The two blank node maps are not equal. Diff:\n{}",
        format_diff(&format!("{expected:?}"), &format!("{actual:?}"), "c14n")
    );
    Ok(())
}

fn read_blank_node_map(url: &str) -> Result<BTreeMap<String, String>> {
    let mut nesting = 0;
    let mut result = BTreeMap::new();
    let mut current_key = None;
    let mut parser = ReaderJsonParser::new(read_file(url)?);
    loop {
        match parser.parse_next()? {
            JsonEvent::StartObject => {
                nesting += 1;
                ensure!(nesting == 1, "Nested objects in blank node maps");
            }
            JsonEvent::ObjectKey(k) => {
                current_key = Some(k.into());
            }
            JsonEvent::String(v) => {
                result.insert(
                    current_key
                        .take()
                        .ok_or_else(|| anyhow!("Unexpected string in blank node map"))?,
                    v.into(),
                );
            }
            JsonEvent::EndObject => nesting -= 1,
            JsonEvent::Eof => return Ok(result),
            _ => bail!("Blank node maps must be a JSON object of strings"),
        }
    }
}
