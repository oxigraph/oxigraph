use errors::*;
use model::vocab::rdf;
use model::Triple;
use model::*;
use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::str::FromStr;
use url::Url;

pub fn read_rdf_xml(
    source: impl BufRead,
    base_uri: impl Into<Option<Url>>,
) -> impl Iterator<Item = Result<Triple>> {
    let mut reader = Reader::from_reader(source);
    reader.expand_empty_elements(true);
    reader.trim_text(true);
    RdfXmlIterator {
        reader,
        namespace_buffer: Vec::default(),
        state: vec![RdfXmlState::Doc {
            base_uri: base_uri.into(),
        }],
        object: None,
        bnodes_map: BTreeMap::default(),
        triples_cache: Vec::default(),
        li_counter: Vec::default(),
    }
}

lazy_static! {
    static ref RDF_ABOUT: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#about").unwrap();
    static ref RDF_DATATYPE: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#datatype").unwrap();
    static ref RDF_DESCRIPTION: NamedNode =
        NamedNode::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#Description").unwrap();
    static ref RDF_ID: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#ID").unwrap();
    static ref RDF_LI: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#li").unwrap();
    static ref RDF_NODE_ID: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#nodeID").unwrap();
    static ref RDF_PARSE_TYPE: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#parseType").unwrap();
    static ref RDF_RDF: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#RDF").unwrap();
    static ref RDF_RESOURCE: Url =
        Url::from_str("http://www.w3.org/1999/02/22-rdf-syntax-ns#resource").unwrap();
}

struct RdfXmlIterator<R: BufRead> {
    reader: Reader<R>,
    namespace_buffer: Vec<u8>,
    state: Vec<RdfXmlState>,
    object: Option<NodeOrText>,
    bnodes_map: BTreeMap<Vec<u8>, BlankNode>,
    triples_cache: Vec<Triple>,
    li_counter: Vec<usize>,
}

#[derive(Clone)]
enum NodeOrText {
    Node(NamedOrBlankNode),
    Text(String),
}

enum RdfXmlState {
    Doc {
        base_uri: Option<Url>,
    },
    RDF {
        base_uri: Option<Url>,
        language: String,
    },
    NodeElt {
        base_uri: Option<Url>,
        language: String,
        subject: NamedOrBlankNode,
    },
    PropertyElt {
        //Resource, Literal or Empty property element
        uri: Url,
        base_uri: Option<Url>,
        language: String,
        subject: NamedOrBlankNode,
        object: Option<NamedOrBlankNode>,
        id_attr: Option<NamedNode>,
        datatype_attr: Option<NamedNode>,
    },
    ParseTypeCollectionPropertyElt {
        uri: NamedNode,
        base_uri: Option<Url>,
        language: String,
        subject: NamedOrBlankNode,
        id_attr: Option<NamedNode>,
    },
    //TODO: ParseTypeOtherProperty and ParseTypeLiteralProperty
}

impl RdfXmlState {
    fn base_uri(&self) -> &Option<Url> {
        match self {
            RdfXmlState::Doc { base_uri, .. } => base_uri,
            RdfXmlState::RDF { base_uri, .. } => base_uri,
            RdfXmlState::NodeElt { base_uri, .. } => base_uri,
            RdfXmlState::PropertyElt { base_uri, .. } => base_uri,
            RdfXmlState::ParseTypeCollectionPropertyElt { base_uri, .. } => base_uri,
        }
    }

    fn language(&self) -> &str {
        match self {
            RdfXmlState::Doc { .. } => "",
            RdfXmlState::RDF { language, .. } => language,
            RdfXmlState::NodeElt { language, .. } => language,
            RdfXmlState::PropertyElt { language, .. } => language,
            RdfXmlState::ParseTypeCollectionPropertyElt { language, .. } => language,
        }
    }
}

impl<R: BufRead> Iterator for RdfXmlIterator<R> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        let mut buffer = Vec::default();
        loop {
            //Finish the stack
            if let Some(triple) = self.triples_cache.pop() {
                return Some(Ok(triple));
            }

            //Read more XML
            match self
                .reader
                .read_namespaced_event(&mut buffer, &mut self.namespace_buffer)
            {
                Ok((_, event)) => match event {
                    Event::Start(event) => {
                        if let Err(error) = self.parse_start_event(&event) {
                            return Some(Err(error));
                        }
                    }
                    Event::Text(event) => {
                        if let Err(error) = self.parse_text_event(&event) {
                            return Some(Err(error));
                        }
                    }
                    Event::End(event) => {
                        if let Err(error) = self.parse_end_event(&event) {
                            return Some(Err(error));
                        }
                    }
                    Event::Eof => return None,
                    _ => (),
                },
                Err(error) => return Some(Err(error.into())),
            }
        }
    }
}

