use std::io::Read;
use protobuf::CodedInputStream;
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term};
use crate::{JellyParseError, JellySyntaxError};
use crate::jelly::rdf::{RdfIri, RdfStreamFrame};
use crate::jelly::rdf::rdf_literal::LiteralKind;
use crate::jelly::rdf::rdf_quad::{Graph, Object, Predicate, Subject};
use crate::jelly::rdf::rdf_stream_row::Row;
use crate::lookup_table::LookupTable;

struct InnerState {
    prefixes: LookupTable,
    names: LookupTable,
    datatypes: LookupTable,
    current_subject: Option<NamedOrBlankNode>,
    current_predicate: Option<NamedNode>,
    current_object: Option<Term>,
    current_graph: Option<GraphName>,
}

impl InnerState {
    fn new() -> Self {
        Self {
            prefixes: LookupTable::new(),
            names: LookupTable::new(),
            datatypes: LookupTable::new(),
            current_subject: None,
            current_predicate: None,
            current_object: None,
            current_graph: None,
        }
    }
    fn jelly_iri_to_named_node(&mut self, iri: &RdfIri) -> Result<NamedNode, JellySyntaxError> {
        if let Some(prefix) =self.prefixes.lookup(&iri.prefix_id) {
            if let Some(name) = self.names.lookup(&iri.name_id) {
                let iri_str = format!("{prefix}{name}");
                Ok(NamedNode::new(iri_str)?)
            } else {
                Err(JellySyntaxError::NameIdNotFound(iri.name_id))
            }
        } else {
            Err(JellySyntaxError::PrefixIdNotFound(iri.prefix_id))
        }
    }

    fn jelly_subject_to_oxi_subject(&mut self, subject: Option<&Subject>) -> Result<NamedOrBlankNode, JellySyntaxError> {
        match subject {
            None => self.current_subject.clone().map_or(Err(JellySyntaxError::NoPreviousSubject), Ok),
            Some(Subject::SIri(iri)) => self.jelly_iri_to_named_node(iri).map(NamedOrBlankNode::NamedNode),
            Some(Subject::SBnode(value)) => Ok(NamedOrBlankNode::BlankNode(BlankNode::new(value)?)),
            _ => todo!(),
        }
    }

    fn jelly_predicate_to_oxi_predicate(&mut self, predicate: Option<&Predicate>) -> Result<NamedNode, JellySyntaxError> {
        match predicate {
            None => self.current_predicate.clone().map_or(Err(JellySyntaxError::NoPreviousPredicate), Ok),
            Some(Predicate::PIri(iri)) => self.jelly_iri_to_named_node(iri),
            _ => todo!(),
        }
    }

    fn jelly_object_to_oxi_object(&mut self, term: Option<&Object>) -> Result<Term, JellySyntaxError> {
        match term {
            None => self.current_object.clone().map_or(Err(JellySyntaxError::NoPreviousObject), Ok),
            Some(Object::OIri(iri)) => self.jelly_iri_to_named_node(iri).map(Term::NamedNode),
            Some(Object::OBnode(value)) => Ok(Term::BlankNode(BlankNode::new(value)?)),
            Some(Object::OLiteral(literal)) => {
                match literal.literalKind.as_ref() {
                    Some(LiteralKind::Datatype(index)) => {
                        if let Some(datatype) = self.datatypes.lookup(&index) {
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

    fn jelly_graph_name_to_oxi_graph_name(&mut self, term: Option<&Graph>) -> Result<GraphName, JellySyntaxError> {
        match term {
            None => self.current_graph.clone().map_or(Err(JellySyntaxError::NoPreviousGraphName), Ok),
            Some(Graph::GIri(iri)) => self.jelly_iri_to_named_node(iri).map(GraphName::NamedNode),
            Some(Graph::GDefaultGraph(_)) => Ok(GraphName::DefaultGraph),
            _ => {
                todo!()
            },
        }
    }
}

pub struct ReaderJellyParser<R: Read> {
    reader: R,
    current_frame: Option<RdfStreamFrame>,
    current_row_index: usize,
    inner: InnerState,
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
                                Some(Row::Options(options)) => {
                                    // TODO: Fail if options already seen and are different

                                    self.inner.prefixes.resize(options.max_prefix_table_size);
                                    self.inner.names.resize(options.max_name_table_size);
                                    self.inner.datatypes.resize(options.max_datatype_table_size);
                                }
                                Some(Row::Prefix(prefix)) => {
                                    if let Err(err) = self.inner.prefixes.push(prefix.id, prefix.value.clone()) {
                                        return Some(Err(err.into()));
                                    }
                                }
                                Some(Row::Name(name)) => {
                                    if let Err(err) = self.inner.names.push(name.id, name.value.clone()) {
                                        return Some(Err(err.into()));
                                    }
                                }
                                Some(Row::Datatype(datatype)) => {
                                    if let Err(err) = self.inner.datatypes.push(datatype.id, datatype.value.clone()) {
                                        return Some(Err(err.into()));
                                    }
                                }
                                Some(Row::Quad(quad)) => {
                                    let mut get_quad = || -> Result<Quad, JellyParseError> {
                                        let subject = self.inner.jelly_subject_to_oxi_subject(quad.subject.as_ref())?;
                                        let predicate = self.inner.jelly_predicate_to_oxi_predicate(quad.predicate.as_ref())?;
                                        let object = self.inner.jelly_object_to_oxi_object(quad.object.as_ref())?;
                                        let graph_name = self.inner.jelly_graph_name_to_oxi_graph_name(quad.graph.as_ref())?;

                                        self.inner.current_subject = Some(subject.clone());
                                        self.inner.current_predicate = Some(predicate.clone());
                                        self.inner.current_object = Some(object.clone());
                                        self.inner.current_graph = Some(graph_name.clone());

                                        self.current_row_index = self.current_row_index + 1;
                                        Ok(Quad::new(subject, predicate, object, graph_name))
                                    };

                                    return Some(get_quad())
                                }
                                _ => {}
                            }
                        }
                    }
                    self.current_row_index = self.current_row_index + 1;
                }
            }
            None => {
                let message;
                {
                    let mut stream = CodedInputStream::new(&mut self.reader);
                    message = stream.read_message::<RdfStreamFrame>();
                }
                match message {
                    Ok(frame) => {
                        self.current_frame = Some(frame);
                        self.current_row_index = 0;
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

#[must_use]
pub struct SliceJellyParser<'a> {
    inner: &'a u64,
}

impl Iterator for SliceJellyParser<'_> {
    type Item = Result<Quad, JellySyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

#[derive(Default, Clone)]
#[must_use]
pub struct JellyParser;

impl JellyParser {
    #[inline]
    pub fn new() -> Self {
        Self {}
    }

    pub fn for_reader<R: Read>(self, reader: R) -> ReaderJellyParser<R> {
        ReaderJellyParser {
            reader,
            current_frame: None,
            current_row_index: 0,
            inner: InnerState::new(),
        }
    }

    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceJellyParser<'_> {
        todo!()
    }
}