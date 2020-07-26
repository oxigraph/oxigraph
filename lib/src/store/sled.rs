//! Store based on the [Sled](https://sled.rs/) key-value database.

use crate::error::UnwrapInfallible;
use crate::model::*;
use crate::sparql::{Query, QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::{load_dataset, load_graph, ReadableEncodedStore, WritableEncodedStore};
use crate::{DatasetSyntax, Error, GraphSyntax, Result};
use sled::{Batch, Config, Iter, Tree};
use std::convert::{Infallible, TryInto};
use std::io::{BufRead, Cursor};
use std::path::Path;
use std::{fmt, str};

/// Store based on the [Sled](https://sled.rs/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update it using SPARQL.
///
/// To use it, the `"sled"` feature needs to be activated.
///
/// Warning: quad insertions and deletions are not (yet) atomic.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
/// use oxigraph::{Result, SledStore};
/// use oxigraph::sparql::{QueryOptions, QueryResult};
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
/// let results: Result<Vec<Quad>> = store.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// if let QueryResult::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// };
/// #
/// # };
/// # remove_dir_all("example.db")?;
/// # Result::Ok(())
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

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

impl SledStore {
    /// Opens a temporary `SledStore` that will be deleted after drop.
    pub fn new() -> Result<Self> {
        Self::do_open(&Config::new().temporary(true))
    }

    /// Opens a `SledStore`
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::do_open(&Config::new().path(path))
    }

    fn do_open(config: &Config) -> Result<Self> {
        let db = config.open()?;
        let new = Self {
            id2str: db.open_tree("id2str")?,
            quads: db.open_tree("quads")?,
        };
        DirectWriter::new(&new).set_first_strings()?;
        Ok(new)
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// See `MemoryStore` for a usage example.
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<Error>>,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        self.prepare_query(query, options)?.exec()
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn prepare_query(
        &self,
        query: impl TryInto<Query, Error = impl Into<Error>>,
        options: QueryOptions,
    ) -> Result<SledPreparedQuery> {
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
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> impl Iterator<Item = Result<Quad>> {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.into());
        let this = self.clone();
        self.encoded_quads_for_pattern(subject, predicate, object, graph_name)
            .map(move |quad| this.decode_quad(&quad?))
    }

    /// Checks if this store contains a given quad
    pub fn contains(&self, quad: &Quad) -> Result<bool> {
        let quad = quad.into();
        self.contains_encoded(&quad)
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
    /// See `MemoryStore` for a usage example.
    pub fn transaction<'a>(
        &'a self,
        f: impl FnOnce(&mut SledTransaction<'a>) -> Result<()>,
    ) -> Result<()> {
        let mut transaction = SledTransaction {
            inner: BatchWriter::new(self),
        };
        f(&mut transaction)?;
        transaction.inner.apply()
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn load_graph(
        &self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: &GraphName,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_graph(
            &mut DirectWriter::new(self),
            reader,
            syntax,
            to_graph_name,
            base_iri,
        )
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the quads in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_dataset(&mut DirectWriter::new(self), reader, syntax, base_iri)
    }

    /// Adds a quad to this store.
    pub fn insert(&self, quad: &Quad) -> Result<()> {
        let mut writer = DirectWriter::new(self);
        let quad = writer.encode_quad(quad)?;
        writer.insert_encoded(&quad)
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) -> Result<()> {
        let quad = quad.into();
        DirectWriter::new(self).remove_encoded(&quad)
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool> {
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

impl StrLookup for SledStore {
    type Error = Error;

    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        Ok(self
            .id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()?)
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

struct DirectWriter<'a> {
    store: &'a SledStore,
    buffer: Vec<u8>,
}

impl<'a> DirectWriter<'a> {
    fn new(store: &'a SledStore) -> Self {
        Self {
            store,
            buffer: Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1),
        }
    }
}

impl<'a> StrContainer for DirectWriter<'a> {
    type Error = Error;

    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.store
            .id2str
            .insert(key.to_be_bytes().as_ref(), value)?;
        Ok(())
    }
}

impl<'a> WritableEncodedStore for DirectWriter<'a> {
    type Error = Error;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        write_spog_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.store.quads.insert(self.buffer.as_slice(), &[])?;
        self.buffer.clear();

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        write_spog_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.store.quads.remove(self.buffer.as_slice())?;
        self.buffer.clear();

        Ok(())
    }
}

struct BatchWriter<'a> {
    store: &'a SledStore,
    quads: Batch,
    id2str: Batch,
    buffer: Vec<u8>,
}

impl<'a> BatchWriter<'a> {
    fn new(store: &'a SledStore) -> Self {
        Self {
            store,
            quads: Batch::default(),
            id2str: Batch::default(),
            buffer: Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1),
        }
    }
}

impl<'a> BatchWriter<'a> {
    fn apply(self) -> Result<()> {
        self.store.id2str.apply_batch(self.id2str)?;
        self.store.quads.apply_batch(self.quads)?;
        Ok(())
    }
}

impl<'a> StrContainer for BatchWriter<'a> {
    type Error = Infallible;

    fn insert_str(&mut self, key: StrHash, value: &str) -> std::result::Result<(), Infallible> {
        self.id2str.insert(key.to_be_bytes().as_ref(), value);
        Ok(())
    }
}

impl<'a> WritableEncodedStore for BatchWriter<'a> {
    type Error = Infallible;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        write_spog_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.quads.insert(self.buffer.as_slice(), &[]);
        self.buffer.clear();

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        write_spog_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.quads.remove(self.buffer.as_slice());
        self.buffer.clear();

        Ok(())
    }
}