impl<R: BufRead> RdfXmlIterator<R> {
    fn parse_start_event(&mut self, event: &BytesStart) -> Result<()> {
        #[derive(PartialEq, Eq)]
        enum RdfXmlParseType {
            Default,
            Collection,
            Literal,
            Resource,
            Other,
        }

        #[derive(PartialEq, Eq)]
        enum RdfXmlNextProduction {
            RDF,
            NodeElt,
            PropertyElt { subject: NamedOrBlankNode },
        }

        let uri = self.resolve_tag_name(event.name())?;

        //We read attributes
        let mut language = String::default();
        let mut base_uri = None;
        if let Some(current_state) = self.state.last() {
            language = current_state.language().to_string();
            base_uri = current_state.base_uri().clone();
        }
        let mut id_attr = None;
        let mut node_id_attr = None;
        let mut about_attr = None;
        let mut property_attrs = Vec::default();
        let mut resource_attr = None;
        let mut datatype_attr = None;
        let mut parse_type = RdfXmlParseType::Default;
        let mut type_attr = None;

        for attribute in event.attributes() {
            let attribute = attribute?;
            match attribute.key {
                b"xml:lang" => {
                    language = attribute.unescape_and_decode_value(&self.reader)?;
                }
                b"xml:base" => {
                    base_uri = Some(self.resolve_uri(&attribute.unescaped_value()?, &None)?)
                }
                key if !key.starts_with(b"xml") => {
                    let attribute_url = self.resolve_attribute_name(key)?;
                    if attribute_url == *RDF_ID {
                        let mut id = Vec::with_capacity(attribute.value.len() + 1);
                        id.push(b'#');
                        id.extend_from_slice(attribute.unescaped_value()?.as_ref());
                        id_attr = Some(id);
                    } else if attribute_url == *RDF_NODE_ID {
                        node_id_attr = Some(
                            self.bnodes_map
                                .entry(attribute.unescaped_value()?.to_vec())
                                .or_insert_with(BlankNode::default)
                                .clone(),
                        );
                    } else if attribute_url == *RDF_ABOUT {
                        about_attr = Some(attribute.unescaped_value()?.to_vec());
                    } else if attribute_url == *RDF_RESOURCE {
                        resource_attr = Some(attribute.unescaped_value()?.to_vec());
                    } else if attribute_url == *RDF_DATATYPE {
                        datatype_attr = Some(attribute.unescaped_value()?.to_vec());
                    } else if attribute_url == *RDF_PARSE_TYPE {
                        parse_type = match attribute.value.as_ref() {
                            b"Collection" => RdfXmlParseType::Collection,
                            b"Literal" => RdfXmlParseType::Literal,
                            b"Resource" => RdfXmlParseType::Resource,
                            _ => RdfXmlParseType::Other,
                        };
                    } else if attribute_url == **rdf::TYPE {
                        type_attr = Some(attribute.unescaped_value()?.to_vec());
                    } else {
                        property_attrs.push((
                            NamedNode::from(attribute_url),
                            attribute.unescape_and_decode_value(&self.reader)?,
                        ));
                    }
                }
                _ => (), //We do not fail for unknown tags in the XML namespace
            }
        }

        //Parsing with the base URI
        let id_attr = match id_attr {
            Some(uri) => Some(NamedNode::from(self.resolve_uri(&uri, &base_uri)?)),
            None => None,
        };
        let about_attr = match about_attr {
            Some(uri) => Some(NamedNode::from(self.resolve_uri(&uri, &base_uri)?)),
            None => None,
        };
        let resource_attr = match resource_attr {
            Some(uri) => Some(NamedNode::from(self.resolve_uri(&uri, &base_uri)?)),
            None => None,
        };
        let datatype_attr = match datatype_attr {
            Some(uri) => Some(NamedNode::from(self.resolve_uri(&uri, &base_uri)?)),
            None => None,
        };
        let type_attr = match type_attr {
            Some(uri) => Some(NamedNode::from(self.resolve_uri(&uri, &base_uri)?)),
            None => None,
        };

        let next_production = match self.state.last() {
            Some(RdfXmlState::Doc { .. }) => RdfXmlNextProduction::RDF,
            Some(RdfXmlState::RDF { .. }) => RdfXmlNextProduction::NodeElt,
            Some(RdfXmlState::NodeElt { subject, .. }) => RdfXmlNextProduction::PropertyElt {
                subject: subject.clone(),
            },
            Some(RdfXmlState::PropertyElt { .. }) => RdfXmlNextProduction::NodeElt {},
            Some(RdfXmlState::ParseTypeCollectionPropertyElt { .. }) => {
                RdfXmlNextProduction::NodeElt {}
            }
            None => return Err("No state in the stack: the XML is not balanced".into()),
        };

        let new_state = match next_production {
            RdfXmlNextProduction::RDF => {
                if uri == *RDF_RDF {
                    RdfXmlState::RDF { base_uri, language }
                } else {
                    self.build_node_elt(
                        NamedNode::from(uri),
                        base_uri,
                        language,
                        id_attr,
                        node_id_attr,
                        about_attr,
                        type_attr,
                        property_attrs,
                    )
                }
            }
            RdfXmlNextProduction::NodeElt => self.build_node_elt(
                NamedNode::from(uri),
                base_uri,
                language,
                id_attr,
                node_id_attr,
                about_attr,
                type_attr,
                property_attrs,
            ),
            RdfXmlNextProduction::PropertyElt { subject } => match parse_type {
                RdfXmlParseType::Default => {
                    if resource_attr.is_some()
                        || node_id_attr.is_some()
                        || !property_attrs.is_empty()
                    {
                        let object: NamedOrBlankNode = match resource_attr {
                            Some(resource_attr) => resource_attr.into(),
                            None => match node_id_attr {
                                Some(node_id_attr) => node_id_attr.into(),
                                None => BlankNode::default().into(),
                            },
                        };
                        self.emit_property_attrs(&object, property_attrs, &language);
                        if let Some(type_attr) = type_attr {
                            self.triples_cache.push(Triple::new(
                                object.clone(),
                                rdf::TYPE.clone(),
                                type_attr,
                            ));
                        }
                        RdfXmlState::PropertyElt {
                            uri,
                            base_uri,
                            language,
                            subject,
                            object: Some(object),
                            id_attr,
                            datatype_attr,
                        }
                    } else {
                        RdfXmlState::PropertyElt {
                            uri,
                            base_uri,
                            language,
                            subject,
                            object: None,
                            id_attr,
                            datatype_attr,
                        }
                    }
                }
                RdfXmlParseType::Literal => {
                    return Err("rdf:parseType=\"Literal\" is not supported yet".into());
                }
                RdfXmlParseType::Resource => self.build_parse_type_resource_property_elt(
                    NamedNode::from(uri),
                    base_uri,
                    language,
                    subject,
                    id_attr,
                ),
                RdfXmlParseType::Collection => {
                    return Err("rdf:parseType=\"Collection\" is not supported yet".into());
                }
                RdfXmlParseType::Other => {
                    return Err("Arbitrary rdf:parseType are not supported yet".into());
                }
            },
        };
        self.state.push(new_state);
        Ok(())
    }

