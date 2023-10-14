use crate::model::{BlankNodeRef, LiteralRef, NamedNodeRef, Term, TermRef};
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

    fn encodedterm_to_hdt_str(&self, encoded_term: Option<&EncodedTerm>) -> Option<String> {
        let term_str = match encoded_term {
            None => None,
            Some(i) => {
                let decoded_term = &self.decode_term(i).unwrap();
                let term = match decoded_term {
                    Term::NamedNode(_) => {
                        let term_str = &decoded_term.to_string();
                        let unquoted = String::from(&term_str[1..term_str.len() - 1]);
                        unquoted
                    }
                    _ => decoded_term.to_string(),
                };

                Some(term)
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
            Some('"') => match s.rfind('"') {
                None => Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("missing right quotation mark in literal string {s}"),
                )),
                Some(index) => {
                    let lex = Arc::from(&s[1..index]);
                    let rest = &s[index + 1..];
                    // literal with no language tag and no datatype
                    if rest.is_empty() {
                        return Ok(EncodedTerm::from(LiteralRef::new_simple_literal(*lex)));
                    }
                    // either language tag or datatype
                    if let Some(tag_index) = rest.find('@') {
                        let tag = Arc::from(&rest[tag_index + 1..]);
                        return Ok(EncodedTerm::from(
                            LiteralRef::new_language_tagged_literal_unchecked(*lex, *tag),
                        ));
                    }
                    // datatype
                    let mut dt_split = rest.split("^^");
                    dt_split.next(); // empty
                    match dt_split.next() {
                        Some(dt) => {
                            let unquoted = &dt[1..dt.len() - 1];
                            let dt = unquoted; // TODO create a datatyped literal.
                            Ok(EncodedTerm::from(LiteralRef::new_simple_literal(*lex)))
                        }
                        None => Err(Error::new(
                            ErrorKind::InvalidData,
                            format!("empty datatype in {s}"),
                        )),
                    }
                }
            },
            Some('_') => Ok(EncodedTerm::from(BlankNodeRef::new_unchecked(*Arc::from(
                &s[2..],
            )))),
            _ => {
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
        let s = self.encodedterm_to_hdt_str(subject);
        let p = self.encodedterm_to_hdt_str(predicate);
        let o = self.encodedterm_to_hdt_str(object);

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
                ex_s.clone(),
                ex_p.clone(),
                ex_o.clone(),
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
