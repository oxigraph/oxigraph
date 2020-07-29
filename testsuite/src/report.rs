use anyhow::Result;
use chrono::{DateTime, Utc};
use oxigraph::model::NamedNode;

#[derive(Debug)]
pub struct TestResult {
    pub test: NamedNode,
    pub outcome: Result<()>,
    pub date: DateTime<Utc>,
}
