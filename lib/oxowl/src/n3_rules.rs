//! N3 rule-based reasoning support for OWL ontologies.
//!
//! This module provides basic support for N3 logical rules in the context
//! of OWL reasoning. N3 extends RDF with formulas and logical implication,
//! allowing for rule-based inference.

use crate::axiom::Axiom;
use crate::entity::{Individual, ObjectProperty, OwlClass};
use crate::expression::ClassExpression;
use crate::ontology::Ontology;
use oxrdf::{BlankNode, Formula, NamedNode, Quad, Term, Triple};
use oxrdf::vocab::rdf;
use rustc_hash::FxHashSet;
use std::collections::HashMap;

/// Represents an N3 logical rule.
///
/// N3 rules have the form: { antecedent } => { consequent }
/// where both antecedent and consequent are formulas (sets of triples).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct N3Rule {
    /// The antecedent (if-part) of the rule
    pub antecedent: Formula,
    /// The consequent (then-part) of the rule
    pub consequent: Formula,
}

impl N3Rule {
    /// Creates a new N3 rule.
    pub fn new(antecedent: Formula, consequent: Formula) -> Self {
        Self {
            antecedent,
            consequent,
        }
    }

    /// Checks if this rule can contribute to OWL reasoning.
    ///
    /// Returns true if the rule expresses patterns that can be
    /// converted to OWL axioms (e.g., subclass relationships,
    /// property assertions).
    pub fn is_owl_compatible(&self) -> bool {
        // Check if the rule expresses simple class hierarchy patterns
        // For example: { ?x a :Dog } => { ?x a :Animal }
        self.is_subclass_pattern() || self.is_property_implication()
    }

    /// Checks if the rule represents a subclass relationship.
    ///
    /// Pattern: { ?x rdf:type ClassA } => { ?x rdf:type ClassB }
    /// This implies ClassA rdfs:subClassOf ClassB
    pub fn is_subclass_pattern(&self) -> bool {
        if self.antecedent.triples().len() != 1 || self.consequent.triples().len() != 1 {
            return false;
        }

        let ant_triple = &self.antecedent.triples()[0];
        let cons_triple = &self.consequent.triples()[0];

        // Both should be type assertions with the same subject variable
        ant_triple.predicate.as_ref() == rdf::TYPE
            && cons_triple.predicate.as_ref() == rdf::TYPE
            && subjects_match(&ant_triple.subject, &cons_triple.subject)
    }

    /// Checks if the rule represents a property implication.
    fn is_property_implication(&self) -> bool {
        // Pattern: { ?x property1 ?y } => { ?x property2 ?y }
        // This could represent property hierarchy or characteristics
        if self.antecedent.triples().len() != 1 || self.consequent.triples().len() != 1 {
            return false;
        }

        let ant_triple = &self.antecedent.triples()[0];
        let cons_triple = &self.consequent.triples()[0];

        // Subject and object should match (same variables)
        subjects_match(&ant_triple.subject, &cons_triple.subject)
            && objects_match(&ant_triple.object, &cons_triple.object)
    }

    /// Converts this N3 rule to OWL axioms if possible.
    pub fn to_owl_axioms(&self) -> Vec<Axiom> {
        let mut axioms = Vec::new();

        if self.is_subclass_pattern() {
            if let Some(axiom) = self.extract_subclass_axiom() {
                axioms.push(axiom);
            }
        }

        axioms
    }

    /// Extracts a SubClassOf axiom from a subclass pattern rule.
    fn extract_subclass_axiom(&self) -> Option<Axiom> {
        let ant_triple = &self.antecedent.triples()[0];
        let cons_triple = &self.consequent.triples()[0];

        // Extract the class IRIs from the object positions
        let sub_class = match &ant_triple.object {
            Term::NamedNode(n) => OwlClass::new(n.clone()),
            _ => return None,
        };

        let super_class = match &cons_triple.object {
            Term::NamedNode(n) => OwlClass::new(n.clone()),
            _ => return None,
        };

        Some(Axiom::SubClassOf {
            sub_class: ClassExpression::Class(sub_class),
            super_class: ClassExpression::Class(super_class),
        })
    }
}

