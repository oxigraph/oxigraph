//! Rule representation for the OWL 2 RL forward chainer.
//!
//! The full rule tables for OWL 2 RL and the RDFS subset are defined in
//! section 4.3 of the W3C OWL 2 Profiles recommendation. This module
//! currently exposes the identifier enum and an empty `RuleSet` builder so
//! downstream code can reference rules by name. The actual rule bodies land
//! alongside the forward chainer in milestone M1.

use rustc_hash::FxHashSet;

/// Stable identifiers for OWL 2 RL and RDFS rules.
///
/// Names mirror the W3C OWL 2 Profiles document. `Rdfs*` variants cover the
/// rules that appear in both profiles and can be used as a lighter weight
/// alternative via [`crate::ReasoningProfile::Rdfs`].
#[expect(
    non_camel_case_types,
    reason = "OWL 2 RL rule identifiers mirror W3C naming, which uses lowercase separators"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum RuleId {
    // Class axioms.
    cax_sco,
    cax_eqc1,
    cax_eqc2,
    cax_dw,
    cax_int,
    cax_uni,

    // Class expression rules.
    cls_int1,
    cls_int2,
    cls_uni,
    cls_hv1,
    cls_hv2,
    cls_nothing2,

    // Property axioms.
    prp_dom,
    prp_rng,
    prp_trp,
    prp_symp,
    prp_inv1,
    prp_inv2,
    prp_spo1,
    prp_eqp1,
    prp_eqp2,
    prp_fp,
    prp_ifp,
    prp_irp,
    prp_asyp,
    prp_pdw,

    // Equality (opt in).
    eq_ref,
    eq_sym,
    eq_trans,
    eq_rep_s,
    eq_rep_p,
    eq_rep_o,

    // Schema rules (a subset, full list lands in M3).
    scm_cls,
    scm_sco,
    scm_op,
    scm_dp,
    scm_spo,
    scm_eqc1,
    scm_eqc2,
    scm_eqp1,
    scm_eqp2,
    scm_dom1,
    scm_rng1,

    // RDFS subset, reused when profile is Rdfs.
    rdfs_subclass_of,
    rdfs_subproperty_of,
    rdfs_domain,
    rdfs_range,
}

impl RuleId {
    /// Human readable identifier, matches the naming used in the W3C
    /// recommendation (for example `prp trp` becomes `"prp-trp"`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::cax_sco => "cax-sco",
            Self::cax_eqc1 => "cax-eqc1",
            Self::cax_eqc2 => "cax-eqc2",
            Self::cax_dw => "cax-dw",
            Self::cax_int => "cax-int",
            Self::cax_uni => "cax-uni",
            Self::cls_int1 => "cls-int1",
            Self::cls_int2 => "cls-int2",
            Self::cls_uni => "cls-uni",
            Self::cls_hv1 => "cls-hv1",
            Self::cls_hv2 => "cls-hv2",
            Self::cls_nothing2 => "cls-nothing2",
            Self::prp_dom => "prp-dom",
            Self::prp_rng => "prp-rng",
            Self::prp_trp => "prp-trp",
            Self::prp_symp => "prp-symp",
            Self::prp_inv1 => "prp-inv1",
            Self::prp_inv2 => "prp-inv2",
            Self::prp_spo1 => "prp-spo1",
            Self::prp_eqp1 => "prp-eqp1",
            Self::prp_eqp2 => "prp-eqp2",
            Self::prp_fp => "prp-fp",
            Self::prp_ifp => "prp-ifp",
            Self::prp_irp => "prp-irp",
            Self::prp_asyp => "prp-asyp",
            Self::prp_pdw => "prp-pdw",
            Self::eq_ref => "eq-ref",
            Self::eq_sym => "eq-sym",
            Self::eq_trans => "eq-trans",
            Self::eq_rep_s => "eq-rep-s",
            Self::eq_rep_p => "eq-rep-p",
            Self::eq_rep_o => "eq-rep-o",
            Self::scm_cls => "scm-cls",
            Self::scm_sco => "scm-sco",
            Self::scm_op => "scm-op",
            Self::scm_dp => "scm-dp",
            Self::scm_spo => "scm-spo",
            Self::scm_eqc1 => "scm-eqc1",
            Self::scm_eqc2 => "scm-eqc2",
            Self::scm_eqp1 => "scm-eqp1",
            Self::scm_eqp2 => "scm-eqp2",
            Self::scm_dom1 => "scm-dom1",
            Self::scm_rng1 => "scm-rng1",
            Self::rdfs_subclass_of => "rdfs-subclass-of",
            Self::rdfs_subproperty_of => "rdfs-subproperty-of",
            Self::rdfs_domain => "rdfs-domain",
            Self::rdfs_range => "rdfs-range",
        }
    }

    /// True if this rule belongs to the equality family (opt in).
    #[must_use]
    pub fn is_equality_rule(self) -> bool {
        matches!(
            self,
            Self::eq_ref
                | Self::eq_sym
                | Self::eq_trans
                | Self::eq_rep_s
                | Self::eq_rep_p
                | Self::eq_rep_o
        )
    }
}

