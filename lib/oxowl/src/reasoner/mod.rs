//! OWL 2 RL reasoning engine.
//!
//! This module implements forward-chaining reasoning based on the OWL 2 RL profile.
//! OWL 2 RL is a polynomial-time decidable profile suitable for rule-based reasoning.

mod rules;

use crate::axiom::Axiom;
use crate::entity::{Individual, ObjectProperty, OwlClass};
use crate::error::{InconsistencyError, OwlError};
use crate::expression::ClassExpression;
use crate::ontology::Ontology;
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::{Duration, Instant};

/// Configuration for the reasoner.
#[derive(Debug, Clone)]
pub struct ReasonerConfig {
    /// Maximum number of iterations for fixpoint computation.
    pub max_iterations: usize,
    /// Maximum time allowed for reasoning (None = unlimited).
    pub timeout: Option<Duration>,
    /// Maximum number of inferred triples to materialize (None = unlimited).
    pub max_inferred_triples: Option<usize>,
    /// Whether to check for inconsistencies.
    pub check_consistency: bool,
    /// Whether to materialize inferred axioms.
    pub materialize: bool,
}

impl Default for ReasonerConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100_000,
            timeout: None,
            max_inferred_triples: None,
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

    /// Inferred property hierarchy: subproperty -> set of superproperties
    property_hierarchy: FxHashMap<ObjectProperty, FxHashSet<ObjectProperty>>,

    /// Property domains: property -> set of domain classes
    property_domains: FxHashMap<ObjectProperty, FxHashSet<OwlClass>>,

    /// Property ranges: property -> set of range classes
    property_ranges: FxHashMap<ObjectProperty, FxHashSet<OwlClass>>,

    /// Inferred individual types: individual -> set of classes
    individual_types: FxHashMap<Individual, FxHashSet<OwlClass>>,

    /// Inferred property values: (subject, property) -> set of objects
    property_values: FxHashMap<(Individual, ObjectProperty), FxHashSet<Individual>>,

    /// Same-as equivalence classes
    same_as: FxHashMap<Individual, FxHashSet<Individual>>,

    /// Different-from pairs
    different_from: FxHashSet<(Individual, Individual)>,

    /// Symmetric properties
    symmetric_properties: FxHashSet<ObjectProperty>,

    /// Transitive properties
    transitive_properties: FxHashSet<ObjectProperty>,

    /// Inverse property mappings: property -> inverse property
    inverse_properties: FxHashMap<ObjectProperty, ObjectProperty>,

    /// Inferred axioms
    inferred_axioms: Vec<Axiom>,

    /// Whether classification has been performed
    classified: bool,

    /// Whether inconsistency was detected
    inconsistent: Option<InconsistencyError>,

    /// Start time for reasoning (used for timeout enforcement)
    start_time: Option<Instant>,
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
            property_hierarchy: FxHashMap::default(),
            property_domains: FxHashMap::default(),
            property_ranges: FxHashMap::default(),
            individual_types: FxHashMap::default(),
            property_values: FxHashMap::default(),
            same_as: FxHashMap::default(),
            different_from: FxHashSet::default(),
            symmetric_properties: FxHashSet::default(),
            transitive_properties: FxHashSet::default(),
            inverse_properties: FxHashMap::default(),
            inferred_axioms: Vec::new(),
            classified: false,
            inconsistent: None,
            start_time: None,
        }
    }

    /// Checks if timeout has been exceeded.
    fn check_timeout(&self) -> Result<(), OwlError> {
        if let (Some(timeout), Some(start)) = (self.config.timeout, self.start_time) {
            if start.elapsed() >= timeout {
                return Err(OwlError::Other(format!(
                    "Reasoning timeout exceeded ({:?})",
                    timeout
                )));
            }
        }
        Ok(())
    }

    /// Checks if materialization limit has been exceeded.
    fn check_materialization_limit(&self) -> Result<(), OwlError> {
        if let Some(limit) = self.config.max_inferred_triples {
            if self.inferred_axioms.len() >= limit {
                return Err(OwlError::Other(format!(
                    "Materialization limit exceeded ({} triples)",
                    limit
                )));
            }
        }
        Ok(())
    }

    /// Initializes the reasoner state from ontology axioms.
    fn initialize(&mut self) {
        // Initialize class hierarchy from SubClassOf axioms
        for axiom in self.ontology.axioms() {
            match axiom {
                Axiom::SubClassOf {
                    sub_class,
                    super_class,
                } => {
                    if let (ClassExpression::Class(sub), ClassExpression::Class(sup)) =
                        (sub_class, super_class)
                    {
                        self.class_hierarchy
                            .entry(sub.clone())
                            .or_default()
                            .insert(sup.clone());
                    }
                }
                Axiom::EquivalentClasses(classes) => {
                    // Equivalent classes are mutual subclasses
                    let named_classes: Vec<_> = classes
                        .iter()
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
                Axiom::ClassAssertion {
                    class: ClassExpression::Class(c),
                    individual,
                } => {
                    self.individual_types
                        .entry(individual.clone())
                        .or_default()
                        .insert(c.clone());
                }
                Axiom::ObjectPropertyAssertion {
                    property,
                    source,
                    target,
                } => {
                    self.property_values
                        .entry((source.clone(), property.clone()))
                        .or_default()
                        .insert(target.clone());
                }
                Axiom::SubObjectPropertyOf {
                    sub_property,
                    super_property,
                } => {
                    // Handle SubObjectPropertyOf for simple properties
                    let sub = sub_property.as_property();
                    let sup = super_property.as_property();
                    self.property_hierarchy
                        .entry(sub.clone())
                        .or_default()
                        .insert(sup.clone());
                }
                Axiom::ObjectPropertyDomain {
                    property,
                    domain: ClassExpression::Class(c),
                } => {
                    self.property_domains
                        .entry(property.clone())
                        .or_default()
                        .insert(c.clone());
                }
                Axiom::ObjectPropertyRange {
                    property,
                    range: ClassExpression::Class(c),
                } => {
                    self.property_ranges
                        .entry(property.clone())
                        .or_default()
                        .insert(c.clone());
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
                            self.different_from
                                .insert((individuals[i].clone(), individuals[j].clone()));
                            self.different_from
                                .insert((individuals[j].clone(), individuals[i].clone()));
                        }
                    }
                }
                Axiom::SymmetricObjectProperty(property) => {
                    self.symmetric_properties.insert(property.clone());
                }
                Axiom::TransitiveObjectProperty(property) => {
                    self.transitive_properties.insert(property.clone());
                }
                Axiom::InverseObjectProperties(p1, p2) => {
                    // Store bidirectional mapping
                    self.inverse_properties.insert(p1.clone(), p2.clone());
                    self.inverse_properties.insert(p2.clone(), p1.clone());
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

    /// Applies RDFS entailment rules.
    ///
    /// This method implements:
    /// - rdfs:subPropertyOf transitivity
    /// - rdfs:domain inference (if P has domain C and x P y, then x rdf:type C)
    /// - rdfs:range inference (if P has range C and x P y, then y rdf:type C)
    fn apply_rdfs_rules(&mut self) {
        // Step 1: Compute transitive closure of property hierarchy (rdfs:subPropertyOf transitivity)
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            let properties: Vec<_> = self.property_hierarchy.keys().cloned().collect();

            for property in properties {
                if let Some(supers) = self.property_hierarchy.get(&property).cloned() {
                    for sup in supers {
                        if let Some(transitive_supers) =
                            self.property_hierarchy.get(&sup).cloned()
                        {
                            let entry = self.property_hierarchy.entry(property.clone()).or_default();
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

        // Step 2: Propagate property domains to superproperties
        // If P rdfs:subPropertyOf Q and P rdfs:domain C, then Q rdfs:domain C
        changed = true;
        iterations = 0;

        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            let properties: Vec<_> = self.property_hierarchy.keys().cloned().collect();

            for property in properties {
                if let Some(domains) = self.property_domains.get(&property).cloned() {
                    if let Some(supers) = self.property_hierarchy.get(&property).cloned() {
                        for sup in supers {
                            let entry = self.property_domains.entry(sup).or_default();
                            for domain in &domains {
                                if entry.insert(domain.clone()) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 3: Propagate property ranges to superproperties
        // If P rdfs:subPropertyOf Q and P rdfs:range C, then Q rdfs:range C
        changed = true;
        iterations = 0;

        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            let properties: Vec<_> = self.property_hierarchy.keys().cloned().collect();

            for property in properties {
                if let Some(ranges) = self.property_ranges.get(&property).cloned() {
                    if let Some(supers) = self.property_hierarchy.get(&property).cloned() {
                        for sup in supers {
                            let entry = self.property_ranges.entry(sup).or_default();
                            for range in &ranges {
                                if entry.insert(range.clone()) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 4: Apply domain inference
        // If P rdfs:domain C and (x, P, y) exists, then x rdf:type C
        for (subject, property) in self.property_values.keys() {
            if let Some(domains) = self.property_domains.get(property) {
                let entry = self.individual_types.entry(subject.clone()).or_default();
                for domain in domains {
                    entry.insert(domain.clone());
                }
            }

            // Also check superproperties of this property
            if let Some(supers) = self.property_hierarchy.get(property) {
                for sup in supers {
                    if let Some(domains) = self.property_domains.get(sup) {
                        let entry = self.individual_types.entry(subject.clone()).or_default();
                        for domain in domains {
                            entry.insert(domain.clone());
                        }
                    }
                }
            }
        }

        // Step 5: Apply range inference
        // If P rdfs:range C and (x, P, y) exists, then y rdf:type C
        for ((_, property), targets) in &self.property_values {
            if let Some(ranges) = self.property_ranges.get(property) {
                for target in targets {
                    let entry = self.individual_types.entry(target.clone()).or_default();
                    for range in ranges {
                        entry.insert(range.clone());
                    }
                }
            }

            // Also check superproperties of this property
            if let Some(supers) = self.property_hierarchy.get(property) {
                for sup in supers {
                    if let Some(ranges) = self.property_ranges.get(sup) {
                        for target in targets {
                            let entry = self.individual_types.entry(target.clone()).or_default();
                            for range in ranges {
                                entry.insert(range.clone());
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

    /// Applies symmetric property rules.
    /// For each (a, P, b) where P is symmetric, infer (b, P, a).
    fn apply_symmetric_property_rules(&mut self) -> bool {
        let mut changed = false;
        let keys: Vec<_> = self.property_values.keys().cloned().collect();

        for (subject, property) in keys {
            if self.symmetric_properties.contains(&property) {
                if let Some(objects) = self
                    .property_values
                    .get(&(subject.clone(), property.clone()))
                    .cloned()
                {
                    for object in objects {
                        // Infer (object, property, subject)
                        let entry = self
                            .property_values
                            .entry((object, property.clone()))
                            .or_default();
                        if entry.insert(subject.clone()) {
                            changed = true;
                        }
                    }
                }
            }
        }

        changed
    }

    /// Applies transitive property rules.
    /// For each (a, P, b) and (b, P, c) where P is transitive, infer (a, P, c).
    fn apply_transitive_property_rules(&mut self) -> bool {
        let mut changed = false;
        let keys: Vec<_> = self.property_values.keys().cloned().collect();

        for (subject, property) in keys {
            if self.transitive_properties.contains(&property) {
                if let Some(middle_objects) = self
                    .property_values
                    .get(&(subject.clone(), property.clone()))
                    .cloned()
                {
                    for middle in middle_objects {
                        // Look for (middle, property, object)
                        if let Some(final_objects) =
                            self.property_values.get(&(middle, property.clone())).cloned()
                        {
                            let entry = self
                                .property_values
                                .entry((subject.clone(), property.clone()))
                                .or_default();
                            for final_obj in final_objects {
                                if entry.insert(final_obj) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        changed
    }

    /// Applies inverse property rules.
    /// For each (a, P, b) where P has inverse Q, infer (b, Q, a).
    fn apply_inverse_property_rules(&mut self) -> bool {
        let mut changed = false;
        let keys: Vec<_> = self.property_values.keys().cloned().collect();

        for (subject, property) in keys {
            if let Some(inverse_property) = self.inverse_properties.get(&property).cloned() {
                if let Some(objects) = self
                    .property_values
                    .get(&(subject.clone(), property.clone()))
                    .cloned()
                {
                    for object in objects {
                        // Infer (object, inverse_property, subject)
                        let entry = self
                            .property_values
                            .entry((object, inverse_property.clone()))
                            .or_default();
                        if entry.insert(subject.clone()) {
                            changed = true;
                        }
                    }
                }
            }
        }

        changed
    }

    /// Checks for inconsistencies.
    fn check_consistency(&mut self) -> Result<(), InconsistencyError> {
        // Check if any individual is both same-as and different-from another
        for (a, b) in &self.different_from {
            if let Some(same) = self.same_as.get(a) {
                if same.contains(b) {
                    return Err(InconsistencyError::new(format!(
                        "{a} is both sameAs and differentFrom {b}"
                    )));
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

        // Generate ObjectPropertyAssertion axioms from property reasoning
        for ((source, property), targets) in &self.property_values {
            for target in targets {
                let axiom = Axiom::ObjectPropertyAssertion {
                    property: property.clone(),
                    source: source.clone(),
                    target: target.clone(),
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

        // Start timing for timeout enforcement
        self.start_time = Some(Instant::now());

        // Step 1: Initialize from ontology axioms
        self.initialize();
        self.check_timeout()?;

        // Step 2: Compute transitive closure of class hierarchy
        self.compute_transitive_closure();
        self.check_timeout()?;

        // Step 3: Apply RDFS rules (property hierarchy, domain, range)
        self.apply_rdfs_rules();
        self.check_timeout()?;

        // Step 4: Propagate types to individuals
        self.propagate_types();
        self.check_timeout()?;

        // Step 5: Apply property reasoning rules with fixpoint iteration
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < self.config.max_iterations {
            changed = false;
            iterations += 1;

            // Check timeout periodically (every 10 iterations for responsiveness)
            if iterations % 10 == 0 {
                self.check_timeout()?;
            }

            // Apply symmetric property rules
            if self.apply_symmetric_property_rules() {
                changed = true;
            }

            // Apply transitive property rules
            if self.apply_transitive_property_rules() {
                changed = true;
            }

            // Apply inverse property rules
            if self.apply_inverse_property_rules() {
                changed = true;
            }
        }

        // Step 6: Check consistency if configured
        if self.config.check_consistency {
            if let Err(e) = self.check_consistency() {
                self.inconsistent = Some(e.clone());
                return Err(OwlError::Inconsistent(e));
            }
        }

        self.check_timeout()?;

        // Step 7: Generate inferred axioms
        self.generate_inferred_axioms();

        // Step 8: Check materialization limit
        self.check_materialization_limit()?;

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
                        s != class
                            && self
                                .class_hierarchy
                                .get(s)
                                .is_some_and(|ss| ss.contains(class))
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
                    supers
                        .iter()
                        .filter(|&sup| {
                            // Check if there's no intermediate class
                            !supers.iter().any(|s| {
                                s != sup
                                    && self
                                        .class_hierarchy
                                        .get(s)
                                        .is_some_and(|ss| ss.contains(sup))
                            })
                        })
                        .collect()
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
                        t != class
                            && self
                                .class_hierarchy
                                .get(t)
                                .is_some_and(|s| s.contains(class))
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
