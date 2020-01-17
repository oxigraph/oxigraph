use crate::store::numeric_encoder::*;
use crate::store::{Store, StoreConnection, StoreRepositoryConnection, StoreTransaction};
use crate::{Repository, Result};
use failure::format_err;
use rocksdb::*;
use std::io::Cursor;
use std::iter::{empty, once};
use std::mem::take;
use std::path::Path;
use std::str;

/// `Repository` implementation based on the [RocksDB](https://rocksdb.org/) key-value store
///
/// To use it, the `"rocksdb"` feature needs to be activated.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
/// use oxigraph::{Repository, RepositoryConnection, RepositoryTransaction, RocksDbRepository, Result};
/// use crate::oxigraph::sparql::{PreparedQuery, QueryOptions};
/// use oxigraph::sparql::QueryResult;
/// # use std::fs::remove_dir_all;
///
/// # {
/// let repository = RocksDbRepository::open("example.db")?;
/// let mut connection = repository.connection()?;
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad)?;
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// let results = prepared_query.exec()?;
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
/// }
/// #
/// # }
/// # remove_dir_all("example.db")?;
/// # Result::Ok(())
/// ```
pub struct RocksDbRepository {
    inner: RocksDbStore,
}

pub type RocksDbRepositoryConnection<'a> = StoreRepositoryConnection<RocksDbStoreConnection<'a>>;

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

struct RocksDbStore {
    db: DB,
}

#[derive(Clone)]
pub struct RocksDbStoreConnection<'a> {
    store: &'a RocksDbStore,
    id2str_cf: &'a ColumnFamily,
    spog_cf: &'a ColumnFamily,
    posg_cf: &'a ColumnFamily,
    ospg_cf: &'a ColumnFamily,
    gspo_cf: &'a ColumnFamily,
    gpos_cf: &'a ColumnFamily,
    gosp_cf: &'a ColumnFamily,
}

impl RocksDbRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inner: RocksDbStore::open(path)?,
        })
    }
}

impl<'a> Repository for &'a RocksDbRepository {
    type Connection = RocksDbRepositoryConnection<'a>;

    fn connection(self) -> Result<StoreRepositoryConnection<RocksDbStoreConnection<'a>>> {
        Ok(self.inner.connection()?.into())
    }
}

impl RocksDbStore {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_compaction_style(DBCompactionStyle::Universal);

        let new = Self {
            db: DB::open_cf(&options, path, &COLUMN_FAMILIES)?,
        };

        let mut transaction = (&new).connection()?.transaction();
        transaction.set_first_strings()?;
        transaction.commit()?;

        Ok(new)
    }
}

impl<'a> Store for &'a RocksDbStore {
    type Connection = RocksDbStoreConnection<'a>;

    fn connection(self) -> Result<RocksDbStoreConnection<'a>> {
        Ok(RocksDbStoreConnection {
            store: self,
            id2str_cf: get_cf(&self.db, ID2STR_CF)?,
            spog_cf: get_cf(&self.db, SPOG_CF)?,
            posg_cf: get_cf(&self.db, POSG_CF)?,
            ospg_cf: get_cf(&self.db, OSPG_CF)?,
            gspo_cf: get_cf(&self.db, GSPO_CF)?,
            gpos_cf: get_cf(&self.db, GPOS_CF)?,
            gosp_cf: get_cf(&self.db, GOSP_CF)?,
        })
    }
}

impl StrLookup for RocksDbStoreConnection<'_> {
    fn get_str(&self, id: u128) -> Result<Option<String>> {
        Ok(self
            .store
            .db
            .get_cf(self.id2str_cf, &id.to_le_bytes())?
            .map(|v| unsafe { String::from_utf8_unchecked(v) }))
    }
}

impl<'a> StoreConnection for RocksDbStoreConnection<'a> {
    type Transaction = RocksDbStoreTransaction<'a>;
    type AutoTransaction = RocksDbStoreAutoTransaction<'a>;

    fn transaction(&self) -> RocksDbStoreTransaction<'a> {
        RocksDbStoreTransaction {
            inner: RocksDbStoreInnerTransaction {
                connection: self.clone(),
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        }
    }

    fn auto_transaction(&self) -> RocksDbStoreAutoTransaction<'a> {
        RocksDbStoreAutoTransaction {
            inner: RocksDbStoreInnerTransaction {
                connection: self.clone(),
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        }
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        buffer.write_spog_quad(quad)?;
        Ok(self
            .store
            .db
            .get_pinned_cf(self.spog_cf, &buffer)?
            .is_some())
    }

