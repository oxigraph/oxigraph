use anyhow::Result;
use clap::Parser;
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::parser_evaluator::register_parser_tests;
use oxigraph_testsuite::report::build_report;
use oxigraph_testsuite::sparql_evaluator::register_sparql_tests;

#[derive(Parser)]
/// Oxigraph testsuite runner
struct Args {
    /// URI of the testsuite manifest(s) to run
    manifest: Vec<String>,
}

fn main() -> Result<()> {
    let matches = Args::parse();

    let mut evaluator = TestEvaluator::default();
    register_parser_tests(&mut evaluator);
    register_sparql_tests(&mut evaluator);
    let manifest = TestManifest::new(matches.manifest);
    let results = evaluator.evaluate(manifest)?;
    print!("{}", build_report(results));
    Ok(())
}
