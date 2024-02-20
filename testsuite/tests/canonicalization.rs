#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn rdf_canon_w3c_testsuite() -> Result<()> {
    check_testsuite("https://w3c.github.io/rdf-canon/tests/manifest.ttl", &[])
}
