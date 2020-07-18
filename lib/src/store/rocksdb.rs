//! Store based on the [RocksDB](https://rocksdb.org/) key-value database.

use crate::model::*;
use crate::sparql::{GraphPattern, QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::{load_dataset, load_graph, ReadableEncodedStore, WritableEncodedStore};
use crate::{DatasetSyntax, GraphSyntax, Result};
use rocksdb::*;
use std::io::BufRead;
use std::mem::take;
use std::path::Path;
use std::str;
use std::sync::Arc;

/// Store based on the [RocksDB](https://rocksdb.org/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update it using SPARQL.
///
/// To use it, the `"rocksdb"` feature needs to be activated.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
/// use oxigraph::{Result, RocksDbStore};
/// use oxigraph::sparql::{QueryOptions, QueryResult};
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = RocksDbStore::open("example.db")?;
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
/// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// let results = prepared_query.exec()?;
/// if let QueryResult::Solutions(mut solutions) =  results {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// #
/// # }
/// # remove_dir_all("example.db")?;
/// # Result::Ok(())
/// ```
#[derive(Clone)]
pub struct RocksDbStore {
    db: Arc<DB>,
}

const ID2STR_CF: &str = "id2str";
const SPOG_CF: &str = "spog";
const POSG_CF: &str = "posg";
const OSPG_CF: &str = "ospg";
const GSPO_CF: &str = "gspo";
const GPOS_CF: &str = "gpos";
const GOSP_CF: &str = "gosp";

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&str; 7] = [
    ID2STR_CF, SPOG_CF, POSG_CF, OSPG_CF, GSPO_CF, GPOS_CF, GOSP_CF,
];

const MAX_TRANSACTION_SIZE: usize = 1024;

#[derive(Clone)]
struct RocksDbStoreHandle<'a> {
    db: &'a DB,
    id2str_cf: &'a ColumnFamily,
    spog_cf: &'a ColumnFamily,
    posg_cf: &'a ColumnFamily,
    ospg_cf: &'a ColumnFamily,
    gspo_cf: &'a ColumnFamily,
    gpos_cf: &'a ColumnFamily,
    gosp_cf: &'a ColumnFamily,
}

impl RocksDbStore {
    /// Opens a `RocksDbStore`
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_compaction_style(DBCompactionStyle::Universal);

        let new = Self {
            db: Arc::new(DB::open_cf(&options, path, &COLUMN_FAMILIES)?),
        };

        let mut transaction = new.handle().auto_transaction();
        transaction.set_first_strings()?;
        transaction.commit()?;

        Ok(new)
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn prepare_query<'a>(
        &'a self,
        query: &str,
        options: QueryOptions<'_>,
    ) -> Result<RocksDbPreparedQuery> {
        Ok(RocksDbPreparedQuery(SimplePreparedQuery::new(
            (*self).clone(),
            query,
            options,
        )?))
    }

    /// This is similar to `prepare_query`, but useful if a SPARQL query has already been parsed, which is the case when building `ServiceHandler`s for federated queries with `SERVICE` clauses. For examples, look in the tests.
    pub fn prepare_query_from_pattern<'a>(
        &'a self,
        graph_pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<RocksDbPreparedQuery> {
        Ok(RocksDbPreparedQuery(SimplePreparedQuery::new_from_pattern(
            (*self).clone(),
            graph_pattern,
            options,
        )?))
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// See `MemoryStore` for a usage example.
    pub fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> impl Iterator<Item = Result<Quad>> + 'a
    where
        Self: 'a,
    {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.into());
        self.handle()
            .encoded_quads_for_pattern(subject, predicate, object, graph_name)
            .map(move |quad| self.decode_quad(&quad?))
    }

    /// Checks if this store contains a given quad
    pub fn contains(&self, quad: &Quad) -> Result<bool> {
        let quad = quad.into();
        self.handle().contains(&quad)
    }

    /// Executes a transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// Nothing is done if the closure returns `Err`.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn transaction<'a>(
        &'a self,
        f: impl FnOnce(&mut RocksDbTransaction<'a>) -> Result<()>,
    ) -> Result<()> {
        let mut transaction = self.handle().transaction();
        f(&mut transaction)?;
        transaction.commit()
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
        let mut transaction = self.handle().auto_transaction();
        load_graph(&mut transaction, reader, syntax, to_graph_name, base_iri)?;
        transaction.commit()
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
        let mut transaction = self.handle().auto_transaction();
        load_dataset(&mut transaction, reader, syntax, base_iri)?;
        transaction.commit()
    }

    /// Adds a quad to this store.
    pub fn insert(&self, quad: &Quad) -> Result<()> {
        let mut transaction = self.handle().auto_transaction();
        let quad = transaction.encode_quad(quad)?;
        transaction.insert_encoded(&quad)?;
        transaction.commit()
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) -> Result<()> {
        let mut transaction = self.handle().auto_transaction();
        let quad = quad.into();
        transaction.remove_encoded(&quad)?;
        transaction.commit()
    }

    fn handle(&self) -> RocksDbStoreHandle<'_> {
        RocksDbStoreHandle {
            db: &self.db,
            id2str_cf: get_cf(&self.db, ID2STR_CF),
            spog_cf: get_cf(&self.db, SPOG_CF),
            posg_cf: get_cf(&self.db, POSG_CF),
            ospg_cf: get_cf(&self.db, OSPG_CF),
            gspo_cf: get_cf(&self.db, GSPO_CF),
            gpos_cf: get_cf(&self.db, GPOS_CF),
            gosp_cf: get_cf(&self.db, GOSP_CF),
        }
    }
}

