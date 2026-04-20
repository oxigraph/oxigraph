oxreason
========

OWL 2 RL reasoning and SHACL validation for [Oxigraph](https://oxigraph.org/).

Status: scaffolding only. Public types, error enums, and method signatures
are in place. Rule evaluation and SHACL constraint checking return
`ReasonError::NotImplemented` or `ValidateError::NotImplemented`. See
`DESIGN.md` in this folder for the plan and milestones, and `TESTING.md`
for the test strategy including the per rule and per constraint integration
tests under `tests/`.

Tracks [oxigraph issue #130](https://github.com/oxigraph/oxigraph/issues/130).

Quick API shape
---------------

```rust
use oxrdf::Graph;
use oxreason::{Reasoner, ReasonerConfig};

let config = ReasonerConfig::owl2_rl();
let reasoner = Reasoner::new(config);

let mut graph = Graph::default();
// ... load triples into graph ...

match reasoner.expand(&mut graph) {
    Ok(report) => println!("inferred {} triples", report.added),
    Err(err) => eprintln!("reasoning failed: {err}"),
}
```

License
-------

Dual licensed under MIT or Apache 2.0, matching the rest of the Oxigraph
workspace.
