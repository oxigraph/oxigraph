use crate::storage::StorageError;
use crate::storage::numeric_encoder::{
    Decoder, EncodedQuad, EncodedTerm, StrHash, StrLookup, insert_term,
};
use hdt::Hdt;
use oxrdf::{BlankNode, NamedNode, Term};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

/// HDT-based read-only storage
#[derive(Clone)]
pub struct HdtStorage {
    hdt: Arc<Hdt>,
    // str_map: Arc<HashMap<StrHash, String>>,
    // term_map: Arc<HashMap<String, EncodedTerm>>,
}

impl HdtStorage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let file = File::open(path).map_err(|e| StorageError::Io(e))?;
        let buf_reader = BufReader::new(file);
        let hdt = Hdt::read(buf_reader).map_err(|e| StorageError::Other(Box::new(e)))?;

        // let mut str_map = HashMap::new();
        // let mut term_map = HashMap::new();

        // Pre-index all strings and terms from the HDT file
        // This is necessary because HDT uses strings directly while Oxigraph uses encoded terms
        // Self::build_indices(&hdt, &mut str_map, &mut term_map)?;

        Ok(Self {
            hdt: Arc::new(hdt),
            // str_map: Arc::new(str_map),
            // term_map: Arc::new(term_map),
        })
    }

    // fn build_indices(
    //     hdt: &Hdt,
    //     str_map: &mut HashMap<StrHash, String>,
    //     term_map: &mut HashMap<String, EncodedTerm>,
    // ) -> Result<(), StorageError> {
    //     // Index all triples to extract unique terms
    //     let all_triples = hdt.triples_with_pattern(None, None, None);

    //     for triple in all_triples {
    //         // Process subject
    //         Self::index_term(&triple[0], str_map, term_map)?;

    //         // Process predicate
    //         Self::index_term(&triple[1], str_map, term_map)?;

    //         // Process object
    //         Self::index_term(&triple[2], str_map, term_map)?;
    //     }

    //     Ok(())
    // }

    // fn index_term(
    //     term_str: &str,
    //     str_map: &mut HashMap<StrHash, String>,
    //     term_map: &mut HashMap<String, EncodedTerm>,
    // ) -> Result<(), StorageError> {
    //     use std::str::FromStr;

    //     if term_map.contains_key(term_str) {
    //         return Ok(());
    //     }

    //     // Parse the term and encode it
    //     let term = match term_str.chars().next().unwrap() {
    //         // Double-quote delimiters are used around the string.
    //         '"' => Term::from_str(term_str).expect("msg"),
    //         // Underscore prefix indicating a Blank Node.
    //         '_' => BlankNode::from_str(term_str).expect("msg").into(),
    //         // Double-quote delimiters not present. Underscore prefix
    //         // not present. Assuming a URI.
    //         _ => {
    //             // Note that Term::from_str() will not work for URIs (NamedNode) when the string is not within "<" and ">" delimiters.
    //             match NamedNode::new(term_str) {
    //                 Ok(n) => n.into(),
    //                 Err(e) => todo!(),
    //             }
    //         }
    //     };

    //     let encoded_term = EncodedTerm::from(term.as_ref());

    //     // // Store string mappings for lookups
    //     // match &encoded_term {
    //     //     EncodedTerm::NamedNode { iri_id } => {
    //     //         str_map.insert(*iri_id, term_str.to_string());
    //     //     }
    //     //     EncodedTerm::BigStringLiteral { value_id } => {
    //     //         str_map.insert(*value_id, term_str.to_string());
    //     //     }
    //     //     EncodedTerm::BigBlankNode { id_id } => {
    //     //         str_map.insert(*id_id, term_str.to_string());
    //     //     }
    //     //     // Handle other variants that use StrHash...
    //     //     _ => {}
    //     // }

    //     term_map.insert(term_str.to_string(), encoded_term);
    //     Ok(())
    // }

    pub fn snapshot(&self) -> HdtStorageReader<'static> {
        HdtStorageReader {
            storage: self.clone(),
            extra: RefCell::new(HashMap::default()),

            _lifetime: std::marker::PhantomData,
        }
    }
}

