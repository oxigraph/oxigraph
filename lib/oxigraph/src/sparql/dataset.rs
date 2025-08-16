use crate::sparql::QueryDataset;
#[cfg(feature = "rdf-12")]
use crate::storage::numeric_encoder::EncodedTriple;
use crate::storage::numeric_encoder::{
    Decoder, EncodedTerm, StrHash, StrHashHasher, StrLookup, insert_term,
};
use crate::storage::{CorruptionError, StorageError, StorageReader};
use oxrdf::Term;
use oxsdatatypes::Boolean;
#[cfg(feature = "rdf-12")]
use spareval::ExpressionTriple;
use spareval::{ExpressionTerm, InternalQuad, QueryableDataset};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::BuildHasherDefault;
use std::iter::empty;
#[cfg(feature = "rdf-12")]
use std::sync::Arc;

pub struct DatasetView<'a> {
    reader: StorageReader<'a>,
    extra: RefCell<HashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
    dataset: EncodedDatasetSpec,
}

impl<'a> DatasetView<'a> {
    pub fn new(reader: StorageReader<'a>, dataset: &QueryDataset) -> Self {
        let dataset = EncodedDatasetSpec {
            default: dataset
                .default_graph_graphs()
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
            named: dataset
                .available_named_graphs()
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
        };
        Self {
            reader,
            extra: RefCell::new(HashMap::default()),
            dataset,
        }
    }

    pub fn insert_str(&self, key: &StrHash, value: &str) {
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            if !matches!(self.reader.contains_str(key), Ok(true)) {
                e.insert(value.to_owned());
            }
        }
    }
}

