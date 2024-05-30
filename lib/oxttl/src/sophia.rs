//! Sophia trait implementation of OxTTL types.
//!
//! Sophia [parsers](sophia_api::parser) and [serializers](sophia_api::serializer) are expected to be reusable,
//! while OxTTL parsers and serializer are "one-shot" (they are consumed by the `parse`/`serialize` method).
//! So this module introduces simple "factory" types,
//! which implement the Sophia traits:
//! * [`N3ParserFactory`]
//! * [`NQuadsParserFactory`]
//! * [`NQuadsSerializerFactory`]
//! * [`NTriplesParserFactory`]
//! * [`NTriplesSerializerFactory`]
//! * [`TriGParserFactory`]
//! * [`TriGSerializerFactory`]
//! * [`TurtleParserFactory`]
//! * [`TurtleSerializerFactory`]
//!
//! Each of this type is a simple wrapper around a closure producing the corresponding OxTTL parser or serializer.
//! For serializers, they also expect the "sink" (usually a [`std::io::Write`]) to which the serializer must write.

use std::io::{Read, Write};

use oxrdf::sophia::{QuadExt, TripleExt};
use sophia_api::{
    parser::{QuadParser, TripleParser},
    quad::Spog,
    serializer::{QuadSerializer, Stringifier, TripleSerializer},
    source::{QuadSource, StreamError, StreamResult, TripleSource},
    term::{BnodeId, IriRef as SoIriRef, SimpleTerm},
};
use thiserror::Error;

use crate::{
    n3::FromReadN3Reader, nquads::FromReadNQuadsReader, ntriples::FromReadNTriplesReader,
    trig::FromReadTriGReader, turtle::FromReadTurtleReader, N3Parser, NQuadsParser,
    NQuadsSerializer, NTriplesParser, NTriplesSerializer, TriGParser, TriGSerializer,
    TurtleParseError, TurtleParser, TurtleSerializer,
};

// Parsers

/// Wraps a [`NQuadsParser`]-producing function, and implements [`sophia_api::parser::QuadParser`].
pub struct N3ParserFactory<F: Fn() -> N3Parser>(pub F);

impl<F, R> QuadParser<R> for N3ParserFactory<F>
where
    F: Fn() -> N3Parser,
    R: Read,
{
    type Source = N3SophiaSource<R>;

    fn parse(&self, data: R) -> Self::Source {
        N3SophiaSource((self.0)().parse_read(data))
    }
}

/// The [`QuadSource`] type returned by [`N3ParserFactory`].
pub struct N3SophiaSource<R: Read>(FromReadN3Reader<R>);

impl<R: Read> QuadSource for N3SophiaSource<R> {
    type Quad<'x> = Spog<SimpleTerm<'x>>;

    type Error = TurtleParseError;

