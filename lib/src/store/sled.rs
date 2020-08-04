//! Store based on the [Sled](https://sled.rs/) key-value database.

use crate::error::invalid_data_error;
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use crate::sparql::{EvaluationError, Query, QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::{
    write_term, Decoder, ReadEncoder, StrContainer, StrHash, StrLookup, TermReader, WithStoreError,
    WriteEncoder, WRITTEN_TERM_MAX_SIZE,
};
use crate::store::{
    dump_dataset, dump_graph, get_encoded_quad_pattern, load_dataset, load_graph,
    ReadableEncodedStore, StoreOrParseError, WritableEncodedStore,
};
use sled::transaction::{
    ConflictableTransactionError, TransactionError, Transactional, TransactionalTree,
    UnabortableTransactionError,
};
use sled::{Config, Iter, Tree};
use std::convert::TryInto;
use std::error::Error;
use std::io::{BufRead, Cursor, Write};
use std::iter::{once, Once};
use std::path::Path;
use std::{fmt, io, str};

/// Store based on the [Sled](https://sled.rs/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update it using SPARQL.
///
/// To use it, the `"sled"` feature needs to be activated.
///
/// Warning: quad insertions and deletions are not (yet) atomic.
///
/// Usage example:
/// ```
/// use oxigraph::SledStore;
/// use oxigraph::sparql::{QueryOptions, QueryResult};
/// use oxigraph::model::*;
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = SledStore::open("example.db")?;
///
/// // insertion
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// store.insert(&quad)?;
///
/// // quad filter
/// let results: Result<Vec<Quad>,_> = store.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// if let QueryResult::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// };
/// #
/// # };
/// # remove_dir_all("example.db")?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct SledStore {
    id2str: Tree,
    quads: Tree,
}

const SPOG_PREFIX: u8 = 1;
const POSG_PREFIX: u8 = 2;
const OSPG_PREFIX: u8 = 3;
const GSPO_PREFIX: u8 = 4;
const GPOS_PREFIX: u8 = 5;
const GOSP_PREFIX: u8 = 6;
type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<StrHash>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<StrHash>;

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

impl SledStore {
    /// Opens a temporary `SledStore` that will be deleted after drop.
    pub fn new() -> Result<Self, io::Error> {
        Self::do_open(&Config::new().temporary(true))
    }

