//! Integration tests for the OWL 2 RL rule engine.
//!
//! M1 rules (cax-sco, prp-dom, prp-rng, prp-spo1, prp-trp) and M2 rules
//! (prp-symp, prp-inv1, prp-inv2, prp-eqp1, prp-eqp2, cax-eqc1, cax-eqc2)
//! assert the inferred triples directly. Equality rules (eq-sym, eq-trans,
//! eq-rep-s, eq-rep-p, eq-rep-o) are gated behind
//! `ReasonerConfig::with_equality_rules` and tested in both on and off
//! configurations. The functional property rules (prp-fp, prp-ifp) sit
//! behind the same flag.
//!
//! The cax-dw disjointness rule is an inconsistency detector, so it surfaces
//! as a `ReasonError::Inconsistent` rather than a materialised triple.
//!
//! TTL fixtures for each rule live in `tests/fixtures/`. The fixtures are
//! not parsed today because the tests build their graphs programmatically
//! to keep dev dependencies empty, but they document what each rule is
//! exercised against.

#![cfg(test)]
#![expect(
    clippy::expect_used,
    clippy::let_underscore_untyped,
    clippy::panic,
    reason = "integration tests assert rule results; some tests discard reports and the inconsistency test panics on an unexpected error variant"
)]

use oxrdf::vocab::{rdf, rdfs};
use oxrdf::{BlankNode, Graph, NamedNode, Triple};
use oxreason::{ReasonError, Reasoner, ReasonerConfig};

fn iri(s: &str) -> NamedNode {
    NamedNode::new_unchecked(s)
}

fn owl(local: &str) -> NamedNode {
    iri(&format!("http://www.w3.org/2002/07/owl#{local}"))
}

fn ex(local: &str) -> NamedNode {
    iri(&format!("https://example.org/ontology#{local}"))
}

fn expand(graph: &mut Graph) -> oxreason::ReasoningReport {
    Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(graph)
        .expect("M1 rules must not return NotImplemented")
}

#[test]
fn prp_trp_transitive_property_closure() {
    // Fixture: tests/fixtures/prp_trp.ttl
    let mut g = Graph::default();
    let has_bo = ex("hasBeneficialOwner");

    g.insert(&Triple::new(has_bo.clone(), rdf::TYPE, owl("TransitiveProperty")));
    g.insert(&Triple::new(ex("VesselA"), has_bo.clone(), ex("ShellCo")));
    g.insert(&Triple::new(ex("ShellCo"), has_bo.clone(), ex("Parent")));
    g.insert(&Triple::new(ex("Parent"), has_bo.clone(), ex("UltimateOwner")));

    let report = expand(&mut g);
    assert!(report.added >= 3, "expected at least 3 inferred edges, got {}", report.added);

    assert!(g.contains(&Triple::new(ex("VesselA"), has_bo.clone(), ex("Parent"))));
    assert!(g.contains(&Triple::new(ex("VesselA"), has_bo.clone(), ex("UltimateOwner"))));
    assert!(g.contains(&Triple::new(ex("ShellCo"), has_bo, ex("UltimateOwner"))));
}

#[test]
fn cax_sco_subclass_of_transitivity() {
    // Fixture: tests/fixtures/cax_sco.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));
    g.insert(&Triple::new(ex("LegalPerson"), rdfs::SUB_CLASS_OF, ex("Entity")));
    g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("LegalPerson"))));
    assert!(g.contains(&Triple::new(ex("Acme"), rdf::TYPE, ex("Entity"))));
}

#[test]
fn prp_dom_domain_inference() {
    // Fixture: tests/fixtures/prp_dom.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("owns"), rdfs::DOMAIN, ex("Entity")));
    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("Entity"))));
}

#[test]
fn prp_rng_range_inference() {
    // Fixture: tests/fixtures/prp_rng.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("owns"), rdfs::RANGE, ex("Asset")));
    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Bike"), rdf::TYPE, ex("Asset"))));
}

#[test]
fn prp_spo1_subproperty_of_inference() {
    // Fixture: tests/fixtures/prp_spo1.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("hasParent"), rdfs::SUB_PROPERTY_OF, ex("hasAncestor")));
    g.insert(&Triple::new(ex("Alice"), ex("hasParent"), ex("Bob")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), ex("hasAncestor"), ex("Bob"))));
}

