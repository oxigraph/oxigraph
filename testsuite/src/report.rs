use anyhow::Result;
use oxigraph::model::{Dataset, NamedNode};
use std::fmt::Write;
use text_diff::{Difference, diff};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Debug)]
pub struct TestResult {
    pub test: NamedNode,
    pub outcome: Result<()>,
    pub date: OffsetDateTime,
}

pub(super) fn dataset_diff(expected: &Dataset, actual: &Dataset) -> String {
    format_diff(
        &normalize_dataset_text(expected),
        &normalize_dataset_text(actual),
        "quads",
    )
}

fn normalize_dataset_text(store: &Dataset) -> String {
    let mut quads: Vec<_> = store.iter().map(|q| q.to_string()).collect();
    quads.sort();
    quads.join("\n")
}

pub(super) fn format_diff(expected: &str, actual: &str, kind: &str) -> String {
    let (_, changeset) = diff(expected, actual, "\n");
    let mut ret = String::new();
    writeln!(
        &mut ret,
        "Note: missing {kind} in yellow and extra {kind} in blue"
    )
    .unwrap();
    for seq in changeset {
        match seq {
            Difference::Same(x) => {
                writeln!(&mut ret, "{x}").unwrap();
            }
            Difference::Add(x) => {
                writeln!(&mut ret, "\x1B[94m{x}\x1B[0m").unwrap();
            }
            Difference::Rem(x) => {
                writeln!(&mut ret, "\x1B[93m{x}\x1B[0m").unwrap();
            }
        }
    }
    ret
}

#[expect(unused_must_use)]
pub fn build_report(results: impl IntoIterator<Item = TestResult>) -> String {
    let mut buffer = String::new();
    writeln!(&mut buffer, "@prefix dc: <http://purl.org/dc/terms/> .");
    writeln!(
        &mut buffer,
        "@prefix doap: <http://usefulinc.com/ns/doap#> ."
    );
    writeln!(&mut buffer, "@prefix earl: <http://www.w3.org/ns/earl#> .");
    writeln!(&mut buffer, "@prefix foaf: <http://xmlns.com/foaf/0.1/> .");
    writeln!(
        &mut buffer,
        "@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> ."
    );
    writeln!(
        &mut buffer,
        "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> ."
    );
    writeln!(&mut buffer);
    writeln!(&mut buffer, "<> foaf:primaryTopic <http://oxigraph.org/> ;");
    writeln!(
        &mut buffer,
        "\tdc:issued \"{}\"^^xsd:dateTime ;",
        OffsetDateTime::now_utc().format(&Rfc3339).unwrap()
    );
    writeln!(
        &mut buffer,
        "\tfoaf:maker <https://thomas.pellissier-tanon.fr/#me> ."
    );
    writeln!(&mut buffer);
    writeln!(
        &mut buffer,
        "<http://oxigraph.org/> a doap:Project, earl:TestSubject, earl:Software ;"
    );
    writeln!(&mut buffer, "\tdoap:name \"Oxigraph\" ;");
    writeln!(&mut buffer, "\tdoap:release [");
    writeln!(
        &mut buffer,
        "\t\tdoap:name \"Oxigraph {}\";",
        env!("CARGO_PKG_VERSION")
    );
    writeln!(
        &mut buffer,
        "\t\tdoap:revision \"{}\" ;",
        env!("CARGO_PKG_VERSION")
    );
    writeln!(&mut buffer, "\t] ;");
    writeln!(
        &mut buffer,
        "\tdoap:developer <https://thomas.pellissier-tanon.fr/#me> ;"
    );
    writeln!(&mut buffer, "\tdoap:homepage <https://oxigraph.org/> ;");
    writeln!(
        &mut buffer,
        "\tdoap:description \"Oxigraph is an embedded triple store.\"@en ;"
    );
    writeln!(&mut buffer, "\tdoap:programming-language \"Rust\" .");
    writeln!(&mut buffer);
    writeln!(
        &mut buffer,
        "<https://thomas.pellissier-tanon.fr/#me> a foaf:Person, earl:Assertor ;"
    );
    writeln!(&mut buffer, "\tfoaf:name \"Thomas Tanon\"; ");
    writeln!(
        &mut buffer,
        "\tfoaf:homepage <https://thomas.pellissier-tanon.fr/> ."
    );
    writeln!(&mut buffer);
    for result in results {
        writeln!(&mut buffer);
        writeln!(&mut buffer, "[");
        writeln!(&mut buffer, "\ta earl:Assertion ;");
        writeln!(
            &mut buffer,
            "\tearl:assertedBy <https://thomas.pellissier-tanon.fr/#me> ;"
        );
        writeln!(&mut buffer, "\tearl:subject <http://oxigraph.org/> ;");
        writeln!(&mut buffer, "\tearl:test {} ;", result.test);
        writeln!(&mut buffer, "\tearl:result [");
        writeln!(&mut buffer, "\t\ta earl:TestResult ;");
        writeln!(
            &mut buffer,
            "\t\tearl:outcome earl:{} ;",
            if result.outcome.is_ok() {
                "passed"
            } else {
                "failed"
            }
        );
        writeln!(
            &mut buffer,
            "\t\tdc:date \"{}\"^^xsd:dateTime",
            result.date.format(&Rfc3339).unwrap()
        );
        writeln!(&mut buffer, "\t] ;");
        writeln!(&mut buffer, "\tearl:mode earl:automatic");
        writeln!(&mut buffer, "] .");
    }
    buffer
}