    fn try_for_some_quad<E, F>(&mut self, mut func: F) -> StreamResult<bool, Self::Error, E>
    where
        E: std::error::Error,
        F: FnMut(Self::Quad<'_>) -> Result<(), E>,
    {
        match self.0.next() {
            None => Ok(false),
            Some(Err(err)) => Err(StreamError::SourceError(err)),
            Some(Ok(quad)) => {
                let s = quad.subject.as_simple();
                let p = quad.predicate.as_simple();
                let o = quad.object.as_simple();
                let g = match &quad.graph_name {
                    oxrdf::GraphName::NamedNode(n) => {
                        Some(SimpleTerm::Iri(SoIriRef::new_unchecked(n.as_str().into())))
                    }
                    oxrdf::GraphName::BlankNode(b) => Some(SimpleTerm::BlankNode(
                        BnodeId::new_unchecked(b.as_str().into()),
                    )),
                    oxrdf::GraphName::DefaultGraph => None,
                };
                func(([s, p, o], g)).map_err(StreamError::SinkError)?;
                Ok(true)
            }
        }
    }
}

/// Wraps a [`NQuadsParser`]-producing function, and implements [`sophia_api::parser::QuadParser`].
pub struct NQuadsParserFactory<F: Fn() -> NQuadsParser>(pub F);

impl<F, R> QuadParser<R> for NQuadsParserFactory<F>
where
    F: Fn() -> NQuadsParser,
    R: Read,
{
    type Source = FromReadNQuadsReader<R>;

    fn parse(&self, data: R) -> Self::Source {
        (self.0)().parse_read(data)
    }
}

/// Wraps a [`NTriplesParser`]-producing function, and implements [`sophia_api::parser::TripleParser`].
pub struct NTriplesParserFactory<F: Fn() -> NTriplesParser>(pub F);

impl<F, R> TripleParser<R> for NTriplesParserFactory<F>
where
    F: Fn() -> NTriplesParser,
    R: Read,
{
    type Source = FromReadNTriplesReader<R>;

    fn parse(&self, data: R) -> Self::Source {
        (self.0)().parse_read(data)
    }
}

/// Wraps a [`TriGParser`]-producing function, and implements [`sophia_api::parser::QuadParser`].
pub struct TriGParserFactory<F: Fn() -> TriGParser>(pub F);

impl<F, R> QuadParser<R> for TriGParserFactory<F>
where
    F: Fn() -> TriGParser,
    R: Read,
{
    type Source = FromReadTriGReader<R>;

    fn parse(&self, data: R) -> Self::Source {
        (self.0)().parse_read(data)
    }
}

/// Wraps a [`TurtleParser`]-producing function, and implements [`sophia_api::parser::TripleParser`].
pub struct TurtleParserFactory<F: Fn() -> TurtleParser>(pub F);

impl<F, R> TripleParser<R> for TurtleParserFactory<F>
where
    F: Fn() -> TurtleParser,
    R: Read,
{
    type Source = FromReadTurtleReader<R>;

    fn parse(&self, data: R) -> Self::Source {
        (self.0)().parse_read(data)
    }
}

// Serializers

/// Wraps a [`NQuadsParser`]-producing function and a [`Write`],
/// and implements [`sophia_api::parser::QuadParser`].
///
/// To get a [`Stringifier`], pass an empty `Vec<u8>` as the 2nd member.
pub struct NQuadsSerializerFactory<F: Fn() -> NQuadsSerializer, W: Write>(pub F, pub W);

impl<F, W> QuadSerializer for NQuadsSerializerFactory<F, W>
where
    F: Fn() -> NQuadsSerializer,
    W: Write,
{
    type Error = SerializeError;

    fn serialize_quads<QS>(
        &mut self,
        mut source: QS,
    ) -> StreamResult<&mut Self, QS::Error, Self::Error>
    where
        QS: QuadSource,
        Self: Sized,
    {
        let mut writer = (self.0)().serialize_to_write(&mut self.1);
        source.try_for_each_quad(|q| {
            let res = q.pass_as_quad_ref(|q| writer.write_quad(q));
            SerializeError::map_result(res)
        })?;
        writer.finish();
        Ok(self)
    }
}

impl<F> Stringifier for NQuadsSerializerFactory<F, Vec<u8>>
where
    F: Fn() -> NQuadsSerializer,
{
    fn as_utf8(&self) -> &[u8] {
        &self.1
    }
}

/// Wraps a [`NTriplesParser`]-producing function and a [`Write`],
/// and implements [`sophia_api::parser::QuadParser`].
///
/// To get a [`Stringifier`], pass an empty `Vec<u8>` as the 2nd member.
pub struct NTriplesSerializerFactory<F: Fn() -> NTriplesSerializer, W: Write>(pub F, pub W);

impl<F, W> TripleSerializer for NTriplesSerializerFactory<F, W>
where
    F: Fn() -> NTriplesSerializer,
    W: Write,
{
    type Error = SerializeError;

    fn serialize_triples<TS>(
        &mut self,
        mut source: TS,
    ) -> StreamResult<&mut Self, TS::Error, Self::Error>
    where
        TS: TripleSource,
        Self: Sized,
    {
        let mut writer = (self.0)().serialize_to_write(&mut self.1);
        source.try_for_each_triple(|q| {
            let res = q.pass_as_triple_ref(|q| writer.write_triple(q));
            SerializeError::map_result(res)
        })?;
        writer.finish();
        Ok(self)
    }
}

impl<F> Stringifier for NTriplesSerializerFactory<F, Vec<u8>>
where
    F: Fn() -> NTriplesSerializer,
{
    fn as_utf8(&self) -> &[u8] {
        &self.1
    }
}

/// Wraps a [`TriGParser`]-producing function and a [`Write`],
/// and implements [`sophia_api::parser::QuadParser`].
///
/// To get a [`Stringifier`], pass an empty `Vec<u8>` as the 2nd member.
pub struct TriGSerializerFactory<F: Fn() -> TriGSerializer, W: Write>(pub F, pub W);

impl<F, W> QuadSerializer for TriGSerializerFactory<F, W>
where
    F: Fn() -> TriGSerializer,
    W: Write,
{
    type Error = SerializeError;

    fn serialize_quads<QS>(
        &mut self,
        mut source: QS,
    ) -> StreamResult<&mut Self, QS::Error, Self::Error>
    where
        QS: QuadSource,
        Self: Sized,
    {
        let mut writer = (self.0)().serialize_to_write(&mut self.1);
        source.try_for_each_quad(|q| {
            let res = q.pass_as_quad_ref(|q| writer.write_quad(q));
            SerializeError::map_result(res)
        })?;
        writer
            .finish()
            .map_err(|e| StreamError::SinkError(e.into()))?;
        Ok(self)
    }
}

impl<F> Stringifier for TriGSerializerFactory<F, Vec<u8>>
where
    F: Fn() -> TriGSerializer,
{
    fn as_utf8(&self) -> &[u8] {
        &self.1
    }
}

/// Wraps a [`TurtleParser`]-producing function and a [`Write`],
/// and implements [`sophia_api::parser::QuadParser`].
///
/// To get a [`Stringifier`], pass an empty `Vec<u8>` as the 2nd member.
pub struct TurtleSerializerFactory<F: Fn() -> TurtleSerializer, W: Write>(pub F, pub W);

impl<F, W> TripleSerializer for TurtleSerializerFactory<F, W>
where
    F: Fn() -> TurtleSerializer,
    W: Write,
{
    type Error = SerializeError;

    fn serialize_triples<TS>(
        &mut self,
        mut source: TS,
    ) -> StreamResult<&mut Self, TS::Error, Self::Error>
    where
        TS: TripleSource,
        Self: Sized,
    {
        let mut writer = (self.0)().serialize_to_write(&mut self.1);
        source.try_for_each_triple(|q| {
            let res = q.pass_as_triple_ref(|q| writer.write_triple(q));
            SerializeError::map_result(res)
        })?;
        writer
            .finish()
            .map_err(|e| StreamError::SinkError(e.into()))?;
        Ok(self)
    }
}

impl<F> Stringifier for TurtleSerializerFactory<F, Vec<u8>>
where
    F: Fn() -> TurtleSerializer,
{
    fn as_utf8(&self) -> &[u8] {
        &self.1
    }
}

/// Error generated by Sophia serializers
#[derive(Error, Debug)]
pub enum SerializeError {
    #[error("IOError: {0}")]
    IO(#[from] std::io::Error),
    #[error("Unsupported generalized RDF")]
    Unsupported,
}

impl SerializeError {
    fn map_result<T>(res: Option<Result<T, std::io::Error>>) -> Result<T, Self> {
        match res {
            None => Err(SerializeError::Unsupported),
            Some(Err(io)) => Err(io.into()),
            Some(Ok(ok)) => Ok(ok),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{NQuadsSerializer, NTriplesSerializer, TriGSerializer, TurtleSerializer};
    use sophia_api::{
        quad::{Quad, Spog},
        source::{QuadSource, TripleSource},
        term::{IriRef, SimpleTerm, Term},
    };

    #[test]
    fn n3() -> Result<(), Box<dyn std::error::Error>> {
        let src = r#"
            @prefix : <tag:>.

            "foo" "bar" {
                :s :p :o
            }.
        "#;
        let v: Vec<Spog<SimpleTerm<'static>>> = N3ParserFactory(|| N3Parser::new())
            .parse_str(src) // Sophia parser API
            .collect_quads()?;
        assert_eq!(v.len(), 2);
        for q in v {
            if q.s().is_literal() {
                assert!(Term::eq(q.s(), "foo"));
                assert!(Term::eq(q.p(), "bar"));
                assert!(q.o().is_blank_node());
                assert!(q.g().is_none());
            } else {
                assert!(Term::eq(q.s(), SoIriRef::new_unchecked("tag:s")));
                assert!(Term::eq(q.p(), SoIriRef::new_unchecked("tag:p")));
                assert!(Term::eq(q.o(), SoIriRef::new_unchecked("tag:o")));
                assert!(q.g().unwrap().is_blank_node());
            }
        }
        Ok(())
    }

    #[test]
    fn nq() -> Result<(), Box<dyn std::error::Error>> {
        let v1 = make_dataset();
        let txt = NQuadsSerializerFactory(|| NQuadsSerializer::new(), vec![])
            .serialize_dataset(&v1)? // Sophia serializer API
            .to_string(); // Sophia stringifier API
        let v2: Vec<Spog<SimpleTerm<'static>>> = NQuadsParserFactory(|| NQuadsParser::new())
            .parse_str(&txt) // Sophia parser API
            .collect_quads()?;
        assert_eq!(v1, v2);
        Ok(())
    }

    #[test]
    fn nt() -> Result<(), Box<dyn std::error::Error>> {
        let v1 = make_graph();
        let txt = NTriplesSerializerFactory(|| NTriplesSerializer::new(), vec![])
            .serialize_graph(&v1)? // Sophia serializer API
            .to_string(); // Sophia stringifier API
        let v2: Vec<[SimpleTerm<'static>; 3]> = NTriplesParserFactory(|| NTriplesParser::new())
            .parse_str(&txt) // Sophia parser API
            .collect_triples()?;
        assert_eq!(v1, v2);
        Ok(())
    }

    #[test]
    fn trig() -> Result<(), Box<dyn std::error::Error>> {
        let v1 = make_dataset();
        let txt = TriGSerializerFactory(|| TriGSerializer::new(), vec![])
            .serialize_dataset(&v1)? // Sophia serializer API
            .to_string(); // Sophia stringifier API
        let v2: Vec<Spog<SimpleTerm<'static>>> = TriGParserFactory(|| TriGParser::new())
            .parse_str(&txt) // Sophia parser API
            .collect_quads()?;
        assert_eq!(v1, v2);
        Ok(())
    }

    #[test]
    fn turtle() -> Result<(), Box<dyn std::error::Error>> {
        let v1 = make_graph();
        let txt = TurtleSerializerFactory(|| TurtleSerializer::new(), vec![])
            .serialize_graph(&v1)? // Sophia serializer API
            .to_string(); // Sophia stringifier API
        let v2: Vec<[SimpleTerm<'static>; 3]> = TurtleParserFactory(|| TurtleParser::new())
            .parse_str(&txt) // Sophia parser API
            .collect_triples()?;
        assert_eq!(v1, v2);
        Ok(())
    }

    fn make_graph() -> Vec<[SimpleTerm<'static>; 3]> {
        [[
            "https://example.org/s",
            "https://example.org/p",
            "https://example.org/o",
        ]]
        .into_iter()
        .map(|trpl| trpl.map(|txt| IriRef::new_unchecked(txt).into_term()))
        .collect()
    }

    fn make_dataset() -> Vec<Spog<SimpleTerm<'static>>> {
        [[
            "https://example.org/s",
            "https://example.org/p",
            "https://example.org/o",
            "https://example.org/g",
        ]]
        .into_iter()
        .map(|q| q.map(|txt| IriRef::new_unchecked(txt).into_term()))
        .map(|[s, p, o, g]| ([s, p, o], Some(g)))
        .collect()
    }
}
