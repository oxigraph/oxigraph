use crate::blank_node::{BlankNode, BlankNodeRef};
use crate::triple::{GraphName, GraphNameRef, Quad, Triple};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// An owned RDF formula (also known as a quoted graph or citation).
///
/// A formula represents a set of RDF statements (triples) that can be referenced
/// as a single entity, identified by a blank node. Formulas are useful for
/// representing nested graphs, quoted statements, or graph literals.
///
/// The default string formatter returns a representation showing the identifier
/// and the contained triples:
/// ```
/// use oxrdf::{BlankNode, Formula, NamedNodeRef, TripleRef};
///
/// let id = BlankNode::default();
/// let ex = NamedNodeRef::new("http://example.com")?;
/// let triple = TripleRef::new(ex, ex, ex).into_owned();
/// let formula = Formula::new(id, vec![triple]);
///
/// assert!(formula.to_string().contains("http://example.com"));
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Formula {
    /// The blank node identifier for this formula
    id: BlankNode,
    /// The triples contained in this formula
    triples: Vec<Triple>,
}

impl Formula {
    /// Creates a new formula with the given identifier and triples.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNode, Formula, NamedNodeRef, TripleRef};
    ///
    /// let id = BlankNode::default();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let triple = TripleRef::new(ex, ex, ex).into_owned();
    /// let formula = Formula::new(id, vec![triple]);
    ///
    /// assert_eq!(formula.triples().len(), 1);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn new(id: impl Into<BlankNode>, triples: Vec<Triple>) -> Self {
        Self {
            id: id.into(),
            triples,
        }
    }

    /// Returns the blank node identifier of this formula.
    #[inline]
    pub fn id(&self) -> &BlankNode {
        &self.id
    }

    /// Returns the triples contained in this formula.
    #[inline]
    pub fn triples(&self) -> &[Triple] {
        &self.triples
    }

    /// Returns a mutable reference to the triples in this formula.
    #[inline]
    pub fn triples_mut(&mut self) -> &mut Vec<Triple> {
        &mut self.triples
    }

    /// Consumes the formula and returns its components.
    #[inline]
    pub fn into_parts(self) -> (BlankNode, Vec<Triple>) {
        (self.id, self.triples)
    }

    /// Returns a borrowed view of this formula.
    #[inline]
    pub fn as_ref(&self) -> FormulaRef<'_> {
        FormulaRef {
            id: self.id.as_ref(),
            triples: &self.triples,
        }
    }

    /// Converts the formula's triples to quads.
    ///
    /// Each triple in the formula is converted to a quad using the formula's
    /// blank node ID as the graph name. This enables formulas to be stored
    /// in quad stores.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNode, Formula, NamedNode, Triple};
    ///
    /// let id = BlankNode::new("f1").unwrap();
    /// let ex = NamedNode::new("http://example.com").unwrap();
    /// let triple = Triple::new(ex.clone(), ex.clone(), ex);
    /// let formula = Formula::new(id.clone(), vec![triple]);
    ///
    /// let quads = formula.to_quads();
    /// assert_eq!(quads.len(), 1);
    /// // The quad's graph name should be the formula's ID
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn to_quads(&self) -> Vec<Quad> {
        let graph_name = GraphName::BlankNode(self.id.clone());
        self.triples
            .iter()
            .map(|triple| Quad {
                subject: triple.subject.clone(),
                predicate: triple.predicate.clone(),
                object: triple.object.clone(),
                graph_name: graph_name.clone(),
            })
            .collect()
    }

    /// Creates a formula from quads.
    ///
    /// This constructor extracts the triples from the provided quads and uses
    /// the graph name from the first quad as the formula's ID. If the graph name
    /// is not a blank node, a new blank node ID is generated.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNode, Formula, GraphName, NamedNode, Quad};
    ///
    /// let id = BlankNode::new("f1").unwrap();
    /// let ex = NamedNode::new("http://example.com").unwrap();
    /// let quad = Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex,
    ///     GraphName::BlankNode(id.clone())
    /// );
    ///
    /// let formula = Formula::from_quads(vec![quad]);
    /// assert_eq!(formula.triples().len(), 1);
    /// assert_eq!(formula.id(), &id);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn from_quads(quads: impl IntoIterator<Item = Quad>) -> Self {
        let mut quads_vec: Vec<Quad> = quads.into_iter().collect();

        // Extract the ID from the first quad's graph name, or create a new blank node
        let id = if let Some(first_quad) = quads_vec.first() {
            match &first_quad.graph_name {
                GraphName::BlankNode(bn) => bn.clone(),
                _ => BlankNode::default(),
            }
        } else {
            BlankNode::default()
        };

        // Convert quads to triples
        let triples = quads_vec.drain(..).map(Triple::from).collect();

        Self::new(id, triples)
    }

    /// Creates formulas from a dataset by grouping quads by their blank node graph names.
    ///
    /// This method scans all quads in the dataset and groups them by blank node graph names.
    /// Each group is converted into a formula. Named graph names and the default graph are ignored.
    ///
    /// This is particularly useful for loading N3 data where formulas are represented as
    /// named graphs with blank node identifiers.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNode, Dataset, Formula, GraphName, NamedNode, Quad};
    ///
    /// let id = BlankNode::new("f1").unwrap();
    /// let ex = NamedNode::new("http://example.com").unwrap();
    /// let mut dataset = Dataset::new();
    /// dataset.insert(Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::BlankNode(id.clone()),
    /// ));
    ///
    /// let formulas = Formula::from_dataset(&dataset);
    /// assert_eq!(formulas.len(), 1);
    /// assert_eq!(formulas[0].id(), &id);
    /// assert_eq!(formulas[0].triples().len(), 1);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn from_dataset(dataset: &crate::Dataset) -> Vec<Self> {
        use crate::triple::TripleRef;
        use std::collections::HashMap;

        let mut formula_map: HashMap<BlankNode, Vec<Triple>> = HashMap::new();

        for quad in dataset.iter() {
            if let GraphNameRef::BlankNode(bn) = quad.graph_name {
                let triple_ref: TripleRef<'_> = quad.into();
                formula_map
                    .entry(bn.into_owned())
                    .or_default()
                    .push(triple_ref.into_owned());
            }
        }

        formula_map
            .into_iter()
            .map(|(id, triples)| Self::new(id, triples))
            .collect()
    }

    /// Creates a Graph from this formula's triples.
    ///
    /// This is useful for validating formula contents with SHACL or other
    /// graph-based operations that require a Graph instance. The formula's
    /// triples are copied into a new Graph instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNode, Formula, Graph, NamedNode, Triple};
    ///
    /// let id = BlankNode::new("f1").unwrap();
    /// let ex = NamedNode::new("http://example.com").unwrap();
    /// let triple = Triple::new(ex.clone(), ex.clone(), ex);
    /// let formula = Formula::new(id, vec![triple.clone()]);
    ///
    /// let graph = formula.to_graph();
    /// assert_eq!(graph.len(), 1);
    /// assert!(graph.contains(triple.as_ref()));
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn to_graph(&self) -> crate::Graph {
        let mut graph = crate::Graph::new();
        for triple in &self.triples {
            graph.insert(triple.as_ref());
        }
        graph
    }
}

