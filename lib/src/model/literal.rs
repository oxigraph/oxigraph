use model::named_node::NamedNode;
use model::vocab::rdf;
use model::vocab::xsd;
use num_traits::identities::Zero;
use num_traits::FromPrimitive;
use num_traits::One;
use num_traits::ToPrimitive;
use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use std::borrow::Cow;
use std::fmt;
use std::option::Option;
use utils::Escaper;

/// A RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal)
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use rudf::model::Literal;
/// use rudf::model::vocab::xsd;
///
/// assert_eq!(
///     "\"foo\\tbar\"",
///     Literal::new_simple_literal("foo\tbar").to_string()
/// );
///
/// assert_eq!(
///     "\"1999-01-01\"^^<http://www.w3.org/2001/XMLSchema#date>",
///     Literal::new_typed_literal("1999-01-01", xsd::DATE.clone()).to_string()
/// );
///
/// assert_eq!(
///     "\"foo\"@en",
///     Literal::new_language_tagged_literal("foo", "en").to_string()
/// );
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Literal(LiteralContent);

#[derive(PartialEq, Eq, Ord, PartialOrd, Debug, Clone, Hash)]
enum LiteralContent {
    SimpleLiteral(String),
    String(String),
    LanguageTaggedString { value: String, language: String },
    Boolean(bool),
    Float(OrderedFloat<f32>),
    Double(OrderedFloat<f64>),
    Integer(i128),
    Decimal(Decimal),
    TypedLiteral { value: String, datatype: NamedNode },
}

impl Literal {
    /// Builds a RDF [simple literal](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal)
    pub fn new_simple_literal(value: impl Into<String>) -> Self {
        Literal(LiteralContent::SimpleLiteral(value.into()))
    }

