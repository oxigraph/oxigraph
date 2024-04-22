use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
use crate::storage::binary_encoder::{
    encode_term, encode_term_pair, encode_term_quad, encode_term_triple, write_gosp_quad,
    write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad, write_pos_quad,
    write_posg_quad, write_spo_quad, write_spog_quad, QuadEncoding, WRITTEN_TERM_MAX_SIZE,
};
pub use crate::storage::error::StorageError;
use crate::storage::numeric_encoder::{insert_term, EncodedQuad, EncodedTerm, StrHash, StrLookup};
use crate::storage::CorruptionError;
use oxrdf::Quad;
use std::cell::{BorrowMutError, Ref, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::mem::transmute;
use std::ops::Deref;
use std::rc::{Rc, Weak};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

const BULK_LOAD_BATCH_SIZE: u64 = 100_000;

/// Low level storage primitives
#[derive(Clone)]
pub struct MemoryStorage {
    content: Arc<RwLock<Content>>,
}

struct Content {
    id2str: HashMap<StrHash, String>,
    spog: BTreeSet<Vec<u8>>,
    posg: BTreeSet<Vec<u8>>,
    ospg: BTreeSet<Vec<u8>>,
    gspo: BTreeSet<Vec<u8>>,
    gpos: BTreeSet<Vec<u8>>,
    gosp: BTreeSet<Vec<u8>>,
    dspo: BTreeSet<Vec<u8>>,
    dpos: BTreeSet<Vec<u8>>,
    dosp: BTreeSet<Vec<u8>>,
    graphs: HashSet<EncodedTerm>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            content: Arc::new(RwLock::new(Content {
                id2str: HashMap::new(),
                spog: BTreeSet::new(),
                posg: BTreeSet::new(),
                ospg: BTreeSet::new(),
                gspo: BTreeSet::new(),
                gpos: BTreeSet::new(),
                gosp: BTreeSet::new(),
                dspo: BTreeSet::new(),
                dpos: BTreeSet::new(),
                dosp: BTreeSet::new(),
                graphs: HashSet::new(),
            })),
        }
    }

    pub fn snapshot(&self) -> MemoryStorageReader {
        MemoryStorageReader {
            content: MemoryStorageReaderContent::Simple(Arc::clone(&self.content)),
        }
    }

    pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
        &'b self,
        f: impl Fn(MemoryStorageWriter<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        f(MemoryStorageWriter {
            content: Rc::new(RefCell::new(
                self.content.write().map_err(poison_corruption_error)?,
            )),
        })
    }

    pub fn bulk_loader(&self) -> MemoryStorageBulkLoader {
        MemoryStorageBulkLoader {
            storage: self.clone(),
            hooks: Vec::new(),
        }
    }
}

pub struct MemoryStorageReader {
    content: MemoryStorageReaderContent,
}

enum MemoryStorageReaderContent {
    Simple(Arc<RwLock<Content>>),
    Transaction(Weak<RefCell<RwLockWriteGuard<'static, Content>>>),
}

impl MemoryStorageReader {
    pub fn len(&self) -> Result<usize, StorageError> {
        let content = self.content()?;
        Ok(content.gspo.len() + content.dspo.len())
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        let content = self.content()?;
        Ok(content.gspo.is_empty() && content.dspo.is_empty())
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        let content = self.content()?;
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(content.dspo.contains(&buffer))
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(content.gspo.contains(&buffer))
        }
    }

