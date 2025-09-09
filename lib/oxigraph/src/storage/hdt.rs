use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
use crate::storage::numeric_encoder::{
    Decoder, EncodedQuad, EncodedTerm, StrHash, StrLookup, insert_term,
};
use crate::storage::StorageError;
use hdt::Hdt;
use oxrdf::{NamedNode, Subject, Term, Triple};
use std::collections::HashMap;
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// HDT-based read-only storage
#[derive(Clone)]
pub struct HdtStorage {
    hdt: Arc<Hdt>,
    decoder: Arc<Decoder>,
    str_map: Arc<HashMap<StrHash, String>>,
    term_map: Arc<HashMap<String, EncodedTerm>>,
}

impl HdtStorage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let file = File::open(path)
            .map_err(|e| StorageError::Io(e))?;
        let buf_reader = BufReader::new(file);
        let hdt = Hdt::read(buf_reader)
            .map_err(|e| StorageError::Other(Box::new(e)))?;
        
        let mut decoder = Decoder::default();
        let mut str_map = HashMap::new();
        let mut term_map = HashMap::new();
        
        // Pre-index all strings and terms from the HDT file
        // This is necessary because HDT uses strings directly while Oxigraph uses encoded terms
        Self::build_indices(&hdt, &mut decoder, &mut str_map, &mut term_map)?;
        
        Ok(Self {
            hdt: Arc::new(hdt),
            decoder: Arc::new(decoder),
            str_map: Arc::new(str_map),
            term_map: Arc::new(term_map),
        })
    }
    
    fn build_indices(
        hdt: &Hdt<BufReader<File>>,
        decoder: &mut Decoder,
        str_map: &mut HashMap<StrHash, String>,
        term_map: &mut HashMap<String, EncodedTerm>,
    ) -> Result<(), StorageError> {
        // Index all triples to extract unique terms
        let all_triples = hdt.triples_with_pattern(None, None, None)
            .map_err(|e| StorageError::Other(Box::new(e)))?;
        
        for triple_result in all_triples {
            let triple = triple_result
                .map_err(|e| StorageError::Other(Box::new(e)))?;
            
            // Process subject
            Self::index_term(&triple.subject, decoder, str_map, term_map)?;
            
            // Process predicate  
            Self::index_term(&triple.predicate.as_str(), decoder, str_map, term_map)?;
            
            // Process object
            Self::index_term(&triple.object, decoder, str_map, term_map)?;
        }
        
        Ok(())
    }
    
    fn index_term(
        term_str: &str,
        decoder: &mut Decoder,
        str_map: &mut HashMap<StrHash, String>,
        term_map: &mut HashMap<String, EncodedTerm>,
    ) -> Result<(), StorageError> {
        if term_map.contains_key(term_str) {
            return Ok(());
        }
        
        // Parse the term and encode it
        let term = if term_str.starts_with('<') && term_str.ends_with('>') {
            // Named node
            let iri = &term_str[1..term_str.len()-1];
            Term::NamedNode(NamedNode::new_unchecked(iri))
        } else if term_str.starts_with("_:") {
            // Blank node
            Term::BlankNode(oxrdf::BlankNode::new_unchecked(&term_str[2..]))
        } else if term_str.starts_with('"') {
            // Literal - simplified parsing for now
            // In a real implementation, you'd need proper literal parsing
            Term::Literal(oxrdf::Literal::new_simple_literal(term_str))
        } else {
            // Fallback to simple literal
            Term::Literal(oxrdf::Literal::new_simple_literal(term_str))
        };
        
        let encoded_term = insert_term(term.as_ref(), decoder);
        
        // Store string mappings for lookups
        match &encoded_term {
            EncodedTerm::NamedNode { iri_id } => {
                str_map.insert(*iri_id, term_str.to_string());
            }
            EncodedTerm::BigStringLiteral { value_id } => {
                str_map.insert(*value_id, term_str.to_string());
            }
            EncodedTerm::BigBlankNode { id_id } => {
                str_map.insert(*id_id, term_str.to_string());
            }
            // Handle other variants that use StrHash...
            _ => {}
        }
        
        term_map.insert(term_str.to_string(), encoded_term);
        Ok(())
    }
    
    pub fn snapshot(&self) -> HdtStorageReader<'static> {
        HdtStorageReader {
            storage: self.clone(),
        }
    }
}