    /// Opens a `SledStore`
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        Self::do_open(&Config::new().path(path))
    }

    fn do_open(config: &Config) -> Result<Self, io::Error> {
        let db = config.open()?;
        Ok(Self {
            id2str: db.open_tree("id2str")?,
            quads: db.open_tree("quads")?,
        })
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// See `MemoryStore` for a usage example.
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResult, EvaluationError> {
        self.prepare_query(query, options)?.exec()
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn prepare_query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<SledPreparedQuery, EvaluationError> {
        Ok(SledPreparedQuery(SimplePreparedQuery::new(
            (*self).clone(),
            query,
            options,
        )?))
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// See `MemoryStore` for a usage example.
    pub fn quads_for_pattern(
        &self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> impl Iterator<Item = Result<Quad, io::Error>> {
        match get_encoded_quad_pattern(self, subject, predicate, object, graph_name) {
            Ok(Some((subject, predicate, object, graph_name))) => QuadsIter::Quads {
                iter: self.encoded_quads_for_pattern(subject, predicate, object, graph_name),
                store: self.clone(),
            },
            Ok(None) => QuadsIter::Empty,
            Err(error) => QuadsIter::Error(once(error)),
        }
    }

    /// Checks if this store contains a given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, io::Error> {
        if let Some(quad) = self.get_encoded_quad(quad.into())? {
            self.contains_encoded(&quad)
        } else {
            Ok(false)
        }
    }

    /// Returns the number of quads in the store
    pub fn len(&self) -> usize {
        self.quads.len() / 6
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
    }

    /// Executes a transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// Nothing is done if the closure returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::*;
    /// use oxigraph::store::sled::SledConflictableTransactionError;
    /// use std::convert::Infallible;
    ///
    /// let store = SledStore::new()?;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    ///
    /// // transaction
    /// store.transaction(|transaction| {
    ///     transaction.insert(&quad)?;
    ///     Ok(()) as Result<(),SledConflictableTransactionError<Infallible>>
    /// })?;
    ///
    /// // quad filter
    /// assert!(store.contains(&quad)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn transaction<T, E>(
        &self,
        f: impl Fn(SledTransaction<'_>) -> Result<T, SledConflictableTransactionError<E>>,
    ) -> Result<T, SledTransactionError<E>> {
        Ok((&self.id2str, &self.quads)
            .transaction(move |(id2str, quads)| Ok(f(SledTransaction { id2str, quads })?))?)
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut this = self;
        load_graph(&mut this, reader, format, to_graph_name.into(), base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the quads in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut this = self;
        load_dataset(&mut this, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store.
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        let mut this = self;
        let quad = this.encode_quad(quad.into())?;
        this.insert_encoded(&quad)
    }

    /// Removes a quad from this store.
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        if let Some(quad) = self.get_encoded_quad(quad.into())? {
            let mut this = self;
            this.remove_encoded(&quad)
        } else {
            Ok(())
        }
    }

    /// Dumps a store graph into a file.
    ///    
    /// See `MemoryStore` for a usage example.
    pub fn dump_graph<'a>(
        &self,
        writer: impl Write,
        format: GraphFormat,
        from_graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), io::Error> {
        dump_graph(
            self.quads_for_pattern(None, None, None, Some(from_graph_name.into()))
                .map(|q| Ok(q?.into())),
            writer,
            format,
        )
    }

    /// Dumps the store dataset into a file.
    ///    
    /// See `MemoryStore` for a usage example.
    pub fn dump_dataset(&self, writer: impl Write, format: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(
            self.quads_for_pattern(None, None, None, None),
            writer,
            format,
        )
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        write_spog_quad(&mut buffer, quad);
        Ok(self.quads.contains_key(buffer)?)
    }

    fn quads(&self) -> DecodingQuadIterator {
        self.inner_quads(&[SPOG_PREFIX])
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingQuadIterator {
        self.inner_quads(encode_term(SPOG_PREFIX, subject))
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(SPOG_PREFIX, subject, predicate))
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_triple(SPOG_PREFIX, subject, predicate, object))
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(OSPG_PREFIX, object, subject))
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingQuadIterator {
        self.inner_quads(encode_term(POSG_PREFIX, predicate))
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(POSG_PREFIX, predicate, object))
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingQuadIterator {
        self.inner_quads(encode_term(OSPG_PREFIX, object))
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingQuadIterator {
        self.inner_quads(encode_term(GSPO_PREFIX, graph_name))
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(GSPO_PREFIX, graph_name, subject))
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_triple(
            GSPO_PREFIX,
            graph_name,
            subject,
            predicate,
        ))
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_triple(GOSP_PREFIX, graph_name, object, subject))
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(GPOS_PREFIX, graph_name, predicate))
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_triple(
            GPOS_PREFIX,
            graph_name,
            predicate,
            object,
        ))
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.inner_quads(encode_term_pair(GPOS_PREFIX, graph_name, object))
    }

    fn inner_quads(&self, prefix: impl AsRef<[u8]>) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: self.quads.scan_prefix(prefix),
        }
    }
}

impl fmt::Display for SledStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.quads_for_pattern(None, None, None, None) {
            writeln!(f, "{}", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl WithStoreError for SledStore {
    type Error = io::Error;
    type StrId = StrHash;
}

impl StrLookup for SledStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, io::Error> {
        self.id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(invalid_data_error)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, io::Error> {
        let id = StrHash::new(value);
        Ok(if self.id2str.contains_key(&id.to_be_bytes())? {
            Some(id)
        } else {
            None
        })
    }
}

impl ReadableEncodedStore for SledStore {
    type QuadsIter = DecodingQuadIterator;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> DecodingQuadIterator {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.inner_quads(encode_term_quad(
                            SPOG_PREFIX,
                            subject,
                            predicate,
                            object,
                            graph_name,
                        )),
                        None => self.quads_for_subject_predicate_object(subject, predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name)
                        }
                        None => self.quads_for_subject_predicate(subject, predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_object_graph(subject, object, graph_name)
                        }
                        None => self.quads_for_subject_object(subject, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_subject_graph(subject, graph_name),
                        None => self.quads_for_subject(subject),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_predicate_object_graph(predicate, object, graph_name)
                        }
                        None => self.quads_for_predicate_object(predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_predicate_graph(predicate, graph_name),
                        None => self.quads_for_predicate(predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.quads_for_object_graph(object, graph_name),
                        None => self.quads_for_object(object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_graph(graph_name),
                        None => self.quads(),
                    },
                },
            },
        }
    }
}