#[test]
fn beneficial_ownership_closure_across_subclasses() {
    // Combined pipeline test that stresses cax-sco plus prp-trp plus
    // prp-dom together over a beneficial ownership shaped graph: a chain
    // from a vessel to an ultimate owner, across class hierarchies.
    let mut g = Graph::default();
    let has_bo = ex("hasBeneficialOwner");

    g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));
    g.insert(&Triple::new(ex("LegalPerson"), rdfs::SUB_CLASS_OF, ex("Entity")));
    g.insert(&Triple::new(has_bo.clone(), rdf::TYPE, owl("TransitiveProperty")));
    g.insert(&Triple::new(has_bo.clone(), rdfs::DOMAIN, ex("Entity")));

    g.insert(&Triple::new(ex("VesselA"), has_bo.clone(), ex("ShellCo")));
    g.insert(&Triple::new(ex("ShellCo"), rdf::TYPE, ex("Company")));
    g.insert(&Triple::new(ex("ShellCo"), has_bo.clone(), ex("UltimateOwner")));
    g.insert(&Triple::new(ex("UltimateOwner"), rdf::TYPE, ex("Company")));

    let _ = expand(&mut g);

    // prp-trp pushes through the chain.
    assert!(g.contains(&Triple::new(ex("VesselA"), has_bo.clone(), ex("UltimateOwner"))));
    // prp-dom classifies the vessel as an Entity (subject of hasBeneficialOwner).
    assert!(g.contains(&Triple::new(ex("VesselA"), rdf::TYPE, ex("Entity"))));
    // cax-sco lifts ShellCo and UltimateOwner up the class hierarchy.
    assert!(g.contains(&Triple::new(ex("ShellCo"), rdf::TYPE, ex("LegalPerson"))));
    assert!(g.contains(&Triple::new(ex("ShellCo"), rdf::TYPE, ex("Entity"))));
    assert!(g.contains(&Triple::new(ex("UltimateOwner"), rdf::TYPE, ex("Entity"))));
}

// M2 rules.

#[test]
fn prp_symp_symmetric_property() {
    // Fixture: tests/fixtures/prp_symp.ttl
    let mut g = Graph::default();
    let married_to = ex("marriedTo");

    g.insert(&Triple::new(married_to.clone(), rdf::TYPE, owl("SymmetricProperty")));
    g.insert(&Triple::new(ex("Alice"), married_to.clone(), ex("Bob")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Bob"), married_to, ex("Alice"))));
}

#[test]
fn prp_inv1_and_inv2_materialise_both_directions() {
    // Fixture: tests/fixtures/prp_inv.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("owns"), owl("inverseOf"), ex("ownedBy")));
    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));
    g.insert(&Triple::new(ex("Shed"), ex("ownedBy"), ex("Carol")));

    let _ = expand(&mut g);

    // prp-inv1: forward direction on the inverseOf fact.
    assert!(g.contains(&Triple::new(ex("Bike"), ex("ownedBy"), ex("Alice"))));
    // prp-inv2: reverse direction on the same inverseOf fact.
    assert!(g.contains(&Triple::new(ex("Carol"), ex("owns"), ex("Shed"))));
}

#[test]
fn prp_eqp_bridges_equivalent_properties() {
    // Fixture: tests/fixtures/prp_eqp.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("hasOwner"),
        owl("equivalentProperty"),
        ex("owner"),
    ));
    g.insert(&Triple::new(ex("Bike"), ex("hasOwner"), ex("Alice")));
    g.insert(&Triple::new(ex("Car"), ex("owner"), ex("Bob")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Bike"), ex("owner"), ex("Alice"))));
    assert!(g.contains(&Triple::new(ex("Car"), ex("hasOwner"), ex("Bob"))));
}

#[test]
fn cax_eqc_bridges_equivalent_classes() {
    // Fixture: tests/fixtures/cax_eqc.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("Person"),
        owl("equivalentClass"),
        ex("Human"),
    ));
    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Person")));
    g.insert(&Triple::new(ex("Bob"), rdf::TYPE, ex("Human")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("Human"))));
    assert!(g.contains(&Triple::new(ex("Bob"), rdf::TYPE, ex("Person"))));
}

// Equality rules ship behind `ReasonerConfig::with_equality_rules`. These
// tests check both the off-by-default behaviour and the on path.

fn expand_with_equality(graph: &mut Graph) -> oxreason::ReasoningReport {
    Reasoner::new(ReasonerConfig::owl2_rl().with_equality_rules(true))
        .expand(graph)
        .expect("equality rules must not return NotImplemented")
}

