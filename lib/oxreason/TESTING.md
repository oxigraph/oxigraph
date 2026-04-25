# Testing strategy for oxreason

The crate has three layers of tests.

## 1. Unit tests (in `src/*.rs`)

Each module has a `#[cfg(test)] mod tests { ... }` block covering:

1. Config builder defaults and setters.
2. RuleSet contents (owl2 rl core vs rdfs vs equality classification).
3. ValidationReport conforming logic.
4. Stub methods returning `NotImplemented`.

These run under `cargo test -p oxreason`.

## 2. Integration tests (in `tests/`)

Structured around the rules and constraints that ship in each milestone:

- `tests/owl2_rl_rules.rs`: one test per OWL 2 RL rule in the M1 and M2
  scope (prp-dom, prp-rng, prp-spo1, prp-trp, cax-sco, prp-symp, prp-inv1,
  prp-inv2, cax-eqc1, cax-eqc2). Every test loads a tiny TTL fixture,
  expands it, and asserts the inferred triples.
- `tests/shacl_core.rs`: one test per SHACL Core constraint component in
  the M4 scope (sh:minCount, sh:maxCount, sh:class, sh:datatype, sh:in,
  sh:pattern, sh:node, sh:qualifiedValueShape). Each test loads a shapes
  graph plus a data graph and asserts the validation report.

While the scaffold is in place, every integration test currently asserts
`matches!(err, ReasonError::NotImplemented(_))` or the SHACL equivalent.
Each test carries a `// TODO M1:` comment describing the expected inferred
triples or violation once the corresponding rule or constraint lands. When
a milestone is implemented, the TODO lines become the real assertions.

Keep one fixture per rule for clarity; do not merge them into a big file.
That way a failure points straight at the rule being tested.

## 3. Conformance suite (future, M5)

The W3C OWL 2 Test Cases and the SHACL test suite run through the oxigraph
`testsuite` crate today for SPARQL. Once oxreason has M2 plus M4
coverage, add a new `testsuite/oxreason-tests` module that runs the
relevant manifests through `Reasoner::expand` and `Validator::validate`.
This is how we claim standards conformance.

## Running tests

```
cargo test -p oxreason
```

For a single integration test:

```
cargo test -p oxreason --test owl2_rl_rules -- prp_trp
```

## Benchmarks

Benchmarks will live in `benches/` once the rule engine has enough substance
to be worth measuring. Use `codspeed-criterion-compat` to match the rest of
the workspace.