impl StrLookup for RocksDbStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        Ok(self
            .db
            .get_cf(get_cf(&self.db, ID2STR_CF), &id.to_be_bytes())?
            .map(String::from_utf8)
            .transpose()?)
    }
}

impl ReadableEncodedStore for RocksDbStore {
    fn encoded_quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a> {
        Box::new(
            self.handle()
                .encoded_quads_for_pattern(subject, predicate, object, graph_name),
        )
    }
}

impl<'a> RocksDbStoreHandle<'a> {
    fn transaction(&self) -> RocksDbTransaction<'a> {
        RocksDbTransaction {
            inner: RocksDbInnerTransaction {
                handle: self.clone(),
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        }
    }

    fn auto_transaction(&self) -> RocksDbAutoTransaction<'a> {
        RocksDbAutoTransaction {
            inner: RocksDbInnerTransaction {
                handle: self.clone(),
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        }
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        write_spog_quad(&mut buffer, quad);
        Ok(self.db.get_pinned_cf(self.spog_cf, &buffer)?.is_some())
    }

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> DecodingIndexIterator<'a> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self
                            .spog_quads(encode_term_quad(subject, predicate, object, graph_name)),
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

    fn quads(&self) -> DecodingIndexIterator<'a> {
        self.spog_quads(Vec::default())
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingIndexIterator<'a> {
        self.spog_quads(encode_term(subject))
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.spog_quads(encode_term_pair(subject, predicate))
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.spog_quads(encode_term_triple(subject, predicate, object))
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.ospg_quads(encode_term_pair(object, subject))
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingIndexIterator<'a> {
        self.posg_quads(encode_term(predicate))
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.posg_quads(encode_term_pair(predicate, object))
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingIndexIterator<'a> {
        self.ospg_quads(encode_term(object))
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingIndexIterator<'a> {
        self.gspo_quads(encode_term(graph_name))
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gspo_quads(encode_term_pair(graph_name, subject))
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gosp_quads(encode_term_triple(graph_name, object, subject))
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gpos_quads(encode_term_pair(graph_name, predicate))
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gpos_quads(encode_term_triple(graph_name, predicate, object))
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator<'a> {
        self.gosp_quads(encode_term_pair(graph_name, object))
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.spog_cf, prefix, QuadEncoding::SPOG)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.posg_cf, prefix, QuadEncoding::POSG)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.ospg_cf, prefix, QuadEncoding::OSPG)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.gspo_cf, prefix, QuadEncoding::GSPO)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.gpos_cf, prefix, QuadEncoding::GPOS)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator<'a> {
        self.inner_quads(self.gosp_cf, prefix, QuadEncoding::GOSP)
    }

    fn inner_quads(
        &self,
        cf: &ColumnFamily,
        prefix: Vec<u8>,
        encoding: QuadEncoding,
    ) -> DecodingIndexIterator<'a> {
        let mut iter = self.db.raw_iterator_cf(cf);
        iter.seek(&prefix);
        DecodingIndexIterator {
            iter,
            prefix,
            encoding,
        }
    }
}

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/) for the `RocksDbStore`.
pub struct RocksDbPreparedQuery(SimplePreparedQuery<RocksDbStore>);

impl RocksDbPreparedQuery {
    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult<'_>> {
        self.0.exec()
    }
}

/// Allows to insert and delete quads during a transaction with the `RocksDbStore`.
pub struct RocksDbTransaction<'a> {
    inner: RocksDbInnerTransaction<'a>,
}