#[test]
fn equality_rules_off_by_default() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Alice"), owl("sameAs"), ex("AliceDoe")));
    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

    let _ = expand(&mut g);

    assert!(!g.contains(&Triple::new(ex("AliceDoe"), ex("owns"), ex("Bike"))));
}

#[test]
fn eq_rules_close_sameas_and_rewrite_positions() {
    // Fixture: tests/fixtures/eq_rules.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("Alice"), owl("sameAs"), ex("AliceDoe")));
    g.insert(&Triple::new(ex("Bike"), owl("sameAs"), ex("Bicycle")));
    g.insert(&Triple::new(ex("owns"), owl("sameAs"), ex("possesses")));

    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

    let _ = expand_with_equality(&mut g);

    // eq-sym: sameAs is symmetric.
    assert!(g.contains(&Triple::new(ex("AliceDoe"), owl("sameAs"), ex("Alice"))));
    // eq-rep-s: subject rewrite.
    assert!(g.contains(&Triple::new(ex("AliceDoe"), ex("owns"), ex("Bike"))));
    // eq-rep-o: object rewrite.
    assert!(g.contains(&Triple::new(ex("Alice"), ex("owns"), ex("Bicycle"))));
    // eq-rep-p: predicate rewrite.
    assert!(g.contains(&Triple::new(ex("Alice"), ex("possesses"), ex("Bike"))));
}

#[test]
fn eq_trans_closes_chain_of_three() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("A"), owl("sameAs"), ex("B")));
    g.insert(&Triple::new(ex("B"), owl("sameAs"), ex("C")));

    let _ = expand_with_equality(&mut g);

    assert!(g.contains(&Triple::new(ex("A"), owl("sameAs"), ex("C"))));
}

// M3 schema rule tests. The ten scm-* rules close the graph under class and
// property hierarchy axioms without touching instance data.

#[test]
fn scm_cls_adds_reflexive_subclass_and_bounds() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Company"), rdf::TYPE, owl("Class")));

    let _ = expand(&mut g);

    // Reflexive subclass and equivalent class.
    assert!(g.contains(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("Company"))));
    assert!(g.contains(&Triple::new(
        ex("Company"),
        owl("equivalentClass"),
        ex("Company"),
    )));
    // Bounds.
    assert!(g.contains(&Triple::new(
        ex("Company"),
        rdfs::SUB_CLASS_OF,
        owl("Thing"),
    )));
    assert!(g.contains(&Triple::new(
        owl("Nothing"),
        rdfs::SUB_CLASS_OF,
        ex("Company"),
    )));
}

#[test]
fn scm_sco_transitively_closes_subclass_chain() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));
    g.insert(&Triple::new(ex("LegalPerson"), rdfs::SUB_CLASS_OF, ex("Entity")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("Entity"))));
}

#[test]
fn scm_op_marks_object_property_reflexive() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("owns"), rdf::TYPE, owl("ObjectProperty")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("owns"), rdfs::SUB_PROPERTY_OF, ex("owns"))));
    assert!(g.contains(&Triple::new(
        ex("owns"),
        owl("equivalentProperty"),
        ex("owns"),
    )));
}

#[test]
fn scm_dp_marks_datatype_property_reflexive() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("hasAge"), rdf::TYPE, owl("DatatypeProperty")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("hasAge"), rdfs::SUB_PROPERTY_OF, ex("hasAge"))));
    assert!(g.contains(&Triple::new(
        ex("hasAge"),
        owl("equivalentProperty"),
        ex("hasAge"),
    )));
}

#[test]
fn scm_eqc1_splits_equivalent_class_into_two_subclass_edges() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Person"), owl("equivalentClass"), ex("Human")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Person"), rdfs::SUB_CLASS_OF, ex("Human"))));
    assert!(g.contains(&Triple::new(ex("Human"), rdfs::SUB_CLASS_OF, ex("Person"))));
}

#[test]
fn scm_eqc2_joins_mutual_subclass_into_equivalent_class() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Person"), rdfs::SUB_CLASS_OF, ex("Human")));
    g.insert(&Triple::new(ex("Human"), rdfs::SUB_CLASS_OF, ex("Person")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Person"), owl("equivalentClass"), ex("Human"))));
}

#[test]
fn scm_eqp1_splits_equivalent_property_into_two_subproperty_edges() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("owns"), owl("equivalentProperty"), ex("possesses")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(
        ex("owns"),
        rdfs::SUB_PROPERTY_OF,
        ex("possesses"),
    )));
    assert!(g.contains(&Triple::new(
        ex("possesses"),
        rdfs::SUB_PROPERTY_OF,
        ex("owns"),
    )));
}

