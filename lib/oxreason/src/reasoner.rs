//! Public reasoner API.
//!
//! The [`Reasoner`] type is the entry point. At this stage it accepts a
//! configuration, exposes `expand` and `expand_into` methods, and returns
//! [`ReasonError::NotImplemented`] from every method. The signatures are
//! committed so downstream crates (pyoxigraph, oxigraph/cli, external
//! callers) can start wiring against them while the rule engine is being
//! built.

use std::convert::Infallible;

use oxrdf::{Dataset, Graph, Triple};

use crate::engine;
use crate::error::{ReasonError, ReasonStreamError};
use crate::rules::RuleSet;

/// Which family of rules the reasoner applies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningProfile {
    /// Apply the full OWL 2 RL profile (default target for issue #130).
    #[default]
    Owl2Rl,
    /// Apply only RDFS entailment rules. Smaller, faster, useful as a
    /// baseline while the OWL 2 RL engine is under construction.
    Rdfs,
    /// Apply a custom rule set provided by the caller.
    Custom,
}

/// Configuration for a [`Reasoner`] instance.
///
/// Built as a small builder so new knobs (equality rules, provenance sinks,
/// cancellation tokens) can be added without breaking callers.
#[derive(Clone, Debug)]
pub struct ReasonerConfig {
    profile: ReasoningProfile,
    include_equality_rules: bool,
    materialise_into_named_graph: Option<oxrdf::NamedNode>,
    custom_rules: Option<RuleSet>,
}

impl ReasonerConfig {
    /// Default configuration targeting OWL 2 RL, equality rules off.
    #[must_use]
    pub fn owl2_rl() -> Self {
        Self {
            profile: ReasoningProfile::Owl2Rl,
            include_equality_rules: false,
            materialise_into_named_graph: None,
            custom_rules: None,
        }
    }

    /// Configuration targeting the RDFS subset only.
    #[must_use]
    pub fn rdfs() -> Self {
        Self {
            profile: ReasoningProfile::Rdfs,
            include_equality_rules: false,
            materialise_into_named_graph: None,
            custom_rules: None,
        }
    }

    /// Configuration for a caller supplied rule set.
    #[must_use]
    pub fn custom(rules: RuleSet) -> Self {
        Self {
            profile: ReasoningProfile::Custom,
            include_equality_rules: false,
            materialise_into_named_graph: None,
            custom_rules: Some(rules),
        }
    }

    /// Enable the OWL 2 RL equality rules (eq ref, eq sym, eq trans,
    /// eq rep s, eq rep p, eq rep o). These are correct but can explode
    /// graph size on noisy data, so they default to off.
    #[must_use]
    pub fn with_equality_rules(mut self, enabled: bool) -> Self {
        self.include_equality_rules = enabled;
        self
    }

    /// Route inferred quads into a dedicated named graph instead of the
    /// source graph. Recommended for auditability.
    #[must_use]
    pub fn into_named_graph(mut self, graph: oxrdf::NamedNode) -> Self {
        self.materialise_into_named_graph = Some(graph);
        self
    }

    /// Current reasoning profile.
    #[must_use]
    pub fn profile(&self) -> ReasoningProfile {
        self.profile
    }

    /// Whether equality rules are enabled.
    #[must_use]
    pub fn equality_rules_enabled(&self) -> bool {
        self.include_equality_rules
    }

    /// Target named graph for inferred quads, if any.
    #[must_use]
    pub fn target_named_graph(&self) -> Option<&oxrdf::NamedNode> {
        self.materialise_into_named_graph.as_ref()
    }

    /// Custom rule set when [`ReasoningProfile::Custom`] is selected.
    #[must_use]
    pub fn custom_rules(&self) -> Option<&RuleSet> {
        self.custom_rules.as_ref()
    }
}

impl Default for ReasonerConfig {
    fn default() -> Self {
        Self::owl2_rl()
    }
}

/// Summary returned by successful reasoning runs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReasoningReport {
    /// Number of triples newly materialised in the target graph.
    pub added: u64,
    /// Number of fixpoint rounds executed before saturation.
    pub rounds: u32,
    /// Number of rule firings across the full run.
    pub firings: u64,
}