    fn parse_end_event(&mut self, _event: &BytesEnd) -> Result<()> {
        if let Some(current_state) = self.state.pop() {
            self.end_state(current_state)?;
        }
        Ok(())
    }

    fn parse_text_event(&mut self, event: &BytesText) -> Result<()> {
        if self.object.is_some() {
            return Err(format!(
                "There is already an object set at byte {}",
                self.reader.buffer_position()
            ).into());
        }
        self.object = Some(NodeOrText::Text(event.unescape_and_decode(&self.reader)?));
        Ok(())
    }

    fn resolve_tag_name(&self, qname: &[u8]) -> Result<Url> {
        let (namespace, local_name) = self.reader.event_namespace(qname, &self.namespace_buffer);
        self.resolve_ns_name(namespace, local_name)
    }

    fn resolve_attribute_name(&self, qname: &[u8]) -> Result<Url> {
        let (namespace, local_name) = self
            .reader
            .attribute_namespace(qname, &self.namespace_buffer);
        self.resolve_ns_name(namespace, local_name)
    }

    fn resolve_ns_name(&self, namespace: Option<&[u8]>, local_name: &[u8]) -> Result<Url> {
        Ok(Url::parse(
            &(match namespace {
                Some(namespace) => self.reader.decode(namespace) + self.reader.decode(local_name),
                None => self.reader.decode(local_name),
            }),
        )?)
    }

    fn resolve_uri(&self, uri: &[u8], base: &Option<Url>) -> Result<Url> {
        Ok(Url::options()
            .base_url(base.as_ref())
            .parse(&self.reader.decode(uri))?)
    }

