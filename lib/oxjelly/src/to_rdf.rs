use std::collections::BTreeMap;
use std::io::Read;
use protobuf::Message;
use oxrdf::{GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term};
use crate::{JellyParseError, JellySyntaxError};
use crate::jelly::rdf::{RdfIri, RdfStreamFrame};
use crate::jelly::rdf::rdf_literal::LiteralKind;
use crate::jelly::rdf::rdf_quad::{Graph, Object, Predicate, Subject};
use crate::jelly::rdf::rdf_stream_row::Row;

pub struct ReaderJellyParser<R: Read> {
    reader: R,
    prefix_map: BTreeMap<u32, String>,
    name_map: BTreeMap<u32, String>,
    datatype_map: BTreeMap<u32, String>,
    current_frame: Option<RdfStreamFrame>,
    current_row_index: usize,
    current_subject: Option<NamedOrBlankNode>,
    current_predicate: Option<NamedNode>,
    current_object: Option<Term>,
    current_graph: Option<GraphName>
}

impl<R: Read> ReaderJellyParser<R> {
    pub fn prefixes(&self) -> JellyPrefixesIter<'_> {
        todo!()
    }

    fn jelly_iri_to_named_node(&self, iri: &RdfIri) -> Result<NamedNode, JellySyntaxError> {
        let prefix = self.prefix_map
            .get(&iri.prefix_id)
            .map_or(Err(JellySyntaxError::PrefixIdNotFound(iri.prefix_id, iri.name_id)), Ok)?;

        let name = self.name_map
            .get(&iri.name_id)
            .map_or(Err(JellySyntaxError::NameIdNotFound(iri.prefix_id, iri.name_id)), Ok)?;

        let iri_str = format!("{prefix}{name}");
        Ok(NamedNode::new(iri_str)?)
    }

    fn jelly_subject_to_oxi_subject(&self, subject: Option<&Subject>) -> Result<NamedOrBlankNode, JellySyntaxError> {
        match subject {
            None => self.current_subject.clone().map_or(Err(JellySyntaxError::NoPreviousSubject), Ok),
            Some(Subject::SIri(iri)) => self.jelly_iri_to_named_node(iri).map(NamedOrBlankNode::NamedNode),
            _ => todo!(),
        }
    }

    fn jelly_predicate_to_oxi_predicate(&self, predicate: Option<&Predicate>) -> Result<NamedNode, JellySyntaxError> {
        match predicate {
            None => self.current_predicate.clone().map_or(Err(JellySyntaxError::NoPreviousPredicate), Ok),
            Some(Predicate::PIri(iri)) => self.jelly_iri_to_named_node(iri),
            _ => todo!(),
        }
    }

    fn jelly_object_to_oxi_object(&self, term: Option<&Object>) -> Result<Term, JellySyntaxError> {
        match term {
            None => self.current_object.clone().map_or(Err(JellySyntaxError::NoPreviousObject), Ok),
            Some(Object::OIri(iri)) => self.jelly_iri_to_named_node(iri).map(Term::NamedNode),
            Some(Object::OLiteral(literal)) => {
                match literal.literalKind.as_ref() {
                    Some(LiteralKind::Datatype(index)) => {
                        if let Some(datatype) = self.datatype_map.get(&index) {
                            let node = NamedNode::new_unchecked(datatype);
                            Ok(Term::Literal(Literal::new_typed_literal(literal.lex.clone(), node)))
                        } else {
                            Err(JellySyntaxError::DatatypeIdNotFound(*index))
                        }
                    }
                    Some(LiteralKind::Langtag(tag)) => {
                        Ok(Term::Literal(Literal::new_language_tagged_literal_unchecked(literal.lex.clone(), tag)))
                    }
                    None => Ok(Term::Literal(Literal::new_simple_literal(literal.lex.clone())))
                }
            }
            _ => todo!(),
        }
    }

    fn jelly_graph_name_to_oxi_graph_name(&self, term: Option<&Graph>) -> Result<GraphName, JellySyntaxError> {
        match term {
            None => self.current_graph.clone().map_or(Err(JellySyntaxError::NoPreviousGraphName), Ok),
            Some(Graph::GIri(iri)) => self.jelly_iri_to_named_node(iri).map(GraphName::NamedNode),
            Some(Graph::GDefaultGraph(_)) => Ok(GraphName::DefaultGraph),
            _ => {
                println!("{:?}", term);
                todo!()
            },
        }
    }
}

