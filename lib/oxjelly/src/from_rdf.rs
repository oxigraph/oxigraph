use std::collections::BTreeSet;
use std::io;
use std::io::Write;
use protobuf::{EnumOrUnknown, Message};
use oxiri::IriParseError;
use oxrdf::{GraphName, GraphNameRef, NamedNode, NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, QuadRef, Term, TermRef};
use crate::jelly::rdf::{PhysicalStreamType, RdfDatatypeEntry, RdfDefaultGraph, RdfIri, RdfLiteral, RdfNameEntry, RdfPrefixEntry, RdfQuad, RdfStreamFrame, RdfStreamOptions, RdfStreamRow};
use crate::jelly::rdf::rdf_literal::LiteralKind;
use crate::jelly::rdf::rdf_quad::{Graph, Object, Predicate, Subject};
use crate::lookup_table::{InverseLookupTable, LookupResult};
use crate::sorted::SortableQuad;

#[derive(Default)]
struct JellyIriConverter {
    prefixes: InverseLookupTable,
    names: InverseLookupTable,
    datatypes: InverseLookupTable,
}

impl JellyIriConverter {
    fn split_iri<'a>(&self, iri: &'a str) -> (&'a str, &'a str) {
        iri.rfind(['#', '/'])
            .map(|index| iri.split_at(index + 1))
            .unwrap_or((iri, ""))
    }

    fn encode_iri(&mut self, iri: &str) -> (RdfIri, Vec<RdfStreamRow>) {
        let mut rows = vec![];

        let (prefix, name) = self.split_iri(iri);

        let prefix_result = self.prefixes.get_or_push(prefix.to_string());
        if let LookupResult::CacheMiss(id) = prefix_result {
            let mut row = RdfStreamRow::default();
            row.set_prefix(RdfPrefixEntry {
                id,
                value: prefix.to_string(),
                ..Default::default()
            });
            rows.push(row);
        }

        let name_result = self.names.get_or_push(name.to_string());
        if let LookupResult::CacheMiss(id) = name_result {
            let mut row = RdfStreamRow::default();
            row.set_name(RdfNameEntry {
                id,
                value: name.to_string(),
                ..Default::default()
            });
            rows.push(row);
        }

        let rdf_iri = RdfIri {
            prefix_id: prefix_result.into(),
            name_id: name_result.into(),
            ..Default::default()
        };

        (rdf_iri, rows)
    }

    fn encode_datatype(&mut self, datatype: &str) -> (LiteralKind, Vec<RdfStreamRow>) {
        let mut rows = vec![];

        let result = self.datatypes.get_or_push(datatype.to_string());
        if let LookupResult::CacheMiss(id) = result {
            let mut row = RdfStreamRow::default();
            row.set_datatype(RdfDatatypeEntry {
                id,
                value: datatype.to_string(),
                ..Default::default()
            });
            rows.push(row);
        }

        (LiteralKind::Datatype(result.into()), rows)
    }

    fn jelly_subject(&mut self, oxi_subject: NamedOrBlankNodeRef<'_>) -> (Subject, Vec<RdfStreamRow>) {
        match oxi_subject {
            NamedOrBlankNodeRef::NamedNode(node) => {
                let (iri, rows) = self.encode_iri(node.as_str());
                (Subject::SIri(iri), rows)
            },
            NamedOrBlankNodeRef::BlankNode(node) => (Subject::SBnode(node.to_string()), vec![]),
        }
    }

    fn jelly_predicate(&mut self, oxi_predicate: NamedNodeRef<'_>) -> (Predicate, Vec<RdfStreamRow>) {
        let (iri, rows) = self.encode_iri(oxi_predicate.as_str());
        (Predicate::PIri(iri), rows)
    }

    fn jelly_object(&mut self, oxi_object: TermRef<'_>) -> (Object, Vec<RdfStreamRow>) {
        match oxi_object {
            TermRef::NamedNode(node) => {
                let (iri, rows) = self.encode_iri(node.as_str());
                (Object::OIri(iri), rows)
            },
            TermRef::BlankNode(node) => (Object::OBnode(node.to_string()), vec![]),
            TermRef::Literal(literal) => {
                let (literal_kind, rows) = literal.language()
                    .map(|lang| (LiteralKind::Langtag(lang.to_string()), vec![]))
                    .unwrap_or(
                        self.encode_datatype(literal.datatype().as_str())
                    );

                let object = Object::OLiteral(RdfLiteral {
                    lex: literal.value().to_string(),
                    literalKind: Some(literal_kind),
                    ..Default::default()
                });

                (object, rows)
            },
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(_) => todo!(),
        }
    }

    fn jelly_graph(&mut self, oxi_graph_name: GraphNameRef<'_>) -> (Graph, Vec<RdfStreamRow>) {
        match oxi_graph_name {
            GraphNameRef::NamedNode(node) => {
                let (iri, rows) = self.encode_iri(node.as_str());
                (Graph::GIri(iri), rows)
            },
            GraphNameRef::BlankNode(node) => (Graph::GBnode(node.to_string()), vec![]),
            GraphNameRef::DefaultGraph => (Graph::GDefaultGraph(RdfDefaultGraph::default()), vec![]),
        }
    }
}