    pub fn quads(&self) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(Vec::new()),
            self.gspo_quads(Vec::new()),
        )
    }

    pub fn quads_for_subject(&self, subject: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term(subject)),
            self.spog_quads(encode_term(subject)),
        )
    }

    pub fn quads_for_subject_predicate(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term_pair(subject, predicate)),
            self.spog_quads(encode_term_pair(subject, predicate)),
        )
    }

    pub fn quads_for_subject_predicate_object(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term_triple(subject, predicate, object)),
            self.spog_quads(encode_term_triple(subject, predicate, object)),
        )
    }

    pub fn quads_for_subject_object(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dosp_quads(encode_term_pair(object, subject)),
            self.ospg_quads(encode_term_pair(object, subject)),
        )
    }

    pub fn quads_for_predicate(
        &self,
        predicate: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dpos_quads(encode_term(predicate)),
            self.posg_quads(encode_term(predicate)),
        )
    }

    pub fn quads_for_predicate_object(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dpos_quads(encode_term_pair(predicate, object)),
            self.posg_quads(encode_term_pair(predicate, object)),
        )
    }

    pub fn quads_for_object(&self, object: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dosp_quads(encode_term(object)),
            self.ospg_quads(encode_term(object)),
        )
    }

    pub fn quads_for_graph(&self, graph_name: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(Vec::new())
        } else {
            self.gspo_quads(encode_term(graph_name))
        })
    }

    pub fn quads_for_subject_graph(
        &self,
        subject: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term(subject))
        } else {
            self.gspo_quads(encode_term_pair(graph_name, subject))
        })
    }

    pub fn quads_for_subject_predicate_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_pair(subject, predicate))
        } else {
            self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
        })
    }

    pub fn quads_for_subject_predicate_object_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_triple(subject, predicate, object))
        } else {
            self.gspo_quads(encode_term_quad(graph_name, subject, predicate, object))
        })
    }

    pub fn quads_for_subject_object_graph(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term_pair(object, subject))
        } else {
            self.gosp_quads(encode_term_triple(graph_name, object, subject))
        })
    }

    pub fn quads_for_predicate_graph(
        &self,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term(predicate))
        } else {
            self.gpos_quads(encode_term_pair(graph_name, predicate))
        })
    }

    pub fn quads_for_predicate_object_graph(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(encode_term_triple(graph_name, predicate, object))
        })
    }

    pub fn quads_for_object_graph(
        &self,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term(object))
        } else {
            self.gosp_quads(encode_term_pair(graph_name, object))
        })
    }

    pub fn named_graphs(&self) -> MemoryDecodingGraphIterator {
        MemoryDecodingGraphIterator {
            iter: self
                .content()
                .unwrap()
                .graphs
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(), // TODO: propagate error?
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        Ok(self.content()?.graphs.contains(graph_name))
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().spog, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().posg, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().ospg, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().gspo, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().gpos, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().gosp, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().dspo, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().dpos, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: Vec<u8>) -> MemoryDecodingQuadIterator {
        Self::inner_quads(&self.content().unwrap().dosp, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        set: &BTreeSet<Vec<u8>>,
        prefix: Vec<u8>,
        encoding: QuadEncoding,
    ) -> MemoryDecodingQuadIterator {
        let start = prefix.clone();

        // We compute the end
        let mut end = prefix;
        let mut i = 1;
        while i <= end.len() && end[end.len() - i] == u8::MAX {
            i += 1;
        }

        let range = if i > end.len() {
            // No end
            set.range(start..)
        } else {
            let k = end.len() - i;
            end[k] += 1;
            set.range(start..end)
        };
        MemoryDecodingQuadIterator {
            iter: range.cloned().collect::<Vec<_>>().into_iter(),
            encoding,
        }
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        Ok(self.content()?.id2str.contains_key(key))
    }

    /// Validates that all the storage invariants held in the data
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn validate(&self) -> Result<(), StorageError> {
        Ok(()) // TODO
    }

    #[allow(unsafe_code)]

    fn content<'a>(&'a self) -> Result<ContentRef<'a>, StorageError> {
        Ok(match &self.content {
            MemoryStorageReaderContent::Simple(reader) => {
                ContentRef::Simple(reader.read().map_err(poison_corruption_error)?)
            }
            MemoryStorageReaderContent::Transaction(reader) => {
                let Some(rc) = reader.upgrade() else {
                    return Err(StorageError::Other(
                        "The transaction is already ended".into(),
                    ));
                };
                let element: Ref<'_, _> = rc.as_ref().borrow();
                // SAFETY: ok because we keep the Rc too inside of ContentRef
                let element = unsafe { transmute::<_, Ref<'a, _>>(element) };
                ContentRef::Transaction {
                    _rc: Rc::clone(&rc),
                    element,
                }
            }
        })
    }
}

impl StrLookup for MemoryStorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self.content()?.id2str.get(key).cloned())
    }
}

enum ContentRef<'a> {
    Simple(RwLockReadGuard<'a, Content>),
    Transaction {
        _rc: Rc<RefCell<RwLockWriteGuard<'static, Content>>>,
        element: Ref<'a, RwLockWriteGuard<'static, Content>>,
    },
}

impl<'a> Deref for ContentRef<'a> {
    type Target = Content;

