//! OWL 2 RL reasoning engine.
//!
//! This module implements forward-chaining reasoning based on the OWL 2 RL profile.
//! OWL 2 RL is a polynomial-time decidable profile suitable for rule-based reasoning.

mod rules;

use crate::axiom::Axiom;
use crate::entity::{OwlClass, ObjectProperty, Individual};
use crate::expression::ClassExpression;
use crate::ontology::Ontology;
use crate::error::{OwlError, InconsistencyError};
use rustc_hash::{FxHashMap, FxHashSet};

/// Configuration for the reasoner.
#[derive(Debug, Clone)]
pub struct ReasonerConfig {
    /// Maximum number of iterations for fixpoint computation.
    pub max_iterations: usize,
    /// Whether to check for inconsistencies.
    pub check_consistency: bool,
    /// Whether to materialize inferred axioms.
    pub materialize: bool,
}

impl Default for ReasonerConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100_000,
            check_consistency: true,
            materialize: true,
        }
    }
}

/// Trait for OWL reasoners.
pub trait Reasoner {
    /// Classifies the ontology (computes all subsumption relationships).
    fn classify(&mut self) -> Result<(), OwlError>;

    /// Checks if the ontology is consistent.
    fn is_consistent(&self) -> Result<bool, OwlError>;

    /// Returns all inferred types for an individual.
    fn get_types(&self, individual: &Individual) -> Vec<&OwlClass>;

    /// Returns all subclasses of a class (including indirect).
    fn get_sub_classes(&self, class: &OwlClass, direct: bool) -> Vec<&OwlClass>;

    /// Returns all superclasses of a class (including indirect).
    fn get_super_classes(&self, class: &OwlClass, direct: bool) -> Vec<&OwlClass>;

    /// Returns all equivalent classes.
    fn get_equivalent_classes(&self, class: &OwlClass) -> Vec<&OwlClass>;

    /// Returns all instances of a class (including via subclass reasoning).
    fn get_instances(&self, class: &OwlClass, direct: bool) -> Vec<&Individual>;

    /// Returns all inferred axioms.
    fn get_inferred_axioms(&self) -> &[Axiom];
}

/// OWL 2 RL forward-chaining reasoner.
#[derive(Debug)]
pub struct RlReasoner<'a> {
    /// Reference to the source ontology
    ontology: &'a Ontology,

    /// Configuration
    config: ReasonerConfig,

    /// Inferred class hierarchy: subclass -> set of superclasses
    class_hierarchy: FxHashMap<OwlClass, FxHashSet<OwlClass>>,

    /// Inferred individual types: individual -> set of classes
    individual_types: FxHashMap<Individual, FxHashSet<OwlClass>>,

    /// Inferred property values: (subject, property) -> set of objects
    property_values: FxHashMap<(Individual, ObjectProperty), FxHashSet<Individual>>,

    /// Same-as equivalence classes
    same_as: FxHashMap<Individual, FxHashSet<Individual>>,

    /// Different-from pairs
    different_from: FxHashSet<(Individual, Individual)>,

    /// Inferred axioms
    inferred_axioms: Vec<Axiom>,

    /// Whether classification has been performed
    classified: bool,

    /// Whether inconsistency was detected
    inconsistent: Option<InconsistencyError>,
}

impl<'a> RlReasoner<'a> {
    /// Creates a new RL reasoner for the given ontology.
    pub fn new(ontology: &'a Ontology) -> Self {
        Self::with_config(ontology, ReasonerConfig::default())
    }

    /// Creates a new RL reasoner with custom configuration.
    pub fn with_config(ontology: &'a Ontology, config: ReasonerConfig) -> Self {
        Self {
            ontology,
            config,
            class_hierarchy: FxHashMap::default(),
            individual_types: FxHashMap::default(),
            property_values: FxHashMap::default(),
            same_as: FxHashMap::default(),
            different_from: FxHashSet::default(),
            inferred_axioms: Vec::new(),
            classified: false,
            inconsistent: None,
        }
    }

