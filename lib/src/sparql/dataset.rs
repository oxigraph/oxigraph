use crate::sparql::EvaluationError;
use crate::store::numeric_encoder::{
    EncodedQuad, EncodedTerm, MemoryStrStore, StrContainer, StrHash, StrId, StrLookup,
    WithStoreError,
};
use crate::store::ReadableEncodedStore;
use std::cell::RefCell;
use std::iter::empty;

pub(crate) struct DatasetView<S: ReadableEncodedStore> {
    store: S,
    extra: RefCell<MemoryStrStore>,
    default_graph_as_union: bool,
}

impl<S: ReadableEncodedStore> DatasetView<S> {
    pub fn new(store: S, default_graph_as_union: bool) -> Self {
        Self {
            store,
            extra: RefCell::new(MemoryStrStore::default()),
            default_graph_as_union,
        }
    }
}

impl<S: ReadableEncodedStore> WithStoreError for DatasetView<S> {
    type Error = EvaluationError;
    type StrId = DatasetStrId<S::StrId>;
}

impl<S: ReadableEncodedStore> StrLookup for DatasetView<S> {
    fn get_str(&self, id: DatasetStrId<S::StrId>) -> Result<Option<String>, EvaluationError> {
        match id {
            DatasetStrId::Store(id) => self.store.get_str(id).map_err(|e| e.into()),
            DatasetStrId::Temporary(id) => Ok(self.extra.borrow().get_str(id)?),
        }
    }

    fn get_str_id(&self, value: &str) -> Result<Option<DatasetStrId<S::StrId>>, EvaluationError> {
        if let Some(id) = self.extra.borrow().get_str_id(value)? {
            Ok(Some(DatasetStrId::Temporary(id)))
        } else {
            Ok(self
                .store
                .get_str_id(value)
                .map_err(|e| e.into())?
                .map(DatasetStrId::Store))
        }
    }
}

impl<S: ReadableEncodedStore> ReadableEncodedStore for DatasetView<S> {
    type QuadsIter =
        Box<dyn Iterator<Item = Result<EncodedQuad<DatasetStrId<S::StrId>>, EvaluationError>>>;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm<Self::StrId>>,
        predicate: Option<EncodedTerm<Self::StrId>>,
        object: Option<EncodedTerm<Self::StrId>>,
        graph_name: Option<EncodedTerm<Self::StrId>>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad<DatasetStrId<S::StrId>>, EvaluationError>>>
    {
        if let Some((subject, predicate, object, graph_name)) =
            try_map_quad_pattern(subject, predicate, object, graph_name)
        {
            if graph_name == None {
                Box::new(
                    map_iter(
                        self.store
                            .encoded_quads_for_pattern(subject, predicate, object, None),
                    )
                    .filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.graph_name != EncodedTerm::DefaultGraph,
                    }),
                )
            } else if graph_name == Some(EncodedTerm::DefaultGraph) && self.default_graph_as_union {
                Box::new(
                    map_iter(
                        self.store
                            .encoded_quads_for_pattern(subject, predicate, object, None),
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
                Box::new(map_iter(self.store.encoded_quads_for_pattern(
                    subject, predicate, object, graph_name,
                )))
            }
        } else {
            Box::new(empty())
        }
    }
}

fn map_iter<'a, I: StrId>(
    iter: impl Iterator<Item = Result<EncodedQuad<I>, impl Into<EvaluationError>>> + 'a,
) -> impl Iterator<Item = Result<EncodedQuad<DatasetStrId<I>>, EvaluationError>> + 'a {
    iter.map(|t| {
        t.map(|q| EncodedQuad {
            subject: q.subject.map_id(DatasetStrId::Store),
            predicate: q.predicate.map_id(DatasetStrId::Store),
            object: q.object.map_id(DatasetStrId::Store),
            graph_name: q.graph_name.map_id(DatasetStrId::Store),
        })
        .map_err(|e| e.into())
    })
}

type QuadPattern<I> = (
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
);

fn try_map_quad_pattern<I: StrId>(
    subject: Option<EncodedTerm<DatasetStrId<I>>>,
    predicate: Option<EncodedTerm<DatasetStrId<I>>>,
    object: Option<EncodedTerm<DatasetStrId<I>>>,
    graph_name: Option<EncodedTerm<DatasetStrId<I>>>,
) -> Option<QuadPattern<I>> {
    Some((
        transpose(subject.map(|t| t.try_map_id(unwrap_store_id)))?,
        transpose(predicate.map(|t| t.try_map_id(unwrap_store_id)))?,
        transpose(object.map(|t| t.try_map_id(unwrap_store_id)))?,
        transpose(graph_name.map(|t| t.try_map_id(unwrap_store_id)))?,
    ))
}

fn transpose<T>(o: Option<Option<T>>) -> Option<Option<T>> {
    match o {
        Some(Some(v)) => Some(Some(v)),
        Some(None) => None,
        None => Some(None),
    }
}

fn unwrap_store_id<I: StrId>(id: DatasetStrId<I>) -> Option<I> {
    match id {
        DatasetStrId::Store(id) => Some(id),
        DatasetStrId::Temporary(_) => None,
    }
}

impl<'a, S: ReadableEncodedStore> StrContainer for &'a DatasetView<S> {
    fn insert_str(&mut self, value: &str) -> Result<Self::StrId, EvaluationError> {
        if let Some(id) = self.store.get_str_id(value).map_err(|e| e.into())? {
            Ok(DatasetStrId::Store(id))
        } else {
            Ok(DatasetStrId::Temporary(
                self.extra.borrow_mut().insert_str(value)?,
            ))
        }
    }
}

#[derive(Eq, PartialEq, Debug, Copy, Clone, Hash)]
pub enum DatasetStrId<I: StrId> {
    Store(I),
    Temporary(StrHash),
}

impl<I: StrId> StrId for DatasetStrId<I> {}