/// A placeholder rule body. The concrete antecedent and consequent types
/// will be added alongside the forward chainer in milestone M1. Keeping the
/// struct opaque here avoids locking the internal representation into the
/// public API too early.
#[derive(Clone, Debug)]
pub struct Rule {
    id: RuleId,
}

impl Rule {
    /// Rule identifier.
    #[must_use]
    pub fn id(&self) -> RuleId {
        self.id
    }

    /// Construct a rule stub by identifier. Used by tests and by callers
    /// assembling a custom [`RuleSet`] before any bodies are available.
    #[must_use]
    pub fn stub(id: RuleId) -> Self {
        Self { id }
    }
}

/// Collection of rules evaluated by a reasoner run. Deduplicated by
/// [`RuleId`].
#[derive(Clone, Debug, Default)]
pub struct RuleSet {
    ids: FxHashSet<RuleId>,
}

impl RuleSet {
    /// Empty rule set.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// OWL 2 RL rule set excluding equality rules.
    #[must_use]
    pub fn owl2_rl_core() -> Self {
        let mut set = Self::empty();
        for id in OWL2_RL_CORE {
            set.ids.insert(*id);
        }
        set
    }

    /// Minimal RDFS rule set.
    #[must_use]
    pub fn rdfs() -> Self {
        let mut set = Self::empty();
        for id in RDFS_RULES {
            set.ids.insert(*id);
        }
        set
    }

    /// Add a rule. No effect if already present.
    pub fn insert(&mut self, rule: &Rule) {
        self.ids.insert(rule.id());
    }

    /// Whether the set contains the given rule.
    #[must_use]
    pub fn contains(&self, id: RuleId) -> bool {
        self.ids.contains(&id)
    }

    /// Number of rules in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Iterate over the rule identifiers in the set.
    pub fn iter(&self) -> impl Iterator<Item = RuleId> + '_ {
        self.ids.iter().copied()
    }
}

const OWL2_RL_CORE: &[RuleId] = &[
    RuleId::cax_sco,
    RuleId::cax_eqc1,
    RuleId::cax_eqc2,
    RuleId::cax_dw,
    RuleId::cax_int,
    RuleId::cax_uni,
    RuleId::prp_dom,
    RuleId::prp_rng,
    RuleId::prp_trp,
    RuleId::prp_symp,
    RuleId::prp_inv1,
    RuleId::prp_inv2,
    RuleId::prp_spo1,
    RuleId::prp_eqp1,
    RuleId::prp_eqp2,
    RuleId::prp_fp,
    RuleId::prp_ifp,
    RuleId::scm_cls,
    RuleId::scm_sco,
    RuleId::scm_op,
    RuleId::scm_dp,
    RuleId::scm_eqc1,
    RuleId::scm_eqc2,
    RuleId::scm_eqp1,
    RuleId::scm_eqp2,
    RuleId::scm_dom1,
    RuleId::scm_rng1,
];

const RDFS_RULES: &[RuleId] = &[
    RuleId::rdfs_subclass_of,
    RuleId::rdfs_subproperty_of,
    RuleId::rdfs_domain,
    RuleId::rdfs_range,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owl2_rl_core_contains_transitive_property_rule() {
        let set = RuleSet::owl2_rl_core();
        assert!(set.contains(RuleId::prp_trp));
        assert!(!set.contains(RuleId::eq_trans));
    }

    #[test]
    fn rdfs_set_is_smaller_than_owl2_rl_core() {
        assert!(RuleSet::rdfs().len() < RuleSet::owl2_rl_core().len());
    }

    #[test]
    fn equality_flag_identifies_equality_rules() {
        assert!(RuleId::eq_trans.is_equality_rule());
        assert!(!RuleId::prp_trp.is_equality_rule());
    }
}
