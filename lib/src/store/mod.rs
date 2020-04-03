//! Provides implementations of the `oxigraph::Repository` trait.

mod memory;
pub(crate) mod numeric_encoder;
#[cfg(feature = "rocksdb")]
mod rocksdb;

pub use crate::sparql::GraphPattern;
pub use crate::store::memory::MemoryRepository;
#[cfg(feature = "rocksdb")]
pub use crate::store::rocksdb::RocksDbRepository;

use crate::model::*;
use crate::repository::RepositoryTransaction;
use crate::sparql::{QueryOptions, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::{DatasetSyntax, Error, GraphSyntax, RepositoryConnection, Result};
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleParser};
use rio_xml::RdfXmlParser;
use std::collections::HashMap;
use std::io::BufRead;
use std::iter::Iterator;

/// Defines the `Store` traits that is used to have efficient binary storage
pub trait Store {
    type Connection: StoreConnection;

    fn connection(self) -> Result<Self::Connection>;
}

/// A connection to a `Store`
pub trait StoreConnection: StrLookup + Sized + Clone {
    type Transaction: StoreTransaction;
    type AutoTransaction: StoreTransaction;

    fn transaction(&self) -> Self::Transaction;

    fn auto_transaction(&self) -> Self::AutoTransaction;

    fn contains(&self, quad: &EncodedQuad) -> Result<bool>;

    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a>;
}

/// A transaction
pub trait StoreTransaction: StrContainer + Sized {
    fn insert(&mut self, quad: &EncodedQuad) -> Result<()>;

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()>;

    fn commit(self) -> Result<()>;
}

/// A `RepositoryConnection` from a `StoreConnection`
#[derive(Clone)]
pub struct StoreRepositoryConnection<S: StoreConnection> {
    inner: S,
}

impl<S: StoreConnection> From<S> for StoreRepositoryConnection<S> {
    fn from(inner: S) -> Self {
        Self { inner }
    }
}

impl<S: StoreConnection> StoreRepositoryConnection<S> {
    #[must_use]
    fn auto_transaction(&self) -> StoreRepositoryTransaction<S::AutoTransaction> {
        StoreRepositoryTransaction {
            inner: self.inner.auto_transaction(),
        }
    }
}

impl<S: StoreConnection> RepositoryConnection for StoreRepositoryConnection<S> {
    type Transaction = StoreRepositoryTransaction<S::Transaction>;
    type PreparedQuery = SimplePreparedQuery<S>;

    fn prepare_query(
        &self,
        query: &str,
        options: QueryOptions<'_>,
    ) -> Result<SimplePreparedQuery<S>> {
        SimplePreparedQuery::new(self.inner.clone(), query, options) //TODO: avoid clone
    }

    fn prepare_query_from_pattern(
        &self,
        pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<Self::PreparedQuery> {
        SimplePreparedQuery::new_from_pattern(self.inner.clone(), pattern, options)
        //TODO: avoid clone
    }

    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<Option<&NamedOrBlankNode>>,
    ) -> Box<dyn Iterator<Item = Result<Quad>> + 'a>
    where
        Self: 'a,
    {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.map_or(ENCODED_DEFAULT_GRAPH, |g| g.into()));
        Box::new(
            self.inner
                .quads_for_pattern(subject, predicate, object, graph_name)
                .map(move |quad| self.inner.decode_quad(&quad?)),
        )
    }

    fn contains(&self, quad: &Quad) -> Result<bool> {
        self.inner.contains(&quad.into())
    }

    fn transaction(&self, f: impl FnOnce(&mut Self::Transaction) -> Result<()>) -> Result<()> {
        let mut transaction = StoreRepositoryTransaction {
            inner: self.inner.transaction(),
        };
        f(&mut transaction)?;
        transaction.inner.commit()
    }

    fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let mut transaction = self.auto_transaction();
        transaction.load_graph(reader, syntax, to_graph_name, base_iri)?;
        transaction.inner.commit()
    }

    fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let mut transaction = self.auto_transaction();
        transaction.load_dataset(reader, syntax, base_iri)?;
        transaction.inner.commit()
    }

    fn insert(&mut self, quad: &Quad) -> Result<()> {
        let mut transaction = self.auto_transaction();
        transaction.insert(quad)?;
        transaction.inner.commit()
    }

    fn remove(&mut self, quad: &Quad) -> Result<()> {
        let mut transaction = self.auto_transaction();
        transaction.remove(quad)?;
        transaction.inner.commit()
    }
}

pub struct StoreRepositoryTransaction<T: StoreTransaction> {
    inner: T,
}

impl<T: StoreTransaction> RepositoryTransaction for StoreRepositoryTransaction<T> {
    fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let base_iri = base_iri.unwrap_or("");
        match syntax {
            GraphSyntax::NTriples => {
                self.load_from_triple_parser(NTriplesParser::new(reader)?, to_graph_name)
            }
            GraphSyntax::Turtle => {
                self.load_from_triple_parser(TurtleParser::new(reader, base_iri)?, to_graph_name)
            }
            GraphSyntax::RdfXml => {
                self.load_from_triple_parser(RdfXmlParser::new(reader, base_iri)?, to_graph_name)
            }
        }
    }

    fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let base_iri = base_iri.unwrap_or("");
        match syntax {
            DatasetSyntax::NQuads => self.load_from_quad_parser(NQuadsParser::new(reader)?),
            DatasetSyntax::TriG => self.load_from_quad_parser(TriGParser::new(reader, base_iri)?),
        }
    }

    fn insert(&mut self, quad: &Quad) -> Result<()> {
        let quad = self.inner.encode_quad(quad)?;
        self.inner.insert(&quad)
    }

    fn remove(&mut self, quad: &Quad) -> Result<()> {
        let quad = quad.into();
        self.inner.remove(&quad)
    }
}

impl<T: StoreTransaction> StoreRepositoryTransaction<T> {
    fn load_from_triple_parser<P: TriplesParser>(
        &mut self,
        mut parser: P,
        to_graph_name: Option<&NamedOrBlankNode>,
    ) -> Result<()>
    where
        Error: From<P::Error>,
    {
        let mut bnode_map = HashMap::default();
        let graph_name = if let Some(graph_name) = to_graph_name {
            self.inner.encode_named_or_blank_node(graph_name)?
        } else {
            EncodedTerm::DefaultGraph
        };
        parser.parse_all(&mut move |t| {
            let quad = self
                .inner
                .encode_rio_triple_in_graph(t, graph_name, &mut bnode_map)?;
            self.inner.insert(&quad)
        })
    }

    fn load_from_quad_parser<P: QuadsParser>(&mut self, mut parser: P) -> Result<()>
    where
        Error: From<P::Error>,
    {
        let mut bnode_map = HashMap::default();
        parser.parse_all(&mut move |q| {
            let quad = self.inner.encode_rio_quad(q, &mut bnode_map)?;
            self.inner.insert(&quad)
        })
    }
}