#[test]
fn scm_eqp2_joins_mutual_subproperty_into_equivalent_property() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("owns"), rdfs::SUB_PROPERTY_OF, ex("possesses")));
    g.insert(&Triple::new(ex("possesses"), rdfs::SUB_PROPERTY_OF, ex("owns")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(
        ex("owns"),
        owl("equivalentProperty"),
        ex("possesses"),
    )));
}

#[test]
fn scm_dom1_propagates_domain_up_subclass_chain() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("owns"), rdfs::DOMAIN, ex("Company")));
    g.insert(&Triple::new(ex("Company"), rdfs::SUB_CLASS_OF, ex("LegalPerson")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("owns"), rdfs::DOMAIN, ex("LegalPerson"))));
}

#[test]
fn scm_rng1_propagates_range_up_subclass_chain() {
    let mut g = Graph::default();
    g.insert(&Triple::new(ex("owns"), rdfs::RANGE, ex("Bike")));
    g.insert(&Triple::new(ex("Bike"), rdfs::SUB_CLASS_OF, ex("Asset")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("owns"), rdfs::RANGE, ex("Asset"))));
}

// Functional and inverse functional property rules. Both are gated behind
// `ReasonerConfig::with_equality_rules` because they emit owl:sameAs facts
// and only make sense alongside the eq-rep-* family.

#[test]
fn prp_fp_functional_property_bundles_duplicate_values() {
    // Fixture: tests/fixtures/prp_fp.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("hasFather"), rdf::TYPE, owl("FunctionalProperty")));
    g.insert(&Triple::new(ex("Alice"), ex("hasFather"), ex("Bob")));
    g.insert(&Triple::new(ex("Alice"), ex("hasFather"), ex("Robert")));

    let _ = expand_with_equality(&mut g);

    assert!(g.contains(&Triple::new(ex("Bob"), owl("sameAs"), ex("Robert"))));
}

#[test]
fn prp_fp_stays_dormant_without_equality_flag() {
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("hasFather"), rdf::TYPE, owl("FunctionalProperty")));
    g.insert(&Triple::new(ex("Alice"), ex("hasFather"), ex("Bob")));
    g.insert(&Triple::new(ex("Alice"), ex("hasFather"), ex("Robert")));

    let _ = expand(&mut g);

    assert!(!g.contains(&Triple::new(ex("Bob"), owl("sameAs"), ex("Robert"))));
}

#[test]
fn prp_ifp_inverse_functional_property_bundles_duplicate_subjects() {
    // Fixture: tests/fixtures/prp_ifp.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("hasEmail"),
        rdf::TYPE,
        owl("InverseFunctionalProperty"),
    ));
    g.insert(&Triple::new(ex("Alice"), ex("hasEmail"), ex("mail1")));
    g.insert(&Triple::new(ex("AliceDoe"), ex("hasEmail"), ex("mail1")));

    let _ = expand_with_equality(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), owl("sameAs"), ex("AliceDoe"))));
}

// cax-dw surfaces inconsistency as a `ReasonError::Inconsistent` rather than
// a materialised triple, so the assertion shape differs from the other rule
// tests.

#[test]
fn cax_dw_disjoint_classes_raise_inconsistent_error() {
    // Fixture: tests/fixtures/cax_dw.ttl
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("Person"), owl("disjointWith"), ex("Organisation")));
    g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Person")));
    g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Organisation")));

    let err = Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut g)
        .expect_err("disjoint types on the same individual must clash");

    match err {
        ReasonError::Inconsistent {
            individual,
            class_a,
            class_b,
        } => {
            // NamedNode/NamedOrBlankNode render as N-Triples (`<iri>`), so
            // match via `contains` rather than exact equality.
            assert!(individual.contains("Acme"), "unexpected individual {individual}");
            let classes = [class_a, class_b];
            assert!(classes.iter().any(|c| c.contains("Person")));
            assert!(classes.iter().any(|c| c.contains("Organisation")));
        }
        other => panic!("expected Inconsistent variant, got {other:?}"),
    }
}

