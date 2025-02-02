use oxrdf::{
    BlankNode, Dataset, GraphNameRef, Literal, NamedNode, QuadRef, SubjectRef, Term, TermRef,
};
#[cfg(feature = "rdf-star")]
use oxrdf::{Subject, Triple};
use oxsdatatypes::{Boolean, DateTime, Decimal, Double, Float, Integer};
#[cfg(feature = "sep-0002")]
use oxsdatatypes::{Date, DayTimeDuration, Duration, Time, YearMonthDuration};
#[cfg(feature = "calendar-ext")]
use oxsdatatypes::{GDay, GMonth, GMonthDay, GYear, GYearMonth};
use rustc_hash::FxHashSet;
use std::convert::Infallible;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::iter::empty;
use std::mem::discriminant;

/// A [RDF dataset](https://www.w3.org/TR/sparql11-query/#rdfDataset) that can be queried using SPARQL
pub trait QueryableDataset: Sized + 'static {
    /// Internal representation of an RDF term
    ///
    /// Can be just an integer that indexes into a dictionary...
    ///
    /// Equality here is the RDF term equality (SPARQL `sameTerm` function)
    type InternalTerm: Clone + Eq + Hash;

    /// Error returned by the dataset.
    type Error: Error + Send + Sync;

    /// Fetches quads according to a pattern
    ///
    /// For `graph_name`, `Some(None)` encodes the default graph and `Some(Some(_))` a named graph
    fn internal_quads_for_pattern(
        &self,
        subject: Option<&Self::InternalTerm>,
        predicate: Option<&Self::InternalTerm>,
        object: Option<&Self::InternalTerm>,
        graph_name: Option<Option<&Self::InternalTerm>>,
    ) -> Box<dyn Iterator<Item = Result<InternalQuad<Self>, Self::Error>>>; // TODO: consider `impl`

    /// Fetches the list of dataset named graphs
    fn internal_named_graphs(
        &self,
    ) -> Box<dyn Iterator<Item = Result<Self::InternalTerm, Self::Error>>> {
        // TODO: consider `impl`
        let mut error = None;
        let graph_names = self
            .internal_quads_for_pattern(None, None, None, None)
            .filter_map(|r| match r {
                Ok(r) => Some(r.graph_name?),
                Err(e) => {
                    error = Some(e);
                    None
                }
            })
            .collect::<FxHashSet<_>>();

        Box::new(
            error
                .map(Err)
                .into_iter()
                .chain(graph_names.into_iter().map(Ok)),
        )
    }

    /// Returns if the dataset contains a given named graph
    fn contains_internal_graph_name(
        &self,
        graph_name: &Self::InternalTerm,
    ) -> Result<bool, Self::Error> {
        Ok(self
            .internal_quads_for_pattern(None, None, None, Some(Some(graph_name)))
            .next()
            .transpose()?
            .is_some())
    }

    /// Builds an internal term from the [`Term`] struct
    fn internalize_term(&self, term: Term) -> Result<Self::InternalTerm, Self::Error>;

    /// Builds a [`Term`] from an internal term
    fn externalize_term(&self, term: Self::InternalTerm) -> Result<Term, Self::Error>;

    // Optional methods that can be overridden for better performances

    /// Builds an [`ExpressionTerm`] from an internal term
    fn externalize_expression_term(
        &self,
        term: Self::InternalTerm,
    ) -> Result<ExpressionTerm, Self::Error> {
        Ok(self.externalize_term(term)?.into())
    }

    /// Builds an internal term from an [`ExpressionTerm`]
    fn internalize_expression_term(
        &self,
        term: ExpressionTerm,
    ) -> Result<Self::InternalTerm, Self::Error> {
        self.internalize_term(term.into())
    }

    /// Computes the term [Effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv)
    fn internal_term_effective_boolean_value(
        &self,
        term: Self::InternalTerm,
    ) -> Result<Option<bool>, Self::Error> {
        Ok(self
            .externalize_expression_term(term)?
            .effective_boolean_value())
    }
}

