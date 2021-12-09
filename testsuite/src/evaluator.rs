use crate::manifest::Test;
use crate::report::TestResult;
use anyhow::{anyhow, Result};
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
                let outcome = if let Some(handler) = self.handlers.get(test.kind.as_str()) {
                    handler(&test)
                } else {
                    Err(anyhow!("The test type {} is not supported", test.kind))
                };
                Ok(TestResult {
                    test: test.id,
                    outcome,
                    date: OffsetDateTime::now_utc(),
                })
            })
            .collect()
    }
}