/// N3 rule extractor that finds logical rules in RDF graphs.
pub struct N3RuleExtractor {
    /// The RDF quads to extract rules from
    quads: Vec<Quad>,
}

impl N3RuleExtractor {
    /// Creates a new rule extractor for the given quads.
    pub fn new(quads: Vec<Quad>) -> Self {
        Self { quads }
    }

    /// Extracts all N3 logical rules from the quads.
    ///
    /// N3 rules are represented using the log:implies predicate:
    /// { antecedent } log:implies { consequent }
    pub fn extract_rules(&self) -> Vec<N3Rule> {
        let mut rules = Vec::new();

        // N3 implication predicate
        let implies = NamedNode::new("http://www.w3.org/2000/10/swap/log#implies");
        if implies.is_err() {
            return rules;
        }
        let implies = implies.unwrap();

        // Find all formulas first
        let formulas = self.extract_formulas();
        let formula_map: HashMap<BlankNode, &Formula> = formulas
            .iter()
            .map(|f| (f.id().clone(), f))
            .collect();

        // Look for implication statements
        for quad in &self.quads {
            if quad.predicate == implies {
                // Subject and object should be blank nodes representing formulas
                if let oxrdf::Subject::BlankNode(ant_id) = &quad.subject {
                    if let Term::BlankNode(cons_id) = &quad.object {
                        if let (Some(&ant_formula), Some(&cons_formula)) =
                            (formula_map.get(ant_id), formula_map.get(cons_id))
                        {
                            rules.push(N3Rule::new(
                                ant_formula.clone(),
                                cons_formula.clone(),
                            ));
                        }
                    }
                }
            }
        }

        rules
    }

    /// Extracts all formulas from the quads.
    fn extract_formulas(&self) -> Vec<Formula> {
        crate::n3_integration::formulas::extract_formulas(&self.quads)
    }

    /// Converts extracted rules to OWL axioms.
    pub fn rules_to_axioms(&self, rules: &[N3Rule]) -> Vec<Axiom> {
        rules
            .iter()
            .flat_map(|rule| rule.to_owl_axioms())
            .collect()
    }
}

/// Extends an ontology with axioms derived from N3 rules.
///
/// This analyzes N3 logical rules in the source data and adds
/// corresponding OWL axioms to the ontology where possible.
pub fn extend_ontology_with_n3_rules(
    ontology: &mut Ontology,
    quads: &[Quad],
) -> usize {
    let extractor = N3RuleExtractor::new(quads.to_vec());
    let rules = extractor.extract_rules();
    let axioms = extractor.rules_to_axioms(&rules);

    let count = axioms.len();
    for axiom in axioms {
        ontology.add_axiom(axiom);
    }

    count
}

/// Helper function to check if two subjects match (typically both variables or both the same IRI).
fn subjects_match(s1: &oxrdf::Subject, s2: &oxrdf::Subject) -> bool {
    // For now, we check if they're both blank nodes (representing variables in formulas)
    // or if they're the same named node
    match (s1, s2) {
        (oxrdf::Subject::BlankNode(_), oxrdf::Subject::BlankNode(_)) => true,
        (oxrdf::Subject::NamedNode(n1), oxrdf::Subject::NamedNode(n2)) => n1 == n2,
        _ => false,
    }
}