#[test]
fn cax_dw_consistent_graph_saturates_normally() {
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("Person"), owl("disjointWith"), ex("Organisation")));
    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Person")));
    g.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Organisation")));

    // No clash on this graph, expansion succeeds.
    let _ = expand(&mut g);
}

#[test]
fn custom_profile_still_returns_not_implemented() {
    // Custom profile is reserved for caller supplied RuleSets. The engine
    // does not execute them yet; this test guards the error path.
    use oxreason::RuleSet;

    let mut g = Graph::default();
    g.insert(&Triple::new(ex("Alice"), ex("owns"), ex("Bike")));

    let reasoner = Reasoner::new(ReasonerConfig::custom(RuleSet::owl2_rl_core()));
    let err = reasoner.expand(&mut g).unwrap_err();
    assert!(matches!(err, ReasonError::NotImplemented(_)));
}

// ---------------------------------------------------------------------------
// M4 rule tests: scm-spo, cls-hv1, cls-hv2, cls-int1, cls-int2, cls-uni,
// plus four inconsistency detectors (cls-nothing2, prp-irp, prp-asyp,
// prp-pdw). The inconsistency detectors surface via
// `ReasonError::InconsistentAxiom` with a human readable message.

#[test]
fn scm_spo_closes_subproperty_chain() {
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("worksFor"), rdfs::SUB_PROPERTY_OF, ex("employedBy")));
    g.insert(&Triple::new(ex("employedBy"), rdfs::SUB_PROPERTY_OF, ex("relatedTo")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(
        ex("worksFor"),
        rdfs::SUB_PROPERTY_OF,
        ex("relatedTo"),
    )));
}

#[test]
fn cls_hv1_propagates_restriction_value_to_instances() {
    // Class c: owl:Restriction on owl:onProperty speaksLanguage with
    // owl:hasValue "Polish". Any instance of c must then speak Polish.
    let mut g = Graph::default();
    let c = BlankNode::default();

    g.insert(&Triple::new(c.clone(), owl("onProperty"), ex("speaksLanguage")));
    g.insert(&Triple::new(c.clone(), owl("hasValue"), ex("Polish")));
    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, c));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(
        ex("Alice"),
        ex("speaksLanguage"),
        ex("Polish"),
    )));
}

#[test]
fn cls_hv2_infers_restriction_membership_from_value() {
    // Instance has speaksLanguage Polish. Inverse direction of cls-hv1:
    // that instance must be a member of the restriction class.
    let mut g = Graph::default();
    let c = BlankNode::default();

    g.insert(&Triple::new(c.clone(), owl("onProperty"), ex("speaksLanguage")));
    g.insert(&Triple::new(c.clone(), owl("hasValue"), ex("Polish")));
    g.insert(&Triple::new(ex("Alice"), ex("speaksLanguage"), ex("Polish")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, c)));
}

#[test]
fn cls_int1_requires_all_members_to_type_instance() {
    // Polish citizen class defined as intersection of Citizen and
    // LivesInPoland. Anyone typed as both must be inferred as a PolishCitizen.
    let mut g = Graph::default();
    let list_head = BlankNode::default();
    let list_tail = BlankNode::default();

    g.insert(&Triple::new(ex("PolishCitizen"), owl("intersectionOf"), list_head.clone()));
    g.insert(&Triple::new(list_head.clone(), rdf::FIRST, ex("Citizen")));
    g.insert(&Triple::new(list_head, rdf::REST, list_tail.clone()));
    g.insert(&Triple::new(list_tail.clone(), rdf::FIRST, ex("LivesInPoland")));
    g.insert(&Triple::new(list_tail, rdf::REST, rdf::NIL));

    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Citizen")));
    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("LivesInPoland")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("PolishCitizen"))));
}

#[test]
fn cls_int1_requires_every_member_to_match() {
    // Only one of the two required types present; intersection must not fire.
    let mut g = Graph::default();
    let list_head = BlankNode::default();
    let list_tail = BlankNode::default();

    g.insert(&Triple::new(ex("PolishCitizen"), owl("intersectionOf"), list_head.clone()));
    g.insert(&Triple::new(list_head.clone(), rdf::FIRST, ex("Citizen")));
    g.insert(&Triple::new(list_head, rdf::REST, list_tail.clone()));
    g.insert(&Triple::new(list_tail.clone(), rdf::FIRST, ex("LivesInPoland")));
    g.insert(&Triple::new(list_tail, rdf::REST, rdf::NIL));

    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Citizen")));

    let _ = expand(&mut g);

    assert!(!g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("PolishCitizen"))));
}