    /// Initializes the reasoner state from ontology axioms.
    fn initialize(&mut self) {
        // Initialize class hierarchy from SubClassOf axioms
        for axiom in self.ontology.axioms() {
            match axiom {
                Axiom::SubClassOf { sub_class, super_class } => {
                    if let (ClassExpression::Class(sub), ClassExpression::Class(sup)) = (sub_class, super_class) {
                        self.class_hierarchy
                            .entry(sub.clone())
                            .or_default()
                            .insert(sup.clone());
                    }
                }
                Axiom::EquivalentClasses(classes) => {
                    // Equivalent classes are mutual subclasses
                    let named_classes: Vec<_> = classes.iter()
                        .filter_map(|c| c.as_class())
                        .cloned()
                        .collect();
                    for i in 0..named_classes.len() {
                        for j in 0..named_classes.len() {
                            if i != j {
                                self.class_hierarchy
                                    .entry(named_classes[i].clone())
                                    .or_default()
                                    .insert(named_classes[j].clone());
                            }
                        }
                    }
                }
                Axiom::ClassAssertion { class, individual } => {
                    if let ClassExpression::Class(c) = class {
                        self.individual_types
                            .entry(individual.clone())
                            .or_default()
                            .insert(c.clone());
                    }
                }
                Axiom::ObjectPropertyAssertion { property, source, target } => {
                    self.property_values
                        .entry((source.clone(), property.clone()))
                        .or_default()
                        .insert(target.clone());
                }
                Axiom::SameIndividual(individuals) => {
                    for i in 0..individuals.len() {
                        for j in 0..individuals.len() {
                            if i != j {
                                self.same_as
                                    .entry(individuals[i].clone())
                                    .or_default()
                                    .insert(individuals[j].clone());
                            }
                        }
                    }
                }
                Axiom::DifferentIndividuals(individuals) => {
                    for i in 0..individuals.len() {
                        for j in (i + 1)..individuals.len() {
                            self.different_from.insert((individuals[i].clone(), individuals[j].clone()));
                            self.different_from.insert((individuals[j].clone(), individuals[i].clone()));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Computes the transitive closure of the class hierarchy.
    fn compute_transitive_closure(&mut self) {
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            let classes: Vec<_> = self.class_hierarchy.keys().cloned().collect();

            for class in classes {
                if let Some(supers) = self.class_hierarchy.get(&class).cloned() {
                    for sup in supers {
                        if let Some(transitive_supers) = self.class_hierarchy.get(&sup).cloned() {
                            let entry = self.class_hierarchy.entry(class.clone()).or_default();
                            for trans_sup in transitive_supers {
                                if entry.insert(trans_sup) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Propagates types to individuals based on class hierarchy.
    fn propagate_types(&mut self) {
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            let individuals: Vec<_> = self.individual_types.keys().cloned().collect();

            for individual in individuals {
                if let Some(types) = self.individual_types.get(&individual).cloned() {
                    for typ in types {
                        // Add all superclasses as types
                        if let Some(supers) = self.class_hierarchy.get(&typ).cloned() {
                            let entry = self.individual_types.entry(individual.clone()).or_default();
                            for sup in supers {
                                if entry.insert(sup) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Checks for inconsistencies.
    fn check_consistency(&mut self) -> Result<(), InconsistencyError> {
        // Check if any individual is both same-as and different-from another
        for (a, b) in &self.different_from {
            if let Some(same) = self.same_as.get(a) {
                if same.contains(b) {
                    return Err(InconsistencyError::new(
                        format!("{} is both sameAs and differentFrom {}", a, b)
                    ));
                }
            }
        }

        // Check for instances of owl:Nothing (unsatisfiable)
        // This would be detected if we had disjoint classes and an individual
        // in both classes

        Ok(())
    }

    /// Generates inferred axioms from the reasoning results.
    fn generate_inferred_axioms(&mut self) {
        if !self.config.materialize {
            return;
        }

        // Generate SubClassOf axioms from transitive closure
        for (sub, supers) in &self.class_hierarchy {
            for sup in supers {
                // Only add if not already in ontology
                let axiom = Axiom::SubClassOf {
                    sub_class: ClassExpression::Class(sub.clone()),
                    super_class: ClassExpression::Class(sup.clone()),
                };
                self.inferred_axioms.push(axiom);
            }
        }

        // Generate ClassAssertion axioms from type propagation
        for (individual, types) in &self.individual_types {
            for typ in types {
                let axiom = Axiom::ClassAssertion {
                    class: ClassExpression::Class(typ.clone()),
                    individual: individual.clone(),
                };
                self.inferred_axioms.push(axiom);
            }
        }
    }
}

impl<'a> Reasoner for RlReasoner<'a> {
    fn classify(&mut self) -> Result<(), OwlError> {
        if self.classified {
            return Ok(());
        }

        // Step 1: Initialize from ontology axioms
        self.initialize();

        // Step 2: Compute transitive closure of class hierarchy
        self.compute_transitive_closure();

        // Step 3: Propagate types to individuals
        self.propagate_types();

        // Step 4: Check consistency if configured
        if self.config.check_consistency {
            if let Err(e) = self.check_consistency() {
                self.inconsistent = Some(e.clone());
                return Err(OwlError::Inconsistent(e));
            }
        }

        // Step 5: Generate inferred axioms
        self.generate_inferred_axioms();

        self.classified = true;
        Ok(())
    }

    fn is_consistent(&self) -> Result<bool, OwlError> {
        Ok(self.inconsistent.is_none())
    }

    fn get_types(&self, individual: &Individual) -> Vec<&OwlClass> {
        self.individual_types
            .get(individual)
            .map(|types| types.iter().collect())
            .unwrap_or_default()
    }

    fn get_sub_classes(&self, class: &OwlClass, direct: bool) -> Vec<&OwlClass> {
        let mut result = Vec::new();

        for (sub, supers) in &self.class_hierarchy {
            if supers.contains(class) {
                if direct {
                    // Check if there's no intermediate class
                    let has_intermediate = supers.iter().any(|s| {
                        s != class && self.class_hierarchy.get(s).map(|ss| ss.contains(class)).unwrap_or(false)
                    });
                    if !has_intermediate {
                        result.push(sub);
                    }
                } else {
                    result.push(sub);
                }
            }
        }

        result
    }

    fn get_super_classes(&self, class: &OwlClass, direct: bool) -> Vec<&OwlClass> {
        if direct {
            // Get direct superclasses (no intermediate)
            self.class_hierarchy
                .get(class)
                .map(|supers| {
                    supers.iter().filter(|&sup| {
                        // Check if there's no intermediate class
                        !supers.iter().any(|s| {
                            s != sup && self.class_hierarchy.get(s).map(|ss| ss.contains(sup)).unwrap_or(false)
                        })
                    }).collect()
                })
                .unwrap_or_default()
        } else {
            self.class_hierarchy
                .get(class)
                .map(|supers| supers.iter().collect())
                .unwrap_or_default()
        }
    }

    fn get_equivalent_classes(&self, class: &OwlClass) -> Vec<&OwlClass> {
        let mut result = Vec::new();

        if let Some(supers) = self.class_hierarchy.get(class) {
            for sup in supers {
                if let Some(their_supers) = self.class_hierarchy.get(sup) {
                    if their_supers.contains(class) {
                        result.push(sup);
                    }
                }
            }
        }

        result
    }

    fn get_instances(&self, class: &OwlClass, direct: bool) -> Vec<&Individual> {
        let mut result = Vec::new();

        for (individual, types) in &self.individual_types {
            if types.contains(class) {
                if direct {
                    // Check if the class is the most specific type
                    let has_more_specific = types.iter().any(|t| {
                        t != class && self.class_hierarchy.get(t).map(|s| s.contains(class)).unwrap_or(false)
                    });
                    if !has_more_specific {
                        result.push(individual);
                    }
                } else {
                    result.push(individual);
                }
            }
        }

        result
    }

    fn get_inferred_axioms(&self) -> &[Axiom] {
        &self.inferred_axioms
    }
}

impl<'a> std::fmt::Display for RlReasoner<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RlReasoner(classified={}, classes={}, individuals={}, inferred={})",
            self.classified,
            self.class_hierarchy.len(),
            self.individual_types.len(),
            self.inferred_axioms.len()
        )
    }
}
