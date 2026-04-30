//! Forward chaining engine (M1 plus M2 plus M3 plus M4).
//!
//! M1 implements the five schema rules `cax-sco`, `prp-dom`, `prp-rng`,
//! `prp-spo1`, `prp-trp`.
//!
//! M2 extends the set with the remaining OWL 2 RL object property and
//! equivalence rules that do not require class expression support:
//! `prp-symp`, `prp-inv1`, `prp-inv2`, `prp-eqp1`, `prp-eqp2`, `cax-eqc1`,
//! `cax-eqc2`, plus the equality rules `eq-sym`, `eq-trans`, `eq-rep-s`,
//! `eq-rep-p`, `eq-rep-o` gated behind `ReasonerConfig::with_equality_rules`.
//!
//! M3 adds the schema rules `scm-cls`, `scm-sco`, `scm-op`, `scm-dp`,
//! `scm-eqc1`, `scm-eqc2`, `scm-eqp1`, `scm-eqp2`, `scm-dom1`, `scm-rng1`.
//! It also ships the functional property rules `prp-fp` and `prp-ifp`
//! (gated behind the same equality flag because they emit `owl:sameAs`),
//! and the inconsistency detector `cax-dw`, which aborts expansion with a
//! [`DisjointClash`] when an individual carries two types that appear in an
//! `owl:disjointWith` pair.
//!
//! M4 adds one more schema rule (`scm-spo`, transitivity of
//! `rdfs:subPropertyOf`), the class expression rules
//! `cls-hv1`, `cls-hv2`, `cls-int1`, `cls-int2`, `cls-uni`, and three
//! additional inconsistency detectors: `cls-nothing2` (any instance of
//! `owl:Nothing`), `prp-irp` (a reflexive edge on an
//! `owl:IrreflexiveProperty`), `prp-asyp` (both directions of an
//! `owl:AsymmetricProperty` between the same individuals), and
//! `prp-pdw` (a pair of individuals related by two
//! `owl:propertyDisjointWith` properties).
//!
//! An `rdfs` profile alias runs only the four RDFS compatible rules and
//! skips every OWL-specific rule (including equality, schema closure, and
//! cax-dw).
//!
//! The engine uses classical semi-naive evaluation: round 1 is a naive scan
//! of the full graph, and every subsequent round joins each rule against a
//! [`DeltaIndex`] of the triples that were actually added in the previous
//! round. For a rule with N antecedents, N branches run, each binding one
//! antecedent to delta and the rest to the full graph. Rules whose data
//! antecedents are symmetric (prp-trp, prp-fp, prp-ifp, eq-trans, scm-sco,
//! scm-eqc2, scm-eqp2) collapse to two branches by that symmetry. Triples
//! are still materialised in place; batched writes and interning land in
//! later milestones.
//!
//! ## Algorithm reference and code map
//!
//! The fixpoint loop in [`expand`] is a direct implementation of the
//! semi-naive Datalog evaluation algorithm described in Abiteboul, Hull,
//! Vianu, *Foundations of Databases* (1995), chapter 13, section 13.1.
//! The original delta-driven body evaluation is from Bancilhon and
//! Ramakrishnan, *An Amateur's Introduction to Recursive Query Processing
//! Strategies*, SIGMOD 1986. Pseudocode straight from the textbook:
//!
//! ```text
//! input  : program P, EDB tuples I
//! output : least fixpoint of P over I
//!
//! T   := I
//! ΔT  := T_P(I) - I              // round 0 derivations
//! while ΔT ≠ ∅:
//!     T'  := T ∪ ΔT
//!     ΔT' := T_P^Δ(T, ΔT) - T'   // re-derive using only rules with a body atom in ΔT
//!     T   := T'
//!     ΔT  := ΔT'
//! return T
//! ```
//!
//! Mapping textbook → code, all line numbers in [`expand`]:
//!
//! - `T := I`: `graph: &mut FlatGraph` arrives interned with the input.
//! - Round counter: `stats.rounds = stats.rounds.saturating_add(1)`.
//! - `ΔT` (previous round's delta): `let delta_ref = delta.as_ref();`
//!   It is `None` on round 1 (degenerates to a naive scan of `T`) and
//!   `Some(DeltaIndex)` thereafter.
//! - `T_P^Δ(T, ΔT)`: each `apply_<rule>(graph, delta_ref, ...)` call.
//!   The rule body restricts at least one antecedent to `delta_ref`
//!   when it is `Some`, and falls back to a full scan of `graph` when
//!   it is `None`.
//! - Buffer of derived tuples (rule output, before subtraction): the
//!   `pending` and `pending_keyed` Vecs.
//! - Subtraction `- T'`: `graph.insert(&triple)` and
//!   `graph.insert_keyed(..)` probe the shadow set first; duplicates
//!   return `false` and never touch the indexes.
//! - `T := T'`: the same `insert*` call appends to `FlatGraph` and
//!   collects the genuinely new triples into `new_triples`.
//! - `ΔT' := …`: `DeltaIndex::build(new_triples)` builds the next
//!   round's delta from this round's novel additions.
//! - `ΔT ≠ ∅`: `if round_added == 0 { return Ok(stats); }` exits the
//!   loop on a quiescent round.
//!
//! Three engineering additions sit on top of the textbook core, all
//! domain-specific to OWL 2 RL rather than to Datalog:
//!
//! 1. [`find_inconsistency`] runs before every rule application so an
//!    OWL 2 RL clash (`cax-dw`, `prp-irp`, `prp-asyp`, `prp-pdw`,
//!    `cls-nothing2`) aborts the run before more vacuous consequences
//!    accumulate. Pure Datalog is monotone, so the textbook does not
//!    model inconsistencies.
//! 2. [`TBoxCache::build`] precomputes class-expression metadata once
//!    per round and is rebuilt only when the previous delta changed it.
//! 3. [`InconsistencyTriggers::scan`] guards the per-round detector
//!    behind a one-time scan of the input, so the detector is skipped
//!    on graphs whose predicate alphabet cannot generate a clash.
//!
//! Restrictions relative to full OWL 2 RL:
//!
//! 1. Classes and properties referenced by the schema rules must be IRIs.
//!    Blank node class expressions (anonymous restrictions) are ignored.
//! 2. Datatype rules (`dt-type1` and friends) still wait on value space
//!    comparisons for datatype literals.
//! 3. Inferred quads are always written back into the same in memory graph.
//!    The `materialise_into_named_graph` config knob is honoured by
//!    `Reasoner::expand_into`, which is still stubbed.

use crate::reasoner::{ReasonerConfig, ReasoningProfile};
use oxrdf::vocab::{rdf, rdfs};
use oxrdf::{
    Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, Term, TermRef, Triple,
    TripleRef,
};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::time::{Duration, Instant};

/// Packed interner ID: 2 kind bits (top) plus 30 bits of per-kind index.
/// Fits in a single `u32`, so the shadow set becomes a hash over three u32
/// triples instead of three owned `String` allocations. A 30 bit index gives
/// ~1B distinct terms per kind, which comfortably clears any graph that fits
/// in memory.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct TermId(u32);

const KIND_SHIFT: u32 = 30;
const ID_MASK: u32 = (1 << KIND_SHIFT) - 1;
const KIND_NAMED: u32 = 0;
const KIND_BLANK: u32 = 1 << KIND_SHIFT;
const KIND_LITERAL: u32 = 2 << KIND_SHIFT;

impl TermId {
    #[inline]
    fn named(idx: u32) -> Self {
        debug_assert!(
            idx <= ID_MASK,
            "TermId named index overflow: top bits reserved for kind tag"
        );
        Self(KIND_NAMED | idx)
    }
    #[inline]
    fn blank(idx: u32) -> Self {
        debug_assert!(
            idx <= ID_MASK,
            "TermId blank index overflow: top bits reserved for kind tag"
        );
        Self(KIND_BLANK | idx)
    }
    #[inline]
    fn literal(idx: u32) -> Self {
        debug_assert!(
            idx <= ID_MASK,
            "TermId literal index overflow: top bits reserved for kind tag"
        );
        Self(KIND_LITERAL | idx)
    }
}

/// Term interner that produces small fixed-size ids for reasoner use.
///
/// Named nodes and blank nodes are keyed by their string form (IRI /
/// identifier). Lookup of an existing term is one `FxHashMap<String, u32>`
/// probe that takes a `&str` through the `Borrow<str>` impl on `String`, so
/// no allocation happens on a hit. On a miss we clone the string once.
///
/// Literals are keyed by the owned `Literal` itself because the value plus
/// datatype (plus optional language tag) form a composite that is awkward
/// to represent as `&str`. Every A-Box literal we see already arrives as an
/// owned `Literal` on the commit path (`pending: Vec<Triple>` carries owned
/// terms), so lookup needs no extra allocation in steady state. Seeding from
/// the input graph pays one `into_owned()` per distinct literal.
struct Interner {
    named_by_iri: FxHashMap<String, u32>,
    blank_by_id: FxHashMap<String, u32>,
    literal_ids: FxHashMap<Literal, u32>,
    named_count: u32,
    blank_count: u32,
    literal_count: u32,
}

impl Interner {
    fn with_capacity(n: usize) -> Self {
        Self {
            named_by_iri: FxHashMap::with_capacity_and_hasher(n, FxBuildHasher),
            blank_by_id: FxHashMap::with_capacity_and_hasher(n / 8 + 1, FxBuildHasher),
            literal_ids: FxHashMap::with_capacity_and_hasher(n / 4 + 1, FxBuildHasher),
            named_count: 0,
            blank_count: 0,
            literal_count: 0,
        }
    }

    #[inline]
    fn intern_named_str(&mut self, iri: &str) -> TermId {
        if let Some(&idx) = self.named_by_iri.get(iri) {
            return TermId::named(idx);
        }
        let idx = self.named_count;
        self.named_count += 1;
        self.named_by_iri.insert(iri.to_owned(), idx);
        TermId::named(idx)
    }

    /// Read-only IRI lookup used by `FlatGraph` clients that want to probe
    /// the interner without allocating a new id for a miss. Returns `None`
    /// when the IRI has never been seen in the input graph; rule bodies then
    /// know a (predicate, object) key cannot be in `by_pred_obj` either.
    #[inline]
    fn lookup_named(&self, iri: &str) -> Option<TermId> {
        self.named_by_iri.get(iri).copied().map(TermId::named)
    }

    /// Read-only blank node lookup. Zero-allocation on hit and on miss.
    #[inline]
    fn lookup_blank(&self, id: &str) -> Option<TermId> {
        self.blank_by_id.get(id).copied().map(TermId::blank)
    }

    /// Read-only literal lookup. The owned `Literal` is the hash key; callers
    /// that only have a `LiteralRef` must allocate a single owned `Literal`
    /// before probing.
    #[inline]
    fn lookup_literal(&self, l: &Literal) -> Option<TermId> {
        self.literal_ids.get(l).copied().map(TermId::literal)
    }

    /// Read-only subject lookup (named or blank), zero allocation.
    #[inline]
    fn lookup_subject(&self, s: &NamedOrBlankNode) -> Option<TermId> {
        match s {
            NamedOrBlankNode::NamedNode(n) => self.lookup_named(n.as_str()),
            NamedOrBlankNode::BlankNode(b) => self.lookup_blank(b.as_str()),
        }
    }

    #[inline]
    fn lookup_subject_ref(&self, s: NamedOrBlankNodeRef<'_>) -> Option<TermId> {
        match s {
            NamedOrBlankNodeRef::NamedNode(n) => self.lookup_named(n.as_str()),
            NamedOrBlankNodeRef::BlankNode(b) => self.lookup_blank(b.as_str()),
        }
    }

    /// Read-only term lookup over an owned [`Term`]. Zero allocation.
    #[inline]
    fn lookup_term(&self, t: &Term) -> Option<TermId> {
        match t {
            Term::NamedNode(n) => self.lookup_named(n.as_str()),
            Term::BlankNode(b) => self.lookup_blank(b.as_str()),
            Term::Literal(l) => self.lookup_literal(l),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => None,
        }
    }

    /// Read-only term lookup over a [`TermRef`]. Allocates one owned
    /// [`Literal`] on literal probes because `FxHashMap<Literal, u32>` cannot
    /// key on a borrow without the unstable `raw_entry_mut` API. Named and
    /// blank lookups are zero-allocation.
    #[inline]
    fn lookup_term_ref(&self, t: TermRef<'_>) -> Option<TermId> {
        match t {
            TermRef::NamedNode(n) => self.lookup_named(n.as_str()),
            TermRef::BlankNode(b) => self.lookup_blank(b.as_str()),
            TermRef::Literal(lref) => self.lookup_literal(&lref.into_owned()),
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(_) => None,
        }
    }

    /// Convenience for the read path: probe all three components of an owned
    /// triple. Returns `None` if any component is not interned, which is the
    /// signal a [`FlatGraph`] uses to short-circuit a `contains` probe.
    #[inline]
    fn lookup_triple(&self, t: &Triple) -> Option<(TermId, TermId, TermId)> {
        let s = self.lookup_subject(&t.subject)?;
        let p = self.lookup_named(t.predicate.as_str())?;
        let o = self.lookup_term(&t.object)?;
        Some((s, p, o))
    }

    #[inline]
    fn intern_blank_str(&mut self, id: &str) -> TermId {
        if let Some(&idx) = self.blank_by_id.get(id) {
            return TermId::blank(idx);
        }
        let idx = self.blank_count;
        self.blank_count += 1;
        self.blank_by_id.insert(id.to_owned(), idx);
        TermId::blank(idx)
    }

    #[inline]
    fn intern_literal_owned(&mut self, l: &Literal) -> TermId {
        if let Some(&idx) = self.literal_ids.get(l) {
            return TermId::literal(idx);
        }
        let idx = self.literal_count;
        self.literal_count += 1;
        self.literal_ids.insert(l.clone(), idx);
        TermId::literal(idx)
    }

    #[inline]
    fn intern_subject(&mut self, s: &NamedOrBlankNode) -> TermId {
        match s {
            NamedOrBlankNode::NamedNode(n) => self.intern_named_str(n.as_str()),
            NamedOrBlankNode::BlankNode(b) => self.intern_blank_str(b.as_str()),
        }
    }

    #[inline]
    fn intern_term(&mut self, t: &Term) -> TermId {
        match t {
            Term::NamedNode(n) => self.intern_named_str(n.as_str()),
            Term::BlankNode(b) => self.intern_blank_str(b.as_str()),
            Term::Literal(l) => self.intern_literal_owned(l),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => unreachable!("rdf-12 embedded triples are not supported here"),
        }
    }

    /// Convenience for the commit path: intern all three components of an
    /// owned triple in one call. Returns the packed tuple used as the key
    /// for the shadow set.
    #[inline]
    fn intern_triple(&mut self, t: &Triple) -> (TermId, TermId, TermId) {
        let s = self.intern_subject(&t.subject);
        let p = self.intern_named_str(t.predicate.as_str());
        let o = self.intern_term(&t.object);
        (s, p, o)
    }
}

/// Lightweight per-rule profiler gated by the `OXREASON_PROFILE` environment
/// variable. Intentionally kept in the main source file (not a feature flag)
/// so it is trivial to turn on ad hoc without recompiling downstream crates.
/// Off by default: construction is a single env read and a small allocation
/// that is never touched again. When on, every rule invocation is bracketed
/// by `Instant::now()` calls; the summary is printed to stderr when
/// `expand` returns.
struct Profiler {
    enabled: bool,
    entries: Vec<(&'static str, Duration, u64, u64)>,
}

impl Profiler {
    fn new() -> Self {
        let enabled = std::env::var("OXREASON_PROFILE").is_ok_and(|v| v != "0" && !v.is_empty());
        Self {
            enabled,
            entries: Vec::new(),
        }
    }

    /// Time a single apply_* call, accumulating per-rule elapsed time and
    /// firing count. When disabled the closure still runs but no timing work
    /// happens. The extra `delta_triples` argument records the delta size the
    /// rule was scanned against (useful for detecting rules that keep running
    /// on empty deltas).
    fn time<F>(&mut self, name: &'static str, delta_triples: u64, f: F) -> u64
    where
        F: FnOnce() -> u64,
    {
        if !self.enabled {
            return f();
        }
        let start = Instant::now();
        let firings = f();
        let elapsed = start.elapsed();
        // Accumulate by name: rules are called once per round, so the same
        // &'static str appears many times. Linear search is fine; the list
        // tops out at ~30 entries.
        if let Some(e) = self.entries.iter_mut().find(|e| e.0 == name) {
            e.1 += elapsed;
            e.2 = e.2.saturating_add(firings);
            e.3 = e.3.saturating_add(delta_triples);
        } else {
            self.entries.push((name, elapsed, firings, delta_triples));
        }
        firings
    }

    /// Time a non-rule block (e.g. dedup/insert, delta build). Same accumulation
    /// path as `time` but with an explicit zero firing count.
    fn time_block<T, F>(&mut self, name: &'static str, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        if !self.enabled {
            return f();
        }
        let start = Instant::now();
        let value = f();
        let elapsed = start.elapsed();
        if let Some(e) = self.entries.iter_mut().find(|e| e.0 == name) {
            e.1 += elapsed;
        } else {
            self.entries.push((name, elapsed, 0, 0));
        }
        value
    }

    #[expect(
        clippy::print_stderr,
        reason = "Profiler emits a one-shot timing report when the OXREASON_PROFILE env var is set; behind a runtime check, never on a hot path."
    )]
    fn report(&self, total: Duration) {
        if !self.enabled {
            return;
        }
        let mut sorted: Vec<&(&'static str, Duration, u64, u64)> = self.entries.iter().collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(e.1));
        let total_ms = total.as_secs_f64() * 1000.0;
        eprintln!("OXREASON_PROFILE total_ms={total_ms:.3}");
        eprintln!(
            "{:<18}  {:>10}  {:>8}  {:>10}  {:>10}",
            "rule", "ms", "pct", "firings", "delta_in"
        );
        for (name, elapsed, firings, delta_in) in sorted {
            let ms = elapsed.as_secs_f64() * 1000.0;
            let pct = if total.as_secs_f64() > 0.0 {
                100.0 * elapsed.as_secs_f64() / total.as_secs_f64()
            } else {
                0.0
            };
            eprintln!("{name:<18}  {ms:>10.3}  {pct:>7.1}%  {firings:>10}  {delta_in:>10}");
        }
    }
}

/// `http://www.w3.org/2002/07/owl#TransitiveProperty`
const OWL_TRANSITIVE_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#TransitiveProperty");
/// `http://www.w3.org/2002/07/owl#SymmetricProperty`
const OWL_SYMMETRIC_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#SymmetricProperty");
/// `http://www.w3.org/2002/07/owl#inverseOf`
const OWL_INVERSE_OF: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#inverseOf");
/// `http://www.w3.org/2002/07/owl#equivalentProperty`
const OWL_EQUIVALENT_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#equivalentProperty");
/// `http://www.w3.org/2002/07/owl#equivalentClass`
const OWL_EQUIVALENT_CLASS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#equivalentClass");
/// `http://www.w3.org/2002/07/owl#sameAs`
const OWL_SAME_AS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#sameAs");
/// `http://www.w3.org/2002/07/owl#Class`
const OWL_CLASS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Class");
/// `http://www.w3.org/2002/07/owl#Thing`
const OWL_THING: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Thing");
/// `http://www.w3.org/2002/07/owl#Nothing`
const OWL_NOTHING: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Nothing");
/// `http://www.w3.org/2002/07/owl#ObjectProperty`
const OWL_OBJECT_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#ObjectProperty");
/// `http://www.w3.org/2002/07/owl#DatatypeProperty`
const OWL_DATATYPE_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#DatatypeProperty");
/// `http://www.w3.org/2002/07/owl#FunctionalProperty`
const OWL_FUNCTIONAL_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#FunctionalProperty");
/// `http://www.w3.org/2002/07/owl#InverseFunctionalProperty`
const OWL_INVERSE_FUNCTIONAL_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#InverseFunctionalProperty");
/// `http://www.w3.org/2002/07/owl#disjointWith`
const OWL_DISJOINT_WITH: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#disjointWith");
/// `http://www.w3.org/2002/07/owl#IrreflexiveProperty`
const OWL_IRREFLEXIVE_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#IrreflexiveProperty");
/// `http://www.w3.org/2002/07/owl#AsymmetricProperty`
const OWL_ASYMMETRIC_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#AsymmetricProperty");
/// `http://www.w3.org/2002/07/owl#propertyDisjointWith`
const OWL_PROPERTY_DISJOINT_WITH: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#propertyDisjointWith");
/// `http://www.w3.org/2002/07/owl#hasValue`
const OWL_HAS_VALUE: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#hasValue");
/// `http://www.w3.org/2002/07/owl#onProperty`
const OWL_ON_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#onProperty");
/// `http://www.w3.org/2002/07/owl#intersectionOf`
const OWL_INTERSECTION_OF: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#intersectionOf");
/// `http://www.w3.org/2002/07/owl#unionOf`
const OWL_UNION_OF: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#unionOf");
/// `http://www.w3.org/2002/07/owl#someValuesFrom`
const OWL_SOME_VALUES_FROM: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#someValuesFrom");
/// `http://www.w3.org/2002/07/owl#allValuesFrom`
const OWL_ALL_VALUES_FROM: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#allValuesFrom");
/// `http://www.w3.org/2002/07/owl#propertyChainAxiom`
const OWL_PROPERTY_CHAIN_AXIOM: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#propertyChainAxiom");