#[test]
fn cls_int2_splits_intersection_membership_into_each_member() {
    let mut g = Graph::default();
    let list_head = BlankNode::default();
    let list_tail = BlankNode::default();

    g.insert(&Triple::new(ex("PolishCitizen"), owl("intersectionOf"), list_head.clone()));
    g.insert(&Triple::new(list_head.clone(), rdf::FIRST, ex("Citizen")));
    g.insert(&Triple::new(list_head, rdf::REST, list_tail.clone()));
    g.insert(&Triple::new(list_tail.clone(), rdf::FIRST, ex("LivesInPoland")));
    g.insert(&Triple::new(list_tail, rdf::REST, rdf::NIL));

    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("PolishCitizen")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("Citizen"))));
    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("LivesInPoland"))));
}

#[test]
fn cls_uni_membership_from_any_union_member() {
    let mut g = Graph::default();
    let list_head = BlankNode::default();
    let list_tail = BlankNode::default();

    g.insert(&Triple::new(ex("Adult"), owl("unionOf"), list_head.clone()));
    g.insert(&Triple::new(list_head.clone(), rdf::FIRST, ex("Student")));
    g.insert(&Triple::new(list_head, rdf::REST, list_tail.clone()));
    g.insert(&Triple::new(list_tail.clone(), rdf::FIRST, ex("Worker")));
    g.insert(&Triple::new(list_tail, rdf::REST, rdf::NIL));

    g.insert(&Triple::new(ex("Alice"), rdf::TYPE, ex("Student")));

    let _ = expand(&mut g);

    assert!(g.contains(&Triple::new(ex("Alice"), rdf::TYPE, ex("Adult"))));
}

#[test]
fn cls_nothing2_raises_inconsistent_axiom() {
    let mut g = Graph::default();

    g.insert(&Triple::new(ex("Acme"), rdf::TYPE, owl("Nothing")));

    let err = Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut g)
        .expect_err("owl:Nothing typing must clash");

    match err {
        ReasonError::InconsistentAxiom { message } => {
            assert!(message.contains("cls-nothing2"), "unexpected message: {message}");
            assert!(message.contains("Acme"), "unexpected message: {message}");
        }
        other => panic!("expected InconsistentAxiom, got {other:?}"),
    }
}

#[test]
fn prp_irp_raises_inconsistent_axiom_on_reflexive_edge() {
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("parentOf"),
        rdf::TYPE,
        owl("IrreflexiveProperty"),
    ));
    g.insert(&Triple::new(ex("Alice"), ex("parentOf"), ex("Alice")));

    let err = Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut g)
        .expect_err("reflexive edge on irreflexive property must clash");

    match err {
        ReasonError::InconsistentAxiom { message } => {
            assert!(message.contains("prp-irp"), "unexpected message: {message}");
        }
        other => panic!("expected InconsistentAxiom, got {other:?}"),
    }
}

#[test]
fn prp_asyp_raises_inconsistent_axiom_on_opposing_edges() {
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("parentOf"),
        rdf::TYPE,
        owl("AsymmetricProperty"),
    ));
    g.insert(&Triple::new(ex("Alice"), ex("parentOf"), ex("Bob")));
    g.insert(&Triple::new(ex("Bob"), ex("parentOf"), ex("Alice")));

    let err = Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut g)
        .expect_err("opposing edges on asymmetric property must clash");

    match err {
        ReasonError::InconsistentAxiom { message } => {
            assert!(message.contains("prp-asyp"), "unexpected message: {message}");
        }
        other => panic!("expected InconsistentAxiom, got {other:?}"),
    }
}

#[test]
fn prp_pdw_raises_inconsistent_axiom_on_disjoint_property_pair() {
    let mut g = Graph::default();

    g.insert(&Triple::new(
        ex("parentOf"),
        owl("propertyDisjointWith"),
        ex("childOf"),
    ));
    g.insert(&Triple::new(ex("Alice"), ex("parentOf"), ex("Bob")));
    g.insert(&Triple::new(ex("Alice"), ex("childOf"), ex("Bob")));

    let err = Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut g)
        .expect_err("two disjoint properties on the same pair must clash");

    match err {
        ReasonError::InconsistentAxiom { message } => {
            assert!(message.contains("prp-pdw"), "unexpected message: {message}");
        }
        other => panic!("expected InconsistentAxiom, got {other:?}"),
    }
}
