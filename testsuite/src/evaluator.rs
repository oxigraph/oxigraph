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
                Ok(TestResult {
                    test: test.id.clone(),
                    outcome: test
                        .kinds
                        .iter()
                        .filter_map(|kind| self.handlers.get(kind.as_str()))
                        .map(|h| h(&test))
                        .reduce(Result::and)
                        .unwrap_or_else(|| bail!("No handler found for test {}", test.id)),
                    date: OffsetDateTime::now_utc(),
                })
            })
            .collect()
    }
}
