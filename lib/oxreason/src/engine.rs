//! Forward chaining engine (M1 plus M2 plus M3 plus M4).
//!
//! M1 implements the five schema rules `cax-sco`, `prp-dom`, `prp-rng`,
//! `prp-spo1`, `prp-trp` from `DESIGN.md` section 9.
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
//! Restrictions relative to full OWL 2 RL:
//!
//! 1. Classes and properties referenced by the schema rules must be IRIs.
//!    Blank node class expressions (anonymous restrictions) are ignored.
//! 2. Datatype rules (`dt-type1` and friends) still wait on value space
//!    comparisons for datatype literals.
//! 3. Inferred quads are always written back into the same in memory graph.
//!    The `materialise_into_named_graph` config knob is honoured by
//!    `Reasoner::expand_into`, which is still stubbed.

use std::time::{Duration, Instant};

use oxrdf::vocab::{rdf, rdfs};
use oxrdf::{
    Graph, NamedNode, NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, Term, TermRef, Triple,
};
use rustc_hash::FxHashMap;

use crate::reasoner::{ReasonerConfig, ReasoningProfile};

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
        let enabled = std::env::var("OXREASON_PROFILE")
            .map(|v| v != "0" && !v.is_empty())
            .unwrap_or(false);
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

    fn report(&self, total: Duration) {
        if !self.enabled {
            return;
        }
        let mut sorted: Vec<&(&'static str, Duration, u64, u64)> = self.entries.iter().collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(e.1));
        eprintln!(
            "OXREASON_PROFILE total_ms={:.3}",
            total.as_secs_f64() * 1000.0
        );
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
            eprintln!(
                "{:<18}  {:>10.3}  {:>7.1}%  {:>10}  {:>10}",
                name, ms, pct, firings, delta_in
            );
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
    fn build(triples: &[Triple]) -> Self {
        let mut by_predicate: FxHashMap<String, Vec<Triple>> = FxHashMap::default();
        for t in triples {
            by_predicate
                .entry(t.predicate.as_str().to_owned())
                .or_default()
                .push(t.clone());
        }
        Self { by_predicate }
    }

    fn for_predicate(&self, p: NamedNodeRef<'_>) -> &[Triple] {
        self.by_predicate
            .get(p.as_str())
            .map_or(&[][..], Vec::as_slice)
    }

    /// Returns true when this delta contains any triple whose predicate is
    /// part of the T-Box trigger set. That set is the six predicates that
    /// `TBoxCache` reads while it materialises restriction, intersection,
    /// and union structures: `owl:hasValue`, `owl:onProperty`,
    /// `owl:intersectionOf`, `owl:unionOf`, `rdf:first`, and `rdf:rest`.
    /// All other predicate deltas leave the cache valid.
    fn touches_tbox(&self) -> bool {
        !self.for_predicate(OWL_HAS_VALUE).is_empty()
            || !self.for_predicate(OWL_ON_PROPERTY).is_empty()
            || !self.for_predicate(OWL_INTERSECTION_OF).is_empty()
            || !self.for_predicate(OWL_UNION_OF).is_empty()
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
}

impl TBoxCache {
    fn build(graph: &Graph) -> Self {
        Self {
            hasvalue_restrictions: collect_hasvalue_restrictions(graph),
            intersection_classes: collect_intersection_classes(graph),
            union_classes: collect_union_classes(graph),
        }
    }
}

/// Run the forward chainer to fixpoint on the given graph.
///
/// Returns the final [`RunStats`] on a clean saturation, or an
/// [`Inconsistency`] describing the first violation found. A clash is
/// surfaced even when encountered on round zero, so the reasoner fails
/// fast on pre-existing inconsistencies in the input graph.
pub(crate) fn expand(graph: &mut Graph, config: &ReasonerConfig) -> Result<RunStats, Inconsistency> {
    let profile = config.profile();
    let equality_on = config.equality_rules_enabled();

    let mut prof = Profiler::new();
    let run_start = Instant::now();

    let mut stats = RunStats::default();
    let mut delta: Option<DeltaIndex> = None;

    // Shadow set of every triple in the graph. Seeded from the input graph
    // once, then kept in lockstep with successful `graph.insert` calls. The
    // profiler showed that `graph.insert` (which touches all six BTreeSet
    // indexes of the underlying dataset) and per-round `FxHashSet<Triple>`
    // deduplication together consumed ~65-78% of reasoning time. Most
    // candidate triples a rule produces are already in the graph from a
    // prior round, so the 6-index insert path runs and fails N-1 out of N
    // times. An owned `FxHashSet<Triple>` probe is a single hash lookup
    // with cheap `Arc` comparisons, so it short-circuits all of that work
    // for duplicates. The memory overhead is O(|graph|) extra triples of
    // pointer-sized members (each owned triple is three `Arc`-backed
    // terms); on LUBM 10000 that is under 1 MB.
    let mut seen_total = prof.time_block("seen.seed", || {
        let mut set: rustc_hash::FxHashSet<Triple> =
            rustc_hash::FxHashSet::with_capacity_and_hasher(graph.len() * 2, Default::default());
        for t in graph.iter() {
            set.insert(t.into_owned());
        }
        set
    });

    // Build the class-expression T-Box cache once up front. It is reused
    // across rounds and only rebuilt when the previous round's delta
    // contains a predicate that could change its contents.
    let mut tbox = prof.time_block("tbox.build", || TBoxCache::build(graph));

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
            let maybe_clash = prof.time_block("inconsistency", || find_inconsistency(graph, triggers));
            if let Some(clash) = maybe_clash {
                return Err(clash);
            }
        }

        let mut pending: Vec<Triple> = Vec::new();
        let mut round_firings: u64 = 0;

        let delta_ref = delta.as_ref();
        let delta_size: u64 = delta_ref.map_or(0, |d| d.by_predicate.values().map(|v| v.len() as u64).sum());

        // RDFS compatible rules. Run in both profiles.
        round_firings = round_firings.saturating_add(prof.time("cax-sco", delta_size, || apply_cax_sco(graph, delta_ref, &mut pending)));
        round_firings = round_firings.saturating_add(prof.time("prp-dom", delta_size, || apply_prp_dom(graph, delta_ref, &mut pending)));
        round_firings = round_firings.saturating_add(prof.time("prp-rng", delta_size, || apply_prp_rng(graph, delta_ref, &mut pending)));
        round_firings = round_firings.saturating_add(prof.time("prp-spo1", delta_size, || apply_prp_spo1(graph, delta_ref, &mut pending)));

        // OWL rules. Skipped when running the Rdfs profile.
        if profile != ReasoningProfile::Rdfs {
            round_firings = round_firings.saturating_add(prof.time("prp-trp", delta_size, || apply_prp_trp(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("prp-symp", delta_size, || apply_prp_symp(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("prp-inv", delta_size, || apply_prp_inv(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("prp-eqp", delta_size, || apply_prp_eqp(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cax-eqc", delta_size, || apply_cax_eqc(graph, delta_ref, &mut pending)));

            // M3 schema rules.
            round_firings = round_firings.saturating_add(prof.time("scm-cls", delta_size, || apply_scm_cls(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-sco", delta_size, || apply_scm_sco(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-op", delta_size, || apply_scm_op(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-dp", delta_size, || apply_scm_dp(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-eqc1", delta_size, || apply_scm_eqc1(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-eqc2", delta_size, || apply_scm_eqc2(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-eqp1", delta_size, || apply_scm_eqp1(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-eqp2", delta_size, || apply_scm_eqp2(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-dom1", delta_size, || apply_scm_dom1(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("scm-rng1", delta_size, || apply_scm_rng1(graph, delta_ref, &mut pending)));

            // M4 rules.
            round_firings = round_firings.saturating_add(prof.time("scm-spo", delta_size, || apply_scm_spo(graph, delta_ref, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cls-hv1", delta_size, || apply_cls_hv1(graph, &tbox, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cls-hv2", delta_size, || apply_cls_hv2(graph, &tbox, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cls-int1", delta_size, || apply_cls_int1(graph, &tbox, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cls-int2", delta_size, || apply_cls_int2(graph, &tbox, &mut pending)));
            round_firings = round_firings.saturating_add(prof.time("cls-uni", delta_size, || apply_cls_uni(graph, &tbox, &mut pending)));

            if equality_on {
                round_firings = round_firings.saturating_add(prof.time("prp-fp", delta_size, || apply_prp_fp(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("prp-ifp", delta_size, || apply_prp_ifp(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("eq-sym", delta_size, || apply_eq_sym(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("eq-trans", delta_size, || apply_eq_trans(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("eq-rep-s", delta_size, || apply_eq_rep_s(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("eq-rep-p", delta_size, || apply_eq_rep_p(graph, delta_ref, &mut pending)));
                round_firings = round_firings.saturating_add(prof.time("eq-rep-o", delta_size, || apply_eq_rep_o(graph, delta_ref, &mut pending)));
            }
        }

        stats.firings = stats.firings.saturating_add(round_firings);

        // Many rules derive the same consequent (for example every
        // cax-sco branch that bridges through a fan-in node pushes the
        // same `x rdf:type d`). Deduplicating the pending batch before
        // probing `graph.insert` avoids rehashing and re-comparing those
        // duplicates against the live graph. `FxHashSet` is used to match
        // the rest of the engine's hashing strategy.
        let new_triples = {
            let mut new_triples: Vec<Triple> = Vec::new();
            let mut seen_time = Duration::ZERO;
            let mut insert_time = Duration::ZERO;
            let profile_split = prof.enabled;
            for triple in pending {
                // `Triple::clone` allocates three owned `String`s (subject,
                // predicate, object IRIs), so it is much more expensive
                // than a typical pointer clone. Most pending triples are
                // duplicates we have already derived, so we test against
                // the shadow set with a borrowed probe first and only
                // clone on a genuinely novel triple.
                let t0 = if profile_split { Some(Instant::now()) } else { None };
                let already = seen_total.contains(&triple);
                if let Some(t) = t0 {
                    seen_time += t.elapsed();
                }
                if already {
                    continue;
                }
                // Novel triple: insert into both shadow set and graph, and
                // push onto `new_triples` for the next delta. One clone for
                // the shadow set; the original `triple` moves into the
                // `new_triples` vec.
                let t1 = if profile_split { Some(Instant::now()) } else { None };
                seen_total.insert(triple.clone());
                graph.insert(&triple);
                if let Some(t) = t1 {
                    insert_time += t.elapsed();
                }
                new_triples.push(triple);
            }
            if profile_split {
                if let Some(e) = prof.entries.iter_mut().find(|e| e.0 == "seen.probe") {
                    e.1 += seen_time;
                } else {
                    prof.entries.push(("seen.probe", seen_time, 0, 0));
                }
                if let Some(e) = prof.entries.iter_mut().find(|e| e.0 == "graph.insert") {
                    e.1 += insert_time;
                } else {
                    prof.entries.push(("graph.insert", insert_time, 0, 0));
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
        // above, so `new_triples` already excludes duplicates.
        let next_delta = prof.time_block("delta.build", || DeltaIndex::build(&new_triples));

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

fn apply_cax_sco(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // cax-sco: if x rdf:type c and c rdfs:subClassOf d then x rdf:type d.
    //
    // Semi-naive splits the work across two branches:
    //   1. delta(subClassOf) joined with graph(type):
    //      a newly added c-subClassOf-d edge can reclassify every existing
    //      x that is a c.
    //   2. graph(subClassOf) joined with delta(type):
    //      an existing c-subClassOf-d edge reclassifies an x only when
    //      x rdf:type c was itself added in the previous round.
    let mut firings: u64 = 0;

    let subclass_pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_CLASS_OF)
        .filter_map(|t| {
            let c = named_node_from_subject(t.subject)?;
            let d = named_node_from_term(t.object)?;
            Some((c, d))
        })
        .collect();

    let Some(d) = delta else {
        // Round 1: naive.
        for (c, dest) in &subclass_pairs {
            let subjects: Vec<NamedOrBlankNode> = graph
                .subjects_for_predicate_object(rdf::TYPE, c.as_ref())
                .map(NamedOrBlankNodeRef::into_owned)
                .collect();
            for x in subjects {
                pending.push(Triple::new(x, rdf::TYPE, dest.clone()));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(subClassOf) × graph(type).
    for t in d.for_predicate(rdfs::SUB_CLASS_OF) {
        let Some(c) = owned_subject_named(&t.subject) else { continue };
        let Some(dest) = owned_object_named(&t.object) else { continue };
        let subjects: Vec<NamedOrBlankNode> = graph
            .subjects_for_predicate_object(rdf::TYPE, c.as_ref())
            .map(NamedOrBlankNodeRef::into_owned)
            .collect();
        for x in subjects {
            pending.push(Triple::new(x, rdf::TYPE, dest.clone()));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(subClassOf) × delta(type).
    let mut new_typings: FxHashMap<NamedNode, Vec<NamedOrBlankNode>> = FxHashMap::default();
    for t in d.for_predicate(rdf::TYPE) {
        let Some(c) = owned_object_named(&t.object) else { continue };
        new_typings.entry(c).or_default().push(t.subject.clone());
    }
    for (c, dest) in &subclass_pairs {
        if let Some(xs) = new_typings.get(c) {
            for x in xs {
                pending.push(Triple::new(x.clone(), rdf::TYPE, dest.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }

    firings
}

fn apply_prp_dom(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-dom: if p rdfs:domain c and x p y then x rdf:type c.
    //
    // Semi-naive:
    //   Branch 1: delta(domain) joined with graph(p).
    //   Branch 2: graph(domain) joined with delta(p).
    let mut firings: u64 = 0;
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::DOMAIN)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let c = named_node_from_term(t.object)?;
            Some((p, c))
        })
        .collect();

    let Some(d) = delta else {
        for (p, c) in &pairs {
            let subjects: Vec<NamedOrBlankNode> = graph
                .triples_for_predicate(p.as_ref())
                .map(|t| t.subject.into_owned())
                .collect();
            for x in subjects {
                pending.push(Triple::new(x, rdf::TYPE, c.clone()));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(domain) × graph(p).
    for t in d.for_predicate(rdfs::DOMAIN) {
        let Some(p) = owned_subject_named(&t.subject) else { continue };
        let Some(c) = owned_object_named(&t.object) else { continue };
        let subjects: Vec<NamedOrBlankNode> = graph
            .triples_for_predicate(p.as_ref())
            .map(|t| t.subject.into_owned())
            .collect();
        for x in subjects {
            pending.push(Triple::new(x, rdf::TYPE, c.clone()));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(domain) × delta(p).
    for (p, c) in &pairs {
        for t in d.for_predicate(p.as_ref()) {
            pending.push(Triple::new(t.subject.clone(), rdf::TYPE, c.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_prp_rng(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-rng: if p rdfs:range c and x p y then y rdf:type c.
    // y must not be a literal (literals cannot be subjects of rdf:type).
    //
    // Semi-naive:
    //   Branch 1: delta(range) × graph(p).
    //   Branch 2: graph(range) × delta(p).
    let mut firings: u64 = 0;
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::RANGE)
        .filter_map(|t| {
            let p = named_node_from_subject(t.subject)?;
            let c = named_node_from_term(t.object)?;
            Some((p, c))
        })
        .collect();

    let Some(d) = delta else {
        for (p, c) in &pairs {
            let objects: Vec<NamedOrBlankNode> = graph
                .triples_for_predicate(p.as_ref())
                .filter_map(|t| term_ref_to_named_or_blank(t.object))
                .collect();
            for y in objects {
                pending.push(Triple::new(y, rdf::TYPE, c.clone()));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(range) × graph(p).
    for t in d.for_predicate(rdfs::RANGE) {
        let Some(p) = owned_subject_named(&t.subject) else { continue };
        let Some(c) = owned_object_named(&t.object) else { continue };
        let objects: Vec<NamedOrBlankNode> = graph
            .triples_for_predicate(p.as_ref())
            .filter_map(|t| term_ref_to_named_or_blank(t.object))
            .collect();
        for y in objects {
            pending.push(Triple::new(y, rdf::TYPE, c.clone()));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(range) × delta(p).
    for (p, c) in &pairs {
        for t in d.for_predicate(p.as_ref()) {
            if let Some(y) = owned_object_named_or_blank(&t.object) {
                pending.push(Triple::new(y, rdf::TYPE, c.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_prp_spo1(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-spo1: if p1 rdfs:subPropertyOf p2 and x p1 y then x p2 y.
    //
    // Semi-naive:
    //   Branch 1: delta(subPropertyOf) × graph(p1).
    //   Branch 2: graph(subPropertyOf) × delta(p1).
    let mut firings: u64 = 0;
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(rdfs::SUB_PROPERTY_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let Some(d) = delta else {
        for (p1, p2) in &pairs {
            let matched: Vec<(NamedOrBlankNode, Term)> = graph
                .triples_for_predicate(p1.as_ref())
                .map(|t| (t.subject.into_owned(), t.object.into_owned()))
                .collect();
            for (x, y) in matched {
                pending.push(Triple::new(x, p2.clone(), y));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    // Branch 1: delta(subPropertyOf) × graph(p1).
    for t in d.for_predicate(rdfs::SUB_PROPERTY_OF) {
        let Some(p1) = owned_subject_named(&t.subject) else { continue };
        let Some(p2) = owned_object_named(&t.object) else { continue };
        let matched: Vec<(NamedOrBlankNode, Term)> = graph
            .triples_for_predicate(p1.as_ref())
            .map(|t| (t.subject.into_owned(), t.object.into_owned()))
            .collect();
        for (x, y) in matched {
            pending.push(Triple::new(x, p2.clone(), y));
            firings = firings.saturating_add(1);
        }
    }

    // Branch 2: graph(subPropertyOf) × delta(p1).
    for (p1, p2) in &pairs {
        for t in d.for_predicate(p1.as_ref()) {
            pending.push(Triple::new(t.subject.clone(), p2.clone(), t.object.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

fn apply_prp_trp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-trp: if p rdf:type owl:TransitiveProperty, x p y, y p z then x p z.
    //
    // Semi-naive joins the two data antecedents against delta in turn:
    //   Branch 1: delta(p) × graph(p) (new first leg x-p-y extends existing y-p-z).
    //   Branch 2: graph(p) × delta(p) (existing x-p-y meets new second leg y-p-z).
    // If the TransitiveProperty declaration itself is new in this round, every
    // edge counts as "new" for this property and the branches collapse to the
    // naive square join.
    let transitive_properties: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_TRANSITIVE_PROPERTY)
        .filter_map(|s| match s {
            NamedOrBlankNodeRef::NamedNode(n) => Some(n.into_owned()),
            NamedOrBlankNodeRef::BlankNode(_) => None,
        })
        .collect();

    // Which properties had their TransitiveProperty declaration arrive in the
    // previous round's delta? An empty set when delta is None.
    let newly_transitive: rustc_hash::FxHashSet<NamedNode> =
        new_property_types(delta, OWL_TRANSITIVE_PROPERTY);

    let mut firings: u64 = 0;
    for p in transitive_properties {
        // Snapshot all (x, p, y) triples first to avoid nested borrows.
        let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
            .triples_for_predicate(p.as_ref())
            .filter_map(|t| {
                let x = t.subject.into_owned();
                let y = term_ref_to_named_or_blank(t.object)?;
                Some((x, y))
            })
            .collect();

        let Some(d) = delta else {
            // Round 1: naive square join.
            firings = firings.saturating_add(join_square(&edges, &edges, &p, pending));
            continue;
        };

        let delta_edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = if newly_transitive.contains(&p) {
            edges.clone()
        } else {
            d.for_predicate(p.as_ref())
                .iter()
                .filter_map(|t| {
                    let y = owned_object_named_or_blank(&t.object)?;
                    Some((t.subject.clone(), y))
                })
                .collect()
        };

        // Branch 1: delta × graph.
        firings = firings.saturating_add(join_square(&delta_edges, &edges, &p, pending));
        // Branch 2: graph × delta.
        firings = firings.saturating_add(join_square(&edges, &delta_edges, &p, pending));
    }
    firings
}

/// Emit every (x, p, z) where (x, y) is in `left` and (y, z) is in `right`.
fn join_square(
    left: &[(NamedOrBlankNode, NamedOrBlankNode)],
    right: &[(NamedOrBlankNode, NamedOrBlankNode)],
    p: &NamedNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    let mut firings: u64 = 0;
    for (x, y) in left {
        for (y2, z) in right {
            if named_or_blank_eq(y, y2) {
                pending.push(Triple::new(x.clone(), p.clone(), z.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// Collect IRIs whose (s rdf:type cls) triple landed in the previous round's
/// delta. Returns an empty set when `delta` is `None`.
fn new_property_types(
    delta: Option<&DeltaIndex>,
    cls: NamedNodeRef<'_>,
) -> rustc_hash::FxHashSet<NamedNode> {
    let Some(d) = delta else {
        return rustc_hash::FxHashSet::default();
    };
    d.for_predicate(rdf::TYPE)
        .iter()
        .filter_map(|t| {
            let Term::NamedNode(n) = &t.object else { return None };
            if n.as_ref() != cls {
                return None;
            }
            owned_subject_named(&t.subject)
        })
        .collect()
}

fn apply_prp_symp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-symp: if p rdf:type owl:SymmetricProperty and x p y then y p x.
    // y must be a resource (literal y cannot appear as a subject).
    //
    // Semi-naive:
    //   Branch 1: delta(schema) × graph(p) for properties newly declared symmetric.
    //   Branch 2: graph(schema) × delta(p) for fresh edges over known symmetric p.
    let symmetric_properties: Vec<NamedNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, OWL_SYMMETRIC_PROPERTY)
        .filter_map(named_node_from_subject)
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for p in &symmetric_properties {
            let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
                .triples_for_predicate(p.as_ref())
                .filter_map(|t| {
                    let x = t.subject.into_owned();
                    let y = term_ref_to_named_or_blank(t.object)?;
                    Some((x, y))
                })
                .collect();
            for (x, y) in edges {
                pending.push(Triple::new(y, p.clone(), x));
                firings = firings.saturating_add(1);
            }
        }
        return firings;
    };

    let newly_symmetric = new_property_types(delta, OWL_SYMMETRIC_PROPERTY);

    for p in &symmetric_properties {
        if newly_symmetric.contains(p) {
            // New schema: every existing edge is a candidate.
            let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
                .triples_for_predicate(p.as_ref())
                .filter_map(|t| {
                    let x = t.subject.into_owned();
                    let y = term_ref_to_named_or_blank(t.object)?;
                    Some((x, y))
                })
                .collect();
            for (x, y) in edges {
                pending.push(Triple::new(y, p.clone(), x));
                firings = firings.saturating_add(1);
            }
        } else {
            // Existing schema: only delta edges are new.
            for t in d.for_predicate(p.as_ref()) {
                let Some(y) = owned_object_named_or_blank(&t.object) else { continue };
                pending.push(Triple::new(y, p.clone(), t.subject.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

fn apply_prp_inv(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-inv1: if p1 owl:inverseOf p2 and x p1 y then y p2 x.
    // prp-inv2: if p1 owl:inverseOf p2 and x p2 y then y p1 x.
    // Combined because the engine applies both directions off the same fact.
    //
    // Semi-naive:
    //   Branch 1: delta(inverseOf) × graph(p1, p2) (new schema, full data scan).
    //   Branch 2: graph(inverseOf) × delta(p1) (fires prp-inv1).
    //   Branch 3: graph(inverseOf) × delta(p2) (fires prp-inv2).
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(OWL_INVERSE_OF)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p1, p2) in &pairs {
            firings = firings.saturating_add(emit_inverse(graph, p1, p2, pending));
            firings = firings.saturating_add(emit_inverse(graph, p2, p1, pending));
        }
        return firings;
    };

    // Branch 1: delta schema × full graph data.
    let mut seen_schema: rustc_hash::FxHashSet<(NamedNode, NamedNode)> = rustc_hash::FxHashSet::default();
    for t in d.for_predicate(OWL_INVERSE_OF) {
        let Some(p1) = owned_subject_named(&t.subject) else { continue };
        let Some(p2) = owned_object_named(&t.object) else { continue };
        seen_schema.insert((p1.clone(), p2.clone()));
        firings = firings.saturating_add(emit_inverse(graph, &p1, &p2, pending));
        firings = firings.saturating_add(emit_inverse(graph, &p2, &p1, pending));
    }

    // Branches 2 and 3: graph schema × delta data.
    for (p1, p2) in &pairs {
        // Skip schema pairs we already fully saturated in Branch 1.
        if seen_schema.contains(&(p1.clone(), p2.clone())) {
            continue;
        }
        // prp-inv1 over delta(p1).
        for t in d.for_predicate(p1.as_ref()) {
            let Some(y) = owned_object_named_or_blank(&t.object) else { continue };
            pending.push(Triple::new(y, p2.clone(), t.subject.clone()));
            firings = firings.saturating_add(1);
        }
        // prp-inv2 over delta(p2).
        for t in d.for_predicate(p2.as_ref()) {
            let Some(y) = owned_object_named_or_blank(&t.object) else { continue };
            pending.push(Triple::new(y, p1.clone(), t.subject.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

/// For each edge `(x, src, y)` in `graph`, push `(y, dst, x)` onto `pending`.
fn emit_inverse(
    graph: &Graph,
    src: &NamedNode,
    dst: &NamedNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    let edges: Vec<(NamedOrBlankNode, NamedOrBlankNode)> = graph
        .triples_for_predicate(src.as_ref())
        .filter_map(|t| {
            let x = t.subject.into_owned();
            let y = term_ref_to_named_or_blank(t.object)?;
            Some((x, y))
        })
        .collect();
    let mut firings: u64 = 0;
    for (x, y) in edges {
        pending.push(Triple::new(y, dst.clone(), x));
        firings = firings.saturating_add(1);
    }
    firings
}

fn apply_prp_eqp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // prp-eqp1: if p1 owl:equivalentProperty p2 and x p1 y then x p2 y.
    // prp-eqp2: if p1 owl:equivalentProperty p2 and x p2 y then x p1 y.
    // Literal objects are fine here: the object keeps its position.
    //
    // Semi-naive:
    //   Branch 1: delta(schema) × graph(p1, p2).
    //   Branch 2: graph(schema) × delta(p1) (fires prp-eqp1).
    //   Branch 3: graph(schema) × delta(p2) (fires prp-eqp2).
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(OWL_EQUIVALENT_PROPERTY)
        .filter_map(|t| {
            let p1 = named_node_from_subject(t.subject)?;
            let p2 = named_node_from_term(t.object)?;
            Some((p1, p2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (p1, p2) in &pairs {
            firings = firings.saturating_add(emit_rename(graph, p1, p2, pending));
            firings = firings.saturating_add(emit_rename(graph, p2, p1, pending));
        }
        return firings;
    };

    let mut seen_schema: rustc_hash::FxHashSet<(NamedNode, NamedNode)> = rustc_hash::FxHashSet::default();
    for t in d.for_predicate(OWL_EQUIVALENT_PROPERTY) {
        let Some(p1) = owned_subject_named(&t.subject) else { continue };
        let Some(p2) = owned_object_named(&t.object) else { continue };
        seen_schema.insert((p1.clone(), p2.clone()));
        firings = firings.saturating_add(emit_rename(graph, &p1, &p2, pending));
        firings = firings.saturating_add(emit_rename(graph, &p2, &p1, pending));
    }

    for (p1, p2) in &pairs {
        if seen_schema.contains(&(p1.clone(), p2.clone())) {
            continue;
        }
        for t in d.for_predicate(p1.as_ref()) {
            pending.push(Triple::new(t.subject.clone(), p2.clone(), t.object.clone()));
            firings = firings.saturating_add(1);
        }
        for t in d.for_predicate(p2.as_ref()) {
            pending.push(Triple::new(t.subject.clone(), p1.clone(), t.object.clone()));
            firings = firings.saturating_add(1);
        }
    }
    firings
}

/// For each edge `(x, src, y)` in `graph`, push `(x, dst, y)` onto `pending`.
/// Used by prp-eqp (object position stays put, predicate is renamed).
fn emit_rename(
    graph: &Graph,
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

fn apply_cax_eqc(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // cax-eqc1: if c1 owl:equivalentClass c2 and x rdf:type c1 then x rdf:type c2.
    // cax-eqc2: if c1 owl:equivalentClass c2 and x rdf:type c2 then x rdf:type c1.
    //
    // Semi-naive:
    //   Branch 1: delta(equivalentClass) × graph(type).
    //   Branch 2: graph(equivalentClass) × delta(type) (indexed by class).
    let pairs: Vec<(NamedNode, NamedNode)> = graph
        .triples_for_predicate(OWL_EQUIVALENT_CLASS)
        .filter_map(|t| {
            let c1 = named_node_from_subject(t.subject)?;
            let c2 = named_node_from_term(t.object)?;
            Some((c1, c2))
        })
        .collect();

    let mut firings: u64 = 0;

    let Some(d) = delta else {
        for (c1, c2) in &pairs {
            firings = firings.saturating_add(reclassify(graph, c1, c2, pending));
            firings = firings.saturating_add(reclassify(graph, c2, c1, pending));
        }
        return firings;
    };

    // Branch 1: delta(equivalentClass) × graph(type). Track which schema
    // pairs we already processed so Branch 2 does not duplicate them.
    let mut seen_schema: rustc_hash::FxHashSet<(NamedNode, NamedNode)> = rustc_hash::FxHashSet::default();
    for t in d.for_predicate(OWL_EQUIVALENT_CLASS) {
        let Some(c1) = owned_subject_named(&t.subject) else { continue };
        let Some(c2) = owned_object_named(&t.object) else { continue };
        seen_schema.insert((c1.clone(), c2.clone()));
        firings = firings.saturating_add(reclassify(graph, &c1, &c2, pending));
        firings = firings.saturating_add(reclassify(graph, &c2, &c1, pending));
    }

    // Branch 2: graph(equivalentClass) × delta(type). Index delta typings
    // once by class so each schema pair does O(1) lookups.
    let new_typings = index_delta_types(d);
    for (c1, c2) in &pairs {
        if seen_schema.contains(&(c1.clone(), c2.clone())) {
            continue;
        }
        if let Some(xs) = new_typings.get(c1) {
            for x in xs {
                pending.push(Triple::new(x.clone(), rdf::TYPE, c2.clone()));
                firings = firings.saturating_add(1);
            }
        }
        if let Some(xs) = new_typings.get(c2) {
            for x in xs {
                pending.push(Triple::new(x.clone(), rdf::TYPE, c1.clone()));
                firings = firings.saturating_add(1);
            }
        }
    }
    firings
}

/// For every (x, rdf:type, from) edge in the graph push (x, rdf:type, to).
fn reclassify(
    graph: &Graph,
    from: &NamedNode,
    to: &NamedNode,
    pending: &mut Vec<Triple>,
) -> u64 {
    let subjects: Vec<NamedOrBlankNode> = graph
        .subjects_for_predicate_object(rdf::TYPE, from.as_ref())
        .map(NamedOrBlankNodeRef::into_owned)
        .collect();
    let mut firings: u64 = 0;
    for x in subjects {
        pending.push(Triple::new(x, rdf::TYPE, to.clone()));
        firings = firings.saturating_add(1);
    }
    firings
}

/// Group delta `rdf:type` triples by their object class. Returned map holds
/// the list of fresh individuals for each class touched in the previous
/// round.
fn index_delta_types(delta: &DeltaIndex) -> FxHashMap<NamedNode, Vec<NamedOrBlankNode>> {
    let mut out: FxHashMap<NamedNode, Vec<NamedOrBlankNode>> = FxHashMap::default();
    for t in delta.for_predicate(rdf::TYPE) {
        let Some(c) = owned_object_named(&t.object) else { continue };
        out.entry(c).or_default().push(t.subject.clone());
    }
    out
}

// ---------------------------------------------------------------------------
// Schema rules (scm-*). Close the graph under class and property hierarchy
// axioms. None of these touch instance data.

fn apply_scm_cls(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
        pending.push(Triple::new(c.clone(), rdfs::SUB_CLASS_OF, OWL_THING.into_owned()));
        pending.push(Triple::new(OWL_NOTHING.into_owned(), rdfs::SUB_CLASS_OF, c));
        firings = firings.saturating_add(4);
    }
    firings
}

fn apply_scm_sco(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
fn build_pivot_index(
    pairs: &[(NamedNode, NamedNode)],
) -> FxHashMap<NamedNode, Vec<NamedNode>> {
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

fn apply_scm_op(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-op: for every p rdf:type owl:ObjectProperty, add
    //   p rdfs:subPropertyOf p
    //   p owl:equivalentProperty p
    //
    // Semi-naive: only fire for properties whose declaration is new.
    let properties: Vec<NamedNode> = if delta.is_some() {
        new_property_types(delta, OWL_OBJECT_PROPERTY).into_iter().collect()
    } else {
        graph
            .subjects_for_predicate_object(rdf::TYPE, OWL_OBJECT_PROPERTY)
            .filter_map(named_node_from_subject)
            .collect()
    };

    let mut firings: u64 = 0;
    for p in properties {
        pending.push(Triple::new(p.clone(), rdfs::SUB_PROPERTY_OF, p.clone()));
        pending.push(Triple::new(p.clone(), OWL_EQUIVALENT_PROPERTY.into_owned(), p));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_dp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
    // scm-dp: same as scm-op but for owl:DatatypeProperty.
    let properties: Vec<NamedNode> = if delta.is_some() {
        new_property_types(delta, OWL_DATATYPE_PROPERTY).into_iter().collect()
    } else {
        graph
            .subjects_for_predicate_object(rdf::TYPE, OWL_DATATYPE_PROPERTY)
            .filter_map(named_node_from_subject)
            .collect()
    };

    let mut firings: u64 = 0;
    for p in properties {
        pending.push(Triple::new(p.clone(), rdfs::SUB_PROPERTY_OF, p.clone()));
        pending.push(Triple::new(p.clone(), OWL_EQUIVALENT_PROPERTY.into_owned(), p));
        firings = firings.saturating_add(2);
    }
    firings
}

fn apply_scm_eqc1(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_scm_eqc2(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_scm_eqp1(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_scm_eqp2(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

    firings = firings.saturating_add(emit_equivalent_property_pairs(&delta_edges, &edges, pending));
    firings = firings.saturating_add(emit_equivalent_property_pairs(&edges, &delta_edges, pending));
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

fn apply_scm_dom1(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_scm_rng1(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_eq_sym(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_eq_trans(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_eq_rep_s(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
    let mut seen_schema: rustc_hash::FxHashSet<(NamedOrBlankNode, NamedOrBlankNode)> =
        rustc_hash::FxHashSet::default();
    for t in d.for_predicate(OWL_SAME_AS) {
        let Some(y) = owned_object_named_or_blank(&t.object) else { continue };
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
    graph: &Graph,
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

fn apply_eq_rep_p(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
    let mut seen_schema: rustc_hash::FxHashSet<(NamedNode, NamedNode)> = rustc_hash::FxHashSet::default();
    for t in d.for_predicate(OWL_SAME_AS) {
        let Some(p1) = owned_subject_named(&t.subject) else { continue };
        let Some(p2) = owned_object_named(&t.object) else { continue };
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

fn apply_eq_rep_o(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
    let mut seen_schema: rustc_hash::FxHashSet<(NamedOrBlankNode, Term)> = rustc_hash::FxHashSet::default();
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
    graph: &Graph,
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

fn apply_prp_fp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_prp_ifp(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
struct InconsistencyTriggers {
    has_disjoint_with: bool,
    has_nothing_declaration: bool,
    has_irreflexive_property: bool,
    has_asymmetric_property: bool,
    has_property_disjoint_with: bool,
}

impl InconsistencyTriggers {
    fn scan(graph: &Graph) -> Self {
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

fn find_inconsistency(graph: &Graph, triggers: InconsistencyTriggers) -> Option<Inconsistency> {
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

fn find_cax_dw_clash(graph: &Graph) -> Option<DisjointClash> {
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
fn find_cls_nothing2(graph: &Graph) -> Option<Inconsistency> {
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
fn find_prp_irp(graph: &Graph) -> Option<Inconsistency> {
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
fn find_prp_asyp(graph: &Graph) -> Option<Inconsistency> {
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
fn find_prp_pdw(graph: &Graph) -> Option<Inconsistency> {
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

fn apply_scm_spo(graph: &Graph, delta: Option<&DeltaIndex>, pending: &mut Vec<Triple>) -> u64 {
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
fn collect_hasvalue_restrictions(graph: &Graph) -> Vec<(NamedOrBlankNode, NamedNode, Term)> {
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

fn apply_cls_hv1(graph: &Graph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_cls_hv2(graph: &Graph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
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
            .filter_map(|t| {
                if t.object == v.as_ref() {
                    Some(t.subject.into_owned())
                } else {
                    None
                }
            })
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
fn parse_rdf_list(graph: &Graph, head: NamedOrBlankNodeRef<'_>) -> Option<Vec<NamedOrBlankNode>> {
    let nil = rdf::NIL;
    let mut out: Vec<NamedOrBlankNode> = Vec::new();
    let mut current: NamedOrBlankNode = head.into_owned();
    let mut seen: rustc_hash::FxHashSet<NamedOrBlankNode> = rustc_hash::FxHashSet::default();
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
                _ => None,
            })?;
        out.push(first);
        // rest
        let rest = graph
            .objects_for_subject_predicate(current.as_ref(), rdf::REST)
            .next()
            .and_then(|t| match t {
                TermRef::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.into_owned())),
                TermRef::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.into_owned())),
                _ => None,
            })?;
        current = rest;
    }
}

/// Collect every (c, members) where `c owl:intersectionOf L` and `L` is a
/// well-formed RDF list of resources.
fn collect_intersection_classes(graph: &Graph) -> Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> {
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
fn collect_union_classes(graph: &Graph) -> Vec<(NamedOrBlankNode, Vec<NamedOrBlankNode>)> {
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

fn apply_cls_int1(graph: &Graph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
    // cls-int1: c owl:intersectionOf (c1 ... cn), x rdf:type ci for all i
    // then x rdf:type c. Per W3C the classes are resources and the list is
    // a well-formed RDF list.
    let mut firings: u64 = 0;
    for (c, members) in &tbox.intersection_classes {
        // Candidate individuals: those typed as the first member.
        let Some(first) = members.first() else { continue };
        let first_ref = first.as_ref();
        let first_term: TermRef<'_> = match first_ref {
            NamedOrBlankNodeRef::NamedNode(n) => TermRef::NamedNode(n),
            NamedOrBlankNodeRef::BlankNode(b) => TermRef::BlankNode(b),
        };
        let candidates: Vec<NamedOrBlankNode> = graph
            .triples_for_predicate(rdf::TYPE)
            .filter_map(|t| {
                if t.object == first_term {
                    Some(t.subject.into_owned())
                } else {
                    None
                }
            })
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

fn apply_cls_int2(graph: &Graph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
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

fn apply_cls_uni(graph: &Graph, tbox: &TBoxCache, pending: &mut Vec<Triple>) -> u64 {
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

    /// Unwrap the `expand` result for tests that expect a consistent graph.
    #[expect(clippy::expect_used, reason = "engine tests assert the Ok path and panic on regression")]
    fn expand_ok(graph: &mut Graph, config: &ReasonerConfig) -> RunStats {
        expand(graph, config).expect("expand must succeed on a consistent graph")
    }

    #[test]
    fn cax_sco_single_step_inference() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));
        g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

        let stats = expand_ok(&mut g, &owl_cfg());

        assert!(stats.added >= 1);
        assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("LegalPerson"))));
    }

    #[test]
    fn cax_sco_chains_through_multiple_subclass_steps() {
        let mut g = Graph::default();
        g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));
        g.insert(&Triple::new(ex("LegalPerson"), rdfs::SUB_CLASS_OF, ex("Entity")));
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
        g.insert(&Triple::new(ex("Parent"), has_bo.clone(), ex("UltimateOwner")));

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
        g.insert(&Triple::new(owns.clone(), OWL_INVERSE_OF.into_owned(), owned_by.clone()));
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
        g.insert(&Triple::new(ex("Company"), rdf::TYPE, OWL_CLASS.into_owned()));

        expand_ok(&mut g, &owl_cfg());

        assert!(g.contains(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("Company"))));
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
        g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));

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
        g.insert(&Triple::new(ex("Company"), rdf::TYPE, OWL_CLASS.into_owned()));

        expand_ok(&mut g, &rdfs_cfg());

        // scm-cls is an OWL rule, so it must stay dormant under the Rdfs
        // profile.
        assert!(!g.contains(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("Company"))));
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

        let err = expand(&mut g, &owl_cfg()).unwrap_err();
        let Inconsistency::DisjointClasses(clash) = err else {
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
        g.insert(&Triple::new(sibling.clone(), rdfs::SUB_PROPERTY_OF, reaches.clone()));
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