impl fmt::Display for Formula {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl Default for Formula {
    /// Creates an empty formula with a random blank node identifier.
    #[inline]
    fn default() -> Self {
        Self {
            id: BlankNode::default(),
            triples: Vec::new(),
        }
    }
}

/// A borrowed RDF formula (also known as a quoted graph or citation).
///
/// A formula represents a set of RDF statements (triples) that can be referenced
/// as a single entity, identified by a blank node. This is the borrowed variant
/// of [`Formula`].
///
/// The default string formatter returns a representation showing the identifier
/// and the contained triples:
/// ```
/// use oxrdf::{BlankNodeRef, FormulaRef, NamedNodeRef, TripleRef};
///
/// let id = BlankNodeRef::new("f1")?;
/// let ex = NamedNodeRef::new("http://example.com")?;
/// let triple = TripleRef::new(ex, ex, ex);
/// let triples = vec![triple.into_owned()];
/// let formula = FormulaRef::new(id, &triples);
///
/// assert_eq!(formula.triples().len(), 1);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct FormulaRef<'a> {
    /// The blank node identifier for this formula
    id: BlankNodeRef<'a>,
    /// The triples contained in this formula
    triples: &'a [Triple],
}

impl<'a> FormulaRef<'a> {
    /// Creates a new borrowed formula with the given identifier and triples.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxrdf::{BlankNodeRef, FormulaRef, NamedNodeRef, TripleRef};
    ///
    /// let id = BlankNodeRef::new("f1")?;
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let triple = TripleRef::new(ex, ex, ex);
    /// let triples = vec![triple.into_owned()];
    /// let formula = FormulaRef::new(id, &triples);
    ///
    /// assert_eq!(formula.id().as_str(), "f1");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub const fn new(id: BlankNodeRef<'a>, triples: &'a [Triple]) -> Self {
        Self { id, triples }
    }

    /// Returns the blank node identifier of this formula.
    #[inline]
    pub const fn id(self) -> BlankNodeRef<'a> {
        self.id
    }

    /// Returns the triples contained in this formula.
    #[inline]
    pub const fn triples(self) -> &'a [Triple] {
        self.triples
    }

    /// Converts this borrowed formula into an owned formula.
    #[inline]
    pub fn into_owned(self) -> Formula {
        Formula {
            id: self.id.into_owned(),
            triples: self.triples.to_vec(),
        }
    }
}