/// HDT storage reader - provides read-only access to HDT data
#[derive(Clone)]
pub struct HdtStorageReader<'a> {
    storage: HdtStorage,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a> HdtStorageReader<'a> {
    pub fn len(&self) -> Result<usize, StorageError> {
        // HDT doesn't provide direct count, so we need to count
        let triples = self.storage.hdt.triples_with_pattern(None, None, None)
            .map_err(|e| StorageError::Other(Box::new(e)))?;
        
        let mut count = 0;
        for _triple in triples {
            count += 1;
        }
        Ok(count)
    }
    
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        let mut triples = self.storage.hdt.triples_with_pattern(None, None, None)
            .map_err(|e| StorageError::Other(Box::new(e)))?;
        Ok(triples.next().is_none())
    }
    
    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        // Convert encoded quad back to string terms for HDT lookup
        let subject = self.decode_term(&quad.subject)?;
        let predicate = self.decode_term(&quad.predicate)?;  
        let object = self.decode_term(&quad.object)?;
        
        // HDT only supports default graph, so check if this is default graph
        if quad.graph_name != EncodedTerm::DefaultGraph {
            return Ok(false);
        }
        
        let mut triples = self.storage.hdt.triples_with_pattern(
            subject.as_deref(),
            predicate.as_deref(),
            object.as_deref()
        ).map_err(|e| StorageError::Other(Box::new(e)))?;
        
        Ok(triples.next().is_some())
    }
    
    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> HdtDecodingQuadIterator<'a> {
        // Convert encoded terms to string patterns for HDT
        let subject_str = subject.and_then(|s| self.decode_term(s).ok().flatten());
        let predicate_str = predicate.and_then(|p| self.decode_term(p).ok().flatten());
        let object_str = object.and_then(|o| self.decode_term(o).ok().flatten());
        
        // HDT only supports default graph
        if let Some(gn) = graph_name {
            if *gn != EncodedTerm::DefaultGraph {
                return HdtDecodingQuadIterator::empty();
            }
        }
        
        let triples_result = self.storage.hdt.triples_with_pattern(
            subject_str.as_deref(),
            predicate_str.as_deref(), 
            object_str.as_deref()
        );
        
        HdtDecodingQuadIterator::new(triples_result, &self.storage)
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
        Ok(self.storage.str_map.contains_key(key))
    }
    
    pub fn validate(&self) -> Result<(), StorageError> {
        // HDT files are validated when opened
        Ok(())
    }
    
    fn decode_term(&self, encoded_term: &EncodedTerm) -> Result<Option<String>, StorageError> {
        match encoded_term {
            EncodedTerm::DefaultGraph => Ok(None),
            EncodedTerm::NamedNode { iri_id } => {
                Ok(self.storage.str_map.get(iri_id).cloned())
            }
            EncodedTerm::BigStringLiteral { value_id } => {
                Ok(self.storage.str_map.get(value_id).cloned())
            }
            EncodedTerm::BigBlankNode { id_id } => {
                Ok(self.storage.str_map.get(id_id).cloned())
            }
            EncodedTerm::SmallStringLiteral(s) => {
                Ok(Some(s.as_str().to_string()))
            }
            EncodedTerm::SmallBlankNode(s) => {
                Ok(Some(format!("_:{}", s.as_str())))
            }
            // Handle other term types...
            _ => {
                // For other types, try to decode using the decoder
                let term = self.storage.decoder.decode_term(encoded_term)
                    .map_err(|e| StorageError::Corruption(e))?;
                Ok(Some(term.to_string()))
            }
        }
    }
}

impl StrLookup for HdtStorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self.storage.str_map.get(key).cloned())
    }
}

/// Iterator over quads from HDT storage
pub struct HdtDecodingQuadIterator<'a> {
    triples_iter: Option<Box<dyn Iterator<Item = Result<Triple, Box<dyn std::error::Error + Send + Sync>>> + 'a>>,
    storage: &'a HdtStorage,
}

impl<'a> HdtDecodingQuadIterator<'a> {
    fn new(
        triples_result: Result<impl Iterator<Item = Result<Triple, Box<dyn std::error::Error + Send + Sync>>> + 'a, Box<dyn std::error::Error + Send + Sync>>,
        storage: &'a HdtStorage,
    ) -> Self {
        match triples_result {
            Ok(iter) => Self {
                triples_iter: Some(Box::new(iter)),
                storage,
            },
            Err(_) => Self::empty_with_storage(storage),
        }
    }
    
    fn empty() -> Self {
        Self {
            triples_iter: None,
            storage: unsafe { std::mem::zeroed() }, // This is a hack for the empty case
        }
    }
    
    fn empty_with_storage(storage: &'a HdtStorage) -> Self {
        Self {
            triples_iter: None,
            storage,
        }
    }
}

impl Iterator for HdtDecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;
    
    fn next(&mut self) -> Option<Self::Item> {
        let triples_iter = self.triples_iter.as_mut()?;
        
        match triples_iter.next()? {
            Ok(triple) => {
                // Convert HDT triple to encoded quad
                let subject = self.storage.term_map.get(&triple.subject)?;
                let predicate = self.storage.term_map.get(triple.predicate.as_str())?;
                let object = self.storage.term_map.get(&triple.object)?;
                
                Some(Ok(EncodedQuad {
                    subject: subject.clone(),
                    predicate: predicate.clone(),  
                    object: object.clone(),
                    graph_name: EncodedTerm::DefaultGraph,
                }))
            }
            Err(e) => Some(Err(StorageError::Other(e))),
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