fn hdt_bgp_str_to_encodedterm(term_str: &str) -> (Term, EncodedTerm) {
    eprintln!("encodedterm from {term_str}");
    // Parse the term and encode it
    let term = match term_str.chars().next().unwrap() {
        // Double-quote delimiters are used around the string.
        '"' => Term::from_str(term_str).expect("msg"),
        // Underscore prefix indicating a Blank Node.
        '_' => BlankNode::from_str(term_str).expect("msg").into(),
        // Double-quote delimiters not present. Underscore prefix
        // not present. Assuming a URI.
        _ => {
            // Note that Term::from_str() will not work for URIs (NamedNode) when the string is not within "<" and ">" delimiters.
            match NamedNode::new(term_str) {
                Ok(n) => n.into(),
                Err(e) => todo!(),
            }
        }
    };

    (term.clone(), EncodedTerm::from(term.as_ref()))
}
/// HDT storage reader - provides read-only access to HDT data
#[derive(Clone)]
pub struct HdtStorageReader<'a> {
    storage: HdtStorage,
    /// In-memory string hashs.
    extra: RefCell<HashMap<StrHash, String>>,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a> HdtStorageReader<'a> {
    pub fn len(&self) -> Result<usize, StorageError> {
        Ok(self.storage.hdt.triples.adjlist_z.len())
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.storage.hdt.triples.adjlist_z.is_empty())
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        eprintln!("contains quad called");
        // Convert encoded quad back to string terms for HDT lookup
        let subject = term_to_hdt_bgp_str(&self.decode_term(&quad.subject)?);
        let predicate = term_to_hdt_bgp_str(&self.decode_term(&quad.predicate)?);
        let object = term_to_hdt_bgp_str(&self.decode_term(&quad.object)?);

        // HDT only supports default graph, so check if this is default graph
        if quad.graph_name != EncodedTerm::DefaultGraph {
            return Ok(false);
        }

        let mut triples = self.storage.hdt.triples_with_pattern(
            Some(subject.as_str()),
            Some(predicate.as_str()),
            Some(object.as_str()),
        );

        Ok(triples.next().is_some())
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        eprintln!("insert_str called");
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            e.insert(value.to_owned());
        }
        eprintln!("{:?}", self.extra.borrow().len());
        return;
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> HdtDecodingQuadIterator<'a> {
        eprintln!("quads_for_pattern");
        // Convert encoded terms to string patterns for HDT
        let subject_term = subject.and_then(|s| self.decode_term(s).ok());
        let predicate_term = predicate.and_then(|p| self.decode_term(p).ok());
        let object_term = object.and_then(|o| self.decode_term(o).ok());

        // HDT only supports default graph
        if let Some(gn) = graph_name {
            if *gn != EncodedTerm::DefaultGraph {
                panic!("hdt does not support named graphs")
            }
        }

        if let Some(t) = &subject_term {
            insert_term(t.into(), subject.unwrap(), &mut |key, value| {
                self.insert_str(key, value);
            })
        }
        if let Some(t) = &predicate_term {
            insert_term(t.into(), predicate.unwrap(), &mut |key, value| {
                self.insert_str(key, value);
            })
        }
        if let Some(t) = &object_term {
            insert_term(t.into(), object.unwrap(), &mut |key, value| {
                self.insert_str(key, value);
            })
        }

        let subject_str = subject_term.and_then(|t| Some(term_to_hdt_bgp_str(&t)));
        let predicate_str = predicate_term.and_then(|t| Some(term_to_hdt_bgp_str(&t)));
        let object_str = object_term.and_then(|t| Some(term_to_hdt_bgp_str(&t)));

        let triples_result = self.storage.hdt.triples_with_pattern(
            subject_str.as_deref(),
            predicate_str.as_deref(),
            object_str.as_deref(),
        );

        // Collect the iterator to eliminate lifetime issues
        let triples_vec: Vec<[Arc<str>; 3]> = triples_result.collect();
        HdtDecodingQuadIterator::new(triples_vec.into_iter(), self.clone())
    }

    pub fn named_graphs(&self) -> HdtDecodingGraphIterator<'a> {
        // HDT only supports the default graph
        HdtDecodingGraphIterator::empty()
    }

    pub fn contains_named_graph(&self, _graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        // HDT only supports the default graph
        Ok(false)
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        eprintln!("contains_str called");
        eprint!("{}", self.extra.borrow().len());
        Ok(self.extra.borrow().contains_key(key))
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        // HDT files are validated when opened
        Ok(())
    }
}