/// Helper function to check if two objects match.
fn objects_match(o1: &Term, o2: &Term) -> bool {
    match (o1, o2) {
        (Term::BlankNode(_), Term::BlankNode(_)) => true,
        (Term::NamedNode(n1), Term::NamedNode(n2)) => n1 == n2,
        (Term::Literal(l1), Term::Literal(l2)) => l1 == l2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{BlankNode, GraphName, NamedNode, Subject, Triple};

    #[test]
    fn test_n3_rule_creation() {
        let ant_formula = Formula::new(BlankNode::default(), vec![]);
        let cons_formula = Formula::new(BlankNode::default(), vec![]);

        let rule = N3Rule::new(ant_formula.clone(), cons_formula.clone());
        assert_eq!(rule.antecedent, ant_formula);
        assert_eq!(rule.consequent, cons_formula);
    }

    #[test]
    fn test_subclass_pattern_detection() {
        let dog = NamedNode::new("http://example.org/Dog").unwrap();
        let animal = NamedNode::new("http://example.org/Animal").unwrap();
        let var = BlankNode::new("x").unwrap();

        let ant_triple = Triple::new(var.clone(), rdf::TYPE, dog);
        let cons_triple = Triple::new(var, rdf::TYPE, animal);

        let ant_formula = Formula::new(BlankNode::default(), vec![ant_triple]);
        let cons_formula = Formula::new(BlankNode::default(), vec![cons_triple]);

        let rule = N3Rule::new(ant_formula, cons_formula);
        assert!(rule.is_subclass_pattern());
        assert!(rule.is_owl_compatible());
    }

    #[test]
    fn test_extract_subclass_axiom() {
        let dog = NamedNode::new("http://example.org/Dog").unwrap();
        let animal = NamedNode::new("http://example.org/Animal").unwrap();
        let var = BlankNode::new("x").unwrap();

        let ant_triple = Triple::new(var.clone(), rdf::TYPE, dog.clone());
        let cons_triple = Triple::new(var, rdf::TYPE, animal.clone());

        let ant_formula = Formula::new(BlankNode::default(), vec![ant_triple]);
        let cons_formula = Formula::new(BlankNode::default(), vec![cons_triple]);

        let rule = N3Rule::new(ant_formula, cons_formula);
        let axioms = rule.to_owl_axioms();

        assert_eq!(axioms.len(), 1);
        match &axioms[0] {
            Axiom::SubClassOf { sub_class, super_class } => {
                assert!(matches!(sub_class, ClassExpression::Class(c) if c.iri() == &dog));
                assert!(matches!(super_class, ClassExpression::Class(c) if c.iri() == &animal));
            }
            _ => panic!("Expected SubClassOf axiom"),
        }
    }

    #[test]
    fn test_rule_extractor_empty() {
        let extractor = N3RuleExtractor::new(vec![]);
        let rules = extractor.extract_rules();
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn test_extend_ontology_with_n3_rules() {
        let mut ontology = Ontology::new(None);
        let quads = vec![];

        let count = extend_ontology_with_n3_rules(&mut ontology, &quads);
        assert_eq!(count, 0); // No rules to extract from empty quads
    }

    #[test]
    fn test_subjects_match() {
        let bn1 = BlankNode::new("x").unwrap();
        let bn2 = BlankNode::new("y").unwrap();
        let nn1 = NamedNode::new("http://example.org/test").unwrap();
        let nn2 = NamedNode::new("http://example.org/test").unwrap();
        let nn3 = NamedNode::new("http://example.org/other").unwrap();

        assert!(subjects_match(
            &Subject::BlankNode(bn1.clone()),
            &Subject::BlankNode(bn2)
        ));
        assert!(subjects_match(
            &Subject::NamedNode(nn1.clone()),
            &Subject::NamedNode(nn2)
        ));
        assert!(!subjects_match(
            &Subject::NamedNode(nn1),
            &Subject::NamedNode(nn3)
        ));
    }

    #[test]
    fn test_objects_match() {
        let bn1 = BlankNode::new("x").unwrap();
        let bn2 = BlankNode::new("y").unwrap();
        let nn1 = NamedNode::new("http://example.org/test").unwrap();
        let nn2 = NamedNode::new("http://example.org/test").unwrap();

        assert!(objects_match(&Term::BlankNode(bn1), &Term::BlankNode(bn2)));
        assert!(objects_match(&Term::NamedNode(nn1.clone()), &Term::NamedNode(nn2)));
    }
}