impl StrContainer for RocksDbTransaction<'_> {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.inner.insert_str(key, value);
        Ok(())
    }
}

impl WritableEncodedStore for RocksDbTransaction<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.insert(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.remove(quad)
    }
}

impl RocksDbTransaction<'_> {
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
        load_graph(self, reader, syntax, to_graph_name, base_iri)
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
        load_dataset(self, reader, syntax, base_iri)
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert(&mut self, quad: &Quad) -> Result<()> {
        let quad = self.encode_quad(quad)?;
        self.insert_encoded(&quad)
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: &Quad) -> Result<()> {
        let quad = quad.into();
        self.remove_encoded(&quad)
    }

    fn commit(self) -> Result<()> {
        self.inner.commit()
    }
}

struct RocksDbAutoTransaction<'a> {
    inner: RocksDbInnerTransaction<'a>,
}

impl StrContainer for RocksDbAutoTransaction<'_> {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.inner.insert_str(key, value);
        Ok(())
    }
}

impl WritableEncodedStore for RocksDbAutoTransaction<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.insert(quad)?;
        self.commit_if_big()
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.remove(quad)?;
        self.commit_if_big()
    }
}

impl RocksDbAutoTransaction<'_> {
    fn commit(self) -> Result<()> {
        self.inner.commit()
    }

    fn commit_if_big(&mut self) -> Result<()> {
        if self.inner.batch.len() > MAX_TRANSACTION_SIZE {
            self.inner.handle.db.write(take(&mut self.inner.batch))?;
        }
        Ok(())
    }
}

struct RocksDbInnerTransaction<'a> {
    handle: RocksDbStoreHandle<'a>,
    batch: WriteBatch,
    buffer: Vec<u8>,
}

impl RocksDbInnerTransaction<'_> {
    fn insert_str(&mut self, key: StrHash, value: &str) {
        self.batch
            .put_cf(self.handle.id2str_cf, &key.to_be_bytes(), value)
    }

    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        write_spog_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.spog_cf, &self.buffer, &[]);
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.posg_cf, &self.buffer, &[]);
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.ospg_cf, &self.buffer, &[]);
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.gspo_cf, &self.buffer, &[]);
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.gpos_cf, &self.buffer, &[]);
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.batch.put_cf(self.handle.gosp_cf, &self.buffer, &[]);
        self.buffer.clear();

        Ok(())
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        write_spog_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.spog_cf, &self.buffer);
        self.buffer.clear();

        write_posg_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.posg_cf, &self.buffer);
        self.buffer.clear();

        write_ospg_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.ospg_cf, &self.buffer);
        self.buffer.clear();

        write_gspo_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.gspo_cf, &self.buffer);
        self.buffer.clear();

        write_gpos_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.gpos_cf, &self.buffer);
        self.buffer.clear();

        write_gosp_quad(&mut self.buffer, quad);
        self.batch.delete_cf(self.handle.gosp_cf, &self.buffer);
        self.buffer.clear();

        Ok(())
    }

    fn commit(self) -> Result<()> {
        self.handle.db.write(self.batch)?;
        Ok(())
    }
}

#[allow(clippy::expect_used)]
fn get_cf<'a>(db: &'a DB, name: &str) -> &'a ColumnFamily {
    db.cf_handle(name)
        .expect("A column family that should exist in RocksDB does not exist")
}

fn encode_term(t: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t);
    vec
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    vec
}

fn encode_term_quad(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm, t4: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    write_term(&mut vec, t4);
    vec
}

struct DecodingIndexIterator<'a> {
    iter: DBRawIterator<'a>,
    prefix: Vec<u8>,
    encoding: QuadEncoding,
}

impl<'a> Iterator for DecodingIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        if let Some(key) = self.iter.key() {
            if key.starts_with(&self.prefix) {
                let result = self.encoding.decode(key);
                self.iter.next();
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[test]
fn store() -> Result<()> {
    use crate::model::*;
    use crate::*;
    use rand::random;
    use std::env::temp_dir;
    use std::fs::remove_dir_all;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::new("http://example.com")?;
    let main_o = Term::from(Literal::from(1));

    let main_quad = Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None);
    let all_o = vec![
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(0), None),
        Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None),
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(2), None),
    ];

    let mut repo_path = temp_dir();
    repo_path.push(random::<u128>().to_string());

    {
        let store = RocksDbStore::open(&repo_path)?;
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
    }

    remove_dir_all(&repo_path)?;
    Ok(())
}