impl<R: Read> Iterator for ReaderJellyParser<R> {
    type Item = Result<Quad, JellyParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.current_frame {
            Some(frame) => {
                loop {
                    match frame.rows.get(self.current_row_index) {
                        None => return None,
                        Some(row) => {
                            match &row.row {
                                Some(Row::Prefix(prefix)) => {
                                    self.prefix_map.insert(prefix.id, prefix.value.clone());
                                    self.current_row_index = self.current_row_index + 1;
                                }
                                Some(Row::Name(name)) => {
                                    println!("new name: {0}", name);
                                    self.name_map.insert(name.id, name.value.clone());
                                    self.current_row_index = self.current_row_index + 1;
                                }
                                Some(Row::Datatype(datatype)) => {
                                    self.datatype_map.insert(datatype.id, datatype.value.clone());
                                    self.current_row_index = self.current_row_index + 1;
                                }
                                Some(Row::Quad(quad)) => {
                                    let subject = self.jelly_subject_to_oxi_subject(quad.subject.as_ref());
                                    let predicate = self.jelly_predicate_to_oxi_predicate(quad.predicate.as_ref());
                                    let object = self.jelly_object_to_oxi_object(quad.object.as_ref());
                                    let graph_name = self.jelly_graph_name_to_oxi_graph_name(quad.graph.as_ref());
                                    return match (subject, predicate, object, graph_name) {
                                        (Err(err), _, _, _) => Some(Err(err.into())),
                                        (_, Err(err), _, _) => Some(Err(err.into())),
                                        (_, _, Err(err), _) => Some(Err(err.into())),
                                        (_, _, _, Err(err)) => Some(Err(err.into())),
                                        (Ok(subject), Ok(predicate), Ok(object), Ok(graph_name)) => {
                                            self.current_subject = Some(subject.clone());
                                            self.current_predicate = Some(predicate.clone());
                                            self.current_object = Some(object.clone());
                                            self.current_graph = Some(graph_name.clone());
                                            self.current_row_index = self.current_row_index + 1;
                                            Some(Ok(Quad::new(subject, predicate, object, graph_name)))
                                        }
                                    }
                                }
                                _ => self.current_row_index = self.current_row_index + 1
                            }
                        }
                    }
                }
            }
            None => {
                match RdfStreamFrame::parse_from_reader(&mut self.reader) {
                    Ok(frame) => {
                        self.current_frame = Some(frame);
                        self.current_row_index = 0;
                        self.current_subject = None;
                        self.current_predicate = None;
                        self.current_object = None;
                        self.current_graph = None;
                        self.next()
                    }
                    Err(err) => Some(
                        Err(JellyParseError::from(JellySyntaxError::from(err)))
                    )
                }
            }
        }
    }
}

pub struct JellyPrefixesIter<'a> {
    prefixes: std::collections::hash_map::Iter<'a, String, String>,
}

impl<'a> Iterator for JellyPrefixesIter<'a> {
    type Item = (&'a str, &'a str);
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

#[must_use]
pub struct SliceJellyParser<'a> {
    inner: &'a u64,
}

impl<'a> SliceJellyParser<'a> {
    pub fn prefixes(&self) -> JellyPrefixesIter<'_> {
        todo!()
    }
}

impl Iterator for SliceJellyParser<'_> {
    type Item = Result<Quad, JellySyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

#[derive(Default, Clone)]
#[must_use]
pub struct JellyParser {}

impl JellyParser {
    #[inline]
    pub fn new() -> Self {
        Self {}
    }

    pub fn for_reader<R: Read>(self, reader: R) -> ReaderJellyParser<R> {
        ReaderJellyParser {
            reader,
            prefix_map: BTreeMap::new(),
            name_map: BTreeMap::new(),
            datatype_map: BTreeMap::new(),
            current_frame: None,
            current_row_index: 0,
            current_subject: None,
            current_predicate: None,
            current_object: None,
            current_graph: None,
        }
    }

    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceJellyParser<'_> {
        todo!()
    }
}