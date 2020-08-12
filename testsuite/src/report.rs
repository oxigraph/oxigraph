use anyhow::Result;
use chrono::{DateTime, Utc};
use oxigraph::model::NamedNode;
use oxigraph::MemoryStore;
use text_diff::{diff, Difference};

#[derive(Debug)]
pub struct TestResult {
    pub test: NamedNode,
    pub outcome: Result<()>,
    pub date: DateTime<Utc>,
}

pub fn store_diff(expected: &MemoryStore, actual: &MemoryStore) -> String {
    let (_, changeset) = diff(
        &normalize_store_text(expected),
        &normalize_store_text(actual),
        "\n",
    );
    let mut ret = String::new();
    ret.push_str("Note: missing quads in yellow and extra quads in blue\n");
    for seq in changeset {
        match seq {
            Difference::Same(x) => {
                ret.push_str(&x);
                ret.push('\n');
            }
            Difference::Add(x) => {
                ret.push_str("\x1B[94m");
                ret.push_str(&x);
                ret.push_str("\x1B[0m");
                ret.push('\n');
            }
            Difference::Rem(x) => {
                ret.push_str("\x1B[93m");
                ret.push_str(&x);
                ret.push_str("\x1B[0m");
                ret.push('\n');
            }
        }
    }
    ret
}

fn normalize_store_text(store: &MemoryStore) -> String {
    let mut quads: Vec<_> = store
        .quads_for_pattern(None, None, None, None)
        .map(|q| q.to_string())
        .collect();
    quads.sort();
    quads.join("\n")
}
