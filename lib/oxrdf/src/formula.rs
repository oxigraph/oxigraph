use crate::blank_node::{BlankNode, BlankNodeRef};
use crate::triple::Triple;
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
            write!(f, " {}", triple)?;
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
    use crate::{NamedNode, NamedNodeRef};

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
}