    fn quads_for_pattern<'b>(
        &'b self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'b> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            let quad = EncodedQuad::new(subject, predicate, object, graph_name);
                            match self.contains(&quad) {
                                Ok(true) => Box::new(once(Ok(quad))),
                                Ok(false) => Box::new(empty()),
                                Err(error) => Box::new(once(Err(error))),
                            }
                        }
                        None => wrap_error(
                            self.quads_for_subject_predicate_object(subject, predicate, object),
                        ),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_predicate(subject, predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_object_graph(subject, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_object(subject, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_subject_graph(subject, graph_name))
                        }
                        None => wrap_error(self.quads_for_subject(subject)),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_predicate_object_graph(predicate, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_predicate_object(predicate, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_predicate_graph(predicate, graph_name))
                        }
                        None => wrap_error(self.quads_for_predicate(predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_object_graph(object, graph_name))
                        }
                        None => wrap_error(self.quads_for_object(object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(self.quads_for_graph(graph_name)),
                        None => wrap_error(self.quads()),
                    },
                },
            },
        }
    }
}

impl<'a> RocksDbStoreConnection<'a> {
    fn quads(&self) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.spog_quads(Vec::default())
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.spog_quads(encode_term(subject)?)
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.spog_quads(encode_term_pair(subject, predicate)?)
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.spog_quads(encode_term_triple(subject, predicate, object)?)
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.ospg_quads(encode_term_pair(object, subject)?)
    }

    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.posg_quads(encode_term(predicate)?)
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.posg_quads(encode_term_pair(predicate, object)?)
    }

    fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.ospg_quads(encode_term(object)?)
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gspo_quads(encode_term(graph_name)?)
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gspo_quads(encode_term_pair(graph_name, subject)?)
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gspo_quads(encode_term_triple(graph_name, subject, predicate)?)
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gosp_quads(encode_term_triple(graph_name, object, subject)?)
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gpos_quads(encode_term_pair(graph_name, predicate)?)
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gpos_quads(encode_term_triple(graph_name, predicate, object)?)
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.gosp_quads(encode_term_pair(graph_name, object)?)
    }

    fn spog_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.spog_cf, prefix, |buffer| {
            Cursor::new(buffer).read_spog_quad()
        })
    }

    fn posg_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.posg_cf, prefix, |buffer| {
            Cursor::new(buffer).read_posg_quad()
        })
    }

    fn ospg_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.ospg_cf, prefix, |buffer| {
            Cursor::new(buffer).read_ospg_quad()
        })
    }

    fn gspo_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.gspo_cf, prefix, |buffer| {
            Cursor::new(buffer).read_gspo_quad()
        })
    }

    fn gpos_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.gpos_cf, prefix, |buffer| {
            Cursor::new(buffer).read_gpos_quad()
        })
    }

    fn gosp_quads(
        &self,
        prefix: Vec<u8>,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.inner_quads(self.gosp_cf, prefix, |buffer| {
            Cursor::new(buffer).read_gosp_quad()
        })
    }

    fn inner_quads(
        &self,
        cf: &ColumnFamily,
        prefix: Vec<u8>,
        decode: impl Fn(&[u8]) -> Result<EncodedQuad> + 'a,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        let mut iter = self.store.db.raw_iterator_cf(cf)?;
        iter.seek(&prefix);
        Ok(DecodingIndexIterator {
            iter,
            prefix,
            decode,
        })
    }
}

pub struct RocksDbStoreTransaction<'a> {
    inner: RocksDbStoreInnerTransaction<'a>,
}

impl StrContainer for RocksDbStoreTransaction<'_> {
    fn insert_str(&mut self, key: u128, value: &str) -> Result<()> {
        self.inner.insert_str(key, value)
    }
}

impl StoreTransaction for RocksDbStoreTransaction<'_> {
    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.insert(quad)
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.remove(quad)
    }

    fn commit(self) -> Result<()> {
        self.inner.commit()
    }
}

pub struct RocksDbStoreAutoTransaction<'a> {
    inner: RocksDbStoreInnerTransaction<'a>,
}

impl StrContainer for RocksDbStoreAutoTransaction<'_> {
    fn insert_str(&mut self, key: u128, value: &str) -> Result<()> {
        self.inner.insert_str(key, value)
    }
}

impl StoreTransaction for RocksDbStoreAutoTransaction<'_> {
    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.insert(quad)?;
        self.commit_if_big()
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.inner.remove(quad)?;
        self.commit_if_big()
    }

    fn commit(self) -> Result<()> {
        self.inner.commit()
    }
}

impl RocksDbStoreAutoTransaction<'_> {
    fn commit_if_big(&mut self) -> Result<()> {
        if self.inner.batch.len() > MAX_TRANSACTION_SIZE {
            self.inner
                .connection
                .store
                .db
                .write(take(&mut self.inner.batch))?;
        }
        Ok(())
    }
}

struct RocksDbStoreInnerTransaction<'a> {
    connection: RocksDbStoreConnection<'a>,
    batch: WriteBatch,
    buffer: Vec<u8>,
}