impl<'a> StrContainer for &'a SledStore {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, io::Error> {
        let key = StrHash::new(value);
        self.id2str.insert(key.to_be_bytes().as_ref(), value)?;
        Ok(key)
    }
}

impl<'a> WritableEncodedStore for &'a SledStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        write_spog_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        write_spog_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        Ok(())
    }
}

/// Allows inserting and deleting quads during a transaction with the `SeldStore`.
pub struct SledTransaction<'a> {
    quads: &'a TransactionalTree,
    id2str: &'a TransactionalTree,
}

impl SledTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    pub fn load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        load_graph(&mut this, reader, format, to_graph_name.into(), base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store. into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        load_dataset(&mut this, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        let quad = this.encode_quad(quad.into())?;
        this.insert_encoded(&quad)
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        if let Some(quad) = this.get_encoded_quad(quad.into())? {
            this.remove_encoded(&quad)
        } else {
            Ok(())
        }
    }
}

impl<'a> WithStoreError for &'a SledTransaction<'a> {
    type Error = SledUnabortableTransactionError;
    type StrId = StrHash;
}

impl<'a> StrLookup for &'a SledTransaction<'a> {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, SledUnabortableTransactionError> {
        self.id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(|e| SledUnabortableTransactionError::Storage(invalid_data_error(e)))
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, SledUnabortableTransactionError> {
        let id = StrHash::new(value);
        Ok(if self.id2str.get(&id.to_be_bytes())?.is_some() {
            Some(id)
        } else {
            None
        })
    }
}

impl<'a> StrContainer for &'a SledTransaction<'a> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, SledUnabortableTransactionError> {
        let key = StrHash::new(value);
        self.id2str.insert(key.to_be_bytes().as_ref(), value)?;
        Ok(key)
    }
}

impl<'a> WritableEncodedStore for &'a SledTransaction<'a> {
    fn insert_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        write_spog_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.quads.insert(buffer.as_slice(), &[])?;
        buffer.clear();

        Ok(())
    }

    fn remove_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        write_spog_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.quads.remove(buffer.as_slice())?;
        buffer.clear();

        Ok(())
    }
}