impl fmt::Display for FormulaRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{ {} |", self.id)?;
        for (i, triple) in self.triples.iter().enumerate() {
            if i > 0 {
                write!(f, " .")?;
            }
            write!(f, " {triple}")?;
        }
        if !self.triples.is_empty() {
            write!(f, " .")?;
        }
        write!(f, " }}")
    }
}

impl<'a> From<&'a Formula> for FormulaRef<'a> {
    #[inline]
    fn from(formula: &'a Formula) -> Self {
        formula.as_ref()
    }
}

impl<'a> From<FormulaRef<'a>> for Formula {
    #[inline]
    fn from(formula: FormulaRef<'a>) -> Self {
        formula.into_owned()
    }
}

impl PartialEq<Formula> for FormulaRef<'_> {
    #[inline]
    fn eq(&self, other: &Formula) -> bool {
        *self == other.as_ref()
    }
}

impl PartialEq<FormulaRef<'_>> for Formula {
    #[inline]
    fn eq(&self, other: &FormulaRef<'_>) -> bool {
        self.as_ref() == *other
    }
}

#[cfg(feature = "serde")]
impl Serialize for Formula {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl Serialize for FormulaRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "Formula")]
        struct Value<'a> {
            id: BlankNodeRef<'a>,
            triples: &'a [Triple],
        }
        Value {
            id: self.id,
            triples: self.triples,
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Formula {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "Formula")]
        struct Value {
            id: BlankNode,
            triples: Vec<Triple>,
        }
        let value = Value::deserialize(deserializer)?;
        Ok(Self::new(value.id, value.triples))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Literal, NamedNode, NamedNodeRef};

    #[test]
    fn new_formula() {
        let id = BlankNode::new("f1").unwrap();
        let formula = Formula::new(id.clone(), vec![]);
        assert_eq!(formula.id().as_str(), "f1");
        assert_eq!(formula.triples().len(), 0);
    }

    #[test]
    fn formula_with_triples() {
        let id = BlankNode::default();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex);
        let formula = Formula::new(id, vec![triple.clone()]);

        assert_eq!(formula.triples().len(), 1);
        assert_eq!(&formula.triples()[0], &triple);
    }

    #[test]
    fn formula_ref_conversion() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex);
        let formula = Formula::new(id.clone(), vec![triple]);

        let formula_ref = formula.as_ref();
        assert_eq!(formula_ref.id().as_str(), "f1");
        assert_eq!(formula_ref.triples().len(), 1);

        let formula2 = formula_ref.into_owned();
        assert_eq!(formula, formula2);
    }

    #[test]
    fn formula_equality() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex);

        let formula = Formula::new(id.clone(), vec![triple.clone()]);
        let triples_vec = vec![triple];
        let formula_ref = FormulaRef::new(id.as_ref(), &triples_vec);

        assert_eq!(formula, formula_ref);
        assert_eq!(formula_ref, formula);
    }

    #[test]
    fn formula_display() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNodeRef::new("http://example.com").unwrap();
        let triple = Triple::new(ex, ex, ex);
        let formula = Formula::new(id, vec![triple]);

        let display = formula.to_string();
        assert!(display.contains("_:f1"));
        assert!(display.contains("http://example.com"));
    }

    #[test]
    fn formula_into_parts() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex);
        let formula = Formula::new(id.clone(), vec![triple.clone()]);

        let (id2, triples) = formula.into_parts();
        assert_eq!(id, id2);
        assert_eq!(triples.len(), 1);
        assert_eq!(&triples[0], &triple);
    }

    #[test]
    fn formula_default() {
        let formula = Formula::default();
        assert_eq!(formula.triples().len(), 0);
        // The id should be a valid blank node
        assert!(!formula.id().as_str().is_empty());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex);
        let formula = Formula::new(id, vec![triple]);

        let json = serde_json::to_string(&formula).unwrap();
        assert!(json.contains("f1"));
        assert!(json.contains("http://example.com"));

        let formula2: Formula = serde_json::from_str(&json).unwrap();
        assert_eq!(formula, formula2);
    }

    #[test]
    fn test_to_quads() {
        use crate::triple::GraphName;

        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let triple = Triple::new(ex.clone(), ex.clone(), ex.clone());
        let formula = Formula::new(id.clone(), vec![triple.clone()]);

        let quads = formula.to_quads();
        assert_eq!(quads.len(), 1);

        let quad = &quads[0];
        assert_eq!(quad.subject, triple.subject);
        assert_eq!(quad.predicate, triple.predicate);
        assert_eq!(quad.object, triple.object);
        assert_eq!(quad.graph_name, GraphName::BlankNode(id));
    }

    #[test]
    fn test_from_quads() {
        use crate::triple::{GraphName, Quad};

        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            ex.clone(),
            GraphName::BlankNode(id.clone()),
        );

        let formula = Formula::from_quads(vec![quad.clone()]);
        assert_eq!(formula.triples().len(), 1);
        assert_eq!(formula.id(), &id);

        let triple = &formula.triples()[0];
        assert_eq!(triple.subject, quad.subject);
        assert_eq!(triple.predicate, quad.predicate);
        assert_eq!(triple.object, quad.object);
    }

    #[test]
    fn test_round_trip_conversion() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let ex2 = NamedNode::new("http://example.org").unwrap();

        let triple1 = Triple::new(ex.clone(), ex.clone(), ex.clone());
        let triple2 = Triple::new(ex2.clone(), ex2.clone(), ex2.clone());
        let original_formula = Formula::new(id.clone(), vec![triple1, triple2]);

        // Convert to quads and back
        let quads = original_formula.to_quads();
        let restored_formula = Formula::from_quads(quads);

        // Check that the formula is preserved
        assert_eq!(restored_formula.id(), &id);
        assert_eq!(restored_formula.triples().len(), 2);
        assert_eq!(restored_formula.triples(), original_formula.triples());
    }

    #[test]
    fn test_from_quads_empty() {
        let formula = Formula::from_quads(vec![]);
        assert_eq!(formula.triples().len(), 0);
        // Should have a valid blank node ID
        assert!(!formula.id().as_str().is_empty());
    }

    #[test]
    fn test_from_quads_with_named_graph() {
        use crate::triple::{GraphName, Quad};

        let ex = NamedNode::new("http://example.com").unwrap();
        let graph_node = NamedNode::new("http://graph.example.com").unwrap();
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            ex.clone(),
            GraphName::NamedNode(graph_node),
        );

        let formula = Formula::from_quads(vec![quad.clone()]);
        assert_eq!(formula.triples().len(), 1);
        // When graph name is not a blank node, a new blank node should be generated
        assert!(!formula.id().as_str().is_empty());

        let triple = &formula.triples()[0];
        assert_eq!(triple.subject, quad.subject);
        assert_eq!(triple.predicate, quad.predicate);
        assert_eq!(triple.object, quad.object);
    }

    #[test]
    fn test_to_quads_empty_formula() {
        let formula = Formula::default();
        let quads = formula.to_quads();
        assert_eq!(quads.len(), 0);
    }

    #[test]
    fn test_multiple_quads_round_trip() {
        use crate::triple::GraphName;
        use crate::Literal;

        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let predicate = NamedNode::new("http://example.com/predicate").unwrap();
        let lit = Literal::new_simple_literal("test");

        let triple1 = Triple::new(ex.clone(), predicate.clone(), ex.clone());
        let triple2 = Triple::new(ex.clone(), predicate.clone(), lit);
        let triple3 = Triple::new(
            BlankNode::new("b1").unwrap(),
            predicate.clone(),
            ex.clone(),
        );

        let original_formula = Formula::new(id.clone(), vec![triple1, triple2, triple3]);

        // Convert to quads
        let quads = original_formula.to_quads();
        assert_eq!(quads.len(), 3);

        // All quads should have the same graph name
        for quad in &quads {
            assert_eq!(
                quad.graph_name,
                GraphName::BlankNode(id.clone())
            );
        }

        // Convert back
        let restored_formula = Formula::from_quads(quads);
        assert_eq!(restored_formula.id(), &id);
        assert_eq!(restored_formula.triples().len(), 3);
        assert_eq!(restored_formula, original_formula);
    }

    #[test]
    fn test_to_graph() {
        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();
        let predicate = NamedNode::new("http://example.com/predicate").unwrap();

        let triple1 = Triple::new(ex.clone(), predicate.clone(), ex.clone());
        let triple2 = Triple::new(ex.clone(), predicate.clone(), Literal::new_simple_literal("test"));

        let formula = Formula::new(id, vec![triple1.clone(), triple2.clone()]);

        // Convert to graph
        let graph = formula.to_graph();

        // Check that all triples are in the graph
        assert_eq!(graph.len(), 2);
        assert!(graph.contains(triple1.as_ref()));
        assert!(graph.contains(triple2.as_ref()));
    }

    #[test]
    fn test_to_graph_empty() {
        let formula = Formula::default();
        let graph = formula.to_graph();
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_from_dataset() {
        use crate::Dataset;

        let id1 = BlankNode::new("f1").unwrap();
        let id2 = BlankNode::new("f2").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();

        let mut dataset = Dataset::new();

        // Add quads to first formula
        let quad1 = Quad::new(
            ex.clone(),
            ex.clone(),
            Literal::new_simple_literal("formula1"),
            GraphName::BlankNode(id1.clone()),
        );
        dataset.insert(quad1.as_ref());

        // Add quads to second formula
        let quad2 = Quad::new(
            ex.clone(),
            ex.clone(),
            Literal::new_simple_literal("formula2"),
            GraphName::BlankNode(id2.clone()),
        );
        dataset.insert(quad2.as_ref());

        // Add quad to default graph (should be ignored)
        let quad3 = Quad::new(
            ex.clone(),
            ex.clone(),
            Literal::new_simple_literal("default"),
            GraphName::DefaultGraph,
        );
        dataset.insert(quad3.as_ref());

        // Extract formulas
        let formulas = Formula::from_dataset(&dataset);

        // Should have two formulas
        assert_eq!(formulas.len(), 2);

        // Check that each formula has the correct content
        for formula in &formulas {
            assert_eq!(formula.triples().len(), 1);
            let triple = &formula.triples()[0];
            assert_eq!(triple.subject, ex.clone().into());
            assert_eq!(triple.predicate, ex.clone());
        }
    }

    #[test]
    fn test_from_dataset_empty() {
        use crate::Dataset;

        let dataset = Dataset::new();
        let formulas = Formula::from_dataset(&dataset);
        assert_eq!(formulas.len(), 0);
    }

    #[test]
    fn test_from_dataset_only_named_graphs() {
        use crate::Dataset;

        let ex = NamedNode::new("http://example.com").unwrap();
        let graph_name = NamedNode::new("http://example.com/graph").unwrap();

        let mut dataset = Dataset::new();

        // Add quad to named graph (not a blank node, should be ignored)
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            ex.clone(),
            GraphName::NamedNode(graph_name),
        );
        dataset.insert(quad.as_ref());

        // Extract formulas - should be empty since only blank node graphs are formulas
        let formulas = Formula::from_dataset(&dataset);
        assert_eq!(formulas.len(), 0);
    }

    #[test]
    fn test_round_trip_dataset_to_formula_to_graph() {
        use crate::Dataset;

        let id = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.com").unwrap();

        let mut dataset = Dataset::new();
        let quad = Quad::new(
            ex.clone(),
            ex.clone(),
            Literal::new_simple_literal("test"),
            GraphName::BlankNode(id.clone()),
        );
        dataset.insert(quad.as_ref());

        // Extract formula and convert to graph
        let formulas = Formula::from_dataset(&dataset);
        assert_eq!(formulas.len(), 1);

        let graph = formulas[0].to_graph();
        assert_eq!(graph.len(), 1);

        let triple = graph.iter().next().unwrap();
        assert_eq!(triple.subject.to_string(), ex.to_string());
        assert_eq!(triple.predicate, ex.as_ref());
    }
}
