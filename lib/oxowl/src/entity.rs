//! OWL 2 entity types (classes, properties, individuals).

use oxrdf::{BlankNode, BlankNodeRef, NamedNode, NamedNodeRef, Term, TermRef};
use std::fmt;

/// An OWL class (owl:Class).
///
/// Classes are sets of individuals. Every class is a subclass of owl:Thing
/// and a superclass of owl:Nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwlClass(NamedNode);

impl OwlClass {
    /// Creates a new OWL class from a named node.
    #[inline]
    pub fn new(iri: NamedNode) -> Self {
        Self(iri)
    }

    /// Creates a new OWL class from an IRI string.
    #[inline]
    pub fn new_from_iri(iri: impl Into<String>) -> Result<Self, oxiri::IriParseError> {
        Ok(Self(NamedNode::new(iri)?))
    }

    /// Returns the IRI of this class.
    #[inline]
    pub fn iri(&self) -> &NamedNode {
        &self.0
    }

    /// Returns a reference to this class.
    #[inline]
    pub fn as_ref(&self) -> OwlClassRef<'_> {
        OwlClassRef(self.0.as_ref())
    }

    /// Converts this class into its underlying named node.
    #[inline]
    pub fn into_inner(self) -> NamedNode {
        self.0
    }
}

impl fmt::Display for OwlClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<NamedNode> for OwlClass {
    fn from(node: NamedNode) -> Self {
        Self(node)
    }
}

impl From<OwlClass> for NamedNode {
    fn from(class: OwlClass) -> Self {
        class.0
    }
}

impl From<OwlClass> for Term {
    fn from(class: OwlClass) -> Self {
        class.0.into()
    }
}

impl AsRef<NamedNode> for OwlClass {
    fn as_ref(&self) -> &NamedNode {
        &self.0
    }
}

/// A reference to an OWL class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwlClassRef<'a>(NamedNodeRef<'a>);

impl<'a> OwlClassRef<'a> {
    /// Creates a new class reference from a named node reference.
    #[inline]
    pub fn new(iri: NamedNodeRef<'a>) -> Self {
        Self(iri)
    }

    /// Returns the IRI of this class.
    #[inline]
    pub fn iri(&self) -> NamedNodeRef<'a> {
        self.0
    }

    /// Converts this reference into an owned class.
    #[inline]
    pub fn into_owned(self) -> OwlClass {
        OwlClass(self.0.into_owned())
    }
}

impl fmt::Display for OwlClassRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> From<&'a OwlClass> for OwlClassRef<'a> {
    fn from(class: &'a OwlClass) -> Self {
        class.as_ref()
    }
}

impl<'a> From<OwlClassRef<'a>> for TermRef<'a> {
    fn from(class: OwlClassRef<'a>) -> Self {
        class.0.into()
    }
}

/// An OWL object property (owl:ObjectProperty).
///
/// Object properties relate individuals to individuals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectProperty(NamedNode);

impl ObjectProperty {
    /// Creates a new object property from a named node.
    #[inline]
    pub fn new(iri: NamedNode) -> Self {
        Self(iri)
    }

    /// Creates a new object property from an IRI string.
    #[inline]
    pub fn new_from_iri(iri: impl Into<String>) -> Result<Self, oxiri::IriParseError> {
        Ok(Self(NamedNode::new(iri)?))
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> &NamedNode {
        &self.0
    }

    /// Returns a reference to this property.
    #[inline]
    pub fn as_ref(&self) -> ObjectPropertyRef<'_> {
        ObjectPropertyRef(self.0.as_ref())
    }

    /// Converts this property into its underlying named node.
    #[inline]
    pub fn into_inner(self) -> NamedNode {
        self.0
    }
}

impl fmt::Display for ObjectProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<NamedNode> for ObjectProperty {
    fn from(node: NamedNode) -> Self {
        Self(node)
    }
}

impl From<ObjectProperty> for NamedNode {
    fn from(prop: ObjectProperty) -> Self {
        prop.0
    }
}

impl From<ObjectProperty> for Term {
    fn from(prop: ObjectProperty) -> Self {
        prop.0.into()
    }
}

impl AsRef<NamedNode> for ObjectProperty {
    fn as_ref(&self) -> &NamedNode {
        &self.0
    }
}

/// A reference to an OWL object property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectPropertyRef<'a>(NamedNodeRef<'a>);