/// OWL 2 RL forward chaining reasoner.
///
/// Constructed from a [`ReasonerConfig`] and then used to expand a graph or
/// dataset. The current implementation is a scaffold: all methods return
/// [`ReasonError::NotImplemented`].
#[derive(Clone, Debug)]
pub struct Reasoner {
    config: ReasonerConfig,
}

impl Reasoner {
    /// Construct a reasoner with the given configuration.
    #[must_use]
    pub fn new(config: ReasonerConfig) -> Self {
        Self { config }
    }

    /// Configuration this reasoner was built with.
    #[must_use]
    pub fn config(&self) -> &ReasonerConfig {
        &self.config
    }

    /// Materialise the reasoning closure of `graph` in place.
    ///
    /// Returns a [`ReasoningReport`] describing how many triples were added
    /// and how much work was done.
    ///
    /// Current behaviour (M1 plus M2 plus M3 plus M4): runs the core rules
    /// cax-sco, prp-dom, prp-rng, prp-spo1, and for the Owl2Rl profile also
    /// prp-trp, prp-symp, prp-inv1, prp-inv2, prp-eqp1, prp-eqp2, cax-eqc1,
    /// cax-eqc2, the schema rules scm-cls, scm-sco, scm-spo, scm-op, scm-dp,
    /// scm-eqc1, scm-eqc2, scm-eqp1, scm-eqp2, scm-dom1, scm-rng1, the
    /// class expression rules cls-hv1, cls-hv2, cls-int1, cls-int2, cls-uni,
    /// and the inconsistency detectors cax-dw, cls-nothing2, prp-irp,
    /// prp-asyp, prp-pdw. The equality rules (eq-sym, eq-trans, eq-rep-s,
    /// eq-rep-p, eq-rep-o) and the functional property rules (prp-fp,
    /// prp-ifp) are gated behind [`ReasonerConfig::with_equality_rules`]
    /// and default to off.
    ///
    /// When cax-dw finds an individual typed as two classes that appear in
    /// an `owl:disjointWith` pair, expansion aborts and returns
    /// [`ReasonError::Inconsistent`] with the offending individual and
    /// classes named. Other clashes (cls-nothing2, prp-irp, prp-asyp,
    /// prp-pdw) surface as [`ReasonError::InconsistentAxiom`] with a
    /// rule-prefixed human message. Returns [`ReasonError::NotImplemented`]
    /// for the Custom profile because the caller-supplied RuleSet is not
    /// yet plugged into the engine.
    pub fn expand(&self, graph: &mut Graph) -> Result<ReasoningReport, ReasonError> {
        match self
            .expand_streaming(graph, |_: &Triple| -> Result<(), Infallible> { Ok(()) })
        {
            Ok(report) => Ok(report),
            Err(ReasonStreamError::Reason(e)) => Err(e),
            Err(ReasonStreamError::Sink(never)) => match never {},
        }
    }

    /// Materialise the reasoning closure of `graph` in place while
    /// streaming every novel inference to `sink`.
    ///
    /// The sink is called once per genuine new triple, after it has been
    /// inserted into `graph`. Sink errors abort the run and surface as
    /// [`ReasonStreamError::Sink`]; engine-level inconsistencies surface
    /// as [`ReasonStreamError::Reason`]. The sink is not invoked for
    /// triples that were already present in the input graph.
    ///
    /// This is the streaming entry point used by
    /// [`oxigraph::store::Store::reason`]: the sink writes each inference
    /// into an open transaction, which lets the store avoid a second
    /// post-expansion pass over the full closure.
    pub fn expand_streaming<F, E>(
        &self,
        graph: &mut Graph,
        mut sink: F,
    ) -> Result<ReasoningReport, ReasonStreamError<E>>
    where
        F: FnMut(&Triple) -> Result<(), E>,
    {
        match self.config.profile {
            ReasoningProfile::Owl2Rl | ReasoningProfile::Rdfs => {
                // Seed the engine's FlatGraph from the caller's Graph. The
                // FlatGraph keeps its own hash indexes plus the term
                // interner; `graph` is only read at this step and written
                // back at the end.
                let mut flat = engine::FlatGraph::with_capacity(graph.len());
                for t in graph.iter() {
                    flat.insert(&t.into_owned());
                }
                let seed_len = flat.len();
                let run = engine::expand(&mut flat, &self.config, &mut sink);

                // Copy the new triples back into the caller's Graph. The
                // seed triples are already there, so we only drain the tail
                // of `flat.into_triples()` past the seed watermark. On error
                // we still write back whatever was materialised up to the
                // point of failure, matching the old in-place behaviour.
                let triples = flat.into_triples();
                for t in &triples[seed_len..] {
                    graph.insert(t);
                }

                map_run_result::<E>(run)
            }
            ReasoningProfile::Custom => Err(ReasonStreamError::Reason(ReasonError::NotImplemented(
                "custom RuleSet evaluation is not wired into the engine yet, see DESIGN.md M2",
            ))),
        }
    }