impl QueryableDataset for Dataset {
    type InternalTerm = Term;
    type Error = Infallible;

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&Term>,
        predicate: Option<&Term>,
        object: Option<&Term>,
        graph_name: Option<Option<&Term>>,
    ) -> Box<dyn Iterator<Item = Result<InternalQuad<Self>, Infallible>>> {
        // Awful implementation, please don't take it as an example

        #[allow(clippy::unnecessary_wraps)]
        fn quad_to_result(quad: QuadRef<'_>) -> Result<InternalQuad<Dataset>, Infallible> {
            Ok(InternalQuad {
                subject: quad.subject.into(),
                predicate: quad.predicate.into(),
                object: quad.object.into_owned(),
                graph_name: match quad.graph_name {
                    GraphNameRef::NamedNode(g) => Some(g.into()),
                    GraphNameRef::BlankNode(g) => Some(g.into()),
                    GraphNameRef::DefaultGraph => None,
                },
            })
        }

        let subject = if let Some(subject) = subject {
            Some(match TermRef::from(subject) {
                TermRef::NamedNode(s) => SubjectRef::from(s),
                TermRef::BlankNode(s) => s.into(),
                TermRef::Literal(_) => return Box::new(empty()),
                #[cfg(feature = "rdf-star")]
                TermRef::Triple(s) => s.into(),
            })
        } else {
            None
        };
        let predicate = if let Some(predicate) = predicate {
            if let TermRef::NamedNode(p) = TermRef::from(predicate) {
                Some(p)
            } else {
                return Box::new(empty());
            }
        } else {
            None
        };
        let object = object.map(TermRef::from);
        let graph_name = if let Some(graph_name) = graph_name {
            Some(if let Some(graph_name) = graph_name {
                match TermRef::from(graph_name) {
                    TermRef::NamedNode(s) => s.into(),
                    TermRef::BlankNode(s) => s.into(),
                    TermRef::Literal(_) => return Box::new(empty()),
                    #[cfg(feature = "rdf-star")]
                    TermRef::Triple(_) => return Box::new(empty()),
                }
            } else {
                GraphNameRef::DefaultGraph
            })
        } else {
            None
        };
        let quads: Vec<_> = if let Some(subject) = subject {
            self.quads_for_subject(subject)
                .filter(|q| {
                    predicate.map_or(true, |t| t == q.predicate)
                        && object.map_or(true, |t| t == q.object)
                        && graph_name
                            .map_or_else(|| !q.graph_name.is_default_graph(), |t| t == q.graph_name)
                })
                .map(quad_to_result)
                .collect()
        } else if let Some(object) = object {
            self.quads_for_object(object)
                .filter(|q| {
                    predicate.map_or(true, |t| t == q.predicate)
                        && graph_name
                            .map_or_else(|| !q.graph_name.is_default_graph(), |t| t == q.graph_name)
                })
                .map(quad_to_result)
                .collect()
        } else if let Some(predicate) = predicate {
            self.quads_for_predicate(predicate)
                .filter(|q| {
                    graph_name
                        .map_or_else(|| !q.graph_name.is_default_graph(), |t| t == q.graph_name)
                })
                .map(quad_to_result)
                .collect()
        } else if let Some(graph_name) = graph_name {
            self.quads_for_graph_name(graph_name)
                .map(quad_to_result)
                .collect()
        } else {
            self.iter()
                .filter(|q| !q.graph_name.is_default_graph())
                .map(quad_to_result)
                .collect()
        };
        Box::new(quads.into_iter())
    }

    fn internalize_term(&self, term: Term) -> Result<Term, Infallible> {
        Ok(term)
    }

    fn externalize_term(&self, term: Term) -> Result<Term, Infallible> {
        Ok(term)
    }
}

pub struct InternalQuad<D: QueryableDataset> {
    pub subject: D::InternalTerm,
    pub predicate: D::InternalTerm,
    pub object: D::InternalTerm,
    /// `None` if the quad is in the default graph
    pub graph_name: Option<D::InternalTerm>,
}