impl<'a> ObjectPropertyRef<'a> {
    /// Creates a new property reference from a named node reference.
    #[inline]
    pub fn new(iri: NamedNodeRef<'a>) -> Self {
        Self(iri)
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> NamedNodeRef<'a> {
        self.0
    }

    /// Converts this reference into an owned property.
    #[inline]
    pub fn into_owned(self) -> ObjectProperty {
        ObjectProperty(self.0.into_owned())
    }
}

impl fmt::Display for ObjectPropertyRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> From<&'a ObjectProperty> for ObjectPropertyRef<'a> {
    fn from(prop: &'a ObjectProperty) -> Self {
        prop.as_ref()
    }
}

impl<'a> From<ObjectPropertyRef<'a>> for TermRef<'a> {
    fn from(prop: ObjectPropertyRef<'a>) -> Self {
        prop.0.into()
    }
}

/// An OWL data property (owl:DatatypeProperty).
///
/// Data properties relate individuals to literals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataProperty(NamedNode);

impl DataProperty {
    /// Creates a new data property from a named node.
    #[inline]
    pub fn new(iri: NamedNode) -> Self {
        Self(iri)
    }

    /// Creates a new data property from an IRI string.
    #[inline]
    pub fn new_from_iri(iri: impl Into<String>) -> Result<Self, oxiri::IriParseError> {
        Ok(Self(NamedNode::new(iri)?))
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> &NamedNode {
        &self.0
    }

    /// Returns a reference to this property.
    #[inline]
    pub fn as_ref(&self) -> DataPropertyRef<'_> {
        DataPropertyRef(self.0.as_ref())
    }

    /// Converts this property into its underlying named node.
    #[inline]
    pub fn into_inner(self) -> NamedNode {
        self.0
    }
}

impl fmt::Display for DataProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<NamedNode> for DataProperty {
    fn from(node: NamedNode) -> Self {
        Self(node)
    }
}

impl From<DataProperty> for NamedNode {
    fn from(prop: DataProperty) -> Self {
        prop.0
    }
}

impl From<DataProperty> for Term {
    fn from(prop: DataProperty) -> Self {
        prop.0.into()
    }
}

impl AsRef<NamedNode> for DataProperty {
    fn as_ref(&self) -> &NamedNode {
        &self.0
    }
}

/// A reference to an OWL data property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataPropertyRef<'a>(NamedNodeRef<'a>);

impl<'a> DataPropertyRef<'a> {
    /// Creates a new property reference from a named node reference.
    #[inline]
    pub fn new(iri: NamedNodeRef<'a>) -> Self {
        Self(iri)
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> NamedNodeRef<'a> {
        self.0
    }

    /// Converts this reference into an owned property.
    #[inline]
    pub fn into_owned(self) -> DataProperty {
        DataProperty(self.0.into_owned())
    }
}

impl fmt::Display for DataPropertyRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> From<&'a DataProperty> for DataPropertyRef<'a> {
    fn from(prop: &'a DataProperty) -> Self {
        prop.as_ref()
    }
}

impl<'a> From<DataPropertyRef<'a>> for TermRef<'a> {
    fn from(prop: DataPropertyRef<'a>) -> Self {
        prop.0.into()
    }
}

/// An OWL annotation property (owl:AnnotationProperty).
///
/// Annotation properties are used for metadata and do not have semantic meaning
/// in OWL reasoning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnnotationProperty(NamedNode);

impl AnnotationProperty {
    /// Creates a new annotation property from a named node.
    #[inline]
    pub fn new(iri: NamedNode) -> Self {
        Self(iri)
    }

    /// Creates a new annotation property from an IRI string.
    #[inline]
    pub fn new_from_iri(iri: impl Into<String>) -> Result<Self, oxiri::IriParseError> {
        Ok(Self(NamedNode::new(iri)?))
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> &NamedNode {
        &self.0
    }

    /// Returns a reference to this property.
    #[inline]
    pub fn as_ref(&self) -> AnnotationPropertyRef<'_> {
        AnnotationPropertyRef(self.0.as_ref())
    }

    /// Converts this property into its underlying named node.
    #[inline]
    pub fn into_inner(self) -> NamedNode {
        self.0
    }
}

