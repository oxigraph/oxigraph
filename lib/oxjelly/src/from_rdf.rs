use std::cmp;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::io::Write;
use protobuf::{EnumOrUnknown, Message, MessageField};
use oxiri::{Iri, IriParseError};
use oxrdf::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
use crate::jelly::rdf::{PhysicalStreamType, RdfDatatypeEntry, RdfDefaultGraph, RdfIri, RdfLiteral, RdfNameEntry, RdfNamespaceDeclaration, RdfPrefixEntry, RdfQuad, RdfStreamFrame, RdfStreamOptions, RdfStreamRow};
use crate::jelly::rdf::rdf_literal::LiteralKind;
use crate::jelly::rdf::rdf_quad::{Graph, Object, Predicate, Subject};
use crate::sorted::{SortableGraphName, SortableObject, SortablePredicate, SortableRdfQuad, SortableSubject};

#[derive(Default, Clone)]
#[must_use]
pub struct JellySerializer {
    stream_name: String,
    namespace_map: BTreeMap<String, RdfIri>,
    prefix_map: BTreeMap<String, u32>,
    next_prefix_id: u32,
    name_map: BTreeMap<String, u32>,
    next_name_id: u32,
    datatype_map: BTreeMap<String, u32>,
    next_datatype_id: u32,
    quads: BTreeSet<SortableRdfQuad>,
}

impl JellySerializer {
    pub fn new(stream_name: impl Into<String>) -> Self {
        Self {
            stream_name: stream_name.into(),
            next_prefix_id: 1,
            next_name_id: 1,
            next_datatype_id: 1,
            ..Default::default()
        }
    }

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        let prefix_iri = Iri::parse(prefix_iri.into())?.into_inner();

        if !self.namespace_map.contains_key(&prefix_iri) {
            self.namespace_map.insert(prefix_name.into(), RdfIri {
                prefix_id: 0,
                name_id: 0,
                ..Default::default()
            });
        }

        Ok(self)
    }

    pub fn for_writer<W: Write>(self, writer: W) -> WriterJellySerializer<W> {
        WriterJellySerializer {
            writer,
            inner: self,
        }
    }
}

#[must_use]
pub struct WriterJellySerializer<W: Write> {
    writer: W,
    inner: JellySerializer,
}

impl<W: Write> WriterJellySerializer<W> {
    fn find_or_create_prefix_id(&mut self, prefix: &str) -> u32 {
        match self.inner.prefix_map.get(prefix) {
            Some(id) => *id,
            None => {
                let id = self.inner.next_prefix_id;
                self.inner.prefix_map.insert(prefix.to_string(), id);
                self.inner.next_prefix_id += 1;
                id
            }
        }
    }

    fn find_or_create_name_id(&mut self, name: &str) -> u32 {
        match self.inner.name_map.get(name) {
            Some(id) => *id,
            None => {
                let id = self.inner.next_name_id;
                self.inner.name_map.insert(name.to_string(), id);
                self.inner.next_name_id += 1;
                id
            }
        }
    }

    fn find_or_create_datatype_id(&mut self, type_: &str) -> u32 {
        match self.inner.datatype_map.get(type_) {
            Some(id) => *id,
            None => {
                let id = self.inner.next_datatype_id;
                self.inner.datatype_map.insert(type_.to_string(), id);
                self.inner.next_datatype_id += 1;
                id
            }
        }
    }

    fn split_iri(&mut self, iri: &str) -> Option<(u32, u32)> {
        iri.rfind(['#', '/'])
            .map(|index| iri.split_at(index + 1))
            .map(|(prefix, name)| (self.find_or_create_prefix_id(prefix), self.find_or_create_name_id(name)))
    }

