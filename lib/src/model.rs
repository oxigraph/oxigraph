//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) using [OxRDF](https://crates.io/crates/oxrdf).

use crate::xsd::*;
use oxrdf::vocab::xsd;
pub use oxrdf::{
    dataset, graph, vocab, BlankNode, BlankNodeIdParseError, BlankNodeRef, Dataset, Graph,
    GraphName, GraphNameRef, IriParseError, LanguageTagParseError, Literal, LiteralRef, NamedNode,
    NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, Quad, QuadRef, Subject, SubjectRef, Term,
    TermParseError, TermRef, Triple, TripleRef,
};

impl From<Float> for Literal {
    #[inline]
    fn from(value: Float) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::FLOAT)
    }
}

impl From<Double> for Literal {
    #[inline]
    fn from(value: Double) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DOUBLE)
    }
}

impl From<Decimal> for Literal {
    #[inline]
    fn from(value: Decimal) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DECIMAL)
    }
}

impl From<DateTime> for Literal {
    #[inline]
    fn from(value: DateTime) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DATE_TIME)
    }
}

impl From<Time> for Literal {
    #[inline]
    fn from(value: Time) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::TIME)
    }
}

impl From<Date> for Literal {
    #[inline]
    fn from(value: Date) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DATE)
    }
}

impl From<GYearMonth> for Literal {
    #[inline]
    fn from(value: GYearMonth) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_YEAR_MONTH)
    }
}

impl From<GYear> for Literal {
    #[inline]
    fn from(value: GYear) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_YEAR)
    }
}

impl From<GMonthDay> for Literal {
    #[inline]
    fn from(value: GMonthDay) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_MONTH_DAY)
    }
}

impl From<GMonth> for Literal {
    #[inline]
    fn from(value: GMonth) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_MONTH)
    }
}

impl From<GDay> for Literal {
    #[inline]
    fn from(value: GDay) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::G_DAY)
    }
}

impl From<Duration> for Literal {
    #[inline]
    fn from(value: Duration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DURATION)
    }
}

impl From<YearMonthDuration> for Literal {
    #[inline]
    fn from(value: YearMonthDuration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::YEAR_MONTH_DURATION)
    }
}

impl From<DayTimeDuration> for Literal {
    #[inline]
    fn from(value: DayTimeDuration) -> Self {
        Self::new_typed_literal(value.to_string(), xsd::DAY_TIME_DURATION)
    }
}
