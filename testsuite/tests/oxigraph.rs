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
fn oxigraph_parser_unchecked_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/parser-unchecked/manifest.ttl",
        &[],
    )
}

#[test]
fn oxigraph_parser_error_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/parser-error/manifest.ttl",
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

#[cfg(target_pointer_width = "64")] // Hashing is different in 32 bits, leading to different ordering
#[test]
fn oxigraph_optimizer_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/oxigraph/oxigraph/tests/sparql-optimization/manifest.ttl",
        &[],
    )
}