    fn build_node_elt(
        &mut self,
        uri: NamedNode,
        base_uri: Option<Url>,
        language: String,
        id_attr: Option<NamedNode>,
        node_id_attr: Option<BlankNode>,
        about_attr: Option<NamedNode>,
        type_attr: Option<NamedNode>,
        property_attrs: Vec<(NamedNode, String)>,
    ) -> RdfXmlState {
        self.object = None; //We reset object return: we are in a list of elements

        let subject = match id_attr {
            Some(id_attr) => id_attr.into(),
            None => match about_attr {
                Some(about_attr) => about_attr.into(),
                None => node_id_attr.unwrap_or_else(BlankNode::default).into(),
            },
        };

        self.emit_property_attrs(&subject, property_attrs, &language);

        if let Some(type_attr) = type_attr {
            self.triples_cache
                .push(Triple::new(subject.clone(), rdf::TYPE.clone(), type_attr));
        }

        if uri != *RDF_DESCRIPTION {
            self.triples_cache
                .push(Triple::new(subject.clone(), rdf::TYPE.clone(), uri));
        }
        self.li_counter.push(0);
        RdfXmlState::NodeElt {
            base_uri,
            language,
            subject: subject.clone(),
        }
    }

    fn build_parse_type_resource_property_elt(
        &mut self,
        uri: NamedNode,
        base_uri: Option<Url>,
        language: String,
        subject: NamedOrBlankNode,
        id_attr: Option<NamedNode>,
    ) -> RdfXmlState {
        let object = BlankNode::default();
        let triple = Triple::new(subject, uri, object.clone());
        if let Some(id_attr) = id_attr {
            self.reify(&triple, id_attr.into());
        }
        self.triples_cache.push(triple);
        self.li_counter.push(0);
        RdfXmlState::NodeElt {
            base_uri,
            language,
            subject: object.into(),
        }
    }

    fn end_state(&mut self, state: RdfXmlState) -> Result<()> {
        match state {
            RdfXmlState::PropertyElt {
                uri,
                language,
                subject,
                id_attr,
                datatype_attr,
                object,
                ..
            } => {
                let predicate = if uri == *RDF_LI {
                    if let Some(li_counter) = self.li_counter.last_mut() {
                        *li_counter += 1;
                        NamedNode::from_str(&format!(
                            "http://www.w3.org/1999/02/22-rdf-syntax-ns#_{}",
                            li_counter
                        ))?
                    } else {
                        NamedNode::from(uri)
                    }
                } else {
                    NamedNode::from(uri)
                };
                let object: Term = match object {
                    Some(object) => object.into(),
                    None => match self.object.clone() {
                        Some(NodeOrText::Node(node)) => node.into(),
                        Some(NodeOrText::Text(text)) => {
                            self.new_literal(text, language, datatype_attr).into()
                        }
                        None => self
                            .new_literal(String::default(), language, datatype_attr)
                            .into(),
                    },
                };
                self.object = None; //We have used self.object
                let triple = Triple::new(subject, predicate, object);
                if let Some(id_attr) = id_attr {
                    self.reify(&triple, id_attr.into());
                }
                self.triples_cache.push(triple);
            }
            RdfXmlState::NodeElt { subject, .. } => {
                self.object = Some(NodeOrText::Node(subject));
                self.li_counter.pop();
            }
            _ => (),
        }
        Ok(())
    }

    fn new_literal(&self, text: String, language: String, datatype: Option<NamedNode>) -> Literal {
        if let Some(datatype) = datatype {
            Literal::new_typed_literal(text, datatype)
        } else if language.is_empty() {
            Literal::new_simple_literal(text)
        } else {
            Literal::new_language_tagged_literal(text, language)
        }
    }

    fn reify(&mut self, triple: &Triple, statement_id: NamedOrBlankNode) {
        self.triples_cache.push(Triple::new(
            statement_id.clone(),
            rdf::OBJECT.clone(),
            triple.object().clone(),
        ));
        self.triples_cache.push(Triple::new(
            statement_id.clone(),
            rdf::PREDICATE.clone(),
            triple.predicate().clone(),
        ));
        self.triples_cache.push(Triple::new(
            statement_id.clone(),
            rdf::SUBJECT.clone(),
            triple.subject().clone(),
        ));
        self.triples_cache.push(Triple::new(
            statement_id,
            rdf::TYPE.clone(),
            rdf::STATEMENT.clone(),
        ));
    }

    fn emit_property_attrs(
        &mut self,
        subject: &NamedOrBlankNode,
        literal_attributes: Vec<(NamedNode, String)>,
        language: &str,
    ) {
        for (literal_predicate, literal_value) in literal_attributes {
            self.triples_cache.push(Triple::new(
                subject.clone(),
                literal_predicate,
                if language.is_empty() {
                    Literal::new_simple_literal(literal_value)
                } else {
                    Literal::new_language_tagged_literal(literal_value, language)
                },
            ));
        }
    }
}
