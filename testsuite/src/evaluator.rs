use crate::manifest::Test;
use crate::report::TestResult;
use anyhow::{bail, Result};
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Default)]
pub struct TestEvaluator {
    handlers: HashMap<String, Box<dyn Fn(&Test) -> Result<()>>>,
}

impl TestEvaluator {
    pub fn register(
        &mut self,
        test_type: impl Into<String>,
        handler: impl Fn(&Test) -> Result<()> + 'static,
    ) {
        self.handlers.insert(test_type.into(), Box::new(handler));
    }

    pub fn evaluate(
        &self,
        manifest: impl Iterator<Item = Result<Test>>,
    ) -> Result<Vec<TestResult>> {
        manifest
            .map(|test| {
                let test = test?;
                let handlers = test
                    .kinds
                    .iter()
                    .filter_map(|kind| self.handlers.get(kind.as_str()))
                    .collect::<Vec<_>>();
                if handlers.len() > 1 {
                    bail!("The test {test} has multiple possible handlers")
                }
                if let Some(handler) = handlers.into_iter().next() {
                    let outcome = handler(&test);
                    return Ok(TestResult {
                        test: test.id,
                        outcome,
                        date: OffsetDateTime::now_utc(),
                    });
                }
                bail!("The test {test} is not supported")
            })
            .collect()
    }
}