/// A term as understood by the expression evaluator
#[derive(Clone)]
pub enum ExpressionTerm {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    StringLiteral(String),
    LangStringLiteral {
        value: String,
        language: String,
    },
    BooleanLiteral(Boolean),
    IntegerLiteral(Integer),
    DecimalLiteral(Decimal),
    FloatLiteral(Float),
    DoubleLiteral(Double),
    DateTimeLiteral(DateTime),
    #[cfg(feature = "sep-0002")]
    DateLiteral(Date),
    #[cfg(feature = "sep-0002")]
    TimeLiteral(Time),
    #[cfg(feature = "calendar-ext")]
    GYearLiteral(GYear),
    #[cfg(feature = "calendar-ext")]
    GYearMonthLiteral(GYearMonth),
    #[cfg(feature = "calendar-ext")]
    GMonthLiteral(GMonth),
    #[cfg(feature = "calendar-ext")]
    GMonthDayLiteral(GMonthDay),
    #[cfg(feature = "calendar-ext")]
    GDayLiteral(GDay),
    #[cfg(feature = "sep-0002")]
    DurationLiteral(Duration),
    #[cfg(feature = "sep-0002")]
    YearMonthDurationLiteral(YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DayTimeDurationLiteral(DayTimeDuration),
    OtherTypedLiteral {
        value: String,
        datatype: NamedNode,
    },
    #[cfg(feature = "rdf-star")]
    Triple(Box<ExpressionTriple>),
}

impl PartialEq for ExpressionTerm {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
            && match (self, other) {
                (Self::NamedNode(l), Self::NamedNode(r)) => l == r,
                (Self::BlankNode(l), Self::BlankNode(r)) => l == r,
                (Self::StringLiteral(l), Self::StringLiteral(r)) => l == r,
                (
                    Self::LangStringLiteral {
                        value: lv,
                        language: ll,
                    },
                    Self::LangStringLiteral {
                        value: rv,
                        language: rl,
                    },
                ) => lv == rv && ll == rl,
                (Self::BooleanLiteral(l), Self::BooleanLiteral(r)) => l == r,
                (Self::IntegerLiteral(l), Self::IntegerLiteral(r)) => l == r,
                (Self::DecimalLiteral(l), Self::DecimalLiteral(r)) => l == r,
                (Self::FloatLiteral(l), Self::FloatLiteral(r)) => l.is_identical_with(*r),
                (Self::DoubleLiteral(l), Self::DoubleLiteral(r)) => l.is_identical_with(*r),
                (Self::DateTimeLiteral(l), Self::DateTimeLiteral(r)) => l == r,
                #[cfg(feature = "sep-0002")]
                (Self::DateLiteral(l), Self::DateLiteral(r)) => l == r,
                #[cfg(feature = "sep-0002")]
                (Self::TimeLiteral(l), Self::TimeLiteral(r)) => l == r,
                #[cfg(feature = "calendar-ext")]
                (Self::GYearMonthLiteral(l), Self::GYearMonthLiteral(r)) => l == r,
                #[cfg(feature = "calendar-ext")]
                (Self::GYearLiteral(l), Self::GYearLiteral(r)) => l == r,
                #[cfg(feature = "calendar-ext")]
                (Self::GMonthLiteral(l), Self::GMonthLiteral(r)) => l == r,
                #[cfg(feature = "calendar-ext")]
                (Self::GMonthDayLiteral(l), Self::GMonthDayLiteral(r)) => l == r,
                #[cfg(feature = "calendar-ext")]
                (Self::GDayLiteral(l), Self::GDayLiteral(r)) => l == r,
                #[cfg(feature = "sep-0002")]
                (Self::DurationLiteral(l), Self::DurationLiteral(r)) => l == r,
                #[cfg(feature = "sep-0002")]
                (Self::YearMonthDurationLiteral(l), Self::YearMonthDurationLiteral(r)) => l == r,
                #[cfg(feature = "sep-0002")]
                (Self::DayTimeDurationLiteral(l), Self::DayTimeDurationLiteral(r)) => l == r,
                (
                    Self::OtherTypedLiteral {
                        value: lv,
                        datatype: ld,
                    },
                    Self::OtherTypedLiteral {
                        value: rv,
                        datatype: rd,
                    },
                ) => lv == rv && ld == rd,
                #[cfg(feature = "rdf-star")]
                (Self::Triple(l), Self::Triple(r)) => l == r,
                (_, _) => unreachable!(),
            }
    }
}