impl fmt::Display for AnnotationProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<NamedNode> for AnnotationProperty {
    fn from(node: NamedNode) -> Self {
        Self(node)
    }
}

impl From<AnnotationProperty> for NamedNode {
    fn from(prop: AnnotationProperty) -> Self {
        prop.0
    }
}

impl From<AnnotationProperty> for Term {
    fn from(prop: AnnotationProperty) -> Self {
        prop.0.into()
    }
}

impl AsRef<NamedNode> for AnnotationProperty {
    fn as_ref(&self) -> &NamedNode {
        &self.0
    }
}

/// A reference to an OWL annotation property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnnotationPropertyRef<'a>(NamedNodeRef<'a>);

impl<'a> AnnotationPropertyRef<'a> {
    /// Creates a new property reference from a named node reference.
    #[inline]
    pub fn new(iri: NamedNodeRef<'a>) -> Self {
        Self(iri)
    }

    /// Returns the IRI of this property.
    #[inline]
    pub fn iri(&self) -> NamedNodeRef<'a> {
        self.0
    }

    /// Converts this reference into an owned property.
    #[inline]
    pub fn into_owned(self) -> AnnotationProperty {
        AnnotationProperty(self.0.into_owned())
    }
}

impl fmt::Display for AnnotationPropertyRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> From<&'a AnnotationProperty> for AnnotationPropertyRef<'a> {
    fn from(prop: &'a AnnotationProperty) -> Self {
        prop.as_ref()
    }
}

impl<'a> From<AnnotationPropertyRef<'a>> for TermRef<'a> {
    fn from(prop: AnnotationPropertyRef<'a>) -> Self {
        prop.0.into()
    }
}

/// An OWL individual (named or anonymous).
///
/// Individuals are the basic objects in the OWL ontology.
/// They can be either named individuals (identified by IRIs) or
/// anonymous individuals (blank nodes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Individual {
    /// A named individual (identified by an IRI).
    Named(NamedNode),
    /// An anonymous individual (blank node).
    Anonymous(BlankNode),
}

impl Individual {
    /// Returns `true` if this is a named individual.
    #[inline]
    pub fn is_named(&self) -> bool {
        matches!(self, Self::Named(_))
    }

    /// Returns `true` if this is an anonymous individual.
    #[inline]
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous(_))
    }

    /// Returns a reference to the named node if this is a named individual.
    #[inline]
    pub fn as_named(&self) -> Option<&NamedNode> {
        match self {
            Self::Named(n) => Some(n),
            Self::Anonymous(_) => None,
        }
    }

    /// Returns a reference to the blank node if this is an anonymous individual.
    #[inline]
    pub fn as_anonymous(&self) -> Option<&BlankNode> {
        match self {
            Self::Named(_) => None,
            Self::Anonymous(b) => Some(b),
        }
    }

    /// Returns a reference to this individual.
    #[inline]
    pub fn as_ref(&self) -> IndividualRef<'_> {
        match self {
            Self::Named(n) => IndividualRef::Named(n.as_ref()),
            Self::Anonymous(b) => IndividualRef::Anonymous(b.as_ref()),
        }
    }
}

impl fmt::Display for Individual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(n) => write!(f, "{}", n),
            Self::Anonymous(b) => write!(f, "{}", b),
        }
    }
}

impl From<NamedNode> for Individual {
    fn from(node: NamedNode) -> Self {
        Self::Named(node)
    }
}

impl From<BlankNode> for Individual {
    fn from(node: BlankNode) -> Self {
        Self::Anonymous(node)
    }
}

impl From<Individual> for Term {
    fn from(individual: Individual) -> Self {
        match individual {
            Individual::Named(n) => n.into(),
            Individual::Anonymous(b) => b.into(),
        }
    }
}