/// Summary of a chaining run, consumed by `Reasoner::expand` to build a
/// `ReasoningReport`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RunStats {
    pub added: u64,
    pub rounds: u32,
    pub firings: u64,
}

/// Information about a cax-dw clash detected during expansion.
#[derive(Clone, Debug)]
pub(crate) struct DisjointClash {
    pub individual: NamedOrBlankNode,
    pub class_a: NamedNode,
    pub class_b: NamedNode,
}

/// Why the engine aborted expansion.
///
/// `DisjointClasses` is the existing cax-dw path and keeps its structured
/// shape so the reasoner API can still surface the offending individual
/// and classes. The other variants cover the four M4 inconsistency checks
/// and carry a pre-formatted human message because their shapes do not
/// line up (some name a pair of individuals, some a single property, some
/// both). The `ReasonError::InconsistentAxiom` variant at the public API
/// boundary renders that message directly.
#[derive(Clone, Debug)]
pub(crate) enum Inconsistency {
    /// cax-dw: an individual holds two types that are declared disjoint.
    DisjointClasses(DisjointClash),
    /// cls-nothing2: an individual is typed as `owl:Nothing`.
    NothingType { individual: String },
    /// prp-irp: a reflexive edge over an `owl:IrreflexiveProperty`.
    IrreflexiveViolation {
        property: String,
        individual: String,
    },
    /// prp-asyp: two opposing edges over an `owl:AsymmetricProperty`.
    AsymmetricViolation {
        property: String,
        subject: String,
        object: String,
    },
    /// prp-pdw: a pair of individuals linked by two
    /// `owl:propertyDisjointWith` properties.
    PropertyDisjointnessViolation {
        property_a: String,
        property_b: String,
        subject: String,
        object: String,
    },
}

impl Inconsistency {
    /// Human readable message used by `ReasonError::InconsistentAxiom`.
    /// Not used for `DisjointClasses` which takes a structured public path.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::DisjointClasses(c) => format!(
                "cax-dw: individual {} is typed as disjoint classes {} and {}",
                c.individual, c.class_a, c.class_b,
            ),
            Self::NothingType { individual } => {
                format!("cls-nothing2: individual {individual} is typed as owl:Nothing")
            }
            Self::IrreflexiveViolation {
                property,
                individual,
            } => format!(
                "prp-irp: irreflexive property {property} has a reflexive edge on {individual}",
            ),
            Self::AsymmetricViolation {
                property,
                subject,
                object,
            } => format!(
                "prp-asyp: asymmetric property {property} relates {subject} and {object} in both directions",
            ),
            Self::PropertyDisjointnessViolation {
                property_a,
                property_b,
                subject,
                object,
            } => format!(
                "prp-pdw: properties {property_a} and {property_b} are declared disjoint but both relate {subject} to {object}",
            ),
        }
    }
}

/// Triples added in the previous round, grouped by predicate so each rule
/// can scan only the patterns relevant to its antecedents.
///
/// This is the "delta" of the semi-naive fixpoint: in round `N` every rule
/// fires against this index for at least one of its antecedents, and binds
/// the rest to the full graph. The saturation argument is that a triple can
/// only be newly derived in round `N+1` if one of the antecedents of its
/// derivation was first added in round `N`; any all-old derivation was
/// already saturated in an earlier round.
///
/// Round 1 treats the delta as `None` and uses the naive scan. That avoids
/// the double work of running both delta-join branches against the full
/// graph on the very first round where delta would equal the graph anyway.
#[derive(Default)]
pub(crate) struct DeltaIndex {
    by_predicate: FxHashMap<String, Vec<Triple>>,
}

impl DeltaIndex {
    /// Consume the owned triples produced by a round's commit loop and group
    /// them by predicate IRI. Takes `Vec<Triple>` by value so each triple is
    /// moved into its bucket instead of cloned: on dense rounds (LUBM
    /// cax-sco emits hundreds of thousands of `x rdf:type d`) this saves
    /// one `Triple::clone` per novel triple on top of the clone
    /// `FlatGraph::insert` already pays.
    fn build(triples: Vec<Triple>) -> Self {
        let mut by_predicate: FxHashMap<String, Vec<Triple>> = FxHashMap::default();
        for t in triples {
            by_predicate
                .entry(t.predicate.as_str().to_owned())
                .or_default()
                .push(t);
        }
        Self { by_predicate }
    }

    fn for_predicate(&self, p: NamedNodeRef<'_>) -> &[Triple] {
        self.by_predicate
            .get(p.as_str())
            .map_or(&[][..], Vec::as_slice)
    }

    /// Returns true when this delta contains any triple whose predicate is
    /// part of the T-Box trigger set. That set is the nine predicates that
    /// `TBoxCache` reads while it materialises restriction, intersection,
    /// union, and property chain structures: `owl:hasValue`, `owl:onProperty`,
    /// `owl:intersectionOf`, `owl:unionOf`, `owl:someValuesFrom`,
    /// `owl:allValuesFrom`, `owl:propertyChainAxiom`, `rdf:first`, and
    /// `rdf:rest`. All other predicate deltas leave the cache valid.
    fn touches_tbox(&self) -> bool {
        !self.for_predicate(OWL_HAS_VALUE).is_empty()
            || !self.for_predicate(OWL_ON_PROPERTY).is_empty()
            || !self.for_predicate(OWL_INTERSECTION_OF).is_empty()
            || !self.for_predicate(OWL_UNION_OF).is_empty()
            || !self.for_predicate(OWL_SOME_VALUES_FROM).is_empty()
            || !self.for_predicate(OWL_ALL_VALUES_FROM).is_empty()
            || !self.for_predicate(OWL_PROPERTY_CHAIN_AXIOM).is_empty()
            || !self.for_predicate(rdf::FIRST).is_empty()
            || !self.for_predicate(rdf::REST).is_empty()
    }
}

/// Snapshot of the class-expression T-Box the class rules iterate over.
///
/// The five M4 class-expression rules (`cls-hv1`, `cls-hv2`, `cls-int1`,
/// `cls-int2`, `cls-uni`) all start each round by walking the graph for
/// `owl:hasValue`/`owl:onProperty` pairs and for RDF lists under
/// `owl:intersectionOf` and `owl:unionOf`. On LUBM-style data the T-Box is
/// small and the A-Box is huge, so repeating that walk every round wastes
/// most of the rule time. Instead we materialise the three relevant tables
/// once before the fixpoint loop and reuse them across rounds, rebuilding
/// only when the previous round actually emitted a trigger predicate.
struct TBoxCache {
    /// One tuple per `owl:Restriction`-shaped node with both
    /// `owl:onProperty` and `owl:hasValue` set.
    hasvalue_restrictions: Vec<(NamedOrBlankNode, NamedNode, Term)>,
    /// One tuple per `c owl:intersectionOf (c1 .. cn)` with a well-formed
    /// list of resource members.
    intersection_classes: Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)>,
    /// One tuple per `c owl:unionOf (c1 .. cn)` with a well-formed list of
    /// resource members.
    union_classes: Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)>,
    /// One tuple per `owl:Restriction`-shaped node with both
    /// `owl:onProperty` and `owl:someValuesFrom` set. Filler is a resource
    /// (named or blank class).
    somevaluesfrom_restrictions: Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)>,
    /// One tuple per `owl:Restriction`-shaped node with both
    /// `owl:onProperty` and `owl:allValuesFrom` set. Filler is a resource.
    allvaluesfrom_restrictions: Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)>,
    /// One tuple per `p owl:propertyChainAxiom (p1 ... pn)` with a
    /// well-formed list of property IRIs. `p` and each `pi` are named
    /// properties; blank node properties are ignored since they cannot
    /// appear as the predicate of a triple in oxrdf.
    property_chains: Vec<(NamedNode, Vec<NamedNode>)>,
}

impl TBoxCache {
    fn build(graph: &FlatGraph) -> Self {
        Self {
            hasvalue_restrictions: collect_hasvalue_restrictions(graph),
            intersection_classes: collect_intersection_classes(graph),
            union_classes: collect_union_classes(graph),
            somevaluesfrom_restrictions: collect_somevaluesfrom_restrictions(graph),
            allvaluesfrom_restrictions: collect_allvaluesfrom_restrictions(graph),
            property_chains: collect_property_chains(graph),
        }
    }
}

/// Reasoner-local triple store keyed by interned `TermId`.
///
/// Replaces `oxrdf::Graph` as the engine's working set. A full OWL 2 RL
/// closure over LUBM / Polish geodata pushes peak RSS above 2 GiB on
/// `oxrdf::Graph`; the six BTreeSet indexes carry ~3 KiB per materialised
/// triple (task #73 measurements). `FlatGraph` drops that to under 1 KiB per
/// triple by storing the owned `Triple` once and layering three u32-keyed
/// hash indexes plus the shadow dedup set on top.
///
/// Layout:
/// - `triples` is the one and only owned copy of every materialised triple.
///   A `u32` offset into this vector is the internal handle every index
///   yields. The vector only grows; triples are never removed because the
///   reasoner is monotonic.
/// - `shadow` is the dedup probe. Keyed on the packed `(TermId, TermId,
///   TermId)` tuple, probed on every commit candidate and on every
///   `contains` call. 12 bytes per entry vs ~80 bytes for an owned `Triple`
///   with three `Arc<str>` headers.
/// - `by_predicate` supports `triples_for_predicate(p)` in O(1) plus a slice
///   walk. Rule bodies that scan every edge for a given predicate hit this
///   path on the hot axis.
/// - `by_pred_obj` supports `subjects_for_predicate_object(p, o)`. Previously
///   `GraphView::by_pred_obj`; the TermId-keyed form landed in task #56 and
///   shaved 4-15% off reasoning time. FlatGraph folds it into the primary
///   store so seeding only walks the input once.
/// - `by_subj_pred` supports `objects_for_subject_predicate(s, p)`. Used by
///   the class-expression rules (`cls-svf1`, `prp-spo2`) that probe a
///   specific edge shape with the subject fixed.
/// - `interner` maps IRI/blank/literal forms to `TermId`. All indexes key
///   off its output so every hot-path probe is a `u32` hash, not a
///   string-keyed BTreeSet descent.
///
/// Task #49 (2026-04-20) tried a reasoner-local hash store backed by
/// `FxHashMap<TermId, Vec<_>>` indexes and regressed on LUBM because every
/// insert hashed the Arc-backed IRI six times. FlatGraph avoids that trap
/// by interning once per novel triple, keying every index on the resulting
/// u32s, and storing the owned triple exactly once in `triples`.
pub(crate) struct FlatGraph {
    /// Primary storage. Every materialised triple lives here exactly once.
    triples: Vec<Triple>,
    /// Parallel to `triples`: the interned id triple for the same slot.
    /// Rule bodies that iterate by-index indexes can read subject/object
    /// TermIds here without hashing the Arc-backed IRI strings. Costs 12
    /// bytes per stored triple (`3 * u32`), negligible next to the owned
    /// `Triple` (~80 bytes) and buys the keyed-insert fast path.
    triple_ids: Vec<(TermId, TermId, TermId)>,
    /// Dedup / contains probe keyed on interned ids.
    shadow: FxHashSet<(TermId, TermId, TermId)>,
    /// `predicate -> [index into triples]`.
    by_predicate: FxHashMap<TermId, Vec<u32>>,
    /// `(predicate, object) -> [index into triples]`.
    by_pred_obj: FxHashMap<(TermId, TermId), Vec<u32>>,
    /// `(subject, predicate) -> [index into triples]`.
    by_subj_pred: FxHashMap<(TermId, TermId), Vec<u32>>,
    /// Term interner. Owns the named/blank/literal string storage that the
    /// `TermId`s in the indexes reference.
    interner: Interner,
}

impl FlatGraph {
    pub(crate) fn with_capacity(n: usize) -> Self {
        Self {
            triples: Vec::with_capacity(n),
            triple_ids: Vec::with_capacity(n),
            shadow: FxHashSet::with_capacity_and_hasher(n * 2, FxBuildHasher),
            by_predicate: FxHashMap::with_capacity_and_hasher(64, FxBuildHasher),
            by_pred_obj: FxHashMap::with_capacity_and_hasher(n / 4 + 1, FxBuildHasher),
            by_subj_pred: FxHashMap::with_capacity_and_hasher(n / 4 + 1, FxBuildHasher),
            interner: Interner::with_capacity(n),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.triples.len()
    }

    /// Read-only IRI lookup over the embedded interner. Rule bodies that
    /// need a `TermId` for a known IRI go through this (for example to key
    /// into `subjects_ids_for_pred_obj_id`).
    #[inline]
    pub(crate) fn lookup_named(&self, iri: &str) -> Option<TermId> {
        self.interner.lookup_named(iri)
    }

    /// Read-only subject lookup over a [`NamedOrBlankNodeRef`]. Rule bodies
    /// that need the interned id of a subject they already hold (for
    /// example to build a consequent's shadow key) go through this instead
    /// of re-interning via the mutable path. Zero allocation on both named
    /// and blank subjects.
    #[inline]
    pub(crate) fn lookup_subject_ref(&self, s: NamedOrBlankNodeRef<'_>) -> Option<TermId> {
        self.interner.lookup_subject_ref(s)
    }

    /// Insert an owned triple. Returns `true` on genuine novelty, `false`
    /// when the triple was already present. Mirrors
    /// [`oxrdf::Graph::insert`]'s semantics so rule bodies that port over
    /// need no change.
    ///
    /// On novelty: interns the three components, updates the shadow set and
    /// all three hash indexes, and pushes the owned `Triple` onto `triples`.
    /// The owned `Triple` is cloned once from the caller's reference; the
    /// term internals are `Arc<str>`-backed so each component clone is a
    /// refcount bump.
    pub(crate) fn insert(&mut self, triple: &Triple) -> bool {
        let key = self.interner.intern_triple(triple);
        self.insert_with_key(triple, key)
    }

    /// Keyed variant of [`insert`]: skips `intern_triple` because the caller
    /// already has the interned key in hand. Hot rules whose antecedents were
    /// probed via `subjects_ids_for_pred_obj_id` can compute the consequent's
    /// key component-by-component as they build the triple, avoiding three
    /// string hashes per pending entry on the commit loop.
    pub(crate) fn insert_keyed(&mut self, triple: &Triple, key: (TermId, TermId, TermId)) -> bool {
        self.insert_with_key(triple, key)
    }

    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "FlatGraph indexes triples with u32; exceeding 2^32 is a hard limit that callers must avoid by partitioning the input."
    )]
    fn insert_with_key(&mut self, triple: &Triple, key: (TermId, TermId, TermId)) -> bool {
        if !self.shadow.insert(key) {
            return false;
        }
        let idx = u32::try_from(self.triples.len())
            .expect("FlatGraph exceeded 2^32 triples; partition the input");
        self.triples.push(triple.clone());
        self.triple_ids.push(key);
        self.by_predicate.entry(key.1).or_default().push(idx);
        self.by_pred_obj
            .entry((key.1, key.2))
            .or_default()
            .push(idx);
        self.by_subj_pred
            .entry((key.0, key.1))
            .or_default()
            .push(idx);
        true
    }

    /// Probe whether an owned triple is present. Zero allocation on the
    /// fast paths; allocates a single owned `Literal` when the object is a
    /// literal because the interner's literal map cannot key on a borrow.
    pub(crate) fn contains(&self, triple: &Triple) -> bool {
        let Some(key) = self.interner.lookup_triple(triple) else {
            return false;
        };
        self.shadow.contains(&key)
    }

    /// Every triple whose predicate is `predicate`. Empty iterator when
    /// the predicate IRI has never been interned.
    pub(crate) fn triples_for_predicate<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let p = predicate.into();
        let idxs: &'a [u32] = self
            .interner
            .lookup_named(p.as_str())
            .and_then(|id| self.by_predicate.get(&id))
            .map_or(&[][..], Vec::as_slice);
        idxs.iter().map(move |&i| self.triples[i as usize].as_ref())
    }

    /// Every triple whose subject is `subject`. Currently a full scan
    /// filtered by interned subject id: FlatGraph does not carry a
    /// subject-only index because the hot-path rules (`cax-sco`,
    /// `prp-dom`, `prp-spo1`, etc.) never need one. The only callers are
    /// the equality-rule helpers (`eq-rep-s`) which are gated behind
    /// `ReasonerConfig::with_equality_rules` and off by default, so
    /// paying a linear probe here is acceptable for now. If the
    /// equality rules land in a hot production workload, adding a
    /// `by_subject: FxHashMap<TermId, Vec<u32>>` is the cheap upgrade.
    pub(crate) fn triples_for_subject<'a, 'b>(
        &'a self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let s_id = self.interner.lookup_subject_ref(subject.into());
        self.triples.iter().filter_map(move |t| {
            let id = match &t.subject {
                NamedOrBlankNode::NamedNode(n) => self.interner.lookup_named(n.as_str()),
                NamedOrBlankNode::BlankNode(b) => self.interner.lookup_blank(b.as_str()),
            };
            (s_id.is_some() && id == s_id).then(|| t.as_ref())
        })
    }

    /// Every triple whose object is `object`. Same cold-path scan
    /// rationale as `triples_for_subject`: only used by the equality
    /// rule `eq-rep-o`, which is off by default.
    pub(crate) fn triples_for_object<'a, 'b>(
        &'a self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let o_id = self.interner.lookup_term_ref(object.into());
        self.triples.iter().filter_map(move |t| {
            let id = match &t.object {
                Term::NamedNode(n) => self.interner.lookup_named(n.as_str()),
                Term::BlankNode(b) => self.interner.lookup_blank(b.as_str()),
                Term::Literal(l) => self.interner.lookup_literal(l),
                #[cfg(feature = "rdf-12")]
                Term::Triple(_) => None,
            };
            (o_id.is_some() && id == o_id).then(|| t.as_ref())
        })
    }

    /// Every subject `s` such that `s predicate object` is in the graph.
    pub(crate) fn subjects_for_predicate_object<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + 'a {
        let p = predicate.into();
        let o = object.into();
        let idxs: &'a [u32] = match (
            self.interner.lookup_named(p.as_str()),
            self.interner.lookup_term_ref(o),
        ) {
            (Some(p_id), Some(o_id)) => self
                .by_pred_obj
                .get(&(p_id, o_id))
                .map_or(&[][..], Vec::as_slice),
            _ => &[][..],
        };
        idxs.iter()
            .map(move |&i| self.triples[i as usize].subject.as_ref())
    }

    /// Every object `o` such that `subject predicate o` is in the graph.
    pub(crate) fn objects_for_subject_predicate<'a, 'b>(
        &'a self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        let s = subject.into();
        let p = predicate.into();
        let idxs: &'a [u32] = match (
            self.interner.lookup_subject_ref(s),
            self.interner.lookup_named(p.as_str()),
        ) {
            (Some(s_id), Some(p_id)) => self
                .by_subj_pred
                .get(&(s_id, p_id))
                .map_or(&[][..], Vec::as_slice),
            _ => &[][..],
        };
        idxs.iter()
            .map(move |&i| self.triples[i as usize].object.as_ref())
    }

    /// Read-only term lookup over a [`TermRef`]. Rule bodies that need to
    /// resolve the interned id of an object they already hold (the object
    /// of a delta triple, most often) go through this instead of the
    /// mutable intern path. Zero allocation on named and blank objects,
    /// one allocation on literals (the interner cannot key on a borrow
    /// without the unstable `raw_entry_mut` API).
    #[inline]
    pub(crate) fn lookup_term_ref(&self, t: TermRef<'_>) -> Option<TermId> {
        self.interner.lookup_term_ref(t)
    }

    /// Given an interned predicate id, yield every indexed triple with
    /// that predicate as a `(&(s_id, p_id, o_id), &Triple)` pair. Rule
    /// bodies whose consequent needs both the component ids (for the
    /// keyed shadow probe) and the owned triple content (for the
    /// `Triple::new` clone) read everything from the parallel
    /// `triple_ids` / `triples` side-vecs in one indexed read. Zero
    /// hashing on the iteration path. Used by `prp-dom`, `prp-rng`, and
    /// `prp-spo1` whose graph legs were previously
    /// `triples_for_predicate(p)` + per-row `intern_triple` on commit.
    pub(crate) fn rows_for_pred_id(
        &self,
        p: TermId,
    ) -> impl Iterator<Item = (&(TermId, TermId, TermId), &Triple)> + '_ {
        let idxs: &[u32] = self.by_predicate.get(&p).map_or(&[][..], Vec::as_slice);
        idxs.iter().map(move |&i| {
            let idx = i as usize;
            (&self.triple_ids[idx], &self.triples[idx])
        })
    }

    /// View-style lookup used by `cax-sco` and `cax-eqc`: given already
    /// interned `(predicate, object)` ids, yield each subject together with
    /// its already-interned [`TermId`]. Rule bodies that need the id to
    /// build the consequent's key for [`insert_keyed`] read both out of the
    /// parallel `triple_ids` side-vec in one indexed read, with no hash
    /// probe on the IRI string.
    pub(crate) fn subjects_ids_for_pred_obj_id(
        &self,
        p: TermId,
        o: TermId,
    ) -> impl Iterator<Item = (TermId, NamedOrBlankNodeRef<'_>)> + '_ {
        let idxs: &[u32] = self.by_pred_obj.get(&(p, o)).map_or(&[][..], Vec::as_slice);
        idxs.iter().map(move |&i| {
            let idx = i as usize;
            (self.triple_ids[idx].0, self.triples[idx].subject.as_ref())
        })
    }

    /// Drain every stored triple into a fresh `Vec`, consuming the store.
    /// Used by the legacy `Reasoner::expand(&mut Graph)` wrapper to rebuild
    /// an `oxrdf::Graph` after the run.
    pub(crate) fn into_triples(self) -> Vec<Triple> {
        self.triples
    }
}