    /// Materialise the reasoning closure starting from an iterator of seed
    /// triples, streaming every novel inference to `sink`.
    ///
    /// Unlike [`Reasoner::expand_streaming`], this does not require the
    /// caller to first build an [`oxrdf::Graph`] with every seed triple.
    /// The iterator is drained into the engine's interned FlatGraph
    /// directly, which keeps peak memory at ~O(|seed| + |inferred|) in
    /// the interned form rather than paying the cost of an intermediate
    /// `oxrdf::Graph` with its six BTreeSet indexes.
    ///
    /// This is the entry point [`oxigraph::store::Store::reason`] uses to
    /// stream quads out of RocksDB: a `Map` from the storage cursor into
    /// owned `Triple`s feeds straight into the reasoner without a second
    /// pass over the dataset.
    ///
    /// Sink errors abort the run and surface as [`ReasonStreamError::Sink`];
    /// engine-level inconsistencies surface as [`ReasonStreamError::Reason`].
    /// The sink is only invoked for genuinely new triples; seeds are never
    /// handed to it, even when they were duplicated within the iterator.
    pub fn expand_streaming_from<I, F, E>(
        &self,
        input: I,
        mut sink: F,
    ) -> Result<ReasoningReport, ReasonStreamError<E>>
    where
        I: IntoIterator<Item = Triple>,
        F: FnMut(&Triple) -> Result<(), E>,
    {
        match self.config.profile {
            ReasoningProfile::Owl2Rl | ReasoningProfile::Rdfs => {
                let iter = input.into_iter();
                let (lower, _) = iter.size_hint();
                let mut flat = engine::FlatGraph::with_capacity(lower);
                for t in iter {
                    flat.insert(&t);
                }
                let run = engine::expand(&mut flat, &self.config, &mut sink);
                map_run_result::<E>(run)
            }
            ReasoningProfile::Custom => Err(ReasonStreamError::Reason(ReasonError::NotImplemented(
                "custom RuleSet evaluation is not wired into the engine yet, see DESIGN.md M2",
            ))),
        }
    }

    /// Materialise the reasoning closure of `src` and write the inferred
    /// quads into `dst`. `src` is not modified.
    ///
    /// Current behaviour: returns [`ReasonError::NotImplemented`].
    #[expect(clippy::unused_self, reason = "stub until M3 lands dataset support")]
    pub fn expand_into(
        &self,
        _src: &Dataset,
        _dst: &mut Dataset,
    ) -> Result<ReasoningReport, ReasonError> {
        Err(ReasonError::NotImplemented(
            "Reasoner::expand_into is not implemented yet, see DESIGN.md",
        ))
    }

    /// Materialise the reasoning closure of a dataset in place.
    ///
    /// Current behaviour: returns [`ReasonError::NotImplemented`].
    #[expect(clippy::unused_self, reason = "stub until M3 lands dataset support")]
    pub fn expand_dataset(&self, _dataset: &mut Dataset) -> Result<ReasoningReport, ReasonError> {
        Err(ReasonError::NotImplemented(
            "Reasoner::expand_dataset is not implemented yet, see DESIGN.md",
        ))
    }
}