/// A reference to an OWL individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndividualRef<'a> {
    /// A named individual (identified by an IRI).
    Named(NamedNodeRef<'a>),
    /// An anonymous individual (blank node).
    Anonymous(BlankNodeRef<'a>),
}

impl<'a> IndividualRef<'a> {
    /// Returns `true` if this is a named individual.
    #[inline]
    pub fn is_named(&self) -> bool {
        matches!(self, Self::Named(_))
    }

    /// Returns `true` if this is an anonymous individual.
    #[inline]
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous(_))
    }

    /// Returns the named node reference if this is a named individual.
    #[inline]
    pub fn as_named(&self) -> Option<NamedNodeRef<'a>> {
        match self {
            Self::Named(n) => Some(*n),
            Self::Anonymous(_) => None,
        }
    }

    /// Returns the blank node reference if this is an anonymous individual.
    #[inline]
    pub fn as_anonymous(&self) -> Option<BlankNodeRef<'a>> {
        match self {
            Self::Named(_) => None,
            Self::Anonymous(b) => Some(*b),
        }
    }

    /// Converts this reference into an owned individual.
    #[inline]
    pub fn into_owned(self) -> Individual {
        match self {
            Self::Named(n) => Individual::Named(n.into_owned()),
            Self::Anonymous(b) => Individual::Anonymous(b.into_owned()),
        }
    }
}

impl fmt::Display for IndividualRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(n) => write!(f, "{}", n),
            Self::Anonymous(b) => write!(f, "{}", b),
        }
    }
}

impl<'a> From<&'a Individual> for IndividualRef<'a> {
    fn from(individual: &'a Individual) -> Self {
        individual.as_ref()
    }
}

impl<'a> From<NamedNodeRef<'a>> for IndividualRef<'a> {
    fn from(node: NamedNodeRef<'a>) -> Self {
        Self::Named(node)
    }
}

impl<'a> From<BlankNodeRef<'a>> for IndividualRef<'a> {
    fn from(node: BlankNodeRef<'a>) -> Self {
        Self::Anonymous(node)
    }
}

impl<'a> From<IndividualRef<'a>> for TermRef<'a> {
    fn from(individual: IndividualRef<'a>) -> Self {
        match individual {
            IndividualRef::Named(n) => n.into(),
            IndividualRef::Anonymous(b) => b.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owl_class() {
        let iri = NamedNode::new_unchecked("http://example.org/Person");
        let class = OwlClass::new(iri.clone());
        assert_eq!(class.iri(), &iri);
        assert_eq!(class.to_string(), iri.to_string());

        let class_ref = class.as_ref();
        assert_eq!(class_ref.iri(), iri.as_ref());
        assert_eq!(class_ref.into_owned(), class);
    }

    #[test]
    fn test_object_property() {
        let iri = NamedNode::new_unchecked("http://example.org/knows");
        let prop = ObjectProperty::new(iri.clone());
        assert_eq!(prop.iri(), &iri);

        let prop_ref = prop.as_ref();
        assert_eq!(prop_ref.into_owned(), prop);
    }

    #[test]
    fn test_data_property() {
        let iri = NamedNode::new_unchecked("http://example.org/age");
        let prop = DataProperty::new(iri.clone());
        assert_eq!(prop.iri(), &iri);
    }

    #[test]
    fn test_annotation_property() {
        let iri = NamedNode::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
        let prop = AnnotationProperty::new(iri.clone());
        assert_eq!(prop.iri(), &iri);
    }

    #[test]
    fn test_individual_named() {
        let iri = NamedNode::new_unchecked("http://example.org/Alice");
        let individual = Individual::Named(iri.clone());
        assert!(individual.is_named());
        assert!(!individual.is_anonymous());
        assert_eq!(individual.as_named(), Some(&iri));
        assert_eq!(individual.as_anonymous(), None);
    }

    #[test]
    fn test_individual_anonymous() {
        let blank = BlankNode::default();
        let individual = Individual::Anonymous(blank.clone());
        assert!(!individual.is_named());
        assert!(individual.is_anonymous());
        assert_eq!(individual.as_named(), None);
        assert_eq!(individual.as_anonymous(), Some(&blank));
    }

    #[test]
    fn test_individual_ref() {
        let iri = NamedNode::new_unchecked("http://example.org/Bob");
        let individual = Individual::Named(iri.clone());
        let individual_ref = individual.as_ref();

        assert!(individual_ref.is_named());
        assert_eq!(individual_ref.as_named(), Some(iri.as_ref()));
        assert_eq!(individual_ref.into_owned(), individual);
    }

    #[test]
    fn test_conversions() {
        let iri = NamedNode::new_unchecked("http://example.org/Test");

        let class: OwlClass = iri.clone().into();
        let node: NamedNode = class.into();
        assert_eq!(node, iri);

        let prop: ObjectProperty = iri.clone().into();
        let term: Term = prop.into();
        assert_eq!(term, Term::NamedNode(iri));
    }
}