impl Eq for ExpressionTerm {}

impl Hash for ExpressionTerm {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        discriminant(self).hash(state);
        match self {
            ExpressionTerm::NamedNode(v) => v.hash(state),
            ExpressionTerm::BlankNode(v) => v.hash(state),
            ExpressionTerm::StringLiteral(v) => v.hash(state),
            ExpressionTerm::LangStringLiteral { value, language } => (value, language).hash(state),
            ExpressionTerm::BooleanLiteral(v) => v.hash(state),
            ExpressionTerm::IntegerLiteral(v) => v.hash(state),
            ExpressionTerm::DecimalLiteral(v) => v.hash(state),
            ExpressionTerm::FloatLiteral(v) => v.to_be_bytes().hash(state),
            ExpressionTerm::DoubleLiteral(v) => v.to_be_bytes().hash(state),
            ExpressionTerm::DateTimeLiteral(v) => v.hash(state),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DateLiteral(v) => v.hash(state),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::TimeLiteral(v) => v.hash(state),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GYearLiteral(v) => v.hash(state),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GYearMonthLiteral(v) => v.hash(state),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GMonthLiteral(v) => v.hash(state),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GMonthDayLiteral(v) => v.hash(state),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GDayLiteral(v) => v.hash(state),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DurationLiteral(v) => v.hash(state),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::YearMonthDurationLiteral(v) => v.hash(state),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DayTimeDurationLiteral(v) => v.hash(state),
            ExpressionTerm::OtherTypedLiteral { value, datatype } => (value, datatype).hash(state),
            #[cfg(feature = "rdf-star")]
            ExpressionTerm::Triple(v) => v.hash(state),
        }
    }
}

impl From<Term> for ExpressionTerm {
    #[inline]
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(t) => Self::NamedNode(t),
            Term::BlankNode(t) => Self::BlankNode(t),
            Term::Literal(t) => {
                let (value, datatype, language) = t.destruct();
                if let Some(language) = language {
                    Self::LangStringLiteral { value, language }
                } else if let Some(datatype) = datatype {
                    parse_typed_literal(&value, datatype.as_str())
                        .unwrap_or(Self::OtherTypedLiteral { value, datatype })
                } else {
                    Self::StringLiteral(value)
                }
            }
            #[cfg(feature = "rdf-star")]
            Term::Triple(t) => Self::Triple(Box::new((*t).into())),
        }
    }
}

impl From<ExpressionTerm> for Term {
    #[inline]
    fn from(term: ExpressionTerm) -> Self {
        match term {
            ExpressionTerm::NamedNode(t) => t.into(),
            ExpressionTerm::BlankNode(t) => t.into(),
            ExpressionTerm::StringLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::LangStringLiteral { value, language } => {
                Literal::new_language_tagged_literal_unchecked(value, language).into()
            }
            ExpressionTerm::BooleanLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::IntegerLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::DecimalLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::FloatLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::DoubleLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::DateTimeLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DateLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::TimeLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GYearLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GYearMonthLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GMonthLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GMonthDayLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "calendar-ext")]
            ExpressionTerm::GDayLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DurationLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::YearMonthDurationLiteral(value) => Literal::from(value).into(),
            #[cfg(feature = "sep-0002")]
            ExpressionTerm::DayTimeDurationLiteral(value) => Literal::from(value).into(),
            ExpressionTerm::OtherTypedLiteral { value, datatype } => {
                Literal::new_typed_literal(value, datatype).into()
            }
            #[cfg(feature = "rdf-star")]
            ExpressionTerm::Triple(t) => Triple::from(*t).into(),
        }
    }
}

