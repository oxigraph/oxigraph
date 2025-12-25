//! OWL 2 RL entailment rules.

/// OWL 2 RL rule identifier.
///
/// These identifiers correspond to the OWL 2 RL/RDF rules from the W3C specification.
/// They are available for debugging, logging, and rule selection purposes.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlRule {
    // Class axiom rules
    CaxSco,  // SubClassOf
    CaxEqc1, // EquivalentClasses (1)
    CaxEqc2, // EquivalentClasses (2)
    CaxDw,   // DisjointClasses

    // Property axiom rules
    PrpDom,  // Domain
    PrpRng,  // Range
    PrpFp,   // FunctionalProperty
    PrpIfp,  // InverseFunctionalProperty
    PrpIrp,  // IrreflexiveProperty
    PrpSymp, // SymmetricProperty
    PrpAsp,  // AsymmetricProperty
    PrpTrp,  // TransitiveProperty
    PrpSpo1, // SubPropertyOf
    PrpSpo2, // PropertyChainAxiom
    PrpEqp1, // EquivalentProperties (1)
    PrpEqp2, // EquivalentProperties (2)
    PrpPdw,  // DisjointProperties
    PrpInv1, // InverseOf (1)
    PrpInv2, // InverseOf (2)

    // Class expression rules
    ClsInt1, // IntersectionOf (1)
    ClsInt2, // IntersectionOf (2)
    ClsUni,  // UnionOf
    ClsCom,  // ComplementOf
    ClsSvf1, // SomeValuesFrom (1)
    ClsSvf2, // SomeValuesFrom (2)
    ClsAvf,  // AllValuesFrom
    ClsHv1,  // HasValue (1)
    ClsHv2,  // HasValue (2)
    ClsOo,   // OneOf
    ClsMaxc1,// MaxCardinality (1)
    ClsMaxc2,// MaxCardinality (2)
    ClsMaxqc1,// MaxQualifiedCardinality (1)
    ClsMaxqc2,// MaxQualifiedCardinality (2)
    ClsMaxqc3,// MaxQualifiedCardinality (3)
    ClsMaxqc4,// MaxQualifiedCardinality (4)

    // Equality rules
    EqRef,   // Reflexivity of =
    EqSym,   // Symmetry of =
    EqTrans, // Transitivity of =
    EqRep,   // Replacement
    EqDiff1, // Different => not same
    EqDiff2, // Different => not same (2)
    EqDiff3, // Different => not same (3)
}

impl RlRule {
    /// Returns all OWL 2 RL rules.
    pub fn all() -> &'static [RlRule] {
        &[
            RlRule::CaxSco, RlRule::CaxEqc1, RlRule::CaxEqc2, RlRule::CaxDw,
            RlRule::PrpDom, RlRule::PrpRng, RlRule::PrpFp, RlRule::PrpIfp,
            RlRule::PrpIrp, RlRule::PrpSymp, RlRule::PrpAsp, RlRule::PrpTrp,
            RlRule::PrpSpo1, RlRule::PrpSpo2, RlRule::PrpEqp1, RlRule::PrpEqp2,
            RlRule::PrpPdw, RlRule::PrpInv1, RlRule::PrpInv2,
            RlRule::ClsInt1, RlRule::ClsInt2, RlRule::ClsUni, RlRule::ClsCom,
            RlRule::ClsSvf1, RlRule::ClsSvf2, RlRule::ClsAvf, RlRule::ClsHv1,
            RlRule::ClsHv2, RlRule::ClsOo, RlRule::ClsMaxc1, RlRule::ClsMaxc2,
            RlRule::ClsMaxqc1, RlRule::ClsMaxqc2, RlRule::ClsMaxqc3, RlRule::ClsMaxqc4,
            RlRule::EqRef, RlRule::EqSym, RlRule::EqTrans, RlRule::EqRep,
            RlRule::EqDiff1, RlRule::EqDiff2, RlRule::EqDiff3,
        ]
    }

    #[expect(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            RlRule::CaxSco => "cax-sco",
            RlRule::CaxEqc1 => "cax-eqc1",
            RlRule::CaxEqc2 => "cax-eqc2",
            RlRule::CaxDw => "cax-dw",
            RlRule::PrpDom => "prp-dom",
            RlRule::PrpRng => "prp-rng",
            RlRule::PrpFp => "prp-fp",
            RlRule::PrpIfp => "prp-ifp",
            RlRule::PrpIrp => "prp-irp",
            RlRule::PrpSymp => "prp-symp",
            RlRule::PrpAsp => "prp-asp",
            RlRule::PrpTrp => "prp-trp",
            RlRule::PrpSpo1 => "prp-spo1",
            RlRule::PrpSpo2 => "prp-spo2",
            RlRule::PrpEqp1 => "prp-eqp1",
            RlRule::PrpEqp2 => "prp-eqp2",
            RlRule::PrpPdw => "prp-pdw",
            RlRule::PrpInv1 => "prp-inv1",
            RlRule::PrpInv2 => "prp-inv2",
            RlRule::ClsInt1 => "cls-int1",
            RlRule::ClsInt2 => "cls-int2",
            RlRule::ClsUni => "cls-uni",
            RlRule::ClsCom => "cls-com",
            RlRule::ClsSvf1 => "cls-svf1",
            RlRule::ClsSvf2 => "cls-svf2",
            RlRule::ClsAvf => "cls-avf",
            RlRule::ClsHv1 => "cls-hv1",
            RlRule::ClsHv2 => "cls-hv2",
            RlRule::ClsOo => "cls-oo",
            RlRule::ClsMaxc1 => "cls-maxc1",
            RlRule::ClsMaxc2 => "cls-maxc2",
            RlRule::ClsMaxqc1 => "cls-maxqc1",
            RlRule::ClsMaxqc2 => "cls-maxqc2",
            RlRule::ClsMaxqc3 => "cls-maxqc3",
            RlRule::ClsMaxqc4 => "cls-maxqc4",
            RlRule::EqRef => "eq-ref",
            RlRule::EqSym => "eq-sym",
            RlRule::EqTrans => "eq-trans",
            RlRule::EqRep => "eq-rep",
            RlRule::EqDiff1 => "eq-diff1",
            RlRule::EqDiff2 => "eq-diff2",
            RlRule::EqDiff3 => "eq-diff3",
        }
    }
}