/// Error returned by a Sled transaction
#[derive(Debug)]
pub enum SledTransactionError<T> {
    /// An failure returned by the API user that have aborted the transaction
    Abort(T),
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Abort(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl<T> From<TransactionError<T>> for SledTransactionError<T> {
    fn from(e: TransactionError<T>) -> Self {
        match e {
            TransactionError::Abort(e) => Self::Abort(e),
            TransactionError::Storage(e) => Self::Storage(e.into()),
        }
    }
}

/// An error returned from the transaction methods.
/// Should be returned as it is
#[derive(Debug)]
pub enum SledUnabortableTransactionError {
    #[doc(hidden)]
    Conflict,
    /// A regular error
    Storage(io::Error),
}

impl fmt::Display for SledUnabortableTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl Error for SledUnabortableTransactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SledUnabortableTransactionError> for EvaluationError {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Io(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl From<StoreOrParseError<SledUnabortableTransactionError, io::Error>>
    for SledUnabortableTransactionError
{
    fn from(e: StoreOrParseError<SledUnabortableTransactionError, io::Error>) -> Self {
        match e {
            StoreOrParseError::Store(e) => e,
            StoreOrParseError::Parse(e) => Self::Storage(e),
        }
    }
}

impl From<UnabortableTransactionError> for SledUnabortableTransactionError {
    fn from(e: UnabortableTransactionError) -> Self {
        match e {
            UnabortableTransactionError::Storage(e) => Self::Storage(e.into()),
            UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

/// An error returned from the transaction closure
#[derive(Debug)]
pub enum SledConflictableTransactionError<T> {
    /// A failure returned by the user that will abort the transaction
    Abort(T),
    #[doc(hidden)]
    Conflict,
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledConflictableTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
            Self::Abort(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledConflictableTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl<T> From<SledUnabortableTransactionError> for SledConflictableTransactionError<T> {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Storage(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl<T> From<SledConflictableTransactionError<T>> for ConflictableTransactionError<T> {
    fn from(e: SledConflictableTransactionError<T>) -> Self {
        match e {
            SledConflictableTransactionError::Abort(e) => ConflictableTransactionError::Abort(e),
            SledConflictableTransactionError::Conflict => ConflictableTransactionError::Conflict,
            SledConflictableTransactionError::Storage(e) => {
                ConflictableTransactionError::Storage(e.into())
            }
        }
    }
}

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/) for the `SledStore`.
pub struct SledPreparedQuery(SimplePreparedQuery<SledStore>);

impl SledPreparedQuery {
    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult, EvaluationError> {
        self.0.exec()
    }
}

fn encode_term(prefix: u8, t: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE + 1);
    vec.push(prefix);
    write_term(&mut vec, t);
    vec
}

fn encode_term_pair(prefix: u8, t1: EncodedTerm, t2: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE + 1);
    vec.push(prefix);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

fn encode_term_triple(prefix: u8, t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE + 1);
    vec.push(prefix);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    vec
}

fn encode_term_quad(
    prefix: u8,
    t1: EncodedTerm,
    t2: EncodedTerm,
    t3: EncodedTerm,
    t4: EncodedTerm,
) -> Vec<u8> {
    let mut vec = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);
    vec.push(prefix);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    write_term(&mut vec, t4);
    vec
}

pub(crate) struct DecodingQuadIterator {
    iter: Iter,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => decode_quad(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

fn write_spog_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(SPOG_PREFIX);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.graph_name);
}

fn write_posg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(POSG_PREFIX);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.graph_name);
}

fn write_ospg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(OSPG_PREFIX);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.graph_name);
}

fn write_gspo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(GSPO_PREFIX);
    write_term(sink, quad.graph_name);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
}

fn write_gpos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(GPOS_PREFIX);
    write_term(sink, quad.graph_name);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
}

fn write_gosp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    sink.push(GOSP_PREFIX);
    write_term(sink, quad.graph_name);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
}

fn decode_quad(encoded: &[u8]) -> Result<EncodedQuad, io::Error> {
    let mut cursor = Cursor::new(&encoded[1..]);
    match encoded[0] {
        SPOG_PREFIX => Ok(cursor.read_spog_quad()?),
        POSG_PREFIX => Ok(cursor.read_posg_quad()?),
        OSPG_PREFIX => Ok(cursor.read_ospg_quad()?),
        GSPO_PREFIX => Ok(cursor.read_gspo_quad()?),
        GPOS_PREFIX => Ok(cursor.read_gpos_quad()?),
        GOSP_PREFIX => Ok(cursor.read_gosp_quad()?),
        _ => Err(invalid_data_error(format!(
            "Invalid quad type identifier: {}",
            encoded[0]
        ))),
    }
}

enum QuadsIter {
    Quads {
        iter: DecodingQuadIterator,
        store: SledStore,
    },
    Error(Once<io::Error>),
    Empty,
}

impl Iterator for QuadsIter {
    type Item = Result<Quad, io::Error>;

    fn next(&mut self) -> Option<Result<Quad, io::Error>> {
        match self {
            Self::Quads { iter, store } => Some(match iter.next()? {
                Ok(quad) => store.decode_quad(&quad).map_err(|e| e.into()),
                Err(error) => Err(error),
            }),
            Self::Error(iter) => iter.next().map(Err),
            Self::Empty => None,
        }
    }
}

#[test]
fn store() -> Result<(), io::Error> {
    use crate::model::*;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::new("http://example.com").unwrap();
    let main_o = Term::from(Literal::from(1));

    let main_quad = Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None);
    let all_o = vec![
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(0), None),
        Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None),
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(2), None),
    ];

    let store = SledStore::new()?;
    store.insert(&main_quad)?;
    for t in &all_o {
        store.insert(t)?;
    }

    let target = vec![main_quad];
    assert_eq!(
        store
            .quads_for_pattern(None, None, None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), None, None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), Some(main_p.as_ref()), None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                None
            )
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                None,
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), None, Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                None,
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                None,
                None,
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(main_p.as_ref()), None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(main_p.as_ref()), Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::DefaultGraph))
            .collect::<Result<Vec<_>, _>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(
                None,
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        target
    );

    Ok(())
}