    pub fn serialize_quad<'a>(&mut self, t: impl Into<QuadRef<'a>>) -> io::Result<()> {
        let oxigraph_quad = t.into();

        let subject = match oxigraph_quad.subject {
            NamedOrBlankNodeRef::NamedNode(node) => {
                let (prefix_id, name_id) = self.split_iri(node.as_str()).unwrap_or((0,0));
                Subject::SIri(RdfIri {
                    prefix_id,
                    name_id,
                    ..Default::default()
                })
            },
            NamedOrBlankNodeRef::BlankNode(node) => Subject::SBnode(node.to_string()),
        };

        let (prefix_id, name_id) = self.split_iri(oxigraph_quad.predicate.as_str()).unwrap_or((0,0));
        let predicate = Predicate::PIri(RdfIri {
            prefix_id,
            name_id,
            ..Default::default()
        });

        let object = match oxigraph_quad.object {
            TermRef::NamedNode(node) => {
                let (prefix_id, name_id) = self.split_iri(node.as_str()).unwrap_or((0,0));
                Object::OIri(RdfIri {
                    prefix_id,
                    name_id,
                    ..Default::default()
                })
            },
            TermRef::BlankNode(node) => Object::OBnode(node.to_string()),
            TermRef::Literal(literal) => {
                let literal_kind = literal.language()
                    .map(|lang| LiteralKind::Langtag(lang.to_string()))
                    .unwrap_or(
                        LiteralKind::Datatype(
                            self.find_or_create_datatype_id(
                                literal.datatype().as_str()
                            )
                        )
                    );

                Object::OLiteral(RdfLiteral {
                    lex: literal.value().to_string(),
                    literalKind: Some(literal_kind),
                    ..Default::default()
                })
            },
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(_) => todo!(),
        };

        let graph = match oxigraph_quad.graph_name {
            GraphNameRef::NamedNode(node) => {
                let (prefix_id, name_id) = self.split_iri(node.as_str()).unwrap_or((0,0));
                Graph::GIri(RdfIri {
                    prefix_id,
                    name_id,
                    ..Default::default()
                })
            },
            GraphNameRef::BlankNode(node) => Graph::GBnode(node.to_string()),
            GraphNameRef::DefaultGraph => Graph::GDefaultGraph(RdfDefaultGraph::default())
        };

        let jelly_quad = SortableRdfQuad {
            subject: SortableSubject(subject),
            predicate: SortablePredicate(predicate),
            object: SortableObject(object),
            graph_name: SortableGraphName(graph),
        };

        self.inner.quads.insert(jelly_quad);

        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        let mut frame = RdfStreamFrame::default();

        let mut options_row = RdfStreamRow::default();
        options_row.set_options(RdfStreamOptions {
            stream_name: self.inner.stream_name,
            physical_type: EnumOrUnknown::new(PhysicalStreamType::PHYSICAL_STREAM_TYPE_QUADS),
            max_prefix_table_size: cmp::min(self.inner.prefix_map.len(), u32::MAX as usize) as u32,
            max_name_table_size: cmp::min(self.inner.name_map.len(), u32::MAX as usize) as u32,
            max_datatype_table_size: cmp::min(self.inner.datatype_map.len(), u32::MAX as usize) as u32,
            version: 2,
            ..Default::default()
        });
        frame.rows.push(options_row);

        for (value, id) in self.inner.prefix_map {
            let mut row = RdfStreamRow::default();
            row.set_prefix(RdfPrefixEntry {
                id,
                value,
                ..Default::default()
            });
            frame.rows.push(row);
        }

        for (name, value) in self.inner.namespace_map {
            let mut row = RdfStreamRow::default();
            row.set_namespace(RdfNamespaceDeclaration {
                name,
                value: MessageField::some(value),
                ..Default::default()
            })
        }

        for (value, id) in self.inner.name_map {
            let mut row = RdfStreamRow::default();
            row.set_name(RdfNameEntry {
                id,
                value,
                ..Default::default()
            });
            frame.rows.push(row);
        }

        for (value, id) in self.inner.datatype_map {
            let mut row = RdfStreamRow::default();
            row.set_datatype(RdfDatatypeEntry {
                id,
                value,
                ..Default::default()
            });
            frame.rows.push(row);
        }

        let mut previous_quad: Option<SortableRdfQuad> = None;
        for current_quad in self.inner.quads {
            let mut new_quad = RdfQuad::default();

            if let Some(previous_quad) = previous_quad {
                if previous_quad.subject != current_quad.subject {
                    new_quad.subject = Some(current_quad.subject.0.clone());
                }
                if previous_quad.predicate != current_quad.predicate {
                    new_quad.predicate = Some(current_quad.predicate.0.clone());
                }
                if previous_quad.object != current_quad.object {
                    new_quad.object = Some(current_quad.object.0.clone());
                }
                if previous_quad.graph_name != current_quad.graph_name {
                    new_quad.graph = Some(current_quad.graph_name.0.clone());
                }
            } else {
                new_quad.subject = Some(current_quad.subject.0.clone());
                new_quad.predicate = Some(current_quad.predicate.0.clone());
                new_quad.object = Some(current_quad.object.0.clone());
                new_quad.graph = Some(current_quad.graph_name.0.clone());
            }

            let mut row = RdfStreamRow::default();
            row.set_quad(new_quad);
            frame.rows.push(row);

            previous_quad = Some(current_quad);
        }

        frame.write_to_writer(&mut self.writer)?;
        Ok(self.writer)
    }
}