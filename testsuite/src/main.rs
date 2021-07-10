use anyhow::Result;
use clap::{App, Arg};
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::parser_evaluator::register_parser_tests;
use oxigraph_testsuite::report::build_report;
use oxigraph_testsuite::sparql_evaluator::register_sparql_tests;

fn main() -> Result<()> {
    let matches = App::new("Oxigraph testsuite runner")
        .arg(
            Arg::with_name("manifest")
                .help("URI of the testsuite manifest to run")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let mut evaluator = TestEvaluator::default();
    register_parser_tests(&mut evaluator);
    register_sparql_tests(&mut evaluator);
    let manifest = TestManifest::new(vec![matches.value_of("manifest").unwrap()]);
    let results = evaluator.evaluate(manifest)?;
    print!("{}", build_report(results));
    Ok(())
}