/// Convert triple string formats from OxRDF to HDT.
fn term_to_hdt_bgp_str(term: &Term) -> String {
    eprintln!("term to hdt called");
    match term {
        Term::NamedNode(named_node) => named_node.clone().into_string(),
        Term::Literal(literal) => literal.to_string(),
        Term::BlankNode(s) => s.to_string(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(triple) => todo!(),
    }
}

impl StrLookup for HdtStorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        eprintln!("lookup called");
        eprintln!("{:?}", self.extra.borrow().len());
        Ok(self.extra.borrow().get(key).cloned())
    }
}

/// Iterator over quads from HDT storage
pub struct HdtDecodingQuadIterator<'a> {
    triples_iter: Option<Box<dyn Iterator<Item = [Arc<str>; 3]> + 'a>>,
    str_hash: HdtStorageReader<'a>,
}

impl<'a> HdtDecodingQuadIterator<'a> {
    fn new(
        triples_result: impl Iterator<Item = [Arc<str>; 3]> + 'a,
        storage: HdtStorageReader<'a>,
    ) -> Self {
        Self {
            triples_iter: Some(Box::new(triples_result)),
            str_hash: storage,
        }
    }

    // fn empty() -> Self {
    //     // This should never actually be called - use empty_with_storage instead
    //     panic!("HdtDecodingQuadIterator::empty() should not be called directly")
    // }

    // fn empty_with_storage(storage: HdtStorage) -> Self {
    //     Self {
    //         triples_iter: None,
    //         storage,
    //     }
    // }
}

impl Iterator for HdtDecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        let triples_iter = self.triples_iter.as_mut()?;

        match triples_iter.next() {
            Some(triple) => {
                // Convert HDT triple to encoded quad
                let (subject_t, subject_enc) = hdt_bgp_str_to_encodedterm(&triple[0]);
                insert_term(subject_t.as_ref(), &subject_enc, &mut |key, value| {
                    self.str_hash.insert_str(key, value);
                });
                let (predicate_t, predicate_enc) = hdt_bgp_str_to_encodedterm(&triple[1]);
                insert_term(predicate_t.as_ref(), &predicate_enc, &mut |key, value| {
                    self.str_hash.insert_str(key, value);
                });
                let (object_t, object_enc) = hdt_bgp_str_to_encodedterm(&triple[2]);
                insert_term(object_t.as_ref(), &object_enc, &mut |key, value| {
                    self.str_hash.insert_str(key, value);
                });

                Some(Ok(EncodedQuad {
                    subject: subject_enc.clone(),
                    predicate: predicate_enc.clone(),
                    object: object_enc.clone(),
                    graph_name: EncodedTerm::DefaultGraph,
                }))
            }
            None => None,
        }
    }
}

/// Iterator over named graphs from HDT storage (always empty since HDT only supports default graph)
pub struct HdtDecodingGraphIterator<'a> {
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a> HdtDecodingGraphIterator<'a> {
    fn empty() -> Self {
        Self {
            _lifetime: std::marker::PhantomData,
        }
    }
}

impl Iterator for HdtDecodingGraphIterator<'_> {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        // HDT only supports default graph, so no named graphs
        None
    }
}