#[derive(Default, Clone)]
#[must_use]
pub struct JellySerializer {
    stream_name: String,
}

impl JellySerializer {
    pub fn new(stream_name: impl Into<String>) -> Self {
        Self {
            stream_name: stream_name.into(),
            ..Default::default()
        }
    }

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        /*let prefix_iri = Iri::parse(prefix_iri.into())?.into_inner();

        if !self.namespace_map.contains_key(&prefix_iri) {
            self.namespace_map.insert(prefix_name.into(), RdfIri {
                prefix_id: 0,
                name_id: 0,
                ..Default::default()
            });
        }*/

        Ok(self)
    }

    pub fn for_writer<W: Write>(self, writer: W) -> WriterJellySerializer<W> {
        WriterJellySerializer {
            writer,
            options_written: false,
            current_frame: Default::default(),
            converter: Default::default(),
            quads: Default::default(),
            inner: self,
        }
    }
}

#[must_use]
pub struct WriterJellySerializer<W: Write> {
    writer: W,
    options_written: bool,
    current_frame: RdfStreamFrame,
    converter: JellyIriConverter,
    quads: BTreeSet<SortableQuad>,
    inner: JellySerializer,
}

impl<W: Write> WriterJellySerializer<W> {
    fn options(&self) -> RdfStreamRow {
        let mut row = RdfStreamRow::default();
        row.set_options(RdfStreamOptions {
            stream_name: self.inner.stream_name.clone(),
            physical_type: EnumOrUnknown::new(PhysicalStreamType::PHYSICAL_STREAM_TYPE_QUADS),
            max_prefix_table_size: self.converter.prefixes.capacity(),
            max_name_table_size: self.converter.names.capacity(),
            max_datatype_table_size: self.converter.datatypes.capacity(),
            version: 2,
            ..Default::default()
        });
        row
    }

    fn flush_quads_to_current_frame(&mut self) {
        let mut previous_subject: Option<NamedOrBlankNode> = None;
        let mut previous_predicate: Option<NamedNode> = None;
        let mut previous_object: Option<Term> = None;
        let mut previous_graph_name: Option<GraphName> = None;

        for current_quad in &self.quads {
            let mut new_jelly_quad = RdfQuad::default();

            let current_subject = &current_quad.0.subject;
            if previous_subject.as_ref() != Some(current_subject) {
                let (subject, mut rows) = self.converter.jelly_subject(current_subject.as_ref());
                self.current_frame.rows.append(&mut rows);
                new_jelly_quad.subject = Some(subject);
                previous_subject = Some(current_subject.clone());
            }

            let current_predicate = &current_quad.0.predicate;
            if previous_predicate.as_ref() != Some(current_predicate) {
                let (predicate, mut rows) = self.converter.jelly_predicate(current_predicate.as_ref());
                self.current_frame.rows.append(&mut rows);
                new_jelly_quad.predicate = Some(predicate);
                previous_predicate = Some(current_predicate.clone());
            }

            let current_object = &current_quad.0.object;
            if previous_object.as_ref() != Some(current_object) {
                let (object, mut rows) = self.converter.jelly_object(current_object.as_ref());
                self.current_frame.rows.append(&mut rows);
                new_jelly_quad.object = Some(object);
                previous_object = Some(current_object.clone());
            }

            let current_graph_name = &current_quad.0.graph_name;
            if previous_graph_name.as_ref() != Some(current_graph_name) {
                let (graph_name, mut rows) = self.converter.jelly_graph(current_graph_name.as_ref());
                self.current_frame.rows.append(&mut rows);
                new_jelly_quad.graph = Some(graph_name);
                previous_graph_name = Some(current_graph_name.clone());
            }

            let mut row = RdfStreamRow::default();
            row.set_quad(new_jelly_quad);
            self.current_frame.rows.push(row);
        }
        self.quads.clear();
    }

    fn flush_current_frame(&mut self) -> io::Result<()> {
        self.flush_quads_to_current_frame();
        self.current_frame.write_length_delimited_to_writer(&mut self.writer)?;
        self.current_frame = RdfStreamFrame::default();
        Ok(())
    }

    pub fn serialize_quad<'a>(&mut self, t: impl Into<QuadRef<'a>>) -> io::Result<()> {
        if !self.options_written {
            self.current_frame.rows.push(self.options());
            self.options_written = true;
        }

        self.quads.insert(SortableQuad(t.into().into_owned()));
        if self.quads.len() >= 3 {
            self.flush_current_frame()?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush_current_frame()?;
        Ok(self.writer)
    }
}