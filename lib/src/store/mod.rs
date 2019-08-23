//! Provides implementations of the `rudf::Repository` trait.

mod memory;
pub(crate) mod numeric_encoder;
#[cfg(feature = "rocksdb")]
mod rocksdb;

pub use crate::store::memory::MemoryRepository;
#[cfg(feature = "rocksdb")]
pub use crate::store::rocksdb::RocksDbRepository;

use crate::model::*;
use crate::sparql::SimplePreparedQuery;
use crate::store::numeric_encoder::*;
use crate::{DatasetSyntax, GraphSyntax, RepositoryConnection, Result};
use rio_api::parser::{QuadParser, TripleParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleParser};
use rio_xml::RdfXmlParser;
use std::collections::HashMap;
use std::io::BufRead;
use std::iter::{once, Iterator};

/// Defines the `Store` traits that is used to have efficient binary storage
pub trait Store {
    type Connection: StoreConnection;

    fn connection(self) -> Result<Self::Connection>;
}

/// A connection to a `Store`
pub trait StoreConnection: StringStore + Sized + Clone {
    fn contains(&self, quad: &EncodedQuad) -> Result<bool>;
    fn insert(&self, quad: &EncodedQuad) -> Result<()>;
    fn remove(&self, quad: &EncodedQuad) -> Result<()>;
    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a>;
    fn encoder(&self) -> Encoder<&Self> {
        Encoder::new(&self)
    }
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

impl<S: StoreConnection> RepositoryConnection for StoreRepositoryConnection<S> {
    type PreparedQuery = SimplePreparedQuery<S>;

    fn prepare_query(&self, query: &str) -> Result<SimplePreparedQuery<S>> {
        SimplePreparedQuery::new(self.inner.clone(), query) //TODO: avoid clone
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
        let encoder = self.inner.encoder();
        let subject = if let Some(subject) = subject {
            match encoder.encode_named_or_blank_node(subject) {
                Ok(subject) => Some(subject),
                Err(error) => return Box::new(once(Err(error))),
            }
        } else {
            None
        };
        let predicate = if let Some(predicate) = predicate {
            match encoder.encode_named_node(predicate) {
                Ok(predicate) => Some(predicate),
                Err(error) => return Box::new(once(Err(error))),
            }
        } else {
            None
        };
        let object = if let Some(object) = object {
            match encoder.encode_term(object) {
                Ok(object) => Some(object),
                Err(error) => return Box::new(once(Err(error))),
            }
        } else {
            None
        };
        let graph_name = if let Some(graph_name) = graph_name {
            Some(if let Some(graph_name) = graph_name {
                match encoder.encode_named_or_blank_node(graph_name) {
                    Ok(graph_name) => graph_name,
                    Err(error) => return Box::new(once(Err(error))),
                }
            } else {
                EncodedTerm::DefaultGraph
            })
        } else {
            None
        };

        Box::new(
            self.inner
                .quads_for_pattern(subject, predicate, object, graph_name)
                .map(move |quad| self.inner.encoder().decode_quad(&quad?)),
        )
    }

    fn load_graph(
        &self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let base_iri = base_iri.unwrap_or(&"");
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
        &self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let base_iri = base_iri.unwrap_or(&"");
        match syntax {
            DatasetSyntax::NQuads => self.load_from_quad_parser(NQuadsParser::new(reader)?),
            DatasetSyntax::TriG => self.load_from_quad_parser(TriGParser::new(reader, base_iri)?),
        }
    }

    fn contains(&self, quad: &Quad) -> Result<bool> {
        self.inner
            .contains(&self.inner.encoder().encode_quad(quad)?)
    }

    fn insert(&self, quad: &Quad) -> Result<()> {
        self.inner.insert(&self.inner.encoder().encode_quad(quad)?)
    }

    fn remove(&self, quad: &Quad) -> Result<()> {
        self.inner.remove(&self.inner.encoder().encode_quad(quad)?)
    }
}

impl<S: StoreConnection> StoreRepositoryConnection<S> {
    fn load_from_triple_parser<P: TripleParser>(
        &self,
        mut parser: P,
        to_graph_name: Option<&NamedOrBlankNode>,
    ) -> Result<()>
    where
        P::Error: Send + Sync + 'static,
    {
        //TODO: handle errors
        let mut bnode_map = HashMap::default();
        let encoder = self.inner.encoder();
        let graph_name = if let Some(graph_name) = to_graph_name {
            encoder.encode_named_or_blank_node(graph_name)?
        } else {
            EncodedTerm::DefaultGraph
        };
        parser.parse_all(&mut move |t| {
            self.inner
                .insert(
                    &encoder
                        .encode_rio_triple_in_graph(t, graph_name, &mut bnode_map)
                        .unwrap(),
                )
                .unwrap()
        })?;
        Ok(())
    }

    fn load_from_quad_parser<P: QuadParser>(&self, mut parser: P) -> Result<()>
    where
        P::Error: Send + Sync + 'static,
    {
        //TODO: handle errors
        let mut bnode_map = HashMap::default();
        let encoder = self.inner.encoder();
        parser.parse_all(&mut move |q| {
            self.inner
                .insert(&encoder.encode_rio_quad(q, &mut bnode_map).unwrap())
                .unwrap()
        })?;
        Ok(())
    }
}