    fn deref(&self) -> &Content {
        match self {
            ContentRef::Simple(r) => r,
            ContentRef::Transaction { element, .. } => element,
        }
    }
}

pub struct MemoryChainedDecodingQuadIterator {
    first: MemoryDecodingQuadIterator,
    second: Option<MemoryDecodingQuadIterator>,
}

impl MemoryChainedDecodingQuadIterator {
    fn new(first: MemoryDecodingQuadIterator) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(first: MemoryDecodingQuadIterator, second: MemoryDecodingQuadIterator) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for MemoryChainedDecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = self.second.as_mut() {
            second.next()
        } else {
            None
        }
    }
}

struct MemoryDecodingQuadIterator {
    iter: std::vec::IntoIter<Vec<u8>>,
    encoding: QuadEncoding,
}

impl Iterator for MemoryDecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.encoding.decode(&self.iter.next()?))
    }
}

pub struct MemoryDecodingGraphIterator {
    iter: std::vec::IntoIter<EncodedTerm>,
}

impl Iterator for MemoryDecodingGraphIterator {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(Ok)
    }
}

pub struct MemoryStorageWriter<'a> {
    content: Rc<RefCell<RwLockWriteGuard<'a, Content>>>,
}

impl<'a> MemoryStorageWriter<'a> {
    #[allow(unsafe_code)]
    pub fn reader(&self) -> MemoryStorageReader {
        // SAFETY: This transmute is safe because we take a weak reference and the only Rc reference used is guarded by the lifetime.
        let content = unsafe { transmute(&self.content) };
        MemoryStorageReader {
            content: MemoryStorageReaderContent::Transaction(Rc::downgrade(content)),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        let encoded = quad.into();
        Ok(if quad.graph_name.is_default_graph() {
            let mut buffer = Vec::new();
            write_spo_quad(&mut buffer, &encoded);
            if self.content.borrow_mut().dspo.insert(buffer) {
                let mut buffer = Vec::new();
                write_pos_quad(&mut buffer, &encoded);
                self.content.borrow_mut().dpos.insert(buffer);

                let mut buffer = Vec::new();
                write_osp_quad(&mut buffer, &encoded);
                self.content.borrow_mut().dosp.insert(buffer);

                self.insert_term(quad.subject.into(), &encoded.subject)?;
                self.insert_term(quad.predicate.into(), &encoded.predicate)?;
                self.insert_term(quad.object, &encoded.object)?;

                true
            } else {
                false
            }
        } else {
            let mut buffer = Vec::new();
            write_spog_quad(&mut buffer, &encoded);
            if self.content.borrow_mut().spog.insert(buffer) {
                let mut buffer = Vec::new();
                write_posg_quad(&mut buffer, &encoded);
                self.content.borrow_mut().posg.insert(buffer);

                let mut buffer = Vec::new();
                write_ospg_quad(&mut buffer, &encoded);
                self.content.borrow_mut().ospg.insert(buffer);

                let mut buffer = Vec::new();
                write_gspo_quad(&mut buffer, &encoded);
                self.content.borrow_mut().gspo.insert(buffer);

                let mut buffer = Vec::new();
                write_gpos_quad(&mut buffer, &encoded);
                self.content.borrow_mut().gpos.insert(buffer);

                let mut buffer = Vec::new();
                write_gosp_quad(&mut buffer, &encoded);
                self.content.borrow_mut().gosp.insert(buffer);

                self.insert_term(quad.subject.into(), &encoded.subject)?;
                self.insert_term(quad.predicate.into(), &encoded.predicate)?;
                self.insert_term(quad.object, &encoded.object)?;

                if self
                    .content
                    .borrow_mut()
                    .graphs
                    .insert(encoded.graph_name.clone())
                {
                    match quad.graph_name {
                        GraphNameRef::NamedNode(graph_name) => {
                            self.insert_term(graph_name.into(), &encoded.graph_name)?;
                        }
                        GraphNameRef::BlankNode(graph_name) => {
                            self.insert_term(graph_name.into(), &encoded.graph_name)?;
                        }
                        GraphNameRef::DefaultGraph => (),
                    }
                }
                true
            } else {
                false
            }
        })
    }

    pub fn insert_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        let encoded_graph_name = EncodedTerm::from(graph_name);
        Ok(
            if self
                .content
                .borrow_mut()
                .graphs
                .insert(encoded_graph_name.clone())
            {
                self.insert_term(graph_name.into(), &encoded_graph_name)?;
                true
            } else {
                false
            },
        )
    }

