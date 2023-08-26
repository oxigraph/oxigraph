#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn oxigraph_parser_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/parser/manifest.ttl",
        &[],
    )
}

#[test]
fn oxigraph_parser_recovery_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/parser-recovery/manifest.ttl",
        &[],
    )
}

#[test]
fn oxigraph_sparql_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/sparql/manifest.ttl",
        &[],
    )
}

#[test]
fn oxigraph_sparql_results_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/sparql-results/manifest.ttl",
        &[],
    )
}

#[test]
fn oxigraph_optimizer_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/sparql-optimization/manifest.ttl",
        &[],
    )
}