impl RocksDbStoreInnerTransaction<'_> {
    fn insert_str(&mut self, key: u128, value: &str) -> Result<()> {
        self.batch
            .put_cf(self.connection.id2str_cf, &key.to_le_bytes(), value)?;
        Ok(())
    }

    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.buffer.write_spog_quad(quad)?;
        self.batch
            .put_cf(self.connection.spog_cf, &self.buffer, &[])?;
        self.buffer.clear();

        self.buffer.write_posg_quad(quad)?;
        self.batch
            .put_cf(self.connection.posg_cf, &self.buffer, &[])?;
        self.buffer.clear();

        self.buffer.write_ospg_quad(quad)?;
        self.batch
            .put_cf(self.connection.ospg_cf, &self.buffer, &[])?;
        self.buffer.clear();

        self.buffer.write_gspo_quad(quad)?;
        self.batch
            .put_cf(self.connection.gspo_cf, &self.buffer, &[])?;
        self.buffer.clear();

        self.buffer.write_gpos_quad(quad)?;
        self.batch
            .put_cf(self.connection.gpos_cf, &self.buffer, &[])?;
        self.buffer.clear();

        self.buffer.write_gosp_quad(quad)?;
        self.batch
            .put_cf(self.connection.gosp_cf, &self.buffer, &[])?;
        self.buffer.clear();

        Ok(())
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.buffer.write_spog_quad(quad)?;
        self.batch
            .delete_cf(self.connection.spog_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_posg_quad(quad)?;
        self.batch
            .delete_cf(self.connection.posg_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_ospg_quad(quad)?;
        self.batch
            .delete_cf(self.connection.ospg_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gspo_quad(quad)?;
        self.batch
            .delete_cf(self.connection.gspo_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gpos_quad(quad)?;
        self.batch
            .delete_cf(self.connection.gpos_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gosp_quad(quad)?;
        self.batch
            .delete_cf(self.connection.gosp_cf, &self.buffer)?;
        self.buffer.clear();

        Ok(())
    }

    fn commit(self) -> Result<()> {
        self.connection.store.db.write(self.batch)?;
        Ok(())
    }
}

fn get_cf<'a>(db: &'a DB, name: &str) -> Result<&'a ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| format_err!("column family {} not found", name))
}

fn wrap_error<'a, E: 'a, I: Iterator<Item = Result<E>> + 'a>(
    iter: Result<I>,
) -> Box<dyn Iterator<Item = Result<E>> + 'a> {
    match iter {
        Ok(iter) => Box::new(iter),
        Err(error) => Box::new(once(Err(error))),
    }
}

fn encode_term(t: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t)?;
    Ok(vec)
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    Ok(vec)
}

fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    vec.write_term(t3)?;
    Ok(vec)
}

struct DecodingIndexIterator<'a, F: Fn(&[u8]) -> Result<EncodedQuad>> {
    iter: DBRawIterator<'a>,
    prefix: Vec<u8>,
    decode: F,
}

impl<'a, F: Fn(&[u8]) -> Result<EncodedQuad>> Iterator for DecodingIndexIterator<'a, F> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        if let Some(key) = self.iter.key() {
            if key.starts_with(&self.prefix) {
                let result = (self.decode)(key);
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
fn repository() -> Result<()> {
    use crate::model::*;
    use crate::*;
    use rand::random;
    use std::env::temp_dir;
    use std::fs::remove_dir_all;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::parse("http://example.com")?;
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
        let repository = RocksDbRepository::open(&repo_path)?;
        let mut connection = repository.connection()?;
        connection.insert(&main_quad)?;
        for t in &all_o {
            connection.insert(&t)?;
        }

        let target = vec![main_quad];
        assert_eq!(
            connection
                .quads_for_pattern(None, None, None, None)
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), None, None, None)
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), Some(&main_p), None, None)
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), Some(&main_p), Some(&main_o), None)
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), Some(&main_p), Some(&main_o), Some(None))
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), Some(&main_p), None, Some(None))
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), None, Some(&main_o), None)
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), None, Some(&main_o), Some(None))
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(Some(&main_s), None, None, Some(None))
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(None, Some(&main_p), None, None)
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(None, Some(&main_p), Some(&main_o), None)
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(None, None, Some(&main_o), None)
                .collect::<Result<Vec<_>>>()?,
            target
        );
        assert_eq!(
            connection
                .quads_for_pattern(None, None, None, Some(None))
                .collect::<Result<Vec<_>>>()?,
            all_o
        );
        assert_eq!(
            connection
                .quads_for_pattern(None, Some(&main_p), Some(&main_o), Some(None))
                .collect::<Result<Vec<_>>>()?,
            target
        );
    }

    remove_dir_all(&repo_path)?;
    Ok(())
}
