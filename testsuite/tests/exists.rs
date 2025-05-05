#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn exists_bnodes_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/afs/SPARQL-exists/tests/exists-bnodes/manifest.ttl",
        &[],
    )
}

#[test]
fn exists_filter_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/afs/SPARQL-exists/tests/exists-filter/manifest.ttl",
        &[],
    )
}

#[test]
fn exists_positions_testsuite() -> Result<()> {
    check_testsuite(
        "https://github.com/afs/SPARQL-exists/tests/exists-positions/manifest.ttl",
        &[],
    )
}
