use crate::model::{BlankNodeRef, NamedNodeRef, Term, TermRef};
use crate::sparql::algebra::QueryDataset;
use crate::sparql::EvaluationError;
use crate::storage::numeric_encoder::{
    insert_term, Decoder, EncodedQuad, EncodedTerm, StrHash, StrHashHasher, StrLookup,
};
use crate::storage::{StorageError, StorageReader};
use hdt::Hdt;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::io::{Error, ErrorKind};
use std::iter::empty;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

/// Boundry between the query evaluator and the storage layer.
pub trait DatasetView: Clone {
    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>>;

    /// Add a hash value for a string to the in-memory hashmap.
    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        let term = term.into();
        let encoded = term.into();

        insert_term(term, &encoded, &mut |key, value| {
            self.insert_str(key, value);
            Ok(())
        })
        .unwrap();
        encoded
    }

    fn insert_str(&self, key: &StrHash, value: &str);
}

my_impl_for! { &T, Rc<T> }
// where
macro_rules! my_impl_for {(
    /* macro input */
    $( // <---------+ repetition operator,
        $Type:ty // | of `:ty`pes, that shall be named with the `$Type` "metavariable"
    ),* $(,)? // <--+ `,`-separated, with an optional trailing `,`
) => (
    /* macro output */
    $( // <- for each of the occurrences in input, emit:
        /// Blanket implementation for references to DatasetView.
        impl<T: DatasetView> DatasetView for $Type {
            // Implementation based on
            //
            // Gjengset, Jon. 2022. “Ergonomic Trait Implementations.”
            // In Rust for Rustaceans, 40–40. San Francisco, CA: No
            // Starch Press.
            //
            // Smith, Mark. 2023. “Rust Trait Implementations and
            // References.”  Judy2k’s Blog (blog). February 22,
            // 2023. https://www.judy.co.uk/blog/rust-traits-and-references/.
            //
            // Note that
            // https://docs.rs/syntactic-for/latest/syntactic_for/#impl-blocks
            // does not work with the Rc<T> syntax.

            fn encoded_quads_for_pattern(
                &self,
                subject: Option<&EncodedTerm>,
                predicate: Option<&EncodedTerm>,
                object: Option<&EncodedTerm>,
                graph_name: Option<&EncodedTerm>,
            ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
                return (**self).encoded_quads_for_pattern(subject, predicate, object, graph_name);
            }

            fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
                return (**self).encode_term(term);
            }

            fn insert_str(&self, key: &StrHash, value: &str) {
                return (**self).insert_str(key, value);
            }
        }
    )*
)}
use my_impl_for;

/// Boundry over a Header-Dictionary-Triplies (HDT) storage layer.
pub struct HDTDatasetView {
    /// Path to the HDT file.
    path: String,

    /// HDT interface.
    hdt: Hdt,

    /// In-memory string hashs.
    extra: RefCell<HashMap<StrHash, String>>,
}

/// Cloning opens the same file again.
impl Clone for HDTDatasetView {
    fn clone(&self) -> HDTDatasetView {
        let file = std::fs::File::open(&self.path).expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        Self {
            path: String::from(&self.path),
            hdt,
            extra: self.extra.clone(),
        }
    }
}

impl HDTDatasetView {
    pub fn new(path: &str) -> Self {
        let file = std::fs::File::open(path).expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        Self {
            path: String::from(path),
            hdt,
            extra: RefCell::new(HashMap::default()),
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

                    // Otherwise use the string directly.
                    _ => Some(decoded_term.to_string()),
                };

                term
            }
        };

        term_str
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
                    Ok(self.encode_term(&term.unwrap()))
                }
            },

            // Underscore prefix indicating an Blank Node.
            Some('_') => Ok(EncodedTerm::from(BlankNodeRef::new_unchecked(*Arc::from(
                &s[2..],
            )))),

            // Double-quote delimiters not present. Underscore prefix
            // not present. Assuming a URI.
            _ => {
                // Note that Term::from_str() will not work for URIs
                // (OxRDF NamedNode) when the string is not within "<"
                // and ">" delimiters.
                let named_node = NamedNodeRef::new(*Arc::from(s)).unwrap();
                self.encode_term(named_node);
                Ok(EncodedTerm::from(named_node))
            }
        }
    }
}

impl DatasetView for HDTDatasetView {
    /// Query the HDT by Basic Graph Pattern (BGP)
    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
        // The graph_name is unused as HDT does not have named graphs.
        if let Some(graph) = graph_name {
            match graph {
                EncodedTerm::DefaultGraph => (),
                _ => panic!("HDT does not support named graphs."),
            }
        }