    /// Builds a RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) with a [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri)
    pub fn new_typed_literal(value: impl Into<String>, datatype: impl Into<NamedNode>) -> Self {
        let value = value.into();
        let datatype = datatype.into();
        Literal(if datatype == *xsd::BOOLEAN {
            match value.as_str() {
                "true" | "1" => LiteralContent::Boolean(true),
                "false" | "0" => LiteralContent::Boolean(false),
                _ => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::STRING {
            LiteralContent::String(value)
        } else if datatype == *xsd::FLOAT {
            match value.parse() {
                Ok(value) => LiteralContent::Float(OrderedFloat(value)),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DOUBLE {
            match value.parse() {
                Ok(value) => LiteralContent::Double(OrderedFloat(value)),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::INTEGER {
            match value.parse() {
                Ok(value) => LiteralContent::Integer(value),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else if datatype == *xsd::DECIMAL {
            match value.parse() {
                Ok(value) => LiteralContent::Decimal(value),
                Err(_) => LiteralContent::TypedLiteral { value, datatype },
            }
        } else {
            LiteralContent::TypedLiteral { value, datatype }
        })
    }

    /// Builds a RDF [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    pub fn new_language_tagged_literal(
        value: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Literal(LiteralContent::LanguageTaggedString {
            value: value.into(),
            language: language.into(),
        })
    }

    /// The literal [lexical form](https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form)
    pub fn value(&self) -> Cow<String> {
        match self.0 {
            LiteralContent::SimpleLiteral(ref value)
            | LiteralContent::String(ref value)
            | LiteralContent::LanguageTaggedString { ref value, .. }
            | LiteralContent::TypedLiteral { ref value, .. } => Cow::Borrowed(value),
            LiteralContent::Boolean(value) => Cow::Owned(value.to_string()),
            LiteralContent::Float(value) => Cow::Owned(value.to_string()),
            LiteralContent::Double(value) => Cow::Owned(value.to_string()),
            LiteralContent::Integer(value) => Cow::Owned(value.to_string()),
            LiteralContent::Decimal(value) => Cow::Owned(value.to_string()),
        }
    }

    /// The literal [language tag](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag) if it is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string).
    /// Language tags are defined by the [BCP47](https://tools.ietf.org/html/bcp47).
    pub fn language(&self) -> Option<&str> {
        match self.0 {
            LiteralContent::LanguageTaggedString { ref language, .. } => Some(language),
            _ => None,
        }
    }

    /// The literal [datatype](https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri).
    ///
    /// The datatype of [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string) is always [rdf:langString](http://www.w3.org/1999/02/22-rdf-syntax-ns#langString).
    /// The datatype of [simple literals](https://www.w3.org/TR/rdf11-concepts/#dfn-simple-literal) is [xsd:string](http://www.w3.org/2001/XMLSchema#string).
    pub fn datatype(&self) -> &NamedNode {
        match self.0 {
            LiteralContent::SimpleLiteral(_) | LiteralContent::String(_) => &xsd::STRING,
            LiteralContent::LanguageTaggedString { .. } => &rdf::LANG_STRING,
            LiteralContent::Boolean(_) => &xsd::BOOLEAN,
            LiteralContent::Float(_) => &xsd::FLOAT,
            LiteralContent::Double(_) => &xsd::DOUBLE,
            LiteralContent::Integer(_) => &xsd::INTEGER,
            LiteralContent::Decimal(_) => &xsd::DECIMAL,
            LiteralContent::TypedLiteral { ref datatype, .. } => datatype,
        }
    }

    /// Checks if it could be considered as an RDF 1.0 [plain literal](https://www.w3.org/TR/rdf-concepts/#dfn-plain-literal).
    ///
    /// It returns true if the literal is a [language-tagged string](https://www.w3.org/TR/rdf11-concepts/#dfn-language-tagged-string)
    /// or have been created by `Literal::new_simple_literal`.
    pub fn is_plain(&self) -> bool {
        match self.0 {
            LiteralContent::SimpleLiteral(_) | LiteralContent::LanguageTaggedString { .. } => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:string](http://www.w3.org/2001/XMLSchema#string) and is valid
    pub fn is_string(&self) -> bool {
        match self.0 {
            LiteralContent::String(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:boolean](http://www.w3.org/2001/XMLSchema#boolean) and is valid
    pub fn is_boolean(&self) -> bool {
        match self.0 {
            LiteralContent::Boolean(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:float](http://www.w3.org/2001/XMLSchema#float) and is valid
    pub fn is_float(&self) -> bool {
        match self.0 {
            LiteralContent::Float(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:double](http://www.w3.org/2001/XMLSchema#double) and is valid
    pub fn is_double(&self) -> bool {
        match self.0 {
            LiteralContent::Double(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:integer](http://www.w3.org/2001/XMLSchema#integer) and is valid
    pub fn is_integer(&self) -> bool {
        match self.0 {
            LiteralContent::Integer(_) => true,
            _ => false,
        }
    }

    /// Checks if the literal has the datatype [xsd:decimal](http://www.w3.org/2001/XMLSchema#decimal) or one of its sub datatype and is valid
    pub fn is_decimal(&self) -> bool {
        match self.0 {
            LiteralContent::Integer(_) | LiteralContent::Decimal(_) => true,
            _ => false,
        }
    }

    /// Returns the [effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv) of the literal if it exists
    pub fn to_bool(&self) -> Option<bool> {
        match self.0 {
            LiteralContent::SimpleLiteral(ref value) | LiteralContent::String(ref value) => {
                Some(!value.is_empty())
            }
            LiteralContent::Boolean(value) => Some(value),
            LiteralContent::Float(value) => Some(!value.is_zero()),
            LiteralContent::Double(value) => Some(!value.is_zero()),
            LiteralContent::Integer(value) => Some(!value.is_zero()),
            LiteralContent::Decimal(value) => Some(!value.is_zero()),
            _ => None,
        }
    }

    /// Returns the value of this literal as an f32 if it exists following the rules of [XPath xsd:float casting](https://www.w3.org/TR/xpath-functions/#casting-to-float)
    pub fn to_float(&self) -> Option<f32> {
        match self.0 {
            LiteralContent::Float(value) => value.to_f32(),
            LiteralContent::Double(value) => value.to_f32(),
            LiteralContent::Integer(value) => value.to_f32(),
            LiteralContent::Decimal(value) => value.to_f32(),
            LiteralContent::Boolean(value) => Some(if value { 1. } else { 0. }),
            LiteralContent::SimpleLiteral(ref value) | LiteralContent::String(ref value) => {
                value.parse().ok()
            }
            _ => None,
        }
    }

    /// Returns the value of this literal as an f64 if it exists following the rules of [XPath xsd:double casting](https://www.w3.org/TR/xpath-functions/#casting-to-double)
    pub fn to_double(&self) -> Option<f64> {
        match self.0 {
            LiteralContent::Float(value) => value.to_f64(),
            LiteralContent::Double(value) => value.to_f64(),
            LiteralContent::Integer(value) => value.to_f64(),
            LiteralContent::Decimal(value) => value.to_f64(),
            LiteralContent::Boolean(value) => Some(if value { 1. } else { 0. }),
            LiteralContent::SimpleLiteral(ref value) | LiteralContent::String(ref value) => {
                value.parse().ok()
            }
            _ => None,
        }
    }

    /// Returns the value of this literal as an i128 if it exists following the rules of [XPath xsd:integer casting](https://www.w3.org/TR/xpath-functions/#casting-to-integer)
    pub fn to_integer(&self) -> Option<i128> {
        match self.0 {
            LiteralContent::Float(value) => value.to_i128(),
            LiteralContent::Double(value) => value.to_i128(),
            LiteralContent::Integer(value) => value.to_i128(),
            LiteralContent::Decimal(value) => value.to_i128(),
            LiteralContent::Boolean(value) => Some(if value { 1 } else { 0 }),
            LiteralContent::SimpleLiteral(ref value) | LiteralContent::String(ref value) => {
                value.parse().ok()
            }
            _ => None,
        }
    }

    /// Returns the value of this literal as Decimal if it exists following the rules of [XPath xsd:decimal casting](https://www.w3.org/TR/xpath-functions/#casting-to-decimal)
    pub(crate) fn to_decimal(&self) -> Option<Decimal> {
        match self.0 {
            LiteralContent::Float(value) => Decimal::from_f32(*value),
            LiteralContent::Double(value) => Decimal::from_f64(*value),
            LiteralContent::Integer(value) => Decimal::from_i128(value),
            LiteralContent::Decimal(value) => Some(value),
            LiteralContent::Boolean(value) => Some(if value {
                Decimal::one()
            } else {
                Decimal::zero()
            }),
            LiteralContent::SimpleLiteral(ref value) | LiteralContent::String(ref value) => {
                value.parse().ok()
            }
            _ => None,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_plain() {
            self.language()
                .map(|lang| write!(f, "\"{}\"@{}", self.value().escape(), lang))
                .unwrap_or_else(|| write!(f, "\"{}\"", self.value().escape()))
        } else {
            write!(f, "\"{}\"^^{}", self.value().escape(), self.datatype())
        }
    }
}

impl<'a> From<&'a str> for Literal {
    fn from(value: &'a str) -> Self {
        Literal(LiteralContent::String(value.into()))
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal(LiteralContent::String(value))
    }
}

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal(LiteralContent::Boolean(value))
    }
}

impl From<i128> for Literal {
    fn from(value: i128) -> Self {
        Literal(LiteralContent::Integer(value))
    }
}

impl From<i64> for Literal {
    fn from(value: i64) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<i32> for Literal {
    fn from(value: i32) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<i16> for Literal {
    fn from(value: i16) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u64> for Literal {
    fn from(value: u64) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u32> for Literal {
    fn from(value: u32) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<u16> for Literal {
    fn from(value: u16) -> Self {
        Literal(LiteralContent::Integer(value.into()))
    }
}

impl From<f32> for Literal {
    fn from(value: f32) -> Self {
        Literal(LiteralContent::Float(value.into()))
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal(LiteralContent::Double(value.into()))
    }
}

impl From<Decimal> for Literal {
    fn from(value: Decimal) -> Self {
        Literal(LiteralContent::Decimal(value))
    }
}