impl From<NamedNode> for ExpressionTerm {
    #[inline]
    fn from(term: NamedNode) -> Self {
        Self::NamedNode(term)
    }
}
impl From<bool> for ExpressionTerm {
    fn from(value: bool) -> Self {
        Self::BooleanLiteral(value.into())
    }
}

impl ExpressionTerm {
    /// Computes the term [Effective boolean value](https://www.w3.org/TR/sparql11-query/#ebv)
    pub(crate) fn effective_boolean_value(&self) -> Option<bool> {
        match self {
            ExpressionTerm::BooleanLiteral(value) => Some((*value).into()),
            ExpressionTerm::StringLiteral(value) => Some(!value.is_empty()),
            ExpressionTerm::FloatLiteral(value) => Some(Boolean::from(*value).into()),
            ExpressionTerm::DoubleLiteral(value) => Some(Boolean::from(*value).into()),
            ExpressionTerm::IntegerLiteral(value) => Some(Boolean::from(*value).into()),
            ExpressionTerm::DecimalLiteral(value) => Some(Boolean::from(*value).into()),
            _ => None,
        }
    }
}

fn parse_typed_literal(value: &str, datatype: &str) -> Option<ExpressionTerm> {
    Some(match datatype {
        "http://www.w3.org/2001/XMLSchema#boolean" => {
            ExpressionTerm::BooleanLiteral(value.parse().ok()?)
        }
        "http://www.w3.org/2001/XMLSchema#string" => ExpressionTerm::StringLiteral(value.into()),
        "http://www.w3.org/2001/XMLSchema#float" => {
            ExpressionTerm::FloatLiteral(value.parse().ok()?)
        }
        "http://www.w3.org/2001/XMLSchema#double" => {
            ExpressionTerm::DoubleLiteral(value.parse().ok()?)
        }
        "http://www.w3.org/2001/XMLSchema#decimal" => {
            ExpressionTerm::DecimalLiteral(value.parse().ok()?)
        }
        "http://www.w3.org/2001/XMLSchema#integer"
        | "http://www.w3.org/2001/XMLSchema#byte"
        | "http://www.w3.org/2001/XMLSchema#short"
        | "http://www.w3.org/2001/XMLSchema#int"
        | "http://www.w3.org/2001/XMLSchema#long"
        | "http://www.w3.org/2001/XMLSchema#unsignedByte"
        | "http://www.w3.org/2001/XMLSchema#unsignedShort"
        | "http://www.w3.org/2001/XMLSchema#unsignedInt"
        | "http://www.w3.org/2001/XMLSchema#unsignedLong"
        | "http://www.w3.org/2001/XMLSchema#positiveInteger"
        | "http://www.w3.org/2001/XMLSchema#negativeInteger"
        | "http://www.w3.org/2001/XMLSchema#nonPositiveInteger"
        | "http://www.w3.org/2001/XMLSchema#nonNegativeInteger" => {
            ExpressionTerm::IntegerLiteral(value.parse().ok()?)
        }
        "http://www.w3.org/2001/XMLSchema#dateTime"
        | "http://www.w3.org/2001/XMLSchema#dateTimeStamp" => {
            ExpressionTerm::DateTimeLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/2001/XMLSchema#time" => ExpressionTerm::TimeLiteral(value.parse().ok()?),
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/2001/XMLSchema#date" => ExpressionTerm::DateLiteral(value.parse().ok()?),
        #[cfg(feature = "calendar-ext")]
        "http://www.w3.org/2001/XMLSchema#gYearMonth" => {
            ExpressionTerm::GYearMonthLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "calendar-ext")]
        "http://www.w3.org/2001/XMLSchema#gYear" => {
            ExpressionTerm::GYearLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "calendar-ext")]
        "http://www.w3.org/2001/XMLSchema#gMonthDay" => {
            ExpressionTerm::GMonthDayLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "calendar-ext")]
        "http://www.w3.org/2001/XMLSchema#gDay" => ExpressionTerm::GDayLiteral(value.parse().ok()?),
        #[cfg(feature = "calendar-ext")]
        "http://www.w3.org/2001/XMLSchema#gMonth" => {
            ExpressionTerm::GMonthLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/2001/XMLSchema#duration" => {
            ExpressionTerm::DurationLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/2001/XMLSchema#yearMonthDuration" => {
            ExpressionTerm::YearMonthDurationLiteral(value.parse().ok()?)
        }
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/2001/XMLSchema#dayTimeDuration" => {
            ExpressionTerm::DayTimeDurationLiteral(value.parse().ok()?)
        }
        _ => return None,
    })
}