        // Get string representations of the Oxigraph EncodedTerms.
        let s = self.encodedterm_to_hdt_bgp_str(subject);
        let p = self.encodedterm_to_hdt_bgp_str(predicate);
        let o = self.encodedterm_to_hdt_bgp_str(object);

        // Query HDT for BGP by string values.
        let results = self
            .hdt
            .triples_with_pattern(s.as_deref(), p.as_deref(), o.as_deref());

        // Create a vector to hold the results.
        let mut v: Vec<Result<EncodedQuad, EvaluationError>> = Vec::new();

        // For each result
        for result in results {
            // Create OxRDF terms for the HDT result.
            let ex_s = self.auto_term(&(*result.0)).unwrap();
            let ex_p = self.auto_term(&(*result.1)).unwrap();
            let ex_o = self.auto_term(&(*result.2)).unwrap();

            // Add the result to the vector.
            v.push(Ok(EncodedQuad::new(
                ex_s,
                ex_p,
                ex_o,
                EncodedTerm::DefaultGraph,
            )));
        }

        return Box::new(v.into_iter());
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            e.insert(value.to_owned());
        }

        return;
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

/// Boundry over a Key-Value Store storage layer.
#[derive(Clone)]
pub struct KVDatasetView {
    reader: StorageReader,
    extra: RefCell<HashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
    dataset: EncodedDatasetSpec,
}

impl KVDatasetView {
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

    fn store_encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> impl Iterator<Item = Result<EncodedQuad, EvaluationError>> + 'static {
        self.reader
            .quads_for_pattern(subject, predicate, object, graph_name)
            .map(|t| t.map_err(Into::into))
    }
}

impl DatasetView for KVDatasetView {
    #[allow(clippy::needless_collect)]
    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
        if let Some(graph_name) = graph_name {
            if graph_name.is_default_graph() {
                if let Some(default_graph_graphs) = &self.dataset.default {
                    if default_graph_graphs.len() == 1 {
                        // Single graph optimization
                        Box::new(
                            self.store_encoded_quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(&default_graph_graphs[0]),
                            )
                            .map(|quad| {
                                let quad = quad?;
                                Ok(EncodedQuad::new(
                                    quad.subject,
                                    quad.predicate,
                                    quad.object,
                                    EncodedTerm::DefaultGraph,
                                ))
                            }),
                        )
                    } else {
                        let iters = default_graph_graphs
                            .iter()
                            .map(|graph_name| {
                                self.store_encoded_quads_for_pattern(
                                    subject,
                                    predicate,
                                    object,
                                    Some(graph_name),
                                )
                            })
                            .collect::<Vec<_>>();
                        Box::new(iters.into_iter().flatten().map(|quad| {
                            let quad = quad?;
                            Ok(EncodedQuad::new(
                                quad.subject,
                                quad.predicate,
                                quad.object,
                                EncodedTerm::DefaultGraph,
                            ))
                        }))
                    }
                } else {
                    Box::new(
                        self.store_encoded_quads_for_pattern(subject, predicate, object, None)
                            .map(|quad| {
                                let quad = quad?;
                                Ok(EncodedQuad::new(
                                    quad.subject,
                                    quad.predicate,
                                    quad.object,
                                    EncodedTerm::DefaultGraph,
                                ))
                            }),
                    )
                }
            } else if self
                .dataset
                .named
                .as_ref()
                .map_or(true, |d| d.contains(graph_name))
            {
                Box::new(self.store_encoded_quads_for_pattern(
                    subject,
                    predicate,
                    object,
                    Some(graph_name),
                ))
            } else {
                Box::new(empty())
            }
        } else if let Some(named_graphs) = &self.dataset.named {
            let iters = named_graphs
                .iter()
                .map(|graph_name| {
                    self.store_encoded_quads_for_pattern(
                        subject,
                        predicate,
                        object,
                        Some(graph_name),
                    )
                })
                .collect::<Vec<_>>();
            Box::new(iters.into_iter().flatten())
        } else {
            Box::new(
                self.store_encoded_quads_for_pattern(subject, predicate, object, None)
                    .filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => !quad.graph_name.is_default_graph(),
                    }),
            )
        }
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            if !matches!(self.reader.contains_str(key), Ok(true)) {
                e.insert(value.to_owned());
            }
        }
    }
}

impl StrLookup for KVDatasetView {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(if let Some(value) = self.extra.borrow().get(key) {
            Some(value.clone())
        } else {
            self.reader.get_str(key)?
        })
    }
}

#[derive(Clone)]
struct EncodedDatasetSpec {
    default: Option<Vec<EncodedTerm>>,
    named: Option<Vec<EncodedTerm>>,
}
