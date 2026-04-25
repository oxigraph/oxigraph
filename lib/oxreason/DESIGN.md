# oxreason: OWL 2 RL and SHACL for Oxigraph

Tracks: [oxigraph issue #130](https://github.com/oxigraph/oxigraph/issues/130).

Motivation: Oxigraph users who need OWL 2 RL reasoning or SHACL validation
today reach for Python `owlrl` plus `pyshacl` on an `rdflib` graph. This
crate moves that pipeline into Rust so the same deployment works natively
in Rust, WASM, and pyoxigraph without a Python sidecar.

## 1. Goals

1. Deliver an OWL 2 RL reasoner that operates directly on an Oxigraph `Store`
   or on any `Dataset` / `Graph` from `oxrdf`.
2. Deliver a SHACL validator surface compatible with SHACL Core, running on
   the same storage backends.
3. Keep the crate `no_std`-free but portable across the same targets Oxigraph
   supports today (native, wasm32 unknown, pyoxigraph).
4. Expose a small, stable public API: `Reasoner`, `Validator`, a handful of
   configuration structs, and a thin integration into `oxigraph::store::Store`
   behind a cargo feature.
5. Avoid taking on a large external dependency tree. The reasoning layer
   should be auditable for sovereign and air gapped deployments.

## 2. Non goals for the initial pass

1. OWL 2 DL reasoning (tableau or hypertableau based). That needs a separate
   crate and is out of scope for issue #130.
2. Incremental or truth maintenance reasoning. The first cut materialises the
   closure and returns the resulting store.
3. SPARQL rules or SWRL. A later revision can add them once the OWL 2 RL core
   is stable.
4. Custom rule DSL. Rules are encoded in Rust for now, serialisation comes
   later if there is demand.

## 3. Relation to existing work

Three Rust crates already cover adjacent ground. The initial scaffold does
not depend on any of them, but the design keeps each as an integration option
for a future milestone.

`reasonable` by Gabe Fierro: hash join based OWL 2 RL engine, Python bindings
live, license Apache 2.0. Strengths: known performance numbers, mature rule
coverage. Weaknesses: independent internal representation, would require a
translation layer to and from Oxigraph terms, and its datalog style engine is
a bigger dependency than we want to bring in blindly for sovereign use.

`inferix` and `horned-owl`: horned-owl is an OWL ontology parser with no
reasoner, inferix is an early stage Datalog engine. Neither maps cleanly to
the Oxigraph store model without adapter code.

`pyshacl` has no Rust equivalent today. Implementing SHACL Core directly on
top of Oxigraph's SPARQL evaluator is the most natural path because every
SHACL Core constraint can be expressed as a SPARQL ASK or SELECT over the
data graph.

Decision: build the first version native to Oxigraph. Treat `reasonable` as a
reference implementation, not a runtime dependency. Revisit after the native
engine has a working forward chaining loop and a benchmark harness.

## 4. Public API sketch

```rust
use oxigraph::store::Store;
use oxreason::{Reasoner, ReasonerConfig, Validator, ValidatorConfig};

// Reasoning: materialise the OWL 2 RL closure of a store.
let reasoner = Reasoner::new(ReasonerConfig::owl2_rl());
let report = reasoner.expand(&store)?;
println!("added {} triples", report.added);

// Validation: run SHACL shapes against the (possibly expanded) store.
let validator = Validator::new(ValidatorConfig::shacl_core(shapes_store));
let conforms = validator.validate(&store)?;
assert!(conforms.is_conforming());
```

Data flow options for `expand`:

1. In place materialisation. `expand(&store)` writes inferred quads back into
   the store's default graph or a dedicated inference graph (configurable).
   This is the common shape callers need, because queries should see inferred
   triples without having to union an extra graph.
2. Out of place materialisation. `expand_into(src, dst)` reads from `src` and
   writes inferred quads into `dst`. Useful for on the fly reasoning over a
   read only store, or for producing a cached closure for downstream
   consumers.

The initial crate exposes both as trait methods on a `Reasoner` value.

## 5. Rule coverage target for OWL 2 RL

The W3C OWL 2 RL profile defines roughly 78 rules across class, property,
equality, datatype, and schema axioms. The first implementation targets the
subset most commonly exercised by OWL 2 RL consumers, plus the rules needed
to make that subset sound:

Class axioms: cax sco, cax eqc1, cax eqc2, cax dw, cax int, cax uni.
Property axioms: prp dom, prp rng, prp trp, prp symp, prp inv1, prp inv2,
prp spo1, prp eqp1, prp eqp2, prp fp, prp ifp.
Equality: eq ref, eq sym, eq trans, eq rep s, eq rep p, eq rep o.
Schema: scm cls, scm sco, scm op, scm dp, scm eqc1, scm eqc2, scm eqp1,
scm eqp2, scm dom1, scm rng1.

Datatype reasoning (dt type1 to dt diff) lands in a second pass, after the
object property rules are proven out.

The rules will be encoded as a table of `Rule { antecedent, consequent }`
values. Each antecedent is a sequence of triple patterns over `NamedNode`,
`BlankNode`, `Literal`, and `Variable`. The evaluator runs fixpoint
iteration over this table, hashing intermediate triples to detect saturation.

## 6. Rule evaluation strategy

1. Represent triples using `oxrdf::TripleRef` internally. Interning IRIs and
   blank nodes through the existing `oxrdf::interning` module keeps memory
   predictable for larger graphs.
2. Build a simple indexed intermediate structure keyed on predicate to make
   pattern lookups cheap. For TransitiveProperty closure and equality chains
   a per subject index helps too.
3. Run semi naive evaluation: on each round, only consider antecedents that
   can match at least one newly added triple. Stop when a round adds nothing.
4. Write back to the store in batches, not triple by triple, to avoid the
   write amplification that would come from hammering RocksDB.

This is a textbook forward chainer. The initial scaffold includes a
`FixpointEngine` trait so the implementation can be swapped for a smarter
engine later without changing the public API.

## 7. SHACL validator strategy

SHACL Core constraints map to SPARQL queries Oxigraph can already execute.
The validator translates each node shape and property shape into one or more
ASK or SELECT queries, runs them through `spareval`, and aggregates results
into a `ValidationReport` that mirrors the shape of `sh:ValidationReport`.

The initial scaffold only defines the types (`Validator`, `ValidatorConfig`,
`ValidationReport`, `ValidationResult`) and a stub that returns a conforming
report for any input. Rule implementations land in a follow up once the
reasoner loop is working.

## 8. Integration with oxigraph crate

`oxreason` is an independent workspace member. The `oxigraph` crate gains an
optional feature `oxreason` (off by default) that re exports the public API
and adds two convenience methods on `Store`:

```rust
impl Store {
    #[cfg(feature = "oxreason")]
    pub fn reason(&self) -> Result<ReasoningReport, ReasonError> { ... }

    #[cfg(feature = "oxreason")]
    pub fn validate(&self, shapes: &Store) -> Result<ValidationReport, ValidateError> { ... }
}
```

Consumers who do not want the reasoning surface pay zero cost. pyoxigraph
and js bindings can opt in per build.

## 9. Milestones

M0 (this change): workspace scaffold, public types, error enums, stubs that
compile and return `Err(NotImplemented)` where logic is missing. Design doc
committed so the rule coverage and API shape can be reviewed before code.

M1: minimal forward chainer with prp dom, prp rng, prp spo1, prp trp,
cax sco. Integration test using a tiny ontology plus instance file.

M2: full OWL 2 RL object property and class rule set, equality rules off by
default (flag them as optional because they explode graph size on noisy
data).

M3: datatype rules, key rules, scm rules, plus tuning (indexed antecedent
matching, batched writes to the store).

M4: SHACL Core validator, feature parity with pyshacl for the most used
constraint components (sh:class, sh:datatype, sh:minCount, sh:maxCount,
sh:pattern, sh:in, sh:node, sh:qualifiedValueShape).

M5: WASM build, pyoxigraph bindings, benchmark harness. At this point
Python consumers can drop their `owlrl` plus `pyshacl` dependency and
call the reasoner through pyoxigraph directly.

## 10. Open questions

1. Do we store inferred triples in a dedicated named graph so they can be
   retracted, or do we fold them into the source graphs? Dedicated graph
   keeps human edits distinguishable from inferences, at the cost of an
   extra graph union at query time.
2. Should equality rules (eq rep s, eq rep p, eq rep o) be opt in at the
   `ReasonerConfig` level? They are correct but expensive on data with
   owl:sameAs chains. Initial default: opt in.
3. How do we surface rule provenance? Some consumers want to know which
   rule introduced a given triple for auditing. A `ProvenanceSink` trait
   on the reasoner covers this without forcing it on callers that do not
   care.
4. What is the upstream policy on adding a reasoning dependency to the
   oxigraph core crate? If maintainers prefer to keep `oxigraph` reasoning
   free, the feature gated re export stays, but the convenience methods can
   move into a separate `oxigraph-reason` helper crate.

## 11. References

1. [OWL 2 Web Ontology Language Profiles, section 4.3 OWL 2 RL](https://www.w3.org/TR/owl2-profiles/#OWL_2_RL).
2. [OWL 2 Web Ontology Language Profiles, section 4.3.1 Reasoning in OWL 2 RL and RDF Graphs using Rules](https://www.w3.org/TR/owl2-profiles/#Reasoning_in_OWL_2_RL_and_RDF_Graphs_using_Rules).
3. [Shapes Constraint Language (SHACL)](https://www.w3.org/TR/shacl/).
4. [reasonable (Rust OWL 2 RL engine)](https://github.com/gtfierro/reasonable).
5. [owlrl (Python)](https://owl-rl.readthedocs.io/).
6. [pyshacl (Python)](https://github.com/RDFLib/pySHACL).