impl<'a> QueryableDataset<'a> for DatasetView<'a> {
    type InternalTerm = EncodedTerm;
    type Error = StorageError;

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<Option<&EncodedTerm>>,
    ) -> impl Iterator<Item = Result<InternalQuad<EncodedTerm>, StorageError>> + use<'a> {
        let iter: Box<dyn Iterator<Item = Result<_, _>>> = if let Some(graph_name) = graph_name {
            if let Some(graph_name) = graph_name {
                if self
                    .dataset
                    .named
                    .as_ref()
                    .is_none_or(|d| d.contains(graph_name))
                {
                    Box::new(
                        self.reader
                            .quads_for_pattern(subject, predicate, object, Some(graph_name))
                            .map(|quad| {
                                let quad = quad?;
                                Ok(InternalQuad {
                                    subject: quad.subject,
                                    predicate: quad.predicate,
                                    object: quad.object,
                                    graph_name: if quad.graph_name.is_default_graph() {
                                        None
                                    } else {
                                        Some(quad.graph_name)
                                    },
                                })
                            }),
                    )
                } else {
                    Box::new(empty())
                }
            } else if let Some(default_graph_graphs) = &self.dataset.default {
                if default_graph_graphs.len() == 1 {
                    // Single graph optimization
                    Box::new(
                        self.reader
                            .quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(&default_graph_graphs[0]),
                            )
                            .map(|quad| {
                                let quad = quad?;
                                Ok(InternalQuad {
                                    subject: quad.subject,
                                    predicate: quad.predicate,
                                    object: quad.object,
                                    graph_name: None,
                                })
                            }),
                    )
                } else {
                    let iters = default_graph_graphs
                        .iter()
                        .map(|graph_name| {
                            self.reader.quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(graph_name),
                            )
                        })
                        .collect::<Vec<_>>();
                    Box::new(iters.into_iter().flatten().map(|quad| {
                        let quad = quad?;
                        Ok(InternalQuad {
                            subject: quad.subject,
                            predicate: quad.predicate,
                            object: quad.object,
                            graph_name: None,
                        })
                    }))
                }
            } else {
                Box::new(
                    self.reader
                        .quads_for_pattern(subject, predicate, object, None)
                        .map(|quad| {
                            let quad = quad?;
                            Ok(InternalQuad {
                                subject: quad.subject,
                                predicate: quad.predicate,
                                object: quad.object,
                                graph_name: None,
                            })
                        }),
                )
            }
        } else if let Some(named_graphs) = &self.dataset.named {
            let iters = named_graphs
                .iter()
                .map(|graph_name| {
                    self.reader
                        .quads_for_pattern(subject, predicate, object, Some(graph_name))
                })
                .collect::<Vec<_>>();
            Box::new(iters.into_iter().flatten().map(|quad| {
                let quad = quad?;
                Ok(InternalQuad {
                    subject: quad.subject,
                    predicate: quad.predicate,
                    object: quad.object,
                    graph_name: if quad.graph_name.is_default_graph() {
                        None
                    } else {
                        Some(quad.graph_name)
                    },
                })
            }))
        } else {
            Box::new(
                self.reader
                    .quads_for_pattern(subject, predicate, object, None)
                    .filter_map(|quad| {
                        let quad = match quad {
                            Ok(quad) => quad,
                            Err(e) => return Some(Err(e)),
                        };
                        Some(Ok(InternalQuad {
                            subject: quad.subject,
                            predicate: quad.predicate,
                            object: quad.object,
                            graph_name: if quad.graph_name.is_default_graph() {
                                return None;
                            } else {
                                Some(quad.graph_name)
                            },
                        }))
                    }),
            )
        };
        iter
    }

    fn internal_named_graphs(
        &self,
    ) -> impl Iterator<Item = Result<EncodedTerm, StorageError>> + use<'a> {
        self.reader.named_graphs()
    }

    fn contains_internal_graph_name(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        self.reader.contains_named_graph(graph_name)
    }

    fn internalize_term(&self, term: Term) -> Result<EncodedTerm, StorageError> {
        let encoded = term.as_ref().into();
        insert_term(term.as_ref(), &encoded, &mut |key, value| {
            self.insert_str(key, value)
        });
        Ok(encoded)
    }

    fn externalize_term(&self, term: EncodedTerm) -> Result<Term, StorageError> {
        self.decode_term(&term)
    }

    fn externalize_expression_term(
        &self,
        term: EncodedTerm,
    ) -> Result<ExpressionTerm, StorageError> {
        Ok(match term {
            EncodedTerm::DefaultGraph => {
                return Err(CorruptionError::new("Unexpected default graph").into());
            }
            EncodedTerm::BooleanLiteral(value) => ExpressionTerm::BooleanLiteral(value),
            EncodedTerm::FloatLiteral(value) => ExpressionTerm::FloatLiteral(value),
            EncodedTerm::DoubleLiteral(value) => ExpressionTerm::DoubleLiteral(value),
            EncodedTerm::IntegerLiteral(value) => ExpressionTerm::IntegerLiteral(value),
            EncodedTerm::DecimalLiteral(value) => ExpressionTerm::DecimalLiteral(value),
            EncodedTerm::DateTimeLiteral(value) => ExpressionTerm::DateTimeLiteral(value),
            EncodedTerm::TimeLiteral(value) => ExpressionTerm::TimeLiteral(value),
            EncodedTerm::DateLiteral(value) => ExpressionTerm::DateLiteral(value),
            EncodedTerm::GYearMonthLiteral(value) => ExpressionTerm::GYearMonthLiteral(value),
            EncodedTerm::GYearLiteral(value) => ExpressionTerm::GYearLiteral(value),
            EncodedTerm::GMonthDayLiteral(value) => ExpressionTerm::GMonthDayLiteral(value),
            EncodedTerm::GDayLiteral(value) => ExpressionTerm::GDayLiteral(value),
            EncodedTerm::GMonthLiteral(value) => ExpressionTerm::GMonthLiteral(value),
            EncodedTerm::DurationLiteral(value) => ExpressionTerm::DurationLiteral(value),
            EncodedTerm::YearMonthDurationLiteral(value) => {
                ExpressionTerm::YearMonthDurationLiteral(value)
            }
            EncodedTerm::DayTimeDurationLiteral(value) => {
                ExpressionTerm::DayTimeDurationLiteral(value)
            }
            #[cfg(feature = "rdf-12")]
            EncodedTerm::Triple(t) => ExpressionTriple::new(
                self.externalize_expression_term(t.subject.clone())?,
                self.externalize_expression_term(t.predicate.clone())?,
                self.externalize_expression_term(t.object.clone())?,
            )
            .ok_or_else(|| CorruptionError::msg("Invalid triple term in the storage"))?
            .into(),
            _ => self.decode_term(&term)?.into(), // No escape
        })
    }

    fn internalize_expression_term(
        &self,
        term: ExpressionTerm,
    ) -> Result<EncodedTerm, StorageError> {
        Ok(match term {
            ExpressionTerm::BooleanLiteral(value) => EncodedTerm::BooleanLiteral(value),
            ExpressionTerm::FloatLiteral(value) => EncodedTerm::FloatLiteral(value),
            ExpressionTerm::DoubleLiteral(value) => EncodedTerm::DoubleLiteral(value),
            ExpressionTerm::IntegerLiteral(value) => EncodedTerm::IntegerLiteral(value),
            ExpressionTerm::DecimalLiteral(value) => EncodedTerm::DecimalLiteral(value),
            ExpressionTerm::DateTimeLiteral(value) => EncodedTerm::DateTimeLiteral(value),
            ExpressionTerm::TimeLiteral(value) => EncodedTerm::TimeLiteral(value),
            ExpressionTerm::DateLiteral(value) => EncodedTerm::DateLiteral(value),
            ExpressionTerm::GYearMonthLiteral(value) => EncodedTerm::GYearMonthLiteral(value),
            ExpressionTerm::GYearLiteral(value) => EncodedTerm::GYearLiteral(value),
            ExpressionTerm::GMonthDayLiteral(value) => EncodedTerm::GMonthDayLiteral(value),
            ExpressionTerm::GDayLiteral(value) => EncodedTerm::GDayLiteral(value),
            ExpressionTerm::GMonthLiteral(value) => EncodedTerm::GMonthLiteral(value),
            ExpressionTerm::DurationLiteral(value) => EncodedTerm::DurationLiteral(value),
            ExpressionTerm::YearMonthDurationLiteral(value) => {
                EncodedTerm::YearMonthDurationLiteral(value)
            }
            ExpressionTerm::DayTimeDurationLiteral(value) => {
                EncodedTerm::DayTimeDurationLiteral(value)
            }
            #[cfg(feature = "rdf-12")]
            ExpressionTerm::Triple(t) => EncodedTerm::Triple(Arc::new(EncodedTriple {
                subject: self.internalize_expression_term(t.subject.into())?,
                predicate: self.internalize_expression_term(t.predicate.into())?,
                object: self.internalize_expression_term(t.object)?,
            })),
            _ => self.internalize_term(term.into())?, // No fast path
        })
    }

    fn internal_term_effective_boolean_value(
        &self,
        term: EncodedTerm,
    ) -> Result<Option<bool>, StorageError> {
        Ok(match term {
            EncodedTerm::BooleanLiteral(value) => Some(value.into()),
            EncodedTerm::SmallStringLiteral(value) => Some(!value.is_empty()),
            EncodedTerm::BigStringLiteral { .. } => {
                Some(false) // A big literal can't be empty
            }
            EncodedTerm::FloatLiteral(value) => Some(Boolean::from(value).into()),
            EncodedTerm::DoubleLiteral(value) => Some(Boolean::from(value).into()),
            EncodedTerm::IntegerLiteral(value) => Some(Boolean::from(value).into()),
            EncodedTerm::DecimalLiteral(value) => Some(Boolean::from(value).into()),
            _ => None,
        })
    }
}

impl StrLookup for DatasetView<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(if let Some(value) = self.extra.borrow().get(key) {
            Some(value.clone())
        } else {
            self.reader.get_str(key)?
        })
    }
}

struct EncodedDatasetSpec {
    default: Option<Vec<EncodedTerm>>,
    named: Option<Vec<EncodedTerm>>,
}
