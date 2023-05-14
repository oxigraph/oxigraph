use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn serd_good_testsuite() -> Result<()> {
    check_testsuite("http://drobilla.net/sw/serd/test/good/manifest.ttl", &[])
}

#[test]
fn serd_bad_testsuite() -> Result<()> {
    check_testsuite("http://drobilla.net/sw/serd/test/bad/manifest.ttl", &[])
}
