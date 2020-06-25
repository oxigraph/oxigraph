use chrono::{DateTime, Utc};
use oxigraph::model::NamedNode;
use oxigraph::Result;

#[derive(Debug)]
pub struct TestResult {
    pub test: NamedNode,
    pub outcome: Result<()>,
    pub date: DateTime<Utc>,
}