    fn insert_term(
        &mut self,
        term: TermRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<(), StorageError> {
        insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    }

    fn insert_str(&mut self, key: &StrHash, value: &str) -> Result<(), StorageError> {
        if self
            .content
            .borrow_mut()
            .id2str
            .entry(*key)
            .or_insert_with(|| value.into())
            == value
        {
            Ok(())
        } else {
            Err(StorageError::Other("Hash conflict for two strings".into()))
        }
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
        let mut buffer = Vec::new();
        Ok(if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            if content.dspo.remove(&buffer) {
                buffer.clear();
                write_pos_quad(&mut buffer, quad);
                content.dpos.remove(&buffer);

                buffer.clear();
                write_osp_quad(&mut buffer, quad);
                content.dosp.remove(&buffer);

                true
            } else {
                false
            }
        } else {
            write_spog_quad(&mut buffer, quad);
            if content.spog.remove(&buffer) {
                buffer.clear();
                write_posg_quad(&mut buffer, quad);
                content.posg.remove(&buffer);

                buffer.clear();
                write_ospg_quad(&mut buffer, quad);
                content.ospg.remove(&buffer);

                buffer.clear();
                write_gspo_quad(&mut buffer, quad);
                content.gspo.remove(&buffer);

                buffer.clear();
                write_gpos_quad(&mut buffer, quad);
                content.gpos.remove(&buffer);

                buffer.clear();
                write_gosp_quad(&mut buffer, quad);
                content.gosp.remove(&buffer);

                true
            } else {
                false
            }
        })
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        if graph_name.is_default_graph() {
            let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
            content.dspo.clear();
            content.dpos.clear();
            content.dosp.clear();
        } else {
            for quad in self.reader().quads_for_graph(&graph_name.into()) {
                self.remove_encoded(&quad?)?;
            }
        }
        Ok(())
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
        content.gspo.clear();
        content.gpos.clear();
        content.gosp.clear();
        content.spog.clear();
        content.posg.clear();
        content.ospg.clear();
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
        content.dspo.clear();
        content.dpos.clear();
        content.dosp.clear();
        content.gspo.clear();
        content.gpos.clear();
        content.gosp.clear();
        content.spog.clear();
        content.posg.clear();
        content.ospg.clear();
        Ok(())
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(
        &mut self,
        graph_name: &EncodedTerm,
    ) -> Result<bool, StorageError> {
        Ok(
            if self
                .content
                .try_borrow_mut()
                .map_err(borrow_mut_error)?
                .graphs
                .remove(graph_name)
            {
                for quad in self.reader().quads_for_graph(graph_name) {
                    self.remove_encoded(&quad?)?;
                }
                true
            } else {
                false
            },
        )
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
        content.gspo.clear();
        content.gpos.clear();
        content.gosp.clear();
        content.spog.clear();
        content.posg.clear();
        content.ospg.clear();
        content.graphs.clear();
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        let mut content = self.content.try_borrow_mut().map_err(borrow_mut_error)?;
        content.dspo.clear();
        content.dpos.clear();
        content.dosp.clear();
        content.gspo.clear();
        content.gpos.clear();
        content.gosp.clear();
        content.spog.clear();
        content.posg.clear();
        content.ospg.clear();
        content.graphs.clear();
        content.id2str.clear();
        Ok(())
    }
}

#[must_use]
pub struct MemoryStorageBulkLoader {
    storage: MemoryStorage,
    hooks: Vec<Box<dyn Fn(u64)>>,
}

impl MemoryStorageBulkLoader {
    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        // TODO: very naïve
        let mut done_counter = 0;
        for quad in quads {
            let quad = quad?;
            self.storage
                .transaction(|mut writer| writer.insert(quad.as_ref()))?;
            done_counter += 1;
            if done_counter % BULK_LOAD_BATCH_SIZE == 0 {
                for hook in &self.hooks {
                    hook(done_counter);
                }
            }
        }
        Ok(())
    }
}

fn poison_corruption_error<T>(_: PoisonError<T>) -> StorageError {
    CorruptionError::msg("Poisoned mutex").into()
}

fn borrow_mut_error(_: BorrowMutError) -> StorageError {
    StorageError::Other("Invalidated lock".into())
}