/// Allows inserting and deleting quads during a transaction with the `SeldStore`.
pub struct SledTransaction<'a> {
    inner: BatchWriter<'a>,
}

impl SledTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    pub fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: &GraphName,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_graph(&mut self.inner, reader, syntax, to_graph_name, base_iri)
    }

    /// Loads a dataset file (i.e. quads) into the store. into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    pub fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_dataset(&mut self.inner, reader, syntax, base_iri)
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert(&mut self, quad: &Quad) {
        let quad = self.inner.encode_quad(quad).unwrap_infallible();
        self.inner.insert_encoded(&quad).unwrap_infallible()
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: &Quad) {
        let quad = quad.into();
        self.inner.remove_encoded(&quad).unwrap_infallible()
    }
}

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/) for the `SledStore`.
pub struct SledPreparedQuery(SimplePreparedQuery<SledStore>);

impl SledPreparedQuery {
    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult> {
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
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
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

fn decode_quad(encoded: &[u8]) -> Result<EncodedQuad> {
    let mut cursor = Cursor::new(&encoded[1..]);
    match encoded[0] {
        SPOG_PREFIX => Ok(cursor.read_spog_quad()?),
        POSG_PREFIX => Ok(cursor.read_posg_quad()?),
        OSPG_PREFIX => Ok(cursor.read_ospg_quad()?),
        GSPO_PREFIX => Ok(cursor.read_gspo_quad()?),
        GPOS_PREFIX => Ok(cursor.read_gpos_quad()?),
        GOSP_PREFIX => Ok(cursor.read_gosp_quad()?),
        _ => Err(Error::msg("Invalid quad type identifier")),
    }
}

#[test]
fn store() -> Result<()> {
    use crate::model::*;
    use crate::*;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::new("http://example.com")?;
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
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), None, None, None)
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), Some(&main_p), None, None)
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), Some(&main_p), Some(&main_o), None)
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(&main_s),
                Some(&main_p),
                Some(&main_o),
                Some(&GraphName::DefaultGraph)
            )
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(&main_s),
                Some(&main_p),
                None,
                Some(&GraphName::DefaultGraph)
            )
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), None, Some(&main_o), None)
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(&main_s),
                None,
                Some(&main_o),
                Some(&GraphName::DefaultGraph)
            )
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), None, None, Some(&GraphName::DefaultGraph))
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(&main_p), None, None)
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(&main_p), Some(&main_o), None)
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, Some(&main_o), None)
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, None, Some(&GraphName::DefaultGraph))
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(
                None,
                Some(&main_p),
                Some(&main_o),
                Some(&GraphName::DefaultGraph)
            )
            .collect::<Result<Vec<_>>>()?,
        target
    );

    Ok(())
}
