use crate::model::{BlankNodeRef, NamedNodeRef, TermRef};
use crate::sparql::QueryDataset;
use crate::storage::numeric_encoder::{
    insert_term, Decoder, EncodedTerm, EncodedTriple, StrHash, StrHashHasher, StrLookup,
};
use crate::storage::{CorruptionError, StorageError, StorageReader};
use hdt::Hdt;
use oxrdf::{QuadRef, Term};
use oxsdatatypes::Boolean;
use spareval::{ExpressionTerm, ExpressionTriple, InternalQuad, QueryableDataset};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::io::{Error, ErrorKind};
use std::iter::empty;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

pub struct HDTDataset {
    /// Path to the HDT file.
    path: String,

    /// HDT interface.
    hdt: Hdt,
}

/// Boundry over a Header-Dictionary-Triplies (HDT) storage layer.
pub struct HDTDatasetView {
    // collection of HDT files in the dataset
    hdts: Vec<HDTDataset>,

    /// In-memory string hashs.
    extra: RefCell<HashMap<StrHash, String>>,
}

/// Cloning opens the same file again.
impl Clone for HDTDatasetView {
    fn clone(&self) -> HDTDatasetView {
        let mut hdts: Vec<HDTDataset> = Vec::new();
        for dataset in self.hdts.iter() {
            let file = std::fs::File::open(&dataset.path).expect("error opening file");
            let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
            hdts.push(HDTDataset {
                path: dataset.path.clone(),
                hdt,
            })
        }

        Self {
            hdts,
            extra: self.extra.clone(),
        }
    }
}

impl HDTDatasetView {
    pub fn new(paths: Vec<String>) -> Self {
        let mut hdts: Vec<HDTDataset> = Vec::new();
        for path in paths.iter() {
            // TODO catch error and proceed to next file?
            let file = std::fs::File::open(path.as_str()).expect("error opening HDT file");
            let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
            hdts.push(HDTDataset {
                path: path.to_string(),
                hdt,
            })
        }

        Self {
            hdts,
            extra: RefCell::new(HashMap::default()),
        }
    }
    pub fn insert_str(&self, key: &StrHash, value: &str) {
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            e.insert(value.to_owned());
        }

        return;
    }

    /// Create the correct OxRDF term for a given resource string.  Slow,
    /// use the appropriate method if you know which type (Literal, URI,
    /// or blank node) the string has. Based on
    /// https://github.com/KonradHoeffner/hdt/blob/871db777db3220dc4874af022287975b31d72d3a/src/hdt_graph.rs#L64
    fn auto_term(&self, s: &str) -> Result<EncodedTerm, Error> {
        match s.chars().next() {
            None => Err(Error::new(ErrorKind::InvalidData, "empty input")),

            // Double-quote delimters are used around the string.
            Some('"') => match s.rfind('"') {
                None => Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("missing right quotation mark in literal string {s}"),
                )),

                Some(_) => {
                    let term = Term::from_str(s);
                    Ok(self.internalize_term(term.unwrap())?)
                }
            },

            // Underscore prefix indicating an Blank Node.
            Some('_') => {
                let term = oxrdf::BlankNode::new(&s[2..]).unwrap();
                let _val = self.internalize_term(term.into());
                Ok(EncodedTerm::from(BlankNodeRef::new_unchecked(*Arc::from(
                    &s[2..],
                ))))
            }

            // Double-quote delimiters not present. Underscore prefix
            // not present. Assuming a URI.
            _ => {
                // Note that Term::from_str() will not work for URIs
                // (OxRDF NamedNode) when the string is not within "<"
                // and ">" delimiters.
                let named_node = NamedNodeRef::new(*Arc::from(s)).unwrap();
                let t = named_node.into();

                Ok(self.internalize_term(t)?)
            }
        }
    }
    /// Convert triple string formats from OxRDF to HDT.
    fn encodedterm_to_hdt_bgp_str(&self, encoded_term: Option<&EncodedTerm>) -> Option<String> {
        let term_str = match encoded_term {
            None => None,
            Some(i) => {
                // It is not possible to get a string representation
                // directly from an EncodedTerm, so it must first be
                // decoded.
                let decoded_term = &self.decode_term(i).unwrap();
                let term = match decoded_term {
                    // Remove double quote delimiters from URIs.
                    Term::NamedNode(named_node) => Some(named_node.clone().into_string()),

                    // Get the string directly from literals and add
                    // quotes to work-around handling of "\n" being
                    // double-escaped.
                    // format!("\"{}\"", literal.value()),
                    Term::Literal(literal) => {
                        if literal.is_plain() {
                            Some(literal.to_string().replace("\\n", "\n"))
                        }
                        // For numbers and other typed literals return
                        // None as the BGP search will need to collect
                        // all possibilities before filtering.
                        else {
                            None
                        }
                    }

                    Term::BlankNode(_s) => Some(decoded_term.to_string()),

                    // Otherwise use the string directly.
                    _ => Some(decoded_term.to_string()),
                };

                term
            }
        };

        term_str
    }
}

