use crate::model::*;
use crate::sparql::{GraphPattern, PreparedQuery, QueryOptions, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::{load_dataset, load_graph, ReadableEncodedStore, WritableEncodedStore};
use crate::{DatasetSyntax, GraphSyntax, Result};
use sled::{Config, Iter, Tree};
use std::io::BufRead;
use std::path::Path;
use std::str;

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
/// use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult};
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = SledStore::open("example.db")?;
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com")?;
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
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
/// }
/// #
/// # }
/// # remove_dir_all("example.db")?;
/// # Result::Ok(())
/// ```
#[derive(Clone)]
pub struct SledStore {
    id2str: Tree,
    spog: Tree,
    posg: Tree,
    ospg: Tree,
    gspo: Tree,
    gpos: Tree,
    gosp: Tree,
}

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

impl SledStore {
    /// Opens a temporary `SledStore` that will be deleted after drop.
    pub fn new() -> Result<Self> {
        Self::do_open(Config::new().temporary(true))
    }

    /// Opens a `SledStore`
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::do_open(Config::new().path(path))
    }

    fn do_open(config: Config) -> Result<Self> {
        let db = config.open()?;
        let new = Self {
            id2str: db.open_tree("id2str")?,
            spog: db.open_tree("spog")?,
            posg: db.open_tree("posg")?,
            ospg: db.open_tree("ospg")?,
            gspo: db.open_tree("gspo")?,
            gpos: db.open_tree("gpos")?,
            gosp: db.open_tree("gosp")?,
        };
        (&new).set_first_strings()?;
        Ok(new)
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn prepare_query<'a>(
        &'a self,
        query: &str,
        options: QueryOptions<'_>,
    ) -> Result<impl PreparedQuery + 'a> {
        SimplePreparedQuery::new((*self).clone(), query, options)
    }

    /// This is similar to `prepare_query`, but useful if a SPARQL query has already been parsed, which is the case when building `ServiceHandler`s for federated queries with `SERVICE` clauses. For examples, look in the tests.
    pub fn prepare_query_from_pattern<'a>(
        &'a self,
        graph_pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<impl PreparedQuery + 'a> {
        SimplePreparedQuery::new_from_pattern((*self).clone(), graph_pattern, options)
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// See `MemoryStore` for a usage example.
    #[allow(clippy::option_option)]
    pub fn quads_for_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<Option<&NamedOrBlankNode>>,
    ) -> impl Iterator<Item = Result<Quad>> {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.map_or(ENCODED_DEFAULT_GRAPH, |g| g.into()));
        let this = self.clone();
        self.encoded_quads_for_pattern_inner(subject, predicate, object, graph_name)
            .map(move |quad| this.decode_quad(&quad?))
    }

    /// Checks if this store contains a given quad
    pub fn contains(&self, quad: &Quad) -> Result<bool> {
        let quad = quad.into();
        self.contains_encoded(&quad)
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
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let mut store = self;
        load_graph(&mut store, reader, syntax, to_graph_name, base_iri)
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
        let mut store = self;
        load_dataset(&mut store, reader, syntax, base_iri)
    }

    /// Adds a quad to this store.
    pub fn insert(&self, quad: &Quad) -> Result<()> {
        let mut store = self;
        let quad = store.encode_quad(quad)?;
        store.insert_encoded(&quad)
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) -> Result<()> {
        let mut store = self;
        let quad = quad.into();
        store.remove_encoded(&quad)
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        write_spog_quad(&mut buffer, quad);
        Ok(self.spog.contains_key(buffer)?)
    }

    fn encoded_quads_for_pattern_inner(
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

    fn quads(&self) -> DecodingQuadIterator {
        self.spog_quads(Vec::default())
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingQuadIterator {
        self.spog_quads(encode_term(subject))
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.spog_quads(encode_term_pair(subject, predicate))
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.spog_quads(encode_term_triple(subject, predicate, object))
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.ospg_quads(encode_term_pair(object, subject))
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingQuadIterator {
        self.posg_quads(encode_term(predicate))
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.posg_quads(encode_term_pair(predicate, object))
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingQuadIterator {
        self.ospg_quads(encode_term(object))
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingQuadIterator {
        self.gspo_quads(encode_term(graph_name))
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gspo_quads(encode_term_pair(graph_name, subject))
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gosp_quads(encode_term_triple(graph_name, object, subject))
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gpos_quads(encode_term_pair(graph_name, predicate))
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gpos_quads(encode_term_triple(graph_name, predicate, object))
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadIterator {
        self.gosp_quads(encode_term_pair(graph_name, object))
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.spog, prefix, QuadEncoding::SPOG)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.posg, prefix, QuadEncoding::POSG)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.ospg, prefix, QuadEncoding::OSPG)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gspo, prefix, QuadEncoding::GSPO)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gpos, prefix, QuadEncoding::GPOS)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gosp, prefix, QuadEncoding::GOSP)
    }

    fn inner_quads(
        &self,
        tree: &Tree,
        prefix: Vec<u8>,
        order: QuadEncoding,
    ) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: tree.scan_prefix(prefix),
            order,
        }
    }
}

impl StrLookup for SledStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        Ok(self
            .id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()?)
    }
}

impl ReadableEncodedStore for SledStore {
    fn encoded_quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a> {
        Box::new(self.encoded_quads_for_pattern_inner(subject, predicate, object, graph_name))
    }
}

impl<'a> StrContainer for &'a SledStore {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.id2str.insert(key.to_be_bytes(), value)?;
        Ok(())
    }
}

impl<'a> WritableEncodedStore for &'a SledStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        //TODO: atomicity
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);

        write_spog_quad(&mut buffer, quad);
        self.spog.insert(&buffer, &[])?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.posg.insert(&buffer, &[])?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.ospg.insert(&buffer, &[])?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.gspo.insert(&buffer, &[])?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.gpos.insert(&buffer, &[])?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.gosp.insert(&buffer, &[])?;
        buffer.clear();

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        //TODO: atomicity
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);

        write_spog_quad(&mut buffer, quad);
        self.spog.remove(&buffer)?;
        buffer.clear();

        write_posg_quad(&mut buffer, quad);
        self.posg.remove(&buffer)?;
        buffer.clear();

        write_ospg_quad(&mut buffer, quad);
        self.ospg.remove(&buffer)?;
        buffer.clear();

        write_gspo_quad(&mut buffer, quad);
        self.gspo.remove(&buffer)?;
        buffer.clear();

        write_gpos_quad(&mut buffer, quad);
        self.gpos.remove(&buffer)?;
        buffer.clear();

        write_gosp_quad(&mut buffer, quad);
        self.gosp.remove(&buffer)?;
        buffer.clear();

        Ok(())
    }
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

struct DecodingQuadIterator {
    iter: Iter,
    order: QuadEncoding,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => self.order.decode(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

#[test]
fn store() -> Result<()> {
    use crate::model::*;
    use crate::*;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::parse("http://example.com")?;
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
            .quads_for_pattern(Some(&main_s), Some(&main_p), Some(&main_o), Some(None))
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), Some(&main_p), None, Some(None))
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
            .quads_for_pattern(Some(&main_s), None, Some(&main_o), Some(None))
            .collect::<Result<Vec<_>>>()?,
        target
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(&main_s), None, None, Some(None))
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
            .quads_for_pattern(None, None, None, Some(None))
            .collect::<Result<Vec<_>>>()?,
        all_o
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(&main_p), Some(&main_o), Some(None))
            .collect::<Result<Vec<_>>>()?,
        target
    );

    Ok(())
}