#[cfg(feature = "rdf-star")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ExpressionTriple {
    pub subject: ExpressionSubject,
    pub predicate: NamedNode,
    pub object: ExpressionTerm,
}

#[cfg(feature = "rdf-star")]
impl From<ExpressionTriple> for ExpressionTerm {
    #[inline]
    fn from(triple: ExpressionTriple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

#[cfg(feature = "rdf-star")]
impl From<Triple> for ExpressionTriple {
    #[inline]
    fn from(triple: Triple) -> Self {
        ExpressionTriple {
            subject: triple.subject.into(),
            predicate: triple.predicate,
            object: triple.object.into(),
        }
    }
}

#[cfg(feature = "rdf-star")]
impl From<ExpressionTriple> for Triple {
    #[inline]
    fn from(triple: ExpressionTriple) -> Self {
        Triple {
            subject: triple.subject.into(),
            predicate: triple.predicate,
            object: triple.object.into(),
        }
    }
}

#[cfg(feature = "rdf-star")]
impl ExpressionTriple {
    pub fn new(
        subject: ExpressionTerm,
        predicate: ExpressionTerm,
        object: ExpressionTerm,
    ) -> Option<Self> {
        if !matches!(
            subject,
            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) | ExpressionTerm::Triple(_)
        ) {
            return None;
        }
        if !matches!(predicate, ExpressionTerm::NamedNode(_)) {
            return None;
        }
        Some(Self {
            subject: match subject {
                ExpressionTerm::NamedNode(s) => ExpressionSubject::NamedNode(s),
                ExpressionTerm::BlankNode(s) => ExpressionSubject::BlankNode(s),
                ExpressionTerm::Triple(s) => ExpressionSubject::Triple(s),
                _ => return None,
            },
            predicate: if let ExpressionTerm::NamedNode(p) = predicate {
                p
            } else {
                return None;
            },
            object,
        })
    }
}

#[cfg(feature = "rdf-star")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ExpressionSubject {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Triple(Box<ExpressionTriple>),
}

#[cfg(feature = "rdf-star")]
impl From<ExpressionSubject> for ExpressionTerm {
    #[inline]
    fn from(subject: ExpressionSubject) -> Self {
        match subject {
            ExpressionSubject::NamedNode(s) => Self::NamedNode(s),
            ExpressionSubject::BlankNode(s) => Self::BlankNode(s),
            ExpressionSubject::Triple(s) => Self::Triple(s),
        }
    }
}

#[cfg(feature = "rdf-star")]
impl From<ExpressionSubject> for Subject {
    #[inline]
    fn from(subject: ExpressionSubject) -> Self {
        match subject {
            ExpressionSubject::NamedNode(s) => s.into(),
            ExpressionSubject::BlankNode(s) => s.into(),
            ExpressionSubject::Triple(s) => Triple::from(*s).into(),
        }
    }
}

#[cfg(feature = "rdf-star")]
impl From<Subject> for ExpressionSubject {
    #[inline]
    fn from(subject: Subject) -> Self {
        match subject {
            Subject::NamedNode(s) => Self::NamedNode(s),
            Subject::BlankNode(s) => Self::BlankNode(s),
            Subject::Triple(s) => ExpressionSubject::Triple(Box::new(ExpressionTriple::from(*s))),
        }
    }
}