impl StrLookup for HDTDatasetView {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(if let Some(value) = self.extra.borrow().get(key) {
            Some(value.clone())
        } else {
            None
        })
    }
}

impl QueryableDataset for HDTDatasetView {
    type InternalTerm = EncodedTerm;
    type Error = StorageError;

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<Option<&EncodedTerm>>,
    ) -> Box<dyn Iterator<Item = Result<InternalQuad<Self>, StorageError>>> {
        if let Some(graph_name) = graph_name {
            if let Some(graph_name) = graph_name {
                match graph_name {
                    EncodedTerm::DefaultGraph => (),
                    _ => panic!("HDT does not support named graphs."),
                }
            }
        }
        // Get string representations of the Oxigraph EncodedTerms.
        println!("{subject:?}");
        let s = self.encodedterm_to_hdt_bgp_str(subject);
        let p = self.encodedterm_to_hdt_bgp_str(predicate);
        let o = self.encodedterm_to_hdt_bgp_str(object);

        // Create a vector to hold the results.
        let mut v: Vec<Result<InternalQuad<_>, StorageError>> = Vec::new();

        for data in self.hdts.iter() {
            // Query HDT for BGP by string values.
            let results = data
                .hdt
                .triples_with_pattern(s.as_deref(), p.as_deref(), o.as_deref());

            // For each result
            for result in results {
                // Create OxRDF terms for the HDT result.
                let ex_s = self.auto_term(&(*result.0)).unwrap();
                let ex_p = self.auto_term(&(*result.1)).unwrap();
                let ex_o = self.auto_term(&(*result.2)).unwrap();

                // Add the result to the vector.
                v.push(Ok(InternalQuad {
                    subject: ex_s,
                    predicate: ex_p,
                    object: ex_o,
                    graph_name: Some(EncodedTerm::DefaultGraph),
                }));
            }
        }

        return Box::new(v.into_iter());
    }

    fn internalize_term(&self, term: Term) -> Result<EncodedTerm, StorageError> {
        let encoded = term.as_ref().into();
        insert_term(term.as_ref(), &encoded, &mut |key, value| {
            self.insert_str(key, value);
            Ok(())
        })?;
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
                return Err(CorruptionError::new("Unexpected default graph").into())
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
            EncodedTerm::Triple(t) => ExpressionTriple::new(
                self.externalize_expression_term(t.subject.clone())?,
                self.externalize_expression_term(t.predicate.clone())?,
                self.externalize_expression_term(t.object.clone())?,
            )
            .ok_or_else(|| CorruptionError::msg("Invalid RDF-star triple term in the storage"))?
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
            ExpressionTerm::Triple(t) => EncodedTriple {
                subject: self.internalize_expression_term(t.subject.into())?,
                predicate: self.internalize_expression_term(t.predicate.into())?,
                object: self.internalize_expression_term(t.object)?,
            }
            .into(),
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

pub struct DatasetView {
    reader: StorageReader,
    extra: RefCell<HashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
    dataset: EncodedDatasetSpec,
}

impl DatasetView {
    pub fn new(reader: StorageReader, dataset: &QueryDataset) -> Self {
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

impl QueryableDataset for DatasetView {
    type InternalTerm = EncodedTerm;
    type Error = StorageError;

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<Option<&EncodedTerm>>,
    ) -> Box<dyn Iterator<Item = Result<InternalQuad<Self>, StorageError>>> {
        if let Some(graph_name) = graph_name {
            if let Some(graph_name) = graph_name {
                if self
                    .dataset
                    .named
                    .as_ref()
                    .map_or(true, |d| d.contains(graph_name))
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
        }
    }

    fn internalize_term(&self, term: Term) -> Result<EncodedTerm, StorageError> {
        let encoded = term.as_ref().into();
        insert_term(term.as_ref(), &encoded, &mut |key, value| {
            self.insert_str(key, value);
            Ok(())
        })?;
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
                return Err(CorruptionError::new("Unexpected default graph").into())
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
            EncodedTerm::Triple(t) => ExpressionTriple::new(
                self.externalize_expression_term(t.subject.clone())?,
                self.externalize_expression_term(t.predicate.clone())?,
                self.externalize_expression_term(t.object.clone())?,
            )
            .ok_or_else(|| CorruptionError::msg("Invalid RDF-star triple term in the storage"))?
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
            ExpressionTerm::Triple(t) => EncodedTriple {
                subject: self.internalize_expression_term(t.subject.into())?,
                predicate: self.internalize_expression_term(t.predicate.into())?,
                object: self.internalize_expression_term(t.object)?,
            }
            .into(),
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

impl StrLookup for DatasetView {
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
