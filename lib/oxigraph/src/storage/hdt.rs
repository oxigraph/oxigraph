use crate::storage::StorageError;
use crate::storage::numeric_encoder::{
    Decoder, EncodedQuad, EncodedTerm, StrHash, StrHashHasher, StrLookup, insert_term,
};
use hdt::Hdt;
use oxrdf::{BlankNode, NamedNode, Term};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::hash::BuildHasherDefault;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

/// HDT-based read-only storage
#[derive(Clone)]
pub struct HdtStorage {
    hdt: Arc<Hdt>,
}

impl HdtStorage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let file = File::open(path).map_err(|e| StorageError::Io(e))?;
        let buf_reader = BufReader::new(file);
        let hdt = Hdt::read(buf_reader).map_err(|e| StorageError::Other(Box::new(e)))?;

        Ok(Self { hdt: Arc::new(hdt) })
    }

    pub fn snapshot(&self) -> HdtStorageReader<'static> {
        HdtStorageReader {
            storage: self.clone(),
            extra: RefCell::new(HashMap::with_hasher(
                BuildHasherDefault::<StrHashHasher>::default(),
            )),
            _lifetime: std::marker::PhantomData,
        }
    }
}

fn hdt_bgp_str_to_encodedterm(term_str: &str) -> (Term, EncodedTerm) {
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
                Err(_e) => todo!(),
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
    extra: RefCell<HashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
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
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            e.insert(value.to_owned());
        }
        return;
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> HdtDecodingQuadIterator<'a> {
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

        let subject_str = subject_term.map(|t| term_to_hdt_bgp_str(&t));
        let predicate_str = predicate_term.map(|t| term_to_hdt_bgp_str(&t));
        let object_str = object_term.map(|t| term_to_hdt_bgp_str(&t));

        let triples_result = self.storage.hdt.triples_with_pattern(
            subject_str.as_deref(),
            predicate_str.as_deref(),
            object_str.as_deref(),
        );

        // Collect and process all triples at once for better performance
        let encoded_triples: Vec<[EncodedTerm; 3]> = triples_result
            .map(|triple| {
                // Convert HDT triple to encoded terms and populate str_hash map
                let (subject_t, subject_enc) = hdt_bgp_str_to_encodedterm(&triple[0]);
                insert_term(subject_t.as_ref(), &subject_enc, &mut |key, value| {
                    self.insert_str(key, value);
                });

                let (predicate_t, predicate_enc) = hdt_bgp_str_to_encodedterm(&triple[1]);
                insert_term(predicate_t.as_ref(), &predicate_enc, &mut |key, value| {
                    self.insert_str(key, value);
                });

                let (object_t, object_enc) = hdt_bgp_str_to_encodedterm(&triple[2]);
                insert_term(object_t.as_ref(), &object_enc, &mut |key, value| {
                    self.insert_str(key, value);
                });

                [subject_enc, predicate_enc, object_enc]
            })
            .collect();

        HdtDecodingQuadIterator::new(encoded_triples.into_iter())
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
        Ok(self.extra.borrow().contains_key(key))
    }

    pub fn validate(&self) -> Result<(), StorageError> {
        // HDT files are validated when opened
        Ok(())
    }
}

/// Convert triple string formats from OxRDF to HDT.
fn term_to_hdt_bgp_str(term: &Term) -> String {
    match term {
        Term::NamedNode(named_node) => named_node.as_str().to_owned(),
        Term::Literal(literal) => literal.to_string(),
        Term::BlankNode(blank_node) => blank_node.to_string(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(_triple) => todo!(),
    }
}

impl StrLookup for HdtStorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self.extra.borrow().get(key).map(|s| s.clone()))
    }
}

/// Iterator over quads from HDT storage
pub struct HdtDecodingQuadIterator<'a> {
    triples_iter: Option<Box<dyn Iterator<Item = [EncodedTerm; 3]> + 'a>>,
}

impl<'a> HdtDecodingQuadIterator<'a> {
    fn new(triples_result: impl Iterator<Item = [EncodedTerm; 3]> + 'a) -> Self {
        Self {
            triples_iter: Some(Box::new(triples_result)),
        }
    }
}

impl Iterator for HdtDecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        let triples_iter = self.triples_iter.as_mut()?;

        triples_iter.next().map(|triple| {
            Ok(EncodedQuad {
                subject: triple[0].clone(),
                predicate: triple[1].clone(),
                object: triple[2].clone(),
                graph_name: EncodedTerm::DefaultGraph,
            })
        })
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