/// Reason why `expand` stopped short of a clean saturation.
///
/// Either the engine detected an inconsistency (cax-dw and friends) or the
/// caller-provided sink returned an error on a freshly materialised triple.
/// The Sink variant is parameterised on the caller's error type so the
/// public reasoner wrappers can propagate the original error without
/// erasing its type.
pub(crate) enum ExpandError<E> {
    /// The engine found an inconsistency and aborted. Carries the same
    /// [`Inconsistency`] payload the legacy error path surfaced.
    Inconsistency(Inconsistency),
    /// The caller-provided sink rejected a freshly materialised triple.
    Sink(E),
}

/// Run the forward chainer to fixpoint on the given graph.
///
/// Each genuine new inference is handed to `sink` before the next rule
/// fires. Sink errors abort the run and surface as
/// [`ExpandError::Sink`]; engine-level inconsistencies surface as
/// [`ExpandError::Inconsistency`]. A clash is surfaced even when
/// encountered on round zero, so the reasoner fails fast on pre-existing
/// inconsistencies in the input graph.
///
/// Callers that only care about materialising into `graph` can pass an
/// infallible no-op sink; see [`crate::reasoner::Reasoner::expand`].
pub(crate) fn expand<F, E>(
    graph: &mut FlatGraph,
    config: &ReasonerConfig,
    sink: &mut F,
) -> Result<RunStats, ExpandError<E>>
where
    F: FnMut(&Triple) -> Result<(), E>,
{
    let profile = config.profile();
    let equality_on = config.equality_rules_enabled();

    let mut prof = Profiler::new();
    let run_start = Instant::now();

    let mut stats = RunStats::default();
    let mut delta: Option<DeltaIndex> = None;

    // `graph` is already interned + indexed + shadow-probed on entry; it is
    // the drop-in replacement for `oxrdf::Graph` that the rule bodies used
    // to query. The separate Interner/GraphView/seen_total setup that this
    // function used to maintain has moved inside `FlatGraph` itself.

    // Build the class-expression T-Box cache once up front. It is reused
    // across rounds and only rebuilt when the previous round's delta
    // contains a predicate that could change its contents.
    let mut tbox = prof.time_block("tbox.build", || TBoxCache::build(graph));

    // Cache the interned id of `rdf:type` once. Rule bodies that query the
    // index on a (rdf:type, class) key look this up every round otherwise.
    // The graph always contains at least one rdf:type triple on non-trivial
    // OWL 2 RL workloads, so the id is populated after seeding; for the
    // degenerate empty-graph case we fall through to the interner and get
    // a freshly minted id.
    let type_id = graph
        .lookup_named(rdf::TYPE.as_str())
        .unwrap_or_else(|| graph.interner.intern_named_str(rdf::TYPE.as_str()));

    // Sample once which inconsistency families this graph can trigger.
    // The gate is conservative: if the input has no `owl:disjointWith`
    // edges, for example, nothing can derive a cax-dw clash later either.
    let triggers = InconsistencyTriggers::scan(graph);
    let run_detectors = profile != ReasoningProfile::Rdfs && triggers.any();

    loop {
        stats.rounds = stats.rounds.saturating_add(1);

        // Inconsistency detectors run first so an inconsistent graph aborts
        // before we spend more work materialising vacuous consequences. The
        // detectors scan the full graph; on round 1 the graph is the input,
        // afterwards it contains every saturated triple, so any clash
        // produced by an earlier rule surfaces on the next round.
        if run_detectors {
            let maybe_clash =
                prof.time_block("inconsistency", || find_inconsistency(graph, triggers));
            if let Some(clash) = maybe_clash {
                return Err(ExpandError::Inconsistency(clash));
            }
        }

        let mut pending: Vec<Triple> = Vec::new();
        // Secondary pending buffer for rules that can hand the commit loop
        // the interned `(s, p, o)` key alongside the owned triple. Values
        // arrive here from `apply_cax_sco` (branch 1 + naive case) today;
        // more rules will migrate as each one is confirmed to win on the
        // bench. The commit loop routes this list through
        // `FlatGraph::insert_keyed`, which skips `intern_triple`'s three
        // Arc<str> hashes per committed triple.
        let mut pending_keyed: Vec<(Triple, (TermId, TermId, TermId))> = Vec::new();
        let mut round_firings: u64 = 0;

        let delta_ref = delta.as_ref();
        let delta_size: u64 =
            delta_ref.map_or(0, |d| d.by_predicate.values().map(|v| v.len() as u64).sum());

        // RDFS compatible rules. Run in both profiles.
        round_firings = round_firings.saturating_add(prof.time("cax-sco", delta_size, || {
            apply_cax_sco(graph, delta_ref, type_id, &mut pending_keyed)
        }));
        round_firings = round_firings.saturating_add(prof.time("prp-dom", delta_size, || {
            apply_prp_dom(graph, delta_ref, type_id, &mut pending_keyed)
        }));
        round_firings = round_firings.saturating_add(prof.time("prp-rng", delta_size, || {
            apply_prp_rng(graph, delta_ref, type_id, &mut pending_keyed)
        }));
        round_firings = round_firings.saturating_add(prof.time("prp-spo1", delta_size, || {
            apply_prp_spo1(graph, delta_ref, &mut pending_keyed)
        }));

        // OWL rules. Skipped when running the Rdfs profile.
        if profile != ReasoningProfile::Rdfs {
            round_firings = round_firings.saturating_add(prof.time("prp-trp", delta_size, || {
                apply_prp_trp(graph, delta_ref, &mut pending_keyed)
            }));
            round_firings = round_firings.saturating_add(prof.time("prp-symp", delta_size, || {
                apply_prp_symp(graph, delta_ref, &mut pending_keyed)
            }));
            round_firings = round_firings.saturating_add(prof.time("prp-inv", delta_size, || {
                apply_prp_inv(graph, delta_ref, &mut pending_keyed)
            }));
            round_firings = round_firings.saturating_add(prof.time("prp-eqp", delta_size, || {
                apply_prp_eqp(graph, delta_ref, &mut pending_keyed)
            }));
            round_firings = round_firings.saturating_add(prof.time("cax-eqc", delta_size, || {
                apply_cax_eqc(graph, delta_ref, type_id, &mut pending_keyed)
            }));

            // M3 schema rules.
            round_firings = round_firings.saturating_add(prof.time("scm-cls", delta_size, || {
                apply_scm_cls(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-sco", delta_size, || {
                apply_scm_sco(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-op", delta_size, || {
                apply_scm_op(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-dp", delta_size, || {
                apply_scm_dp(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-eqc1", delta_size, || {
                apply_scm_eqc1(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-eqc2", delta_size, || {
                apply_scm_eqc2(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-eqp1", delta_size, || {
                apply_scm_eqp1(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-eqp2", delta_size, || {
                apply_scm_eqp2(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-dom1", delta_size, || {
                apply_scm_dom1(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("scm-rng1", delta_size, || {
                apply_scm_rng1(graph, delta_ref, &mut pending)
            }));

            // M4 rules.
            round_firings = round_firings.saturating_add(prof.time("scm-spo", delta_size, || {
                apply_scm_spo(graph, delta_ref, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-hv1", delta_size, || {
                apply_cls_hv1(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-hv2", delta_size, || {
                apply_cls_hv2(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-int1", delta_size, || {
                apply_cls_int1(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-int2", delta_size, || {
                apply_cls_int2(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-uni", delta_size, || {
                apply_cls_uni(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-svf1", delta_size, || {
                apply_cls_svf1(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-svf2", delta_size, || {
                apply_cls_svf2(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("cls-avf", delta_size, || {
                apply_cls_avf(graph, &tbox, &mut pending)
            }));
            round_firings = round_firings.saturating_add(prof.time("prp-spo2", delta_size, || {
                apply_prp_spo2(graph, &tbox, &mut pending)
            }));

            if equality_on {
                round_firings =
                    round_firings.saturating_add(prof.time("prp-fp", delta_size, || {
                        apply_prp_fp(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("prp-ifp", delta_size, || {
                        apply_prp_ifp(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("eq-sym", delta_size, || {
                        apply_eq_sym(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("eq-trans", delta_size, || {
                        apply_eq_trans(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("eq-rep-s", delta_size, || {
                        apply_eq_rep_s(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("eq-rep-p", delta_size, || {
                        apply_eq_rep_p(graph, delta_ref, &mut pending)
                    }));
                round_firings =
                    round_firings.saturating_add(prof.time("eq-rep-o", delta_size, || {
                        apply_eq_rep_o(graph, delta_ref, &mut pending)
                    }));
            }
        }

        stats.firings = stats.firings.saturating_add(round_firings);

        // Many rules derive the same consequent (for example every
        // cax-sco branch that bridges through a fan-in node pushes the
        // same `x rdf:type d`). The commit loop probes the shadow set
        // first; duplicates never touch the hash indexes.
        //
        // `FlatGraph::insert` handles intern + shadow probe + index update
        // + owned push in one call. Returns `true` on genuine novelty, and
        // we hand every such triple to the sink before pushing it onto the
        // delta buffer. Errors abort the run: the engine has already
        // recorded the inference in `graph`, so a re-run from the same
        // graph would continue from the point of failure.
        let new_triples = {
            let mut new_triples: Vec<Triple> = Vec::new();
            let mut insert_time = Duration::ZERO;
            let mut insert_keyed_time = Duration::ZERO;
            let profile_split = prof.enabled;
            // Keyed path first: rules that already know the interned key
            // skip `intern_triple`'s three Arc<str> hashes.
            for (triple, key) in pending_keyed {
                let t0 = profile_split.then(Instant::now);
                let novel = graph.insert_keyed(&triple, key);
                if let Some(t) = t0 {
                    insert_keyed_time += t.elapsed();
                }
                if !novel {
                    continue;
                }
                sink(&triple).map_err(ExpandError::Sink)?;
                new_triples.push(triple);
            }
            for triple in pending {
                let t0 = profile_split.then(Instant::now);
                let novel = graph.insert(&triple);
                if let Some(t) = t0 {
                    insert_time += t.elapsed();
                }
                if !novel {
                    continue;
                }
                sink(&triple).map_err(ExpandError::Sink)?;
                new_triples.push(triple);
            }
            if profile_split {
                if let Some(e) = prof.entries.iter_mut().find(|e| e.0 == "flat.insert") {
                    e.1 += insert_time;
                } else {
                    prof.entries.push(("flat.insert", insert_time, 0, 0));
                }
                if let Some(e) = prof.entries.iter_mut().find(|e| e.0 == "flat.insert_keyed") {
                    e.1 += insert_keyed_time;
                } else {
                    prof.entries
                        .push(("flat.insert_keyed", insert_keyed_time, 0, 0));
                }
            }
            new_triples
        };
        let round_added = new_triples.len() as u64;
        stats.added = stats.added.saturating_add(round_added);

        if round_added == 0 {
            prof.report(run_start.elapsed());
            return Ok(stats);
        }

        // Build the delta for the next round from the triples that were
        // actually new to the graph. Dedup happens at `graph.insert` time
        // above, so `new_triples` already excludes duplicates. `build`
        // consumes the Vec to move each triple into its predicate bucket
        // instead of cloning it again.
        let next_delta = prof.time_block("delta.build", || DeltaIndex::build(new_triples));

        // Rebuild the class-expression cache only if the delta actually
        // touched one of its trigger predicates. In practice no
        // currently-implemented rule emits `owl:hasValue`, `owl:onProperty`,
        // `owl:intersectionOf`, `owl:unionOf`, `rdf:first`, or `rdf:rest`,
        // so this almost never fires on OWL 2 RL workloads. The check is
        // still here because `eq-rep-*` (behind the equality flag) and
        // `prp-spo1` (if the user declares a subPropertyOf chain reaching
        // one of those predicates) could in principle touch the T-Box.
        if next_delta.touches_tbox() {
            tbox = prof.time_block("tbox.build", || TBoxCache::build(graph));
        }

        delta = Some(next_delta);
    }
}

// ---------------------------------------------------------------------------
// Rule bodies.
// Each function matches its antecedent against the graph and pushes
// candidate consequent triples onto `pending`. Deduplication against the
// existing graph happens later when `pending` is drained.

fn apply_cax_sco(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    type_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // cax-sco: if x rdf:type c and c rdfs:subClassOf d then x rdf:type d.
    //
    // The graph(type) leg of each branch goes through
    // `FlatGraph::subjects_ids_for_pred_obj_id`, keyed on already-interned
    // `(TermId, TermId)` pairs. Every subject the bucket yields arrives
    // with its `TermId` attached, so the consequent's shadow-probe key can
    // be assembled component-by-component (`(x_id, type_id, dest_id)`) and
    // handed straight to `FlatGraph::insert_keyed`, bypassing the three
    // string hashes that `intern_triple` would otherwise do on each
    // committed triple. Branch 2 still pushes to the plain `pending` list
    // because its subjects come from the predicate-keyed delta index and
    // do not carry an id.
    //
    // Semi-naive structure:
    //   Branch 1: delta(subClassOf) × graph(type).
    //   Branch 2: graph(subClassOf) × delta(type).
    let mut firings: u64 = 0;

    // Cache each subClassOf pair together with the interned ids of both `c`
    // and `d`. The inner loop then hands two u32s straight to the index
    // and to the keyed pending buffer. The `NamedNode` form of `c` is kept
    // only for joining against delta(type) in branch 2. `lookup_named` is
    // expected to hit because every IRI the graph contains was interned at
    // seed time; the `?` is a soundness hedge.
    let subclass_pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c = named_node_from_subject(t.subject)?;
            let c_id = graph.lookup_named(c.as_str())?;
            let d = named_node_from_term(t.object)?;
            let d_id = graph.lookup_named(d.as_str())?;
            Some((c, c_id, d, d_id))
        })
        .collect();

    let Some(d) = delta else {
        // Round 1: naive. Index lookup is O(1) per pair.
        for (_, c_id, dest, dest_id) in &subclass_pairs {
            for (x_id, x) in graph.subjects_ids_for_pred_obj_id(type_id, *c_id) {
                let triple = Triple::new(x.into_owned(), rdf::TYPE, dest.clone());
                pending_keyed.push((triple, (x_id, type_id, *dest_id)));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(subClassOf) × graph(type) via the index.
    for t in d.for_predicate(rdfs::SUB_CLASS_OF) {
        let Some(c) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(dest) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(c_id) = graph.lookup_named(c.as_str()) else {
            continue;
        };
        let Some(dest_id) = graph.lookup_named(dest.as_str()) else {
            continue;
        };
        for (x_id, x) in graph.subjects_ids_for_pred_obj_id(type_id, c_id) {
            let triple = Triple::new(x.into_owned(), rdf::TYPE, dest.clone());
            pending_keyed.push((triple, (x_id, type_id, dest_id)));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(subClassOf) × delta(type).
    // Delta iteration still uses NamedNode-keyed grouping because the delta
    // index is keyed by predicate IRI, not TermId. Each subject `x` in a
    // fresh rdf:type triple was interned the round it entered the graph,
    // so `lookup_subject_ref` hits and the consequent's key can be built
    // from `(x_id, type_id, dest_id)` for the keyed commit path. Blank or
    // named subjects whose id cannot be resolved fall back to the plain
    // pending list (defensive; not expected on well-formed graphs).
    let mut new_typings: FxHashMap<NamedNode, Vec<(TermId, NamedOrBlankNode)>> =
        FxHashMap::default();
    for t in d.for_predicate(rdf::TYPE) {
        let Some(c) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
            continue;
        };
        new_typings
            .entry(c)
            .or_default()
            .push((x_id, t.subject.clone()));
    }
    for (c, _, dest, dest_id) in &subclass_pairs {
        if let Some(xs) = new_typings.get(c) {
            for (x_id, x) in xs {
                let triple = Triple::new(x.clone(), rdf::TYPE, dest.clone());
                pending_keyed.push((triple, (*x_id, type_id, *dest_id)));
                firings = firings.saturating_add(1);
            }
        }
    }

    firings
}

fn apply_prp_dom(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    type_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-dom: if p rdfs:domain c and x p y then x rdf:type c.
    //
    // The graph(p) leg goes through `FlatGraph::rows_for_pred_id`, which
    // hands back each matching triple together with its interned
    // `(s_id, p_id, o_id)` tuple. The consequent's key
    // `(x_id, type_id, c_id)` is assembled from that tuple and the pair
    // cache, and the triple is pushed onto `pending_keyed` so the commit
    // loop bypasses `intern_triple`'s three Arc<str> hashes.
    //
    // Semi-naive:
    //   Branch 1: delta(domain) joined with graph(p).
    //   Branch 2: graph(domain) joined with delta(p).
    let mut firings: u64 = 0;

    // Cache each (p, c) pair together with the interned ids of both. The
    // inner loop then feeds `rows_for_pred_id(p_id)` directly and pairs
    // each row's `s_id` with `type_id` and `c_id` for the keyed push.
    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(rdfs::DOMAIN)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let p_id = graph.lookup_named(p.as_str())?;
            let c = named_node_from_term(t.object)?;
            let c_id = graph.lookup_named(c.as_str())?;
            Some((p, p_id, c, c_id))
        })
        .collect();

    let Some(d) = delta else {
        for (_, p_id, c, c_id) in &pairs {
            for (ids, t) in graph.rows_for_pred_id(*p_id) {
                let triple = Triple::new(t.subject.clone(), rdf::TYPE, c.clone());
                pending_keyed.push((triple, (ids.0, type_id, *c_id)));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(domain) × graph(p).
    for t in d.for_predicate(rdfs::DOMAIN) {
        let Some(p) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p_id) = graph.lookup_named(p.as_str()) else {
            continue;
        };
        let Some(c) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(c_id) = graph.lookup_named(c.as_str()) else {
            continue;
        };
        for (ids, row) in graph.rows_for_pred_id(p_id) {
            let triple = Triple::new(row.subject.clone(), rdf::TYPE, c.clone());
            pending_keyed.push((triple, (ids.0, type_id, c_id)));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(domain) × delta(p). Delta iteration is still
    // NamedNode-keyed because `DeltaIndex::by_predicate` is keyed by IRI,
    // so each delta triple's subject id is resolved via
    // `lookup_subject_ref`, one hash per triple.
    for (p, _, c, c_id) in &pairs {
        for t in d.for_predicate(p.as_ref()) {
            let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let triple = Triple::new(t.subject.clone(), rdf::TYPE, c.clone());
            pending_keyed.push((triple, (x_id, type_id, *c_id)));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_prp_rng(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    type_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-rng: if p rdfs:range c and x p y then y rdf:type c.
    // y must not be a literal (literals cannot be subjects of rdf:type).
    //
    // Graph leg goes through `rows_for_pred_id(p_id)`. Each row carries
    // the object's interned id in `ids.2`; if the row's object is a
    // resource (NamedNode or BlankNode) that id is the consequent's
    // subject id and the push goes to `pending_keyed`. Literal objects
    // are skipped because rdf:type cannot take a literal subject.
    //
    // Semi-naive:
    //   Branch 1: delta(range) × graph(p).
    //   Branch 2: graph(range) × delta(p).
    let mut firings: u64 = 0;

    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(rdfs::RANGE)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let p_id = graph.lookup_named(p.as_str())?;
            let c = named_node_from_term(t.object)?;
            let c_id = graph.lookup_named(c.as_str())?;
            Some((p, p_id, c, c_id))
        })
        .collect();

    let Some(d) = delta else {
        for (_, p_id, c, c_id) in &pairs {
            for (ids, row) in graph.rows_for_pred_id(*p_id) {
                let Some(y) = term_ref_to_named_or_blank(row.object.as_ref()) else {
                    continue;
                };
                let triple = Triple::new(y, rdf::TYPE, c.clone());
                pending_keyed.push((triple, (ids.2, type_id, *c_id)));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(range) × graph(p).
    for t in d.for_predicate(rdfs::RANGE) {
        let Some(p) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p_id) = graph.lookup_named(p.as_str()) else {
            continue;
        };
        let Some(c) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(c_id) = graph.lookup_named(c.as_str()) else {
            continue;
        };
        for (ids, row) in graph.rows_for_pred_id(p_id) {
            let Some(y) = term_ref_to_named_or_blank(row.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(y, rdf::TYPE, c.clone());
            pending_keyed.push((triple, (ids.2, type_id, c_id)));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(range) × delta(p). Each delta triple's object id
    // is resolved via `lookup_term_ref`, one hash per triple. Literal
    // objects are filtered out by the NamedOrBlankNode conversion.
    for (p, _, c, c_id) in &pairs {
        for t in d.for_predicate(p.as_ref()) {
            let Some(y) = owned_object_named_or_blank(&t.object) else {
                continue;
            };
            let Some(y_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(y, rdf::TYPE, c.clone());
            pending_keyed.push((triple, (y_id, type_id, *c_id)));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_prp_spo1(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-spo1: if p1 rdfs:subPropertyOf p2 and x p1 y then x p2 y.
    //
    // The graph(p1) leg goes through `rows_for_pred_id(p1_id)`, yielding
    // each row's `(s_id, p1_id, o_id)` tuple alongside the owned triple.
    // The consequent's key is `(s_id, p2_id, o_id)`: same subject and
    // object ids, predicate id swapped for the super-property's. `p2_id`
    // is resolved once per pair at cache build time, so the hot loop
    // builds the key with zero hashing.
    //
    // Semi-naive:
    //   Branch 1: delta(subPropertyOf) × graph(p1).
    //   Branch 2: graph(subPropertyOf) × delta(p1).
    let mut firings: u64 = 0;

    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(rdfs::SUB_PROPERTY_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p1_id = graph.lookup_named(p1.as_str())?;
            let p2 = named_node_from_term(t.object)?;
            let p2_id = graph.lookup_named(p2.as_str())?;
            Some((p1, p1_id, p2, p2_id))
        })
        .collect();

    let Some(d) = delta else {
        for (_, p1_id, p2, p2_id) in &pairs {
            for (ids, row) in graph.rows_for_pred_id(*p1_id) {
                let triple = Triple::new(row.subject.clone(), p2.clone(), row.object.clone());
                pending_keyed.push((triple, (ids.0, *p2_id, ids.2)));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(subPropertyOf) × graph(p1).
    for t in d.for_predicate(rdfs::SUB_PROPERTY_OF) {
        let Some(p1) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p1_id) = graph.lookup_named(p1.as_str()) else {
            continue;
        };
        let Some(p2) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(p2_id) = graph.lookup_named(p2.as_str()) else {
            continue;
        };
        for (ids, row) in graph.rows_for_pred_id(p1_id) {
            let triple = Triple::new(row.subject.clone(), p2.clone(), row.object.clone());
            pending_keyed.push((triple, (ids.0, p2_id, ids.2)));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(subPropertyOf) × delta(p1). Each delta triple's
    // subject and object ids are resolved via the interner, two hashes
    // per triple. `p2_id` is already cached.
    for (p1, _, p2, p2_id) in &pairs {
        for t in d.for_predicate(p1.as_ref()) {
            let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let Some(y_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(t.subject.clone(), p2.clone(), t.object.clone());
            pending_keyed.push((triple, (x_id, *p2_id, y_id)));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_prp_trp(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-trp: if p rdf:type owl:TransitiveProperty, x p y, y p z then x p z.
    //
    // Semi-naive joins the two data antecedents against delta in turn:
    //   Branch 1: delta(p) × graph(p) (new first leg x-p-y extends existing y-p-z).
    //   Branch 2: graph(p) × delta(p) (existing x-p-y meets new second leg y-p-z).
    // If the TransitiveProperty declaration itself is new in this round, every
    // edge counts as "new" for this property and the branches collapse to the
    // naive square join.
    //
    // Edge tuples carry the interned ids of both endpoints so
    // `join_square_keyed` can bucket `right` by `y_id` (u32 probe) and
    // emit consequents straight into `pending_keyed` without touching
    // `intern_triple`. Graph edges pick their ids out of
    // `rows_for_pred_id` for free; delta edges resolve their subject and
    // object ids once per triple via the interner.
    let transitive_properties: Vec<(NamedNode, TermId)> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_TRANSITIVE_PROPERTY)
        .filter_map(|s| match s {
            NamedOrBlankNodeRef::NamedNode(n) => {
                let owned = n.into_owned();
                let n_id = graph.lookup_named(owned.as_str())?;
                Some((owned, n_id))
            }
            NamedOrBlankNodeRef::BlankNode(_) => None,
        })
        .collect();

    // Which properties had their TransitiveProperty declaration arrive in the
    // previous round's delta? An empty set when delta is None.
    let newly_transitive: FxHashSet<NamedNode> = new_property_types(delta, OWL_TRANSITIVE_PROPERTY);

    let mut firings: u64 = 0;
    for (p, p_id) in transitive_properties {
        // Edges carry (x_id, y_id, x, y). Literal objects are skipped
        // because the consequent y-p-z can never be introduced by a
        // literal y anyway.
        let edges: Vec<(TermId, TermId, NamedOrBlankNode, NamedOrBlankNode)> = graph
            .rows_for_pred_id(p_id)
            .filter_map(|(ids, row)| {
                let y = term_ref_to_named_or_blank(row.object.as_ref())?;
                Some((ids.0, ids.2, row.subject.clone(), y))
            })
            .collect();

        let Some(d) = delta else {
            // Round 1: naive square join.
            firings =
                firings.saturating_add(join_square_keyed(&edges, &edges, &p, p_id, pending_keyed));
            continue;
        };

        let delta_edges: Vec<(TermId, TermId, NamedOrBlankNode, NamedOrBlankNode)> =
            if newly_transitive.contains(&p) {
                edges.clone()
            } else {
                d.for_predicate(p.as_ref())
                    .iter()
                    .filter_map(|t| {
                        let y = owned_object_named_or_blank(&t.object)?;
                        let x_id = graph.lookup_subject_ref(t.subject.as_ref())?;
                        let y_id = graph.lookup_term_ref(t.object.as_ref())?;
                        Some((x_id, y_id, t.subject.clone(), y))
                    })
                    .collect()
            };

        // Branch 1: delta × graph.
        firings = firings.saturating_add(join_square_keyed(
            &delta_edges,
            &edges,
            &p,
            p_id,
            pending_keyed,
        ));
        // Branch 2: graph × delta.
        firings = firings.saturating_add(join_square_keyed(
            &edges,
            &delta_edges,
            &p,
            p_id,
            pending_keyed,
        ));
    }
    firings
}

/// Emit every (x, p, z) where (x, y) is in `left` and (y, z) is in `right`.
///
/// Buckets `right` by `y_id` so each probe from `left` is a u32 hash
/// lookup. The pre-interner variant keyed the bucket on `NamedOrBlankNode`
/// and hashed owned IRIs on every probe; moving to `TermId` keys makes
/// the join O(n) in cheap u32 compares and hands the commit loop the
/// fully-formed `(x_id, p_id, z_id)` tuple so `FlatGraph::insert_keyed`
/// skips `intern_triple`'s three Arc<str> hashes per consequent.
fn join_square_keyed(
    left: &[(TermId, TermId, NamedOrBlankNode, NamedOrBlankNode)],
    right: &[(TermId, TermId, NamedOrBlankNode, NamedOrBlankNode)],
    p: &NamedNode,
    p_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let mut right_by_first: FxHashMap<TermId, Vec<(TermId, &NamedOrBlankNode)>> =
        FxHashMap::with_capacity_and_hasher(right.len(), FxBuildHasher);
    for (y2_id, z_id, _y2, z) in right {
        right_by_first.entry(*y2_id).or_default().push((*z_id, z));
    }
    let mut firings: u64 = 0;
    for (x_id, y_id, x, _y) in left {
        if let Some(zs) = right_by_first.get(y_id) {
            for (z_id, z) in zs {
                let triple = Triple::new(x.clone(), p.clone(), (*z).clone());
                pending_keyed.push((triple, (*x_id, p_id, *z_id)));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// Collect IRIs whose (s rdf:type cls) triple landed in the previous round's
/// delta. Returns an empty set when `delta` is `None`.
fn new_property_types(delta: Option<&DeltaIndex>, cls: NamedNodeRef<'_>) -> FxHashSet<NamedNode> {
    let Some(d) = delta else {
        return FxHashSet::default();
    };
    d.for_predicate(rdf::TYPE)
        .iter()
        .filter_map(|t| {
            let Term::NamedNode(n) = &t.object else {
                return None;
            };
            if n.as_ref() != cls {
                return None;
            }
            owned_subject_named(&t.subject)
        })
        .collect()
}

fn apply_prp_symp(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-symp: if p rdf:type owl:SymmetricProperty and x p y then y p x.
    // y must be a resource (literal y cannot appear as a subject).
    //
    // Semi-naive:
    //   Branch 1: delta(schema) × graph(p) for properties newly declared symmetric.
    //   Branch 2: graph(schema) × delta(p) for fresh edges over known symmetric p.
    //
    // Graph legs iterate `rows_for_pred_id(p_id)`, which hands back each
    // triple alongside its `(s_id, p_id, o_id)` interned key. The
    // consequent `(y, p, x)` is keyed as `(o_id, p_id, s_id)` (subject
    // and object swap) and pushed to `pending_keyed` so the commit loop
    // skips `intern_triple`'s three Arc<str> hashes.
    let symmetric_properties: Vec<(NamedNode, TermId)> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_SYMMETRIC_PROPERTY)
        .filter_map(|s| {
            let n = named_node_from_subject(s)?;
            let n_id = graph.lookup_named(n.as_str())?;
            Some((n, n_id))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p, p_id) in &symmetric_properties {
            for (ids, row) in graph.rows_for_pred_id(*p_id) {
                let Some(y) = term_ref_to_named_or_blank(row.object.as_ref()) else {
                    continue;
                };
                let triple = Triple::new(y, p.clone(), row.subject.clone());
                pending_keyed.push((triple, (ids.2, *p_id, ids.0)));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    let newly_symmetric = new_property_types(delta, OWL_SYMMETRIC_PROPERTY);

    for (p, p_id) in &symmetric_properties {
        if newly_symmetric.contains(p) {
            // New schema: every existing edge is a candidate.
            for (ids, row) in graph.rows_for_pred_id(*p_id) {
                let Some(y) = term_ref_to_named_or_blank(row.object.as_ref()) else {
                    continue;
                };
                let triple = Triple::new(y, p.clone(), row.subject.clone());
                pending_keyed.push((triple, (ids.2, *p_id, ids.0)));
                firings = firings.saturating_add(1);
            }
        } else {
            // Existing schema: only delta edges are new. Resolve the
            // subject and object ids via the interner, one pair of
            // hashes per delta triple.
            for t in d.for_predicate(p.as_ref()) {
                let Some(y) = owned_object_named_or_blank(&t.object) else {
                    continue;
                };
                let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                    continue;
                };
                let Some(y_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                    continue;
                };
                let triple = Triple::new(y, p.clone(), t.subject.clone());
                pending_keyed.push((triple, (y_id, *p_id, x_id)));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_prp_inv(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-inv1: if p1 owl:inverseOf p2 and x p1 y then y p2 x.
    // prp-inv2: if p1 owl:inverseOf p2 and x p2 y then y p1 x.
    // Combined because the engine applies both directions off the same fact.
    //
    // Semi-naive:
    //   Branch 1: delta(inverseOf) × graph(p1, p2) (new schema, full data scan).
    //   Branch 2: graph(inverseOf) × delta(p1) (fires prp-inv1).
    //   Branch 3: graph(inverseOf) × delta(p2) (fires prp-inv2).
    //
    // Pair cache carries interned ids for both properties so the graph
    // legs go through `emit_inverse_keyed`, which iterates
    // `rows_for_pred_id` and synthesises the swapped key
    // `(o_id, dst_id, s_id)` straight from each row's interned tuple.
    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(OWL_INVERSE_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            let p1_id = graph.lookup_named(p1.as_str())?;
            let p2_id = graph.lookup_named(p2.as_str())?;
            Some((p1, p1_id, p2, p2_id))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p1, p1_id, p2, p2_id) in &pairs {
            firings = firings.saturating_add(emit_inverse_keyed(
                graph,
                *p1_id,
                p2,
                *p2_id,
                pending_keyed,
            ));
            firings = firings.saturating_add(emit_inverse_keyed(
                graph,
                *p2_id,
                p1,
                *p1_id,
                pending_keyed,
            ));
        }
        return firings;
    };

    // Branch 1: delta schema × full graph data.
    let mut seen_schema: FxHashSet<(NamedNode, NamedNode)> = FxHashSet::default();
    for t in d.for_predicate(OWL_INVERSE_OF) {
        let Some(p1) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p2) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(p1_id) = graph.lookup_named(p1.as_str()) else {
            continue;
        };
        let Some(p2_id) = graph.lookup_named(p2.as_str()) else {
            continue;
        };
        seen_schema.insert((p1.clone(), p2.clone()));
        firings =
            firings.saturating_add(emit_inverse_keyed(graph, p1_id, &p2, p2_id, pending_keyed));
        firings =
            firings.saturating_add(emit_inverse_keyed(graph, p2_id, &p1, p1_id, pending_keyed));
    }

    // Branches 2 and 3: graph schema × delta data. Each delta triple's
    // subject and object ids are resolved via the interner, one hash
    // each.
    for (p1, p1_id, p2, p2_id) in &pairs {
        // Skip schema pairs we already fully saturated in Branch 1.
        if seen_schema.contains(&(p1.clone(), p2.clone())) {
            continue;
        }
        // prp-inv1 over delta(p1). Consequent: (y, p2, x).
        for t in d.for_predicate(p1.as_ref()) {
            let Some(y) = owned_object_named_or_blank(&t.object) else {
                continue;
            };
            let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let Some(y_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(y, p2.clone(), t.subject.clone());
            pending_keyed.push((triple, (y_id, *p2_id, x_id)));
            firings = firings.saturating_add(1);
        }
        // prp-inv2 over delta(p2). Consequent: (y, p1, x).
        for t in d.for_predicate(p2.as_ref()) {
            let Some(y) = owned_object_named_or_blank(&t.object) else {
                continue;
            };
            let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let Some(y_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(y, p1.clone(), t.subject.clone());
            pending_keyed.push((triple, (y_id, *p1_id, x_id)));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

/// For each row `(x, src, y)` in `graph.rows_for_pred_id(src_id)`, push
/// `((y, dst, x), (y_id, dst_id, x_id))` onto `pending_keyed`. Skips
/// `intern_triple`'s three Arc<str> hashes per emit.
fn emit_inverse_keyed(
    graph: &FlatGraph,
    src_id: TermId,
    dst: &NamedNode,
    dst_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    let mut firings: u64 = 0;
    for (ids, row) in graph.rows_for_pred_id(src_id) {
        let Some(y) = term_ref_to_named_or_blank(row.object.as_ref()) else {
            continue;
        };
        let triple = Triple::new(y, dst.clone(), row.subject.clone());
        pending_keyed.push((triple, (ids.2, dst_id, ids.0)));
        firings = firings.saturating_add(1);
    }
    firings
}

fn apply_prp_eqp(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // prp-eqp1: if p1 owl:equivalentProperty p2 and x p1 y then x p2 y.
    // prp-eqp2: if p1 owl:equivalentProperty p2 and x p2 y then x p1 y.
    // Literal objects are fine here: the object keeps its position.
    //
    // Semi-naive:
    //   Branch 1: delta(schema) × graph(p1, p2).
    //   Branch 2: graph(schema) × delta(p1) (fires prp-eqp1).
    //   Branch 3: graph(schema) × delta(p2) (fires prp-eqp2).
    //
    // Pair cache resolves interned ids once. Graph legs iterate
    // `rows_for_pred_id`, lifting `(s_id, o_id)` straight off each row
    // and committing `(s_id, dst_id, o_id)` keyed triples. Delta legs
    // hash subject and object once each.
    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(OWL_EQUIVALENT_PROPERTY)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            let p1_id = graph.lookup_named(p1.as_str())?;
            let p2_id = graph.lookup_named(p2.as_str())?;
            Some((p1, p1_id, p2, p2_id))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p1, p1_id, p2, p2_id) in &pairs {
            firings =
                firings.saturating_add(emit_rename_keyed(graph, *p1_id, p2, *p2_id, pending_keyed));
            firings =
                firings.saturating_add(emit_rename_keyed(graph, *p2_id, p1, *p1_id, pending_keyed));
        }
        return firings;
    };

    let mut seen_schema: FxHashSet<(NamedNode, NamedNode)> = FxHashSet::default();
    for t in d.for_predicate(OWL_EQUIVALENT_PROPERTY) {
        let Some(p1) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p2) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(p1_id) = graph.lookup_named(p1.as_str()) else {
            continue;
        };
        let Some(p2_id) = graph.lookup_named(p2.as_str()) else {
            continue;
        };
        seen_schema.insert((p1.clone(), p2.clone()));
        firings =
            firings.saturating_add(emit_rename_keyed(graph, p1_id, &p2, p2_id, pending_keyed));
        firings =
            firings.saturating_add(emit_rename_keyed(graph, p2_id, &p1, p1_id, pending_keyed));
    }

    for (p1, p1_id, p2, p2_id) in &pairs {
        if seen_schema.contains(&(p1.clone(), p2.clone())) {
            continue;
        }
        // prp-eqp1 over delta(p1). Rewrite to p2.
        for t in d.for_predicate(p1.as_ref()) {
            let Some(s_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let Some(o_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(t.subject.clone(), p2.clone(), t.object.clone());
            pending_keyed.push((triple, (s_id, *p2_id, o_id)));
            firings = firings.saturating_add(1);
        }
        // prp-eqp2 over delta(p2). Rewrite to p1.
        for t in d.for_predicate(p2.as_ref()) {
            let Some(s_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
                continue;
            };
            let Some(o_id) = graph.lookup_term_ref(t.object.as_ref()) else {
                continue;
            };
            let triple = Triple::new(t.subject.clone(), p1.clone(), t.object.clone());
            pending_keyed.push((triple, (s_id, *p1_id, o_id)));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

/// For each row `(x, src, y)` in `graph.rows_for_pred_id(src_id)`, push
/// `((x, dst, y), (x_id, dst_id, y_id))` onto `pending_keyed`. Used by
/// prp-eqp where the object position stays put and the predicate is
/// renamed. Skips `intern_triple`'s three Arc<str> hashes per emit.
fn emit_rename_keyed(
    graph: &FlatGraph,
    src_id: TermId,
    dst: &NamedNode,
    dst_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    let mut firings: u64 = 0;
    for (ids, row) in graph.rows_for_pred_id(src_id) {
        let triple = Triple::new(row.subject.clone(), dst.clone(), row.object.clone());
        pending_keyed.push((triple, (ids.0, dst_id, ids.2)));
        firings = firings.saturating_add(1);
    }
    firings
}

/// Unkeyed square-join helper retained for the equality rule family
/// (`eq-trans` and friends), which stays on the original `pending` path
/// until someone benches keyed conversions for rules that are off by
/// default and not part of LUBM's hot path.
fn join_square(
    left: &[(NamedOrBlankNode, NamedOrBlankNode)],
    right: &[(NamedOrBlankNode, NamedOrBlankNode)],
    p: &NamedNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let mut right_by_first: FxHashMap<&NamedOrBlankNode, Vec<&NamedOrBlankNode>> =
        FxHashMap::with_capacity_and_hasher(right.len(), FxBuildHasher);
    for (y2, z) in right {
        right_by_first.entry(y2).or_default().push(z);
    }
    let mut firings: u64 = 0;
    for (x, y) in left {
        if let Some(zs) = right_by_first.get(y) {
            for z in zs {
                pending.push(Triple::new(x.clone(), p.clone(), (*z).clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// Unkeyed predicate-rename helper retained for `eq-rep-p`. Same
/// rationale as `join_square`: equality rules are off-by-default and
/// outside LUBM's profile, so conversion waits on a benchmark-led reason
/// to do it.
fn emit_rename(
    graph: &FlatGraph,
    src: &NamedNode,
    dst: &NamedNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    let edges: Vec<(NamedOrBlankNode, Term)> = graph
        .triples_for_predicate(src.as_ref())
        .map(|t| (t.subject.into_owned(), t.object.into_owned()))
        .collect();
    let mut firings: u64 = 0;
    for (x, y) in edges {
        pending.push(Triple::new(x, dst.clone(), y));
        firings = firings.saturating_add(1);
    }
    firings
}

fn apply_cax_eqc(
    graph: &FlatGraph,
    delta: Option<&DeltaIndex>,
    type_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    // cax-eqc1: if c1 owl:equivalentClass c2 and x rdf:type c1 then x rdf:type c2.
    // cax-eqc2: if c1 owl:equivalentClass c2 and x rdf:type c2 then x rdf:type c1.
    //
    // Semi-naive:
    //   Branch 1: delta(equivalentClass) × graph(type).
    //   Branch 2: graph(equivalentClass) × delta(type) (indexed by class).
    //
    // The graph(type) leg goes through
    // `FlatGraph::subjects_ids_for_pred_obj_id`, which returns each subject
    // paired with its interned `TermId`. The `(x_id, type_id, to_id)` key
    // is assembled right there so the consequent commits through
    // `FlatGraph::insert_keyed` and skips `intern_triple`'s three Arc<str>
    // hashes per emitted triple. Branch 2 does the same by resolving the
    // delta subject's id once via `lookup_subject_ref` when indexing.
    let pairs: Vec<(NamedNode, TermId, NamedNode, TermId)> = graph
        .triples_for_predicate(OWL_EQUIVALENT_CLASS)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            let c1_id = graph.lookup_named(c1.as_str())?;
            let c2_id = graph.lookup_named(c2.as_str())?;
            Some((c1, c1_id, c2, c2_id))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (c1, c1_id, c2, c2_id) in &pairs {
            firings = firings.saturating_add(reclassify(
                graph,
                *c1_id,
                c2,
                *c2_id,
                type_id,
                pending_keyed,
            ));
            firings = firings.saturating_add(reclassify(
                graph,
                *c2_id,
                c1,
                *c1_id,
                type_id,
                pending_keyed,
            ));
        }
        return firings;
    };

    // Branch 1: delta(equivalentClass) × graph(type). Track which schema
    // pairs we already processed so Branch 2 does not duplicate them.
    let mut seen_schema: FxHashSet<(NamedNode, NamedNode)> = FxHashSet::default();
    for t in d.for_predicate(OWL_EQUIVALENT_CLASS) {
        let Some(c1) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(c2) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(c1_id) = graph.lookup_named(c1.as_str()) else {
            continue;
        };
        let Some(c2_id) = graph.lookup_named(c2.as_str()) else {
            continue;
        };
        seen_schema.insert((c1.clone(), c2.clone()));
        firings =
            firings.saturating_add(reclassify(graph, c1_id, &c2, c2_id, type_id, pending_keyed));
        firings =
            firings.saturating_add(reclassify(graph, c2_id, &c1, c1_id, type_id, pending_keyed));
    }

    // Branch 2: graph(equivalentClass) × delta(type). Index delta typings
    // once by class together with each subject's interned id so each
    // schema pair emits (x, rdf:type, c) triples with the full key in
    // hand.
    let new_typings = index_delta_types_with_ids(graph, d);
    for (c1, c1_id, c2, c2_id) in &pairs {
        if seen_schema.contains(&(c1.clone(), c2.clone())) {
            continue;
        }
        if let Some(xs) = new_typings.get(c1) {
            for (x_id, x) in xs {
                let triple = Triple::new(x.clone(), rdf::TYPE, c2.clone());
                pending_keyed.push((triple, (*x_id, type_id, *c2_id)));
                firings = firings.saturating_add(1);
            }
        }
        if let Some(xs) = new_typings.get(c2) {
            for (x_id, x) in xs {
                let triple = Triple::new(x.clone(), rdf::TYPE, c1.clone());
                pending_keyed.push((triple, (*x_id, type_id, *c1_id)));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// For every (x, rdf:type, from) edge in the graph push (x, rdf:type, to)
/// onto the keyed commit buffer. Goes through
/// `FlatGraph::subjects_ids_for_pred_obj_id` so the lookup hashes a
/// `(TermId, TermId)` pair and the consequent's key is assembled
/// component-by-component without re-interning.
fn reclassify(
    graph: &FlatGraph,
    from_id: TermId,
    to: &NamedNode,
    to_id: TermId,
    type_id: TermId,
    pending_keyed: &mut Vec<(Triple, (TermId, TermId, TermId))>,
) -> u64 {
    let mut firings: u64 = 0;
    for (x_id, x) in graph.subjects_ids_for_pred_obj_id(type_id, from_id) {
        let triple = Triple::new(x.into_owned(), rdf::TYPE, to.clone());
        pending_keyed.push((triple, (x_id, type_id, to_id)));
        firings = firings.saturating_add(1);
    }
    firings
}

/// Group delta `rdf:type` triples by their object class, carrying each
/// subject's interned id alongside the owned subject. The id probe goes
/// through `lookup_subject_ref`, one hash per delta triple, and saves the
/// three hashes that `intern_triple` would otherwise do on commit.
fn index_delta_types_with_ids(
    graph: &FlatGraph,
    delta: &DeltaIndex,
) -> FxHashMap<NamedNode, Vec<(TermId, NamedOrBlankNode)>> {
    let mut out: FxHashMap<NamedNode, Vec<(TermId, NamedOrBlankNode)>> = FxHashMap::default();
    for t in delta.for_predicate(rdf::TYPE) {
        let Some(c) = owned_object_named(&t.object) else {
            continue;
        };
        let Some(x_id) = graph.lookup_subject_ref(t.subject.as_ref()) else {
            continue;
        };
        out.entry(c).or_default().push((x_id, t.subject.clone()));
    }
    out
}

// ---------------------------------------------------------------------------
// Schema rules (scm-*). Close the graph under class and property hierarchy
// axioms. None of these touch instance data.

fn apply_scm_cls(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-cls: for every c rdf:type owl:Class, add
    //   c rdfs:subClassOf c
    //   c owl:equivalentClass c
    //   c rdfs:subClassOf owl:Thing
    //   owl:Nothing rdfs:subClassOf c
    //
    // Semi-naive: with delta, only fire for classes whose declaration is new.
    let classes: Vec<NamedNode> = if let Some(d) = delta {
        new_property_types(Some(d), OWL_CLASS).into_iter().collect()
    } else {
        graph
            .subjects_for_predicate_object(rdf::TYPE, OWL_CLASS)
            .filter_map(named_node_from_subject)
            .collect()
    };

    let mut firings: u64 = 0;
    for c in classes {
        pending.push(Triple::new(c.clone(), rdfs::SUB_CLASS_OF, c.clone()));
        pending.push(Triple::new(
            c.clone(),
            OWL_EQUIVALENT_CLASS.into_owned(),
            c.clone(),
        ));
        pending.push(Triple::new(
            c.clone(),
            rdfs::SUB_CLASS_OF,
            OWL_THING.into_owned(),
        ));
        pending.push(Triple::new(OWL_NOTHING.into_owned(), rdfs::SUB_CLASS_OF, c));
        firings = firings.saturating_add(4);
    }
    firings
}

fn apply_scm_sco(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-sco: transitivity of rdfs:subClassOf.
    //
    // Semi-naive:
    //   Branch 1: delta(sco) × graph(sco).
    //   Branch 2: graph(sco) × delta(sco).
    let edges: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            Some((c1, c2))
        })
        .collect();

    let mut firings: u64 = 0;

    // A predicate on a hash index by the pivot column `c2` lets each chain
    // join run in expected O(|left|) instead of O(|left| * |right|). The
    // class hierarchy in OWL 2 RL can get wide (thousands of classes), so
    // this matters in practice even when the hierarchy itself is sparse.
    let right_by_pivot = build_pivot_index(&edges);

    let Some(d) = delta else {
        firings = firings.saturating_add(join_chain_hashed(
            &edges,
            &right_by_pivot,
            rdfs::SUB_CLASS_OF,
            pending,
        ));
        return firings;
    };

    let delta_edges: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_CLASS_OF)
        .iter()
        .filter_map(|t| {
            let c1 = owned_subject_named(&t.subject)?;
            let c2 = owned_object_named(&t.object)?;
            Some((c1, c2))
        })
        .collect();

    let delta_by_pivot = build_pivot_index(&delta_edges);

    // Branch 1: delta(sco) × graph(sco). Pivot on graph's left column.
    firings = firings.saturating_add(join_chain_hashed(
        &delta_edges,
        &right_by_pivot,
        rdfs::SUB_CLASS_OF,
        pending,
    ));
    // Branch 2: graph(sco) × delta(sco). Pivot on delta's left column.
    firings = firings.saturating_add(join_chain_hashed(
        &edges,
        &delta_by_pivot,
        rdfs::SUB_CLASS_OF,
        pending,
    ));
    firings
}

/// Build a hash index keyed by the left column of a pair list. Used to
/// accelerate transitive-closure joins where the pivot column of the right
/// side is the first element of each tuple (i.e. for a chain join
/// `(a, b) x (b, c)` indexing `(b, c)` by `b`).
fn build_pivot_index(pairs: &[(NamedNode, NamedNode)]) -> FxHashMap<NamedNode, Vec<NamedNode>> {
    let mut out: FxHashMap<NamedNode, Vec<NamedNode>> = FxHashMap::default();
    for (k, v) in pairs {
        out.entry(k.clone()).or_default().push(v.clone());
    }
    out
}

/// Hash-indexed chain join: for each `(c1, c2)` in `left` look up every
/// `c3` such that `(c2, c3)` sits in the index, and push
/// `(c1, predicate, c3)` onto `pending`. Expected cost is
/// `O(|left| + matches)` rather than the `O(|left| * |right|)` of the
/// nested-loop version.
fn join_chain_hashed(
    left: &[(NamedNode, NamedNode)],
    right_by_pivot: &FxHashMap<NamedNode, Vec<NamedNode>>,
    predicate: NamedNodeRef<'_>,
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (c1, c2) in left {
        if let Some(targets) = right_by_pivot.get(c2) {
            for c3 in targets {
                pending.push(Triple::new(c1.clone(), predicate, c3.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_scm_op(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-op: for every p rdf:type owl:ObjectProperty, add
    //   p rdfs:subPropertyOf p
    //   p owl:equivalentProperty p
    //
    // Semi-naive: only fire for properties whose declaration is new.
    let properties: Vec<NamedNode> = if delta.is_some() {
        new_property_types(delta, OWL_OBJECT_PROPERTY)
            .into_iter()
            .collect()
    } else {
        graph
            .subjects_for_predicate_object(rdf::TYPE, OWL_OBJECT_PROPERTY)
            .filter_map(named_node_from_subject)
            .collect()
    };

    let mut firings: u64 = 0;
    for p in properties {
        pending.push(Triple::new(p.clone(), rdfs::SUB_PROPERTY_OF, p.clone()));
        pending.push(Triple::new(
            p.clone(),
            OWL_EQUIVALENT_PROPERTY.into_owned(),
            p,
        ));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_dp(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-dp: same as scm-op but for owl:DatatypeProperty.
    let properties: Vec<NamedNode> = if delta.is_some() {
        new_property_types(delta, OWL_DATATYPE_PROPERTY)
            .into_iter()
            .collect()
    } else {
        graph
            .subjects_for_predicate_object(rdf::TYPE, OWL_DATATYPE_PROPERTY)
            .filter_map(named_node_from_subject)
            .collect()
    };

    let mut firings: u64 = 0;
    for p in properties {
        pending.push(Triple::new(p.clone(), rdfs::SUB_PROPERTY_OF, p.clone()));
        pending.push(Triple::new(
            p.clone(),
            OWL_EQUIVALENT_PROPERTY.into_owned(),
            p,
        ));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_eqc1(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-eqc1: c1 owl:equivalentClass c2 then
    //   c1 rdfs:subClassOf c2, c2 rdfs:subClassOf c1.
    //
    // Semi-naive: only iterate new owl:equivalentClass edges.
    let pairs: Vec<(NamedNode, NamedNode)> = if let Some(d) = delta {
        d.for_predicate(OWL_EQUIVALENT_CLASS)
            .iter()
            .filter_map(|t| {
                let c1 = owned_subject_named(&t.subject)?;
                let c2 = owned_object_named(&t.object)?;
                Some((c1, c2))
            })
            .collect()
    } else {
        graph
            .triples_for_predicate(OWL_EQUIVALENT_CLASS)
            .filter_map(|t| {
                let c1 = named_node_from_subject(t.subject)?;
                let c2 = named_node_from_term(t.object)?;
                Some((c1, c2))
            })
            .collect()
    };

    let mut firings: u64 = 0;
    for (c1, c2) in pairs {
        pending.push(Triple::new(c1.clone(), rdfs::SUB_CLASS_OF, c2.clone()));
        pending.push(Triple::new(c2, rdfs::SUB_CLASS_OF, c1));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_eqc2(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-eqc2: c1 rdfs:subClassOf c2 and c2 rdfs:subClassOf c1 then
    //   c1 owl:equivalentClass c2.
    //
    // Semi-naive:
    //   Branch 1: delta(sco) × graph(sco).
    //   Branch 2: graph(sco) × delta(sco).
    let edges: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            Some((c1, c2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        firings = firings.saturating_add(emit_equivalent_pairs(&edges, &edges, pending));
        return firings;
    };

    let delta_edges: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_CLASS_OF)
        .iter()
        .filter_map(|t| {
            let c1 = owned_subject_named(&t.subject)?;
            let c2 = owned_object_named(&t.object)?;
            Some((c1, c2))
        })
        .collect();

    firings = firings.saturating_add(emit_equivalent_pairs(&delta_edges, &edges, pending));
    firings = firings.saturating_add(emit_equivalent_pairs(&edges, &delta_edges, pending));
    firings
}

/// For (c1, c2) in `left` and (c2b, c1b) in `right` with c1 != c2 and the
/// edges forming a mutual subclass, emit c1 owl:equivalentClass c2.
fn emit_equivalent_pairs(
    left: &[(NamedNode, NamedNode)],
    right: &[(NamedNode, NamedNode)],
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (c1, c2) in left {
        for (c2b, c1b) in right {
            if c1 == c1b && c2 == c2b && c1 != c2 {
                pending.push(Triple::new(
                    c1.clone(),
                    OWL_EQUIVALENT_CLASS.into_owned(),
                    c2.clone(),
                ));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_scm_eqp1(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-eqp1: p1 owl:equivalentProperty p2 then
    //   p1 rdfs:subPropertyOf p2, p2 rdfs:subPropertyOf p1.
    //
    // Semi-naive: only iterate new owl:equivalentProperty edges.
    let pairs: Vec<(NamedNode, NamedNode)> = if let Some(d) = delta {
        d.for_predicate(OWL_EQUIVALENT_PROPERTY)
            .iter()
            .filter_map(|t| {
                let p1 = owned_subject_named(&t.subject)?;
                let p2 = owned_object_named(&t.object)?;
                Some((p1, p2))
            })
            .collect()
    } else {
        graph
            .triples_for_predicate(OWL_EQUIVALENT_PROPERTY)
            .filter_map(|t| {
                let p1 = named_node_from_subject(t.subject)?;
                let p2 = named_node_from_term(t.object)?;
                Some((p1, p2))
            })
            .collect()
    };

    let mut firings: u64 = 0;
    for (p1, p2) in pairs {
        pending.push(Triple::new(p1.clone(), rdfs::SUB_PROPERTY_OF, p2.clone()));
        pending.push(Triple::new(p2, rdfs::SUB_PROPERTY_OF, p1));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_eqp2(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-eqp2: p1 rdfs:subPropertyOf p2 and p2 rdfs:subPropertyOf p1 then
    //   p1 owl:equivalentProperty p2.
    //
    // Semi-naive:
    //   Branch 1: delta(spo) × graph(spo).
    //   Branch 2: graph(spo) × delta(spo).
    let edges: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_PROPERTY_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        firings = firings.saturating_add(emit_equivalent_property_pairs(&edges, &edges, pending));
        return firings;
    };

    let delta_edges: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_PROPERTY_OF)
        .iter()
        .filter_map(|t| {
            let p1 = owned_subject_named(&t.subject)?;
            let p2 = owned_object_named(&t.object)?;
            Some((p1, p2))
        })
        .collect();

    firings = firings.saturating_add(emit_equivalent_property_pairs(
        &delta_edges,
        &edges,
        pending,
    ));
    firings = firings.saturating_add(emit_equivalent_property_pairs(
        &edges,
        &delta_edges,
        pending,
    ));
    firings
}

/// Mirror of [`emit_equivalent_pairs`] for owl:equivalentProperty.
fn emit_equivalent_property_pairs(
    left: &[(NamedNode, NamedNode)],
    right: &[(NamedNode, NamedNode)],
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (p1, p2) in left {
        for (p2b, p1b) in right {
            if p1 == p1b && p2 == p2b && p1 != p2 {
                pending.push(Triple::new(
                    p1.clone(),
                    OWL_EQUIVALENT_PROPERTY.into_owned(),
                    p2.clone(),
                ));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_scm_dom1(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-dom1: p rdfs:domain c1 and c1 rdfs:subClassOf c2 then p rdfs:domain c2.
    //
    // Semi-naive:
    //   Branch 1: delta(domain) × graph(sco).
    //   Branch 2: graph(domain) × delta(sco).
    let domains: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::DOMAIN)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let c1 = named_node_from_term(t.object)?;
            Some((p, c1))
        })
        .collect();
    let subclasses: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            Some((c1, c2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        firings = firings.saturating_add(join_predicate_domain_range(
            &domains,
            &subclasses,
            rdfs::DOMAIN,
            pending,
        ));
        return firings;
    };

    let delta_domains: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::DOMAIN)
        .iter()
        .filter_map(|t| {
            let p = owned_subject_named(&t.subject)?;
            let c = owned_object_named(&t.object)?;
            Some((p, c))
        })
        .collect();
    let delta_sco: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_CLASS_OF)
        .iter()
        .filter_map(|t| {
            let c1 = owned_subject_named(&t.subject)?;
            let c2 = owned_object_named(&t.object)?;
            Some((c1, c2))
        })
        .collect();

    firings = firings.saturating_add(join_predicate_domain_range(
        &delta_domains,
        &subclasses,
        rdfs::DOMAIN,
        pending,
    ));
    firings = firings.saturating_add(join_predicate_domain_range(
        &domains,
        &delta_sco,
        rdfs::DOMAIN,
        pending,
    ));
    firings
}

/// Propagate a property axiom (domain or range) along a subclass chain:
/// for each (p, c1) in `axioms` and (c1, c2) in `chain`, emit
/// (p, predicate, c2).
fn join_predicate_domain_range(
    axioms: &[(NamedNode, NamedNode)],
    chain: &[(NamedNode, NamedNode)],
    predicate: NamedNodeRef<'_>,
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (p, c1) in axioms {
        for (c1b, c2) in chain {
            if c1 == c1b {
                pending.push(Triple::new(p.clone(), predicate.into_owned(), c2.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_scm_rng1(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-rng1: p rdfs:range c1 and c1 rdfs:subClassOf c2 then p rdfs:range c2.
    //
    // Semi-naive:
    //   Branch 1: delta(range) × graph(sco).
    //   Branch 2: graph(range) × delta(sco).
    let ranges: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::RANGE)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let c1 = named_node_from_term(t.object)?;
            Some((p, c1))
        })
        .collect();
    let subclasses: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            Some((c1, c2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        firings = firings.saturating_add(join_predicate_domain_range(
            &ranges,
            &subclasses,
            rdfs::RANGE,
            pending,
        ));
        return firings;
    };

    let delta_ranges: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::RANGE)
        .iter()
        .filter_map(|t| {
            let p = owned_subject_named(&t.subject)?;
            let c = owned_object_named(&t.object)?;
            Some((p, c))
        })
        .collect();
    let delta_sco: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_CLASS_OF)
        .iter()
        .filter_map(|t| {
            let c1 = owned_subject_named(&t.subject)?;
            let c2 = owned_object_named(&t.object)?;
            Some((c1, c2))
        })
        .collect();

    firings = firings.saturating_add(join_predicate_domain_range(
        &delta_ranges,
        &subclasses,
        rdfs::RANGE,
        pending,
    ));
    firings = firings.saturating_add(join_predicate_domain_range(
        &ranges,
        &delta_sco,
        rdfs::RANGE,
        pending,
    ));
    firings
}

// ---------------------------------------------------------------------------
// Equality rules. Gated behind `ReasonerConfig::with_equality_rules` because
// they explode graph size on noisy data. Skipped entirely in the Rdfs
// profile.

fn apply_eq_sym(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // eq-sym: if x owl:sameAs y then y owl:sameAs x.
    //
    // Semi-naive: only iterate delta(sameAs) edges.
    let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = if let Some(d) = delta {
        d.for_predicate(OWL_SAME_AS)
            .iter()
            .filter_map(|t| {
                let y = owned_object_named_or_blank(&t.object)?;
                Some((t.subject.clone(), y))
            })
            .collect()
    } else {
        graph
            .triples_for_predicate(OWL_SAME_AS)
            .filter_map(|t| {
                let x = t.subject.into_owned();
                let y = term_ref_to_named_or_blank(t.object)?;
                Some((x, y))
            })
            .collect()
    };

    let mut firings: u64 = 0;
    for (x, y) in edges {
        pending.push(Triple::new(y, OWL_SAME_AS.into_owned(), x));
        firings = firings.saturating_add(1);
    }
    firings
}

fn apply_eq_trans(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // eq-trans: if x owl:sameAs y, y owl:sameAs z then x owl:sameAs z.
    // Same snapshot-then-join shape as prp-trp.
    //
    // Semi-naive:
    //   Branch 1: delta(sameAs) × graph(sameAs).
    //   Branch 2: graph(sameAs) × delta(sameAs).
    let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
        .triples_for_predicate(OWL_SAME_AS)
        .filter_map(|t| {
            let x = t.subject.into_owned();
            let y = term_ref_to_named_or_blank(t.object)?;
            Some((x, y))
        })
        .collect();

    let mut firings: u64 = 0;

    let sameas_p = OWL_SAME_AS.into_owned();

    let Some(d) = delta else {
        firings = firings.saturating_add(join_square(&edges, &edges, &sameas_p, pending));
        return firings;
    };

    let delta_edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = d
        .for_predicate(OWL_SAME_AS)
        .iter()
        .filter_map(|t| {
            let y = owned_object_named_or_blank(&t.object)?;
            Some((t.subject.clone(), y))
        })
        .collect();

    firings = firings.saturating_add(join_square(&delta_edges, &edges, &sameas_p, pending));
    firings = firings.saturating_add(join_square(&edges, &delta_edges, &sameas_p, pending));
    firings
}

fn apply_eq_rep_s(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // eq-rep-s: if x owl:sameAs y and x p o then y p o.
    // The rewritten triple uses y as subject, so y must be a resource.
    //
    // Semi-naive:
    //   Branch 1: delta(sameAs) × graph(x, *, *).
    //   Branch 2: graph(sameAs) × delta(subject = x).
    let pairs: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
        .triples_for_predicate(OWL_SAME_AS)
        .filter_map(|t| {
            let x = t.subject.into_owned();
            let y = term_ref_to_named_or_blank(t.object)?;
            Some((x, y))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (x, y) in pairs {
            firings = firings.saturating_add(emit_rewrite_subject(graph, &x, &y, pending));
        }
        return firings;
    };

    // Branch 1: delta(sameAs) × graph.
    let mut seen_schema: FxHashSet<(NamedOrBlankNode, NamedOrBlankNode)> = FxHashSet::default();
    for t in d.for_predicate(OWL_SAME_AS) {
        let Some(y) = owned_object_named_or_blank(&t.object) else {
            continue;
        };
        seen_schema.insert((t.subject.clone(), y.clone()));
        firings = firings.saturating_add(emit_rewrite_subject(graph, &t.subject, &y, pending));
    }

    // Branch 2: graph(sameAs) × delta triples whose subject matches `x`.
    // Index delta triples by subject once, skipping owl:sameAs rows which are
    // handled by eq-sym/eq-trans.
    let mut by_subject: FxHashMap<NamedOrBlankNode, Vec<(NamedNode, Term)>> = FxHashMap::default();
    for triples in d.by_predicate.values() {
        for t in triples {
            if t.predicate.as_ref() == OWL_SAME_AS {
                continue;
            }
            by_subject
                .entry(t.subject.clone())
                .or_default()
                .push((t.predicate.clone(), t.object.clone()));
        }
    }
    for (x, y) in pairs {
        if seen_schema.contains(&(x.clone(), y.clone())) {
            continue;
        }
        if let Some(entries) = by_subject.get(&x) {
            for (p, o) in entries {
                pending.push(Triple::new(y.clone(), p.clone(), o.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// For each triple `(x, p, o)` in `graph` where `p != owl:sameAs`, push
/// `(y, p, o)` onto `pending`. Helper for eq-rep-s semi-naive branches.
fn emit_rewrite_subject(
    graph: &FlatGraph,
    x: &NamedOrBlankNode,
    y: &NamedOrBlankNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    let triples_for_x: Vec<(NamedNode, Term)> = graph
        .triples_for_subject(x.as_ref())
        .filter_map(|t| {
            if t.predicate == OWL_SAME_AS {
                return None;
            }
            Some((t.predicate.into_owned(), t.object.into_owned()))
        })
        .collect();
    let mut firings: u64 = 0;
    for (p, o) in triples_for_x {
        pending.push(Triple::new(y.clone(), p, o));
        firings = firings.saturating_add(1);
    }
    firings
}

fn apply_eq_rep_p(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // eq-rep-p: if p1 owl:sameAs p2 and x p1 o then x p2 o.
    // p2 becomes a predicate, so both p1 and p2 must be IRIs.
    //
    // Semi-naive:
    //   Branch 1: delta(sameAs) × graph(p1).
    //   Branch 2: graph(sameAs) × delta(p1).
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(OWL_SAME_AS)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p1, p2) in pairs {
            firings = firings.saturating_add(emit_rename(graph, &p1, &p2, pending));
        }
        return firings;
    };

    // Branch 1: delta(sameAs) × graph(p1).
    let mut seen_schema: FxHashSet<(NamedNode, NamedNode)> = FxHashSet::default();
    for t in d.for_predicate(OWL_SAME_AS) {
        let Some(p1) = owned_subject_named(&t.subject) else {
            continue;
        };
        let Some(p2) = owned_object_named(&t.object) else {
            continue;
        };
        seen_schema.insert((p1.clone(), p2.clone()));
        firings = firings.saturating_add(emit_rename(graph, &p1, &p2, pending));
    }

    // Branch 2: graph(sameAs) × delta(p1).
    for (p1, p2) in &pairs {
        if seen_schema.contains(&(p1.clone(), p2.clone())) {
            continue;
        }
        for t in d.for_predicate(p1.as_ref()) {
            pending.push(Triple::new(t.subject.clone(), p2.clone(), t.object.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_eq_rep_o(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // eq-rep-o: if o1 owl:sameAs o2 and x p o1 then x p o2.
    // o1 lives in the object position. Index by object so we avoid scanning
    // the whole graph.
    //
    // Semi-naive:
    //   Branch 1: delta(sameAs) × graph(?, ?, o1).
    //   Branch 2: graph(sameAs) × delta triples with object = o1.
    let pairs: Vec<(NamedOrBlankNode, Term)> = graph
        .triples_for_predicate(OWL_SAME_AS)
        .map(|t| (t.subject.into_owned(), t.object.into_owned()))
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (o1, o2) in pairs {
            firings = firings.saturating_add(emit_rewrite_object(graph, &o1, &o2, pending));
        }
        return firings;
    };

    // Branch 1: delta(sameAs) × graph(?, ?, o1).
    let mut seen_schema: FxHashSet<(NamedOrBlankNode, Term)> = FxHashSet::default();
    for t in d.for_predicate(OWL_SAME_AS) {
        let o1: NamedOrBlankNode = t.subject.clone();
        let o2: Term = t.object.clone();
        seen_schema.insert((o1.clone(), o2.clone()));
        firings = firings.saturating_add(emit_rewrite_object(graph, &o1, &o2, pending));
    }

    // Branch 2: graph(sameAs) × delta triples with object = o1. Index delta
    // once by its object position (Term). Skip owl:sameAs rows.
    let mut by_object: FxHashMap<Term, Vec<(NamedOrBlankNode, NamedNode)>> = FxHashMap::default();
    for triples in d.by_predicate.values() {
        for t in triples {
            if t.predicate.as_ref() == OWL_SAME_AS {
                continue;
            }
            by_object
                .entry(t.object.clone())
                .or_default()
                .push((t.subject.clone(), t.predicate.clone()));
        }
    }
    for (o1, o2) in pairs {
        if seen_schema.contains(&(o1.clone(), o2.clone())) {
            continue;
        }
        let o1_term: Term = match &o1 {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        if let Some(rows) = by_object.get(&o1_term) {
            for (x, p) in rows {
                pending.push(Triple::new(x.clone(), p.clone(), o2.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// For each triple `(x, p, o1)` in `graph` where `p != owl:sameAs`, push
/// `(x, p, o2)` onto `pending`.
fn emit_rewrite_object(
    graph: &FlatGraph,
    o1: &NamedOrBlankNode,
    o2: &Term,
    pending: &mut Vec<Triple>,
) -> u64 {
    let o1_term: Term = match o1 {
        NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
        NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
    };
    let triples: Vec<(NamedOrBlankNode, NamedNode)> = graph
        .triples_for_object(o1_term.as_ref())
        .filter_map(|t| {
            if t.predicate == OWL_SAME_AS {
                return None;
            }
            Some((t.subject.into_owned(), t.predicate.into_owned()))
        })
        .collect();
    let mut firings: u64 = 0;
    for (x, p) in triples {
        pending.push(Triple::new(x, p, o2.clone()));
        firings = firings.saturating_add(1);
    }
    firings
}

// ---------------------------------------------------------------------------
// Functional and inverse functional property rules. Gated behind the
// equality flag because they only produce useful closures once the
// eq-rep-* family is also running.

fn apply_prp_fp(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-fp: if p rdf:type owl:FunctionalProperty and x p y1 and x p y2
    // then y1 owl:sameAs y2.
    // y1 and y2 must both be resources since owl:sameAs only relates
    // individuals (literal subjects are not permitted in the consequent).
    //
    // Semi-naive:
    //   Branch 1: delta(schema) × graph^2 (new schema, all edges joined).
    //   Branch 2: delta(p) × graph(p) and graph(p) × delta(p) (new edges).
    let properties: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_FUNCTIONAL_PROPERTY)
        .filter_map(named_node_from_subject)
        .collect();

    let newly_functional = new_property_types(delta, OWL_FUNCTIONAL_PROPERTY);

    let mut firings: u64 = 0;
    for p in properties {
        let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
            .triples_for_predicate(p.as_ref())
            .filter_map(|t| {
                let x = t.subject.into_owned();
                let y = term_ref_to_named_or_blank(t.object)?;
                Some((x, y))
            })
            .collect();

        let Some(d) = delta else {
            firings = firings.saturating_add(join_same_subject(&edges, &edges, pending));
            continue;
        };

        if newly_functional.contains(&p) {
            firings = firings.saturating_add(join_same_subject(&edges, &edges, pending));
            continue;
        }

        let delta_edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = d
            .for_predicate(p.as_ref())
            .iter()
            .filter_map(|t| {
                let y = owned_object_named_or_blank(&t.object)?;
                Some((t.subject.clone(), y))
            })
            .collect();

        firings = firings.saturating_add(join_same_subject(&delta_edges, &edges, pending));
        firings = firings.saturating_add(join_same_subject(&edges, &delta_edges, pending));
    }
    firings
}

/// For (x1, y1) in `left` and (x2, y2) in `right` with matching subjects but
/// distinct objects, emit (y1, owl:sameAs, y2). Used by prp-fp.
fn join_same_subject(
    left: &[(NamedOrBlankNode, NamedOrBlankNode)],
    right: &[(NamedOrBlankNode, NamedOrBlankNode)],
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (x1, y1) in left {
        for (x2, y2) in right {
            if named_or_blank_eq(x1, x2) && !named_or_blank_eq(y1, y2) {
                pending.push(Triple::new(
                    y1.clone(),
                    OWL_SAME_AS.into_owned(),
                    y2.clone(),
                ));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_prp_ifp(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-ifp: if p rdf:type owl:InverseFunctionalProperty and x1 p y and
    // x2 p y then x1 owl:sameAs x2.
    //
    // Semi-naive: identical structure to prp-fp but matching on the object.
    let properties: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_INVERSE_FUNCTIONAL_PROPERTY)
        .filter_map(named_node_from_subject)
        .collect();

    let newly_ifp = new_property_types(delta, OWL_INVERSE_FUNCTIONAL_PROPERTY);

    let mut firings: u64 = 0;
    for p in properties {
        let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
            .triples_for_predicate(p.as_ref())
            .filter_map(|t| {
                let x = t.subject.into_owned();
                let y = term_ref_to_named_or_blank(t.object)?;
                Some((x, y))
            })
            .collect();

        let Some(d) = delta else {
            firings = firings.saturating_add(join_same_object(&edges, &edges, pending));
            continue;
        };

        if newly_ifp.contains(&p) {
            firings = firings.saturating_add(join_same_object(&edges, &edges, pending));
            continue;
        }

        let delta_edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = d
            .for_predicate(p.as_ref())
            .iter()
            .filter_map(|t| {
                let y = owned_object_named_or_blank(&t.object)?;
                Some((t.subject.clone(), y))
            })
            .collect();

        firings = firings.saturating_add(join_same_object(&delta_edges, &edges, pending));
        firings = firings.saturating_add(join_same_object(&edges, &delta_edges, pending));
    }
    firings
}

/// For (x1, y1) in `left` and (x2, y2) in `right` with matching objects but
/// distinct subjects, emit (x1, owl:sameAs, x2). Used by prp-ifp.
fn join_same_object(
    left: &[(NamedOrBlankNode, NamedOrBlankNode)],
    right: &[(NamedOrBlankNode, NamedOrBlankNode)],
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (x1, y1) in left {
        for (x2, y2) in right {
            if named_or_blank_eq(y1, y2) && !named_or_blank_eq(x1, x2) {
                pending.push(Triple::new(
                    x1.clone(),
                    OWL_SAME_AS.into_owned(),
                    x2.clone(),
                ));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

// ---------------------------------------------------------------------------
// Inconsistency detection. Scans the current graph for any violation of
// the five OWL 2 RL clash rules (cax-dw, cls-nothing2, prp-irp, prp-asyp,
// prp-pdw) and returns the first one found. The order is fixed: cax-dw
// first so existing behaviour is preserved, then cls-nothing2, prp-irp,
// prp-asyp, prp-pdw.

/// Snapshot of which inconsistency families the graph can actually
/// trigger. Each flag gates the corresponding detector: if the graph
/// contains no `owl:disjointWith` edges, for example, no later round can
/// produce a cax-dw clash, so the detector is skipped.
///
/// All currently-implemented rules produce A-Box triples only (rdf:type
/// and individual property edges). None of them emit `owl:disjointWith`,
/// `owl:IrreflexiveProperty`, `owl:AsymmetricProperty`, or
/// `owl:propertyDisjointWith`, so the T-Box side of these detectors is
/// frozen and can be sampled once. `x rdf:type owl:Nothing` can in
/// theory be produced by cax-sco if the user declares a subclass chain
/// into `owl:Nothing`, but only if the source class had instances in
/// the first place, so we still sample up front and live with the
/// conservative gate. Future rules that change this assumption must
/// update this struct and the gating logic.
#[derive(Default, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag gates a distinct OWL 2 RL inconsistency detector that toggles independently. A combined enum/state machine would lose the per-rule semantics that the gating logic reads off the struct."
)]
struct InconsistencyTriggers {
    has_disjoint_with: bool,
    has_nothing_declaration: bool,
    has_irreflexive_property: bool,
    has_asymmetric_property: bool,
    has_property_disjoint_with: bool,
}

impl InconsistencyTriggers {
    fn scan(graph: &FlatGraph) -> Self {
        Self {
            has_disjoint_with: graph
                .triples_for_predicate(OWL_DISJOINT_WITH)
                .next()
                .is_some(),
            has_nothing_declaration: graph
                .subjects_for_predicate_object(rdf::TYPE, OWL_NOTHING)
                .next()
                .is_some(),
            has_irreflexive_property: graph
                .subjects_for_predicate_object(rdf::TYPE, OWL_IRREFLEXIVE_PROPERTY)
                .next()
                .is_some(),
            has_asymmetric_property: graph
                .subjects_for_predicate_object(rdf::TYPE, OWL_ASYMMETRIC_PROPERTY)
                .next()
                .is_some(),
            has_property_disjoint_with: graph
                .triples_for_predicate(OWL_PROPERTY_DISJOINT_WITH)
                .next()
                .is_some(),
        }
    }

    fn any(self) -> bool {
        self.has_disjoint_with
            || self.has_nothing_declaration
            || self.has_irreflexive_property
            || self.has_asymmetric_property
            || self.has_property_disjoint_with
    }
}

fn find_inconsistency(graph: &FlatGraph, triggers: InconsistencyTriggers) -> Option<Inconsistency> {
    if triggers.has_disjoint_with {
        if let Some(c) = find_cax_dw_clash(graph) {
            return Some(Inconsistency::DisjointClasses(c));
        }
    }
    if triggers.has_nothing_declaration {
        if let Some(c) = find_cls_nothing2(graph) {
            return Some(c);
        }
    }
    if triggers.has_irreflexive_property {
        if let Some(c) = find_prp_irp(graph) {
            return Some(c);
        }
    }
    if triggers.has_asymmetric_property {
        if let Some(c) = find_prp_asyp(graph) {
            return Some(c);
        }
    }
    if triggers.has_property_disjoint_with {
        if let Some(c) = find_prp_pdw(graph) {
            return Some(c);
        }
    }
    None
}

fn find_cax_dw_clash(graph: &FlatGraph) -> Option<DisjointClash> {
    // Build the symmetric disjointness relation once per round.
    let mut pairs: Vec<(NamedNode, NamedNode)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_DISJOINT_WITH) {
        let Some(a) = named_node_from_subject(t.subject) else {
            continue;
        };
        let Some(b) = named_node_from_term(t.object) else {
            continue;
        };
        // Keep both directions so the later lookup can ignore order.
        pairs.push((a.clone(), b.clone()));
        pairs.push((b, a));
    }

    if pairs.is_empty() {
        return None;
    }

    for (a, b) in &pairs {
        let individuals_a: Vec<NamedOrBlankNode> = graph
            .subjects_for_predicate_object(rdf::TYPE, a.as_ref())
            .map(NamedOrBlankNodeRef::into_owned)
            .collect();
        for x in individuals_a {
            if graph.contains(&Triple::new(x.clone(), rdf::TYPE, b.clone())) {
                return Some(DisjointClash {
                    individual: x,
                    class_a: a.clone(),
                    class_b: b.clone(),
                });
            }
        }
    }
    None
}

/// cls-nothing2: if `x rdf:type owl:Nothing` then the graph is inconsistent.
fn find_cls_nothing2(graph: &FlatGraph) -> Option<Inconsistency> {
    let mut first = graph.subjects_for_predicate_object(rdf::TYPE, OWL_NOTHING);
    first
        .next()
        .map(NamedOrBlankNodeRef::into_owned)
        .map(|x| Inconsistency::NothingType {
            individual: x.to_string(),
        })
}

/// prp-irp: if `p rdf:type owl:IrreflexiveProperty` and `x p x` then the
/// graph is inconsistent.
fn find_prp_irp(graph: &FlatGraph) -> Option<Inconsistency> {
    let irreflexive: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_IRREFLEXIVE_PROPERTY)
        .filter_map(named_node_from_subject)
        .collect();
    for p in irreflexive {
        for t in graph.triples_for_predicate(p.as_ref()) {
            let Some(object) = term_ref_to_named_or_blank(t.object) else {
                continue;
            };
            let subject = t.subject.into_owned();
            if named_or_blank_eq(&subject, &object) {
                return Some(Inconsistency::IrreflexiveViolation {
                    property: p.to_string(),
                    individual: subject.to_string(),
                });
            }
        }
    }
    None
}

/// prp-asyp: if `p rdf:type owl:AsymmetricProperty` and both `x p y` and
/// `y p x` hold then the graph is inconsistent.
fn find_prp_asyp(graph: &FlatGraph) -> Option<Inconsistency> {
    let asymmetric: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_ASYMMETRIC_PROPERTY)
        .filter_map(named_node_from_subject)
        .collect();
    for p in asymmetric {
        let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
            .triples_for_predicate(p.as_ref())
            .filter_map(|t| {
                let x = t.subject.into_owned();
                let y = term_ref_to_named_or_blank(t.object)?;
                Some((x, y))
            })
            .collect();
        for (x, y) in &edges {
            if named_or_blank_eq(x, y) {
                // Reflexive edges on an asymmetric property are a separate
                // violation shape, but spec-wise they already clash so we
                // still report via prp-asyp rather than silently accept.
                return Some(Inconsistency::AsymmetricViolation {
                    property: p.to_string(),
                    subject: x.to_string(),
                    object: y.to_string(),
                });
            }
            let reverse = Triple::new(
                y.clone(),
                p.clone(),
                match x {
                    NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
                    NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
                },
            );
            if graph.contains(&reverse) {
                return Some(Inconsistency::AsymmetricViolation {
                    property: p.to_string(),
                    subject: x.to_string(),
                    object: y.to_string(),
                });
            }
        }
    }
    None
}

/// prp-pdw: if `p1 owl:propertyDisjointWith p2`, `x p1 y`, and `x p2 y`
/// all hold then the graph is inconsistent.
fn find_prp_pdw(graph: &FlatGraph) -> Option<Inconsistency> {
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(OWL_PROPERTY_DISJOINT_WITH)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();
    for (p1, p2) in &pairs {
        let edges: Vec<(NamedOrBlankNode, Term)> = graph
            .triples_for_predicate(p1.as_ref())
            .map(|t| (t.subject.into_owned(), t.object.into_owned()))
            .collect();
        for (x, y) in edges {
            if graph.contains(&Triple::new(x.clone(), p2.clone(), y.clone())) {
                return Some(Inconsistency::PropertyDisjointnessViolation {
                    property_a: p1.to_string(),
                    property_b: p2.to_string(),
                    subject: x.to_string(),
                    object: y.to_string(),
                });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// M4 schema and class expression rules.

fn apply_scm_spo(graph: &FlatGraph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-spo: transitivity of rdfs:subPropertyOf.
    //
    // Semi-naive:
    //   Branch 1: delta(spo) × graph(spo).
    //   Branch 2: graph(spo) × delta(spo).
    let edges: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_PROPERTY_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let mut firings: u64 = 0;

    let right_by_pivot = build_pivot_index(&edges);

    let Some(d) = delta else {
        firings = firings.saturating_add(join_chain_hashed(
            &edges,
            &right_by_pivot,
            rdfs::SUB_PROPERTY_OF,
            pending,
        ));
        return firings;
    };

    let delta_edges: Vec<(NamedNode, NamedNode)> = d
        .for_predicate(rdfs::SUB_PROPERTY_OF)
        .iter()
        .filter_map(|t| {
            let p1 = owned_subject_named(&t.subject)?;
            let p2 = owned_object_named(&t.object)?;
            Some((p1, p2))
        })
        .collect();

    let delta_by_pivot = build_pivot_index(&delta_edges);

    firings = firings.saturating_add(join_chain_hashed(
        &delta_edges,
        &right_by_pivot,
        rdfs::SUB_PROPERTY_OF,
        pending,
    ));
    firings = firings.saturating_add(join_chain_hashed(
        &edges,
        &delta_by_pivot,
        rdfs::SUB_PROPERTY_OF,
        pending,
    ));
    firings
}

/// Collect every `owl:Restriction`-like pair (c, p, v) where `c` is a
/// subject carrying both `owl:onProperty p` and `owl:hasValue v`. The
/// class `c` may be a blank node (typical for anonymous restrictions).
/// Literal values are supported on the value side.
fn collect_hasvalue_restrictions(graph: &FlatGraph) -> Vec<(NamedOrBlankNode, NamedNode, Term)> {
    let mut out: Vec<(NamedOrBlankNode, NamedNode, Term)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_HAS_VALUE) {
        let c = t.subject.into_owned();
        let v = t.object.into_owned();
        // Find the matching onProperty edge for `c`.
        let on_property = graph
            .objects_for_subject_predicate(c.as_ref(), OWL_ON_PROPERTY)
            .find_map(|o| match o {
                TermRef::NamedNode(n) => Some(n.into_owned()),
                _ => None,
            });
        if let Some(p) = on_property {
            out.push((c, p, v));
        }
    }
    out
}

fn apply_cls_hv1(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-hv1: c owl:hasValue v, c owl:onProperty p, x rdf:type c => x p v.
    //
    // The restriction list comes from the T-Box cache so we do not walk the
    // schema every round. We still filter by the value type: if v is a
    // literal, the consequent is a literal-tailed triple, which is fine;
    // if v is a resource, the consequent still has x as the subject.
    let mut firings: u64 = 0;
    for (c, p, v) in &tbox.hasvalue_restrictions {
        let individuals: Vec<NamedOrBlankNode> = graph
            .subjects_for_predicate_object(rdf::TYPE, c.as_ref())
            .map(NamedOrBlankNodeRef::into_owned)
            .collect();
        for x in individuals {
            pending.push(Triple::new(x, p.clone(), v.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_cls_hv2(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-hv2: c owl:hasValue v, c owl:onProperty p, x p v => x rdf:type c.
    // Here c can be a blank node (anonymous restriction) so the inferred
    // type keeps its Term shape via NamedOrBlankNode.
    let mut firings: u64 = 0;
    for (c, p, v) in &tbox.hasvalue_restrictions {
        let c_term: Term = match c {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        // Find all x such that (x, p, v).
        let subjects: Vec<NamedOrBlankNode> = graph
            .triples_for_predicate(p.as_ref())
            .filter(|t| t.object == v.as_ref())
            .map(|t| t.subject.into_owned())
            .collect();
        for x in subjects {
            pending.push(Triple::new(x, rdf::TYPE, c_term.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

/// Walk an RDF list headed at `head` and return its resource members. A
/// literal member or a missing rdf:first yields `None`. The list must
/// terminate in `rdf:nil`; cyclic lists are truncated once a node repeats.
fn parse_rdf_list(
    graph: &FlatGraph,
    head: NamedOrBlankNodeRef<'_>,
) -> Option<Vec<NamedOrBlankNode>> {
    let nil = rdf::NIL;
    let mut out: Vec<NamedOrBlankNode> = Vec::new();
    let mut current: NamedOrBlankNode = head.into_owned();
    let mut seen: FxHashSet<NamedOrBlankNode> = FxHashSet::default();
    loop {
        if current.as_ref() == NamedOrBlankNodeRef::NamedNode(nil) {
            return Some(out);
        }
        if !seen.insert(current.clone()) {
            // Cycle detected; return what we have so no infinite loops.
            return Some(out);
        }
        // first
        let first = graph
            .objects_for_subject_predicate(current.as_ref(), rdf::FIRST)
            .next()
            .and_then(|t| match t {
                TermRef::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.into_owned())),
                TermRef::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.into_owned())),
                TermRef::Literal(_) => None,
                #[cfg(feature = "rdf-12")]
                TermRef::Triple(_) => None,
            })?;
        out.push(first);
        // rest
        let rest = graph
            .objects_for_subject_predicate(current.as_ref(), rdf::REST)
            .next()
            .and_then(|t| match t {
                TermRef::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.into_owned())),
                TermRef::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.into_owned())),
                TermRef::Literal(_) => None,
                #[cfg(feature = "rdf-12")]
                TermRef::Triple(_) => None,
            })?;
        current = rest;
    }
}

/// Collect every (c, members) where `c owl:intersectionOf L` and `L` is a
/// well-formed RDF list of resources.
fn collect_intersection_classes(
    graph: &FlatGraph,
) -> Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> {
    let mut out: Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_INTERSECTION_OF) {
        let Some(head) = term_ref_to_named_or_blank(t.object) else {
            continue;
        };
        let Some(members) = parse_rdf_list(graph, head.as_ref()) else {
            continue;
        };
        if !members.is_empty() {
            out.push((t.subject.into_owned(), members));
        }
    }
    out
}

/// Collect every (c, members) where `c owl:unionOf L` and `L` is a
/// well-formed RDF list of resources.
fn collect_union_classes(graph: &FlatGraph) -> Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> {
    let mut out: Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_UNION_OF) {
        let Some(head) = term_ref_to_named_or_blank(t.object) else {
            continue;
        };
        let Some(members) = parse_rdf_list(graph, head.as_ref()) else {
            continue;
        };
        if !members.is_empty() {
            out.push((t.subject.into_owned(), members));
        }
    }
    out
}

fn apply_cls_int1(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-int1: c owl:intersectionOf (c1 ... cn), x rdf:type ci for all i
    // then x rdf:type c. Per W3C the classes are resources and the list is
    // a well-formed RDF list.
    let mut firings: u64 = 0;
    for (c, members) in &tbox.intersection_classes {
        // Candidate individuals: those typed as the first member.
        let Some(first) = members.first() else {
            continue;
        };
        let first_ref = first.as_ref();
        let first_term: TermRef<'_> = match first_ref {
            NamedOrBlankNodeRef::NamedNode(n) => TermRef::NamedNode(n),
            NamedOrBlankNodeRef::BlankNode(b) => TermRef::BlankNode(b),
        };
        let candidates: Vec<NamedOrBlankNode> = graph
            .triples_for_predicate(rdf::TYPE)
            .filter(|t| t.object == first_term)
            .map(|t| t.subject.into_owned())
            .collect();
        let c_term: Term = match c {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        for x in candidates {
            let mut all = true;
            for m in members.iter().skip(1) {
                let m_term: Term = match m {
                    NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
                    NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
                };
                if !graph.contains(&Triple::new(x.clone(), rdf::TYPE, m_term)) {
                    all = false;
                    break;
                }
            }
            if all {
                pending.push(Triple::new(x, rdf::TYPE, c_term.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_cls_int2(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-int2: c owl:intersectionOf (c1 ... cn), x rdf:type c
    // then x rdf:type ci for all i.
    let mut firings: u64 = 0;
    for (c, members) in &tbox.intersection_classes {
        let individuals: Vec<NamedOrBlankNode> = graph
            .subjects_for_predicate_object(rdf::TYPE, c.as_ref())
            .map(NamedOrBlankNodeRef::into_owned)
            .collect();
        for x in individuals {
            for m in members {
                let m_term: Term = match m {
                    NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
                    NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
                };
                pending.push(Triple::new(x.clone(), rdf::TYPE, m_term));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_cls_uni(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-uni: c owl:unionOf (c1 ... cn), x rdf:type ci for any one i
    // then x rdf:type c.
    let mut firings: u64 = 0;
    for (c, members) in &tbox.union_classes {
        let c_term: Term = match c {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        for m in members {
            let individuals: Vec<NamedOrBlankNode> = graph
                .subjects_for_predicate_object(rdf::TYPE, m.as_ref())
                .map(NamedOrBlankNodeRef::into_owned)
                .collect();
            for x in individuals {
                pending.push(Triple::new(x, rdf::TYPE, c_term.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// Collect every `owl:Restriction`-like pair (c, p, y) where `c` carries
/// both `owl:onProperty p` and `owl:someValuesFrom y`. `y` must be a
/// resource class (named or blank node); literal fillers are skipped.
fn collect_somevaluesfrom_restrictions(
    graph: &FlatGraph,
) -> Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)> {
    let mut out: Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_SOME_VALUES_FROM) {
        let Some(y) = term_ref_to_named_or_blank(t.object) else {
            continue;
        };
        let c = t.subject.into_owned();
        let on_property = graph
            .objects_for_subject_predicate(c.as_ref(), OWL_ON_PROPERTY)
            .find_map(|o| match o {
                TermRef::NamedNode(n) => Some(n.into_owned()),
                _ => None,
            });
        if let Some(p) = on_property {
            out.push((c, p, y));
        }
    }
    out
}

/// Collect every `owl:Restriction`-like pair (c, p, y) where `c` carries
/// both `owl:onProperty p` and `owl:allValuesFrom y`. `y` must be a
/// resource class.
fn collect_allvaluesfrom_restrictions(
    graph: &FlatGraph,
) -> Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)> {
    let mut out: Vec<(NamedOrBlankNode, NamedNode, NamedOrBlankNode)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_ALL_VALUES_FROM) {
        let Some(y) = term_ref_to_named_or_blank(t.object) else {
            continue;
        };
        let c = t.subject.into_owned();
        let on_property = graph
            .objects_for_subject_predicate(c.as_ref(), OWL_ON_PROPERTY)
            .find_map(|o| match o {
                TermRef::NamedNode(n) => Some(n.into_owned()),
                _ => None,
            });
        if let Some(p) = on_property {
            out.push((c, p, y));
        }
    }
    out
}

/// Collect every `p owl:propertyChainAxiom (p1 ... pn)` with a well-formed
/// list of property IRIs. `p` must be a named property; blank node
/// properties are skipped since they cannot sit in the predicate position
/// of an oxrdf triple. Chain members that resolve to a blank node are also
/// skipped (the entire chain is dropped in that case).
fn collect_property_chains(graph: &FlatGraph) -> Vec<(NamedNode, Vec<NamedNode>)> {
    let mut out: Vec<(NamedNode, Vec<NamedNode>)> = Vec::new();
    for t in graph.triples_for_predicate(OWL_PROPERTY_CHAIN_AXIOM) {
        let p = match t.subject {
            NamedOrBlankNodeRef::NamedNode(n) => n.into_owned(),
            NamedOrBlankNodeRef::BlankNode(_) => continue,
        };
        let Some(head) = term_ref_to_named_or_blank(t.object) else {
            continue;
        };
        let Some(members) = parse_rdf_list(graph, head.as_ref()) else {
            continue;
        };
        if members.is_empty() {
            continue;
        }
        let mut chain: Vec<NamedNode> = Vec::with_capacity(members.len());
        let mut all_named = true;
        for m in members {
            match m {
                NamedOrBlankNode::NamedNode(n) => chain.push(n),
                NamedOrBlankNode::BlankNode(_) => {
                    all_named = false;
                    break;
                }
            }
        }
        if all_named && !chain.is_empty() {
            out.push((p, chain));
        }
    }
    out
}

fn apply_cls_svf1(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-svf1: c owl:someValuesFrom y, c owl:onProperty p,
    //           u p v, v rdf:type y  =>  u rdf:type c.
    //
    // For each (c, p, y), scan every `u p v` triple and check whether `v`
    // carries `rdf:type y`. The filler `y` is a resource, so `v` must be a
    // named or blank node for the type probe to succeed. cls-svf2 handles
    // the owl:Thing filler degenerate case separately.
    let mut firings: u64 = 0;
    let thing_term = Term::NamedNode(OWL_THING.into_owned());
    for (c, p, y) in &tbox.somevaluesfrom_restrictions {
        let y_term: Term = match y {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        if y_term == thing_term {
            continue;
        }
        let c_term: Term = match c {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        for t in graph.triples_for_predicate(p.as_ref()) {
            let Some(v_subject) = term_ref_to_named_or_blank(t.object) else {
                continue;
            };
            if graph.contains(&Triple::new(v_subject, rdf::TYPE, y_term.clone())) {
                let u = t.subject.into_owned();
                pending.push(Triple::new(u, rdf::TYPE, c_term.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_cls_svf2(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-svf2: c owl:someValuesFrom owl:Thing, c owl:onProperty p,
    //           u p v  =>  u rdf:type c.
    //
    // Degenerate case of cls-svf1 where the filler is owl:Thing, so any
    // resource `v` on the object side satisfies the filler check
    // vacuously. Literals on the object side are skipped because owl:Thing
    // ranges over individuals in OWL 2 RL.
    let mut firings: u64 = 0;
    let thing_term = Term::NamedNode(OWL_THING.into_owned());
    for (c, p, y) in &tbox.somevaluesfrom_restrictions {
        let y_term: Term = match y {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        if y_term != thing_term {
            continue;
        }
        let c_term: Term = match c {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        for t in graph.triples_for_predicate(p.as_ref()) {
            if term_ref_to_named_or_blank(t.object).is_none() {
                continue;
            }
            let u = t.subject.into_owned();
            pending.push(Triple::new(u, rdf::TYPE, c_term.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_cls_avf(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-avf: c owl:allValuesFrom y, c owl:onProperty p,
    //          u rdf:type c, u p v  =>  v rdf:type y.
    //
    // For each (c, p, y), enumerate individuals `u` typed as `c`, then for
    // each `u p v` edge push `v rdf:type y`. The object `v` must be a
    // resource so the consequent is well-formed.
    let mut firings: u64 = 0;
    for (c, p, y) in &tbox.allvaluesfrom_restrictions {
        let y_term: Term = match y {
            NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
            NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
        };
        let individuals: Vec<NamedOrBlankNode> = graph
            .subjects_for_predicate_object(rdf::TYPE, c.as_ref())
            .map(NamedOrBlankNodeRef::into_owned)
            .collect();
        for u in individuals {
            for o in graph.objects_for_subject_predicate(u.as_ref(), p.as_ref()) {
                let Some(v) = term_ref_to_named_or_blank(o) else {
                    continue;
                };
                pending.push(Triple::new(v, rdf::TYPE, y_term.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_prp_spo2(graph: &FlatGraph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // prp-spo2: p owl:propertyChainAxiom (p1 ... pn),
    //           u1 p1 u2, u2 p2 u3, ..., un pn u(n+1)
    //           =>  u1 p u(n+1).
    //
    // Naive chain traversal: for each chain, start with the set of
    // `u1 p1 u2` triples, then for each step extend by looking up
    // `current p(i+1) ?`. Intermediate and final endpoints must be
    // resources because they sit in the subject position of the next
    // link or in the object position of the inferred `u1 p u(n+1)` edge
    // whose property is an object property (chain axioms in OWL 2 RL are
    // restricted to object properties).
    let mut firings: u64 = 0;
    for (p, chain) in &tbox.property_chains {
        if chain.is_empty() {
            continue;
        }
        let first = &chain[0];
        let mut frontier: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = Vec::new();
        for t in graph.triples_for_predicate(first.as_ref()) {
            let Some(current) = term_ref_to_named_or_blank(t.object) else {
                continue;
            };
            let u1 = t.subject.into_owned();
            frontier.push((u1, current));
        }
        for pi in chain.iter().skip(1) {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier: Vec<(NamedOrBlankNode, NamedOrBlankNode)> =
                Vec::with_capacity(frontier.len());
            for (u1, current) in &frontier {
                for o in graph.objects_for_subject_predicate(current.as_ref(), pi.as_ref()) {
                    let Some(next) = term_ref_to_named_or_blank(o) else {
                        continue;
                    };
                    next_frontier.push((u1.clone(), next));
                }
            }
            frontier = next_frontier;
        }
        for (u1, last) in frontier {
            let last_term: Term = match last {
                NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n),
                NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b),
            };
            pending.push(Triple::new(u1, p.clone(), last_term));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

// ---------------------------------------------------------------------------
// Small helpers.

fn named_node_from_subject(subject: NamedOrBlankNodeRef<'_>) -> Option<NamedNode> {
    match subject {
        NamedOrBlankNodeRef::NamedNode(n) => Some(n.into_owned()),
        NamedOrBlankNodeRef::BlankNode(_) => None,
    }
}

fn named_node_from_term(term: TermRef<'_>) -> Option<NamedNode> {
    match term {
        TermRef::NamedNode(n) => Some(n.into_owned()),
        _ => None,
    }
}

fn term_ref_to_named_or_blank(term: TermRef<'_>) -> Option<NamedOrBlankNode> {
    match term {
        TermRef::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.into_owned())),
        TermRef::BlankNode(n) => Some(NamedOrBlankNode::BlankNode(n.into_owned())),
        TermRef::Literal(_) => None,
        #[cfg(feature = "rdf-12")]
        TermRef::Triple(_) => None,
    }
}

fn named_or_blank_eq(left: &NamedOrBlankNode, right: &NamedOrBlankNode) -> bool {
    match (left, right) {
        (NamedOrBlankNode::NamedNode(a), NamedOrBlankNode::NamedNode(b)) => a == b,
        (NamedOrBlankNode::BlankNode(a), NamedOrBlankNode::BlankNode(b)) => a == b,
        _ => false,
    }
}

// Owned-triple helpers mirror the `named_node_from_*` family but accept
// references into owned `Triple` fields, which is what delta iteration
// produces. They are intentionally tiny so the rule bodies stay readable.

fn owned_subject_named(subject: &NamedOrBlankNode) -> Option<NamedNode> {
    match subject {
        NamedOrBlankNode::NamedNode(n) => Some(n.clone()),
        NamedOrBlankNode::BlankNode(_) => None,
    }
}

fn owned_object_named(term: &Term) -> Option<NamedNode> {
    match term {
        Term::NamedNode(n) => Some(n.clone()),
        _ => None,
    }
}

fn owned_object_named_or_blank(term: &Term) -> Option<NamedOrBlankNode> {
    match term {
        Term::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.clone())),
        Term::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.clone())),
        Term::Literal(_) => None,
        #[cfg(feature = "rdf-12")]
        Term::Triple(_) => None,
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::panic, clippy::explicit_iter_loop)]
    use super::*;
    use oxrdf::{Graph, Triple};

    fn ex(local: &str) -> NamedNode {
        NamedNode::new_unchecked(format!("https://example.org/ontology#{local}"))
    }

    fn owl_cfg() -> ReasonerConfig {
        ReasonerConfig::owl2_rl()
    }

    fn rdfs_cfg() -> ReasonerConfig {
        ReasonerConfig::rdfs()
    }

    fn owl_with_equality() -> ReasonerConfig {
        ReasonerConfig::owl2_rl().with_equality_rules(true)
    }

    /// Build a `FlatGraph` from the seed triples currently in `graph`.
    /// Shared helper between `expand_ok` and the few tests that drive
    /// `expand` directly to inspect its error path.
    fn flat_from_graph(graph: &Graph) -> FlatGraph {
        let mut flat = FlatGraph::with_capacity(graph.len());
        for t in graph.iter() {
            flat.insert(&t.into_owned());
        }
        flat
    }

    /// Copy every triple from `flat` back into `graph`. Used by `expand_ok`
    /// to restore the drop-in-place semantics of the old
    /// `expand(&mut Graph)` API for test assertions that call
    /// `graph.contains(...)` afterwards.
    fn drain_flat_into(flat: FlatGraph, graph: &mut Graph) {
        for t in flat.into_triples() {
            graph.insert(&t);
        }
    }

    /// Unwrap the `expand` result for tests that expect a consistent graph.
    fn expand_ok(graph: &mut Graph, config: &ReasonerConfig) -> RunStats {
        let mut flat = flat_from_graph(graph);
        let result = expand(&mut flat, config, &mut |_: &Triple| -> Result<
            (),
            std::convert::Infallible,
        > { Ok(()) });
        let stats = match result {
            Ok(stats) => stats,
            Err(ExpandError::Inconsistency(i)) => {
                panic!("expand must succeed on a consistent graph: {}", i.message())
            }
            Err(ExpandError::Sink(never)) => match never {},
        };
        drain_flat_into(flat, graph);
        stats
    }

    #[test]
    fn cax_sco_single_step_inference() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            ex("LegalPerson"),
        ));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

        let stats = expand_ok(&mut g, &owl_cfg());

        assert!(stats.added >= 1);
        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("LegalPerson"))));
    }

    #[test]
    fn cax_sco_chains_through_multiple_subclass_steps() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            ex("LegalPerson"),
        ));
        g.insert(&Triple::new(
            ex("LegalPerson"),
            rdfs::SUB_CLASS_OF,
            ex("Entity"),
        ));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("LegalPerson"))));
        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("Entity"))));
    }

    #[test]
    fn prp_trp_closes_a_chain_of_three() {
        let has_bo = ex("hasBeneficialOwner");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            has_bo.clone(),
            rdf::TYPE,
            OWL_TRANSITIVE_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("VesselA"), has_bo.clone(), ex("ShellCo")));
        g.insert(&Triple::new(ex("ShellCo"), has_bo.clone(), ex("Parent")));
        g.insert(&Triple::new(
            ex("Parent"),
            has_bo.clone(),
            ex("UltimateOwner"),
        ));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(
            ex("VesselA"),
            has_bo.clone(),
            ex("UltimateOwner"),
        )));
    }

    #[test]
    fn rdfs_profile_skips_prp_trp() {
        let has_bo = ex("hasBeneficialOwner");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            has_bo.clone(),
            rdf::TYPE,
            OWL_TRANSITIVE_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("A"), has_bo.clone(), ex("B")));
        g.insert(&Triple::new(ex("B"), has_bo.clone(), ex("C")));

        expand_ok(&mut g, &rdfs_cfg());

        assert!(!g.contains(&Triple::new(ex("A"), has_bo, ex("C"))));
    }

    #[test]
    fn fixpoint_terminates_even_when_nothing_matches() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("Alice"), ex("knows"), ex("Bob")));

        let stats = expand_ok(&mut g, &owl_cfg());

        assert_eq!(stats.added, 0);
        assert!(stats.rounds >= 1);
    }

    // ---- M2 rule tests ----

    #[test]
    fn prp_symp_materialises_reverse_edge() {
        let married = ex("marriedTo");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            married.clone(),
            rdf::TYPE,
            OWL_SYMMETRIC_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("Alice"), married.clone(), ex("Bob")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Bob"), married, ex("Alice"))));
    }

    #[test]
    fn prp_inv_applies_in_both_directions() {
        let owns = ex("owns");
        let owned_by = ex("ownedBy");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            owns.clone(),
            OWL_INVERSE_OF.into_owned(),
            owned_by.clone(),
        ));
        g.insert(&Triple::new(ex("Alice"), owns.clone(), ex("Bike")));
        g.insert(&Triple::new(ex("Shed"), owned_by.clone(), ex("Carol")));

        expand_ok(&mut g, &owl_cfg());

        // prp-inv1: Alice owns Bike then Bike ownedBy Alice.
        assert!(g.contains(&Triple::new(ex("Bike"), owned_by, ex("Alice"))));
        // prp-inv2: Shed ownedBy Carol then Carol owns Shed.
        assert!(g.contains(&Triple::new(ex("Carol"), owns, ex("Shed"))));
    }

    #[test]
    fn prp_eqp_bridges_equivalent_properties() {
        let has_owner = ex("hasOwner");
        let owner = ex("owner");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            has_owner.clone(),
            OWL_EQUIVALENT_PROPERTY.into_owned(),
            owner.clone(),
        ));
        g.insert(&Triple::new(ex("Bike"), has_owner.clone(), ex("Alice")));
        g.insert(&Triple::new(ex("Car"), owner.clone(), ex("Bob")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Bike"), owner, ex("Alice"))));
        assert!(g.contains(&Triple::new(ex("Car"), has_owner, ex("Bob"))));
    }

    #[test]
    fn cax_eqc_bridges_equivalent_classes() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Person"),
            OWL_EQUIVALENT_CLASS.into_owned(),
            ex("Human"),
        ));
        g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Person")));
        g.insert(&Triple::new(ex("Bob"), rdf::TYPE, ex("Human")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("Human"))));
        assert!(g.contains(&Triple::new(ex("Bob"), rdf::TYPE, ex("Person"))));
    }

    // ---- Equality rule tests ----

    #[test]
    fn equality_rules_are_off_by_default() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Alice"),
            OWL_SAME_AS.into_owned(),
            ex("AliceDoe"),
        ));
        g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

        expand_ok(&mut g, &owl_cfg());

        // Without equality rules, sameAs must not propagate triples.
        assert!(!g.contains(&Triple::new(ex("AliceDoe"), ex("owns"), ex("Bike"))));
    }

    #[test]
    fn eq_sym_and_trans_close_sameas_chains() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("A"), OWL_SAME_AS.into_owned(), ex("B")));
        g.insert(&Triple::new(ex("B"), OWL_SAME_AS.into_owned(), ex("C")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(ex("B"), OWL_SAME_AS.into_owned(), ex("A"))));
        assert!(g.contains(&Triple::new(ex("A"), OWL_SAME_AS.into_owned(), ex("C"))));
        assert!(g.contains(&Triple::new(ex("C"), OWL_SAME_AS.into_owned(), ex("A"))));
    }

    #[test]
    fn eq_rep_s_rewrites_subject() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Alice"),
            OWL_SAME_AS.into_owned(),
            ex("AliceDoe"),
        ));
        g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(ex("AliceDoe"), ex("owns"), ex("Bike"))));
    }

    #[test]
    fn eq_rep_o_rewrites_object() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Bike"),
            OWL_SAME_AS.into_owned(),
            ex("Bicycle"),
        ));
        g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(ex("Alice"), ex("owns"), ex("Bicycle"))));
    }

    #[test]
    fn eq_rep_p_rewrites_predicate() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("owns"),
            OWL_SAME_AS.into_owned(),
            ex("possesses"),
        ));
        g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(ex("Alice"), ex("possesses"), ex("Bike"))));
    }

    // ---- M3 schema rule tests ----

    #[test]
    fn scm_cls_emits_reflexive_subclass_and_bounds() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Company"),
            rdf::TYPE,
            OWL_CLASS.into_owned(),
        ));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            ex("Company")
        )));
        assert!(g.contains(&Triple::new(
            ex("Company"),
            OWL_EQUIVALENT_CLASS.into_owned(),
            ex("Company"),
        )));
        assert!(g.contains(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            OWL_THING.into_owned(),
        )));
        assert!(g.contains(&Triple::new(
            OWL_NOTHING.into_owned(),
            rdfs::SUB_CLASS_OF,
            ex("Company"),
        )));
    }

    #[test]
    fn scm_sco_is_transitive() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("A"), rdfs::SUB_CLASS_OF, ex("B")));
        g.insert(&Triple::new(ex("B"), rdfs::SUB_CLASS_OF, ex("C")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("A"), rdfs::SUB_CLASS_OF, ex("C"))));
    }

    #[test]
    fn scm_op_and_scm_dp_mark_property_reflexive() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("owns"),
            rdf::TYPE,
            OWL_OBJECT_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(
            ex("hasAge"),
            rdf::TYPE,
            OWL_DATATYPE_PROPERTY.into_owned(),
        ));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("owns"), rdfs::SUB_PROPERTY_OF, ex("owns"))));
        assert!(g.contains(&Triple::new(
            ex("hasAge"),
            OWL_EQUIVALENT_PROPERTY.into_owned(),
            ex("hasAge"),
        )));
    }

    #[test]
    fn scm_eqc_is_bidirectional() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Person"),
            OWL_EQUIVALENT_CLASS.into_owned(),
            ex("Human"),
        ));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Person"), rdfs::SUB_CLASS_OF, ex("Human"))));
        assert!(g.contains(&Triple::new(ex("Human"), rdfs::SUB_CLASS_OF, ex("Person"))));

        // And the reverse direction via scm-eqc2 from the two subclass edges
        // combined (Person equivalentClass Human is already there, but
        // scm-eqc2 still needs to be triggerable from scratch).
        let mut g2 = Graph::default();
        g2.insert(&Triple::new(ex("Person"), rdfs::SUB_CLASS_OF, ex("Human")));
        g2.insert(&Triple::new(ex("Human"), rdfs::SUB_CLASS_OF, ex("Person")));
        expand_ok(&mut g2, &owl_cfg());
        assert!(g2.contains(&Triple::new(
            ex("Person"),
            OWL_EQUIVALENT_CLASS.into_owned(),
            ex("Human"),
        )));
    }

    #[test]
    fn scm_dom1_walks_domain_up_subclass_chain() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("owns"), rdfs::DOMAIN, ex("Company")));
        g.insert(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            ex("LegalPerson"),
        ));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("owns"), rdfs::DOMAIN, ex("LegalPerson"))));
    }

    #[test]
    fn scm_rng1_walks_range_up_subclass_chain() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("owns"), rdfs::RANGE, ex("Bike")));
        g.insert(&Triple::new(ex("Bike"), rdfs::SUB_CLASS_OF, ex("Asset")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("owns"), rdfs::RANGE, ex("Asset"))));
    }

    #[test]
    fn rdfs_profile_does_not_emit_scm_rules() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Company"),
            rdf::TYPE,
            OWL_CLASS.into_owned(),
        ));

        expand_ok(&mut g, &rdfs_cfg());

        // scm-cls is an OWL rule, so it must stay dormant under the Rdfs
        // profile.
        assert!(!g.contains(&Triple::new(
            ex("Company"),
            rdfs::SUB_CLASS_OF,
            ex("Company")
        )));
    }

    // ---- Functional and inverse functional property rule tests ----

    #[test]
    fn prp_fp_materialises_sameas_between_duplicate_values() {
        let has_father = ex("hasFather");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            has_father.clone(),
            rdf::TYPE,
            OWL_FUNCTIONAL_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("Alice"), has_father.clone(), ex("Bob")));
        g.insert(&Triple::new(ex("Alice"), has_father, ex("Robert")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(
            ex("Bob"),
            OWL_SAME_AS.into_owned(),
            ex("Robert"),
        )));
    }

    #[test]
    fn prp_fp_stays_dormant_without_equality_flag() {
        let has_father = ex("hasFather");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            has_father.clone(),
            rdf::TYPE,
            OWL_FUNCTIONAL_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("Alice"), has_father.clone(), ex("Bob")));
        g.insert(&Triple::new(ex("Alice"), has_father, ex("Robert")));

        expand_ok(&mut g, &owl_cfg());

        assert!(!g.contains(&Triple::new(
            ex("Bob"),
            OWL_SAME_AS.into_owned(),
            ex("Robert"),
        )));
    }

    #[test]
    fn prp_ifp_materialises_sameas_between_duplicate_subjects() {
        let email = ex("hasEmail");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            email.clone(),
            rdf::TYPE,
            OWL_INVERSE_FUNCTIONAL_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("Alice"), email.clone(), ex("mail1")));
        g.insert(&Triple::new(ex("AliceDoe"), email, ex("mail1")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(
            ex("Alice"),
            OWL_SAME_AS.into_owned(),
            ex("AliceDoe"),
        )));
    }

    // ---- cax-dw inconsistency tests ----

    #[test]
    fn cax_dw_detects_clash_on_disjoint_classes() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Person"),
            OWL_DISJOINT_WITH.into_owned(),
            ex("Organisation"),
        ));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Person")));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Organisation")));

        let mut flat = flat_from_graph(&g);
        let err = expand(&mut flat, &owl_cfg(), &mut |_: &Triple| -> Result<
            (),
            std::convert::Infallible,
        > { Ok(()) })
        .unwrap_err();
        let ExpandError::Inconsistency(Inconsistency::DisjointClasses(clash)) = err else {
            panic!("expected DisjointClasses variant");
        };

        assert_eq!(clash.individual, NamedOrBlankNode::NamedNode(ex("Acme")));
        let classes = [clash.class_a.clone(), clash.class_b.clone()];
        assert!(classes.contains(&ex("Person")));
        assert!(classes.contains(&ex("Organisation")));
    }

    #[test]
    fn cax_dw_ignored_under_rdfs_profile() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Person"),
            OWL_DISJOINT_WITH.into_owned(),
            ex("Organisation"),
        ));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Person")));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Organisation")));

        // Rdfs profile must not check cax-dw; expansion succeeds.
        expand_ok(&mut g, &rdfs_cfg());
    }

    #[test]
    fn cax_dw_consistent_graph_saturates_normally() {
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Person"),
            OWL_DISJOINT_WITH.into_owned(),
            ex("Organisation"),
        ));
        g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Person")));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Organisation")));

        expand_ok(&mut g, &owl_cfg());
    }

    // ---- Semi-naive propagation tests ----
    //
    // These tests pick shapes that can only close over multiple rounds. The
    // round 1 scan is naive, round 2+ is semi-naive, so a multi-round saturation
    // proves the delta-join branches are firing and producing the same closure
    // as a pure naive evaluator would.

    #[test]
    fn semi_naive_propagates_cax_sco_across_three_rounds() {
        // C1 sco C2 sco C3 sco C4, and Acme is typed C1. A single round only
        // infers one step, so reaching `Acme rdf:type C4` demands three rounds
        // of delta propagation.
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("C1"), rdfs::SUB_CLASS_OF, ex("C2")));
        g.insert(&Triple::new(ex("C2"), rdfs::SUB_CLASS_OF, ex("C3")));
        g.insert(&Triple::new(ex("C3"), rdfs::SUB_CLASS_OF, ex("C4")));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("C1")));

        let stats = expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("C2"))));
        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("C3"))));
        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("C4"))));
        // Semi-naive needs at least two rounds past the initial naive pass,
        // plus the no-op terminator, so four total is the floor here.
        assert!(stats.rounds >= 3);
    }

    #[test]
    fn semi_naive_closes_transitive_property_in_one_fixpoint() {
        // Linear chain over a transitive property: A→B→C→D→E. The naive
        // round 1 emits all length-2 reaches (A→C, B→D, C→E). Subsequent
        // semi-naive rounds must reach A→D, B→E, A→E to saturate.
        let reaches = ex("reaches");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            reaches.clone(),
            rdf::TYPE,
            OWL_TRANSITIVE_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(ex("A"), reaches.clone(), ex("B")));
        g.insert(&Triple::new(ex("B"), reaches.clone(), ex("C")));
        g.insert(&Triple::new(ex("C"), reaches.clone(), ex("D")));
        g.insert(&Triple::new(ex("D"), reaches.clone(), ex("E")));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("A"), reaches.clone(), ex("C"))));
        assert!(g.contains(&Triple::new(ex("A"), reaches.clone(), ex("D"))));
        assert!(g.contains(&Triple::new(ex("A"), reaches.clone(), ex("E"))));
        assert!(g.contains(&Triple::new(ex("B"), reaches.clone(), ex("E"))));
    }

    #[test]
    fn semi_naive_handles_late_arriving_schema() {
        // The TransitiveProperty declaration comes through inference in a
        // later round: owl:sameAs renames `sibling` to `reaches`, but here we
        // simulate the effect by adding a subPropertyOf chain that surfaces
        // the schema axiom after round 1. The semi-naive path must still
        // register edges over `reaches` that were derived by prp-spo1.
        let reaches = ex("reaches");
        let sibling = ex("sibling");
        let mut g = Graph::default();
        g.insert(&Triple::new(
            reaches.clone(),
            rdf::TYPE,
            OWL_TRANSITIVE_PROPERTY.into_owned(),
        ));
        g.insert(&Triple::new(
            sibling.clone(),
            rdfs::SUB_PROPERTY_OF,
            reaches.clone(),
        ));
        g.insert(&Triple::new(ex("A"), sibling.clone(), ex("B")));
        g.insert(&Triple::new(ex("B"), sibling, ex("C")));

        expand_ok(&mut g, &owl_cfg());

        // prp-spo1 copies both `sibling` edges to `reaches`, and prp-trp
        // then closes the chain A-reaches-C.
        assert!(g.contains(&Triple::new(ex("A"), reaches.clone(), ex("B"))));
        assert!(g.contains(&Triple::new(ex("B"), reaches.clone(), ex("C"))));
        assert!(g.contains(&Triple::new(ex("A"), reaches, ex("C"))));
    }

    #[test]
    fn semi_naive_eq_rep_o_rewrites_chained_sameas() {
        // Bike sameAs Bicycle, Bicycle sameAs Cycle. eq-trans must derive
        // Bike sameAs Cycle in round 2, and eq-rep-o must then rewrite the
        // (Alice owns Bike) triple into (Alice owns Cycle) in round 3.
        let mut g = Graph::default();
        g.insert(&Triple::new(
            ex("Bike"),
            OWL_SAME_AS.into_owned(),
            ex("Bicycle"),
        ));
        g.insert(&Triple::new(
            ex("Bicycle"),
            OWL_SAME_AS.into_owned(),
            ex("Cycle"),
        ));
        g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

        expand_ok(&mut g, &owl_with_equality());

        assert!(g.contains(&Triple::new(ex("Alice"), ex("owns"), ex("Bicycle"))));
        assert!(g.contains(&Triple::new(ex("Alice"), ex("owns"), ex("Cycle"))));
    }
}
