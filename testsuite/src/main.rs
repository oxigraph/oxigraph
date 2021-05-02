use anyhow::Result;
use argh::FromArgs;
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::parser_evaluator::register_parser_tests;
use oxigraph_testsuite::report::build_report;
use oxigraph_testsuite::sparql_evaluator::register_sparql_tests;

#[derive(FromArgs)]
/// Oxigraph testsuite runner
struct Args {
    /// URI of the testsuite manifest to run
    #[argh(positional)]
    manifest: String,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    let mut evaluator = TestEvaluator::default();
    register_parser_tests(&mut evaluator);
    register_sparql_tests(&mut evaluator);
    let manifest = TestManifest::new(vec![args.manifest]);
    let results = evaluator.evaluate(manifest)?;
    print!("{}", build_report(results));
    Ok(())
}