/// Project an `engine::expand` result onto the public
/// [`ReasonStreamError`] shape. Shared by every `expand_*` entry point so
/// the inconsistency mapping stays in one place.
fn map_run_result<E>(
    run: Result<engine::RunStats, engine::ExpandError<E>>,
) -> Result<ReasoningReport, ReasonStreamError<E>> {
    match run {
        Ok(stats) => Ok(ReasoningReport {
            added: stats.added,
            rounds: stats.rounds,
            firings: stats.firings,
        }),
        Err(engine::ExpandError::Inconsistency(engine::Inconsistency::DisjointClasses(clash))) => {
            Err(ReasonStreamError::Reason(ReasonError::Inconsistent {
                individual: clash.individual.to_string(),
                class_a: clash.class_a.to_string(),
                class_b: clash.class_b.to_string(),
            }))
        }
        Err(engine::ExpandError::Inconsistency(other)) => Err(ReasonStreamError::Reason(
            ReasonError::InconsistentAxiom {
                message: other.message(),
            },
        )),
        Err(engine::ExpandError::Sink(e)) => Err(ReasonStreamError::Sink(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder_defaults_to_owl2_rl() {
        let c = ReasonerConfig::default();
        assert_eq!(c.profile(), ReasoningProfile::Owl2Rl);
        assert!(!c.equality_rules_enabled());
        assert!(c.target_named_graph().is_none());
    }

    #[test]
    fn config_builder_toggles_equality_rules() {
        let c = ReasonerConfig::owl2_rl().with_equality_rules(true);
        assert!(c.equality_rules_enabled());
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test asserts the Ok path and panics on regression")]
    fn expand_on_empty_graph_returns_zero_added() {
        let reasoner = Reasoner::new(ReasonerConfig::owl2_rl());
        let mut g = Graph::default();
        let report = reasoner.expand(&mut g).expect("empty graph must reason cleanly");
        assert_eq!(report.added, 0);
        assert!(report.rounds >= 1);
    }

    #[test]
    fn expand_custom_profile_still_returns_not_implemented() {
        let reasoner = Reasoner::new(ReasonerConfig::custom(RuleSet::owl2_rl_core()));
        let mut g = Graph::default();
        let err = reasoner.expand(&mut g).unwrap_err();
        assert!(matches!(err, ReasonError::NotImplemented(_)));
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test asserts the Ok path and panics on regression")]
    fn expand_streaming_from_seeds_without_intermediate_graph() {
        use oxrdf::{NamedNode, vocab::{rdf, rdfs}};

        let company = NamedNode::new_unchecked("https://example.org/Company");
        let legal = NamedNode::new_unchecked("https://example.org/LegalPerson");
        let acme = NamedNode::new_unchecked("https://example.org/Acme");

        let seeds = vec![
            Triple::new(company.clone(), rdfs::SUB_CLASS_OF, legal.clone()),
            Triple::new(acme.clone(), rdf::TYPE, company),
        ];

        let reasoner = Reasoner::new(ReasonerConfig::owl2_rl());
        let mut inferred: Vec<Triple> = Vec::new();
        let report = reasoner
            .expand_streaming_from(seeds, |t: &Triple| -> Result<(), Infallible> {
                inferred.push(t.clone());
                Ok(())
            })
            .expect("iterator-seeded run must succeed on a consistent seed");

        assert!(report.added >= 1);
        let expected = Triple::new(acme, rdf::TYPE, legal);
        assert!(inferred.iter().any(|t| t == &expected));
    }

    #[test]
    fn expand_into_and_expand_dataset_remain_not_implemented() {
        let reasoner = Reasoner::new(ReasonerConfig::owl2_rl());
        let src = Dataset::default();
        let mut dst = Dataset::default();
        assert!(matches!(
            reasoner.expand_into(&src, &mut dst).unwrap_err(),
            ReasonError::NotImplemented(_)
        ));
        let mut ds = Dataset::default();
        assert!(matches!(
            reasoner.expand_dataset(&mut ds).unwrap_err(),
            ReasonError::NotImplemented(_)
        ));
    }
}
