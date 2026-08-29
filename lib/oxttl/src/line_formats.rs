//! Shared parser implementation for N-Triples and N-Quads.

use crate::MIN_BUFFER_SIZE;
use crate::lexer::{N3Lexer, N3LexerMode, N3LexerOptions, N3String, N3Token, to_lowercase};
use crate::toolkit::{Lexer, Parser, RuleRecognizer, RuleRecognizerError, TokenOrLineJump};
use oxrdf::vocab::rdf;
#[cfg(feature = "rdf-12")]
use oxrdf::{BaseDirection, Triple};
use oxrdf::{
    BlankNode, BlankNodeRef, GraphName, GraphNameRef, Literal, LiteralRef, NamedNode, NamedNodeRef,
    NamedOrBlankNode, Quad, QuadRef, Term, TermRef,
};
use oxstr::OxString;

#[derive(Clone, Copy)]
pub(crate) struct TextRange {
    start: usize,
    end: usize,
}

impl TextRange {
    fn get(self, arena: &str) -> &str {
        &arena[self.start..self.end]
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NodeDescriptor {
    Named(TextRange),
    Blank(TextRange),
}

impl NodeDescriptor {
    fn as_ref(self, arena: &str) -> oxrdf::NamedOrBlankNodeRef<'_> {
        match self {
            Self::Named(value) => NamedNodeRef::new_unchecked(value.get(arena)).into(),
            Self::Blank(value) => BlankNodeRef::new_unchecked(value.get(arena)).into(),
        }
    }

    #[cfg(feature = "rdf-12")]
    fn into_owned(self, arena: &str) -> NamedOrBlankNode {
        self.as_ref(arena).into_owned()
    }
}

pub(crate) enum TermDescriptor {
    Named(TextRange),
    Blank(TextRange),
    Simple(TextRange),
    LanguageTagged {
        value: TextRange,
        language: TextRange,
    },
    #[cfg(feature = "rdf-12")]
    DirectionalLanguageTagged {
        value: TextRange,
        language: TextRange,
        direction: BaseDirection,
    },
    Typed {
        value: TextRange,
        datatype: TextRange,
    },
    #[cfg(feature = "rdf-12")]
    // `TermRef::Triple` requires an addressable `Triple`. Boxing keeps the
    // ordinary-term descriptor small and allocates only when a triple term is parsed.
    Triple(Box<Triple>),
}

impl TermDescriptor {
    fn as_ref<'a>(&'a self, arena: &'a str) -> TermRef<'a> {
        match self {
            Self::Named(value) => NamedNodeRef::new_unchecked(value.get(arena)).into(),
            Self::Blank(value) => BlankNodeRef::new_unchecked(value.get(arena)).into(),
            Self::Simple(value) => LiteralRef::new_simple_literal(value.get(arena)).into(),
            Self::LanguageTagged { value, language } => {
                LiteralRef::new_language_tagged_literal_unchecked(
                    value.get(arena),
                    language.get(arena),
                )
                .into()
            }
            #[cfg(feature = "rdf-12")]
            Self::DirectionalLanguageTagged {
                value,
                language,
                direction,
            } => LiteralRef::new_directional_language_tagged_literal_unchecked(
                value.get(arena),
                language.get(arena),
                *direction,
            )
            .into(),
            Self::Typed { value, datatype } => LiteralRef::new_typed_literal(
                value.get(arena),
                NamedNodeRef::new_unchecked(datatype.get(arena)),
            )
            .into(),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => triple.as_ref().into(),
        }
    }

    #[cfg(feature = "rdf-12")]
    fn into_owned(self, arena: &str) -> Term {
        match self {
            Self::Triple(triple) => triple.into(),
            term => term.as_ref(arena).into_owned(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) enum GraphDescriptor {
    #[default]
    Default,
    Named(TextRange),
    Blank(TextRange),
}

pub(crate) struct BorrowedQuad {
    arena: String,
    subject: NodeDescriptor,
    predicate: TextRange,
    object: TermDescriptor,
    graph_name: GraphDescriptor,
}

impl BorrowedQuad {
    pub(crate) fn as_ref(&self) -> QuadRef<'_> {
        QuadRef {
            subject: self.subject.as_ref(&self.arena),
            predicate: NamedNodeRef::new_unchecked(self.predicate.get(&self.arena)),
            object: self.object.as_ref(&self.arena),
            graph_name: match self.graph_name {
                GraphDescriptor::Default => GraphNameRef::DefaultGraph,
                GraphDescriptor::Named(value) => {
                    NamedNodeRef::new_unchecked(value.get(&self.arena)).into()
                }
                GraphDescriptor::Blank(value) => {
                    BlankNodeRef::new_unchecked(value.get(&self.arena)).into()
                }
            },
        }
    }
}

pub(crate) trait NQuadsOutputBuilder {
    const BUFFER_OUTPUT_UNTIL_LINE_END: bool;

    type Subject;
    type Predicate;
    type Object;
    type LiteralValue;
    type GraphName;
    type Output;

    fn named_subject(&mut self, value: N3String<'_>) -> Self::Subject;
    fn blank_subject(&mut self, value: &str) -> Self::Subject;
    fn predicate(&mut self, value: N3String<'_>) -> Self::Predicate;
    fn named_object(&mut self, value: N3String<'_>) -> Self::Object;
    fn blank_object(&mut self, value: &str) -> Self::Object;
    fn literal_value(&mut self, value: N3String<'_>) -> Self::LiteralValue;
    fn simple_literal(&mut self, value: Self::LiteralValue) -> Self::Object;
    fn language_tagged_literal(
        &mut self,
        value: Self::LiteralValue,
        language: &str,
        #[cfg(feature = "rdf-12")] direction: Option<BaseDirection>,
    ) -> Self::Object;
    fn typed_literal(&mut self, value: Self::LiteralValue, datatype: N3String<'_>) -> Self::Object;
    #[cfg(feature = "rdf-12")]
    fn triple(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
    ) -> Self::Object;
    fn named_graph(&mut self, value: N3String<'_>) -> Self::GraphName;
    fn blank_graph(&mut self, value: &str) -> Self::GraphName;
    fn default_graph(&mut self) -> Self::GraphName;
    fn quad(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
        graph_name: Self::GraphName,
    ) -> Self::Output;
    fn clear(&mut self) {}
    fn reuse_output(&mut self, _output: Self::Output) {}
}

#[derive(Default)]
pub(crate) struct OwnedNQuadsOutputBuilder;

impl NQuadsOutputBuilder for OwnedNQuadsOutputBuilder {
    const BUFFER_OUTPUT_UNTIL_LINE_END: bool = false;

    type Subject = NamedOrBlankNode;
    type Predicate = NamedNode;
    type Object = Term;
    type LiteralValue = OxString;
    type GraphName = GraphName;
    type Output = Quad;

    fn named_subject(&mut self, value: N3String<'_>) -> Self::Subject {
        NamedNode::new_unchecked(value.into_owned()).into()
    }

    fn blank_subject(&mut self, value: &str) -> Self::Subject {
        BlankNode::new_unchecked(OxString::new_owned(value)).into()
    }

    fn predicate(&mut self, value: N3String<'_>) -> Self::Predicate {
        NamedNode::new_unchecked(value.into_owned())
    }

    fn named_object(&mut self, value: N3String<'_>) -> Self::Object {
        NamedNode::new_unchecked(value.into_owned()).into()
    }

    fn blank_object(&mut self, value: &str) -> Self::Object {
        BlankNode::new_unchecked(OxString::new_owned(value)).into()
    }

    fn literal_value(&mut self, value: N3String<'_>) -> Self::LiteralValue {
        value.into_owned()
    }

    fn simple_literal(&mut self, value: Self::LiteralValue) -> Self::Object {
        Literal::new_simple_literal(value).into()
    }

    fn language_tagged_literal(
        &mut self,
        value: Self::LiteralValue,
        language: &str,
        #[cfg(feature = "rdf-12")] direction: Option<BaseDirection>,
    ) -> Self::Object {
        #[cfg(feature = "rdf-12")]
        if let Some(direction) = direction {
            return Literal::new_directional_language_tagged_literal_unchecked(
                value,
                to_lowercase(language),
                direction,
            )
            .into();
        }
        Literal::new_language_tagged_literal_unchecked(value, to_lowercase(language)).into()
    }

    fn typed_literal(&mut self, value: Self::LiteralValue, datatype: N3String<'_>) -> Self::Object {
        Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype.into_owned())).into()
    }

    #[cfg(feature = "rdf-12")]
    fn triple(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
    ) -> Self::Object {
        Triple {
            subject,
            predicate,
            object,
        }
        .into()
    }

    fn named_graph(&mut self, value: N3String<'_>) -> Self::GraphName {
        NamedNode::new_unchecked(value.into_owned()).into()
    }

    fn blank_graph(&mut self, value: &str) -> Self::GraphName {
        BlankNode::new_unchecked(OxString::new_owned(value)).into()
    }

    fn default_graph(&mut self) -> Self::GraphName {
        GraphName::DefaultGraph
    }

    fn quad(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
        graph_name: Self::GraphName,
    ) -> Self::Output {
        Quad {
            subject,
            predicate,
            object,
            graph_name,
        }
    }
}

#[derive(Default)]
pub(crate) struct BorrowedNQuadsOutputBuilder {
    arena: String,
}

impl BorrowedNQuadsOutputBuilder {
    #[inline]
    fn text(&mut self, value: &str) -> TextRange {
        let start = self.arena.len();
        self.arena.push_str(value);
        TextRange {
            start,
            end: self.arena.len(),
        }
    }

    #[inline]
    fn lowercase_text(&mut self, value: &str) -> TextRange {
        let start = self.arena.len();
        self.arena.extend(
            value
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        TextRange {
            start,
            end: self.arena.len(),
        }
    }
}

impl NQuadsOutputBuilder for BorrowedNQuadsOutputBuilder {
    const BUFFER_OUTPUT_UNTIL_LINE_END: bool = true;

    type Subject = NodeDescriptor;
    type Predicate = TextRange;
    type Object = TermDescriptor;
    type LiteralValue = TextRange;
    type GraphName = GraphDescriptor;
    type Output = BorrowedQuad;

    #[inline]
    fn named_subject(&mut self, value: N3String<'_>) -> Self::Subject {
        NodeDescriptor::Named(self.text(value.as_str()))
    }

    #[inline]
    fn blank_subject(&mut self, value: &str) -> Self::Subject {
        NodeDescriptor::Blank(self.text(value))
    }

    #[inline]
    fn predicate(&mut self, value: N3String<'_>) -> Self::Predicate {
        self.text(value.as_str())
    }

    #[inline]
    fn named_object(&mut self, value: N3String<'_>) -> Self::Object {
        TermDescriptor::Named(self.text(value.as_str()))
    }

    #[inline]
    fn blank_object(&mut self, value: &str) -> Self::Object {
        TermDescriptor::Blank(self.text(value))
    }

    #[inline]
    fn literal_value(&mut self, value: N3String<'_>) -> Self::LiteralValue {
        self.text(value.as_str())
    }

    #[inline]
    fn simple_literal(&mut self, value: Self::LiteralValue) -> Self::Object {
        TermDescriptor::Simple(value)
    }

    #[inline]
    fn language_tagged_literal(
        &mut self,
        value: Self::LiteralValue,
        language: &str,
        #[cfg(feature = "rdf-12")] direction: Option<BaseDirection>,
    ) -> Self::Object {
        let language = self.lowercase_text(language);
        #[cfg(feature = "rdf-12")]
        if let Some(direction) = direction {
            return TermDescriptor::DirectionalLanguageTagged {
                value,
                language,
                direction,
            };
        }
        TermDescriptor::LanguageTagged { value, language }
    }

    #[inline]
    fn typed_literal(&mut self, value: Self::LiteralValue, datatype: N3String<'_>) -> Self::Object {
        let datatype = self.text(datatype.as_str());
        TermDescriptor::Typed { value, datatype }
    }

    #[cfg(feature = "rdf-12")]
    #[inline]
    fn triple(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
    ) -> Self::Object {
        TermDescriptor::Triple(Box::new(Triple {
            subject: subject.into_owned(&self.arena),
            predicate: NamedNodeRef::new_unchecked(predicate.get(&self.arena)).into_owned(),
            object: object.into_owned(&self.arena),
        }))
    }

    #[inline]
    fn named_graph(&mut self, value: N3String<'_>) -> Self::GraphName {
        GraphDescriptor::Named(self.text(value.as_str()))
    }

    #[inline]
    fn blank_graph(&mut self, value: &str) -> Self::GraphName {
        GraphDescriptor::Blank(self.text(value))
    }

    #[inline]
    fn default_graph(&mut self) -> Self::GraphName {
        GraphDescriptor::Default
    }

    #[inline]
    fn quad(
        &mut self,
        subject: Self::Subject,
        predicate: Self::Predicate,
        object: Self::Object,
        graph_name: Self::GraphName,
    ) -> Self::Output {
        BorrowedQuad {
            arena: std::mem::take(&mut self.arena),
            subject,
            predicate,
            object,
            graph_name,
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.arena.clear();
    }

    #[inline]
    fn reuse_output(&mut self, mut output: Self::Output) {
        output.arena.clear();
        if output.arena.capacity() > self.arena.capacity() {
            self.arena = output.arena;
        }
    }
}

pub(crate) struct NQuadsRecognizer<B: NQuadsOutputBuilder = OwnedNQuadsOutputBuilder> {
    state: NQuadsState<B::LiteralValue>,
    #[cfg(feature = "rdf-12")]
    quoted_triples: Vec<QuotedTripleFrame<B::Subject, B::Predicate>>,
    subject: Option<B::Subject>,
    predicate: Option<B::Predicate>,
    object: Option<B::Object>,
    pending_graph_name: Option<B::GraphName>,
    output_builder: B,
    lenient: bool,
}

pub(crate) type BorrowedNQuadsRecognizer = NQuadsRecognizer<BorrowedNQuadsOutputBuilder>;

pub(crate) struct NQuadsRecognizerContext {
    with_graph_name: bool,
    lexer_options: N3LexerOptions,
}

enum NQuadsState<L> {
    ExpectSubject,
    ExpectPredicate,
    ExpectedObject,
    ExpectPossibleGraphOrEndOfQuotedTriple,
    ExpectDot,
    ExpectLiteralAnnotationOrGraphNameOrDot {
        value: L,
    },
    ExpectLiteralDatatype {
        value: L,
    },
    ExpectLineJump,
    RecoverToLineJump,
    #[cfg(feature = "rdf-12")]
    AfterQuotedTriple,
}

#[cfg(feature = "rdf-12")]
struct QuotedTripleFrame<S, P> {
    subject: S,
    predicate: P,
}

impl<B: NQuadsOutputBuilder> RuleRecognizer for NQuadsRecognizer<B> {
    type TokenRecognizer = N3Lexer;
    type Output = B::Output;
    type Context = NQuadsRecognizerContext;

    fn set_error_recovery_state(&mut self) {
        #[cfg(feature = "rdf-12")]
        self.quoted_triples.clear();
        self.state = NQuadsState::RecoverToLineJump;
        self.subject = None;
        self.predicate = None;
        self.object = None;
        self.pending_graph_name = None;
        self.output_builder.clear();
    }

    fn recognize_next(
        &mut self,
        token: TokenOrLineJump<N3Token<'_>>,
        context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        let state = std::mem::replace(&mut self.state, NQuadsState::RecoverToLineJump);
        match state {
            NQuadsState::ExpectSubject => match token {
                TokenOrLineJump::Token(token) => match token {
                    N3Token::IriRef(s) => {
                        self.subject = Some(self.output_builder.named_subject(s));
                        self.state = NQuadsState::ExpectPredicate;
                    }
                    N3Token::BlankNodeLabel(s) => {
                        self.subject = Some(self.output_builder.blank_subject(s));
                        self.state = NQuadsState::ExpectPredicate;
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The subject of a triple must be an IRI or a blank node",
                    ),
                },
                TokenOrLineJump::LineJump => {
                    #[cfg(feature = "rdf-12")]
                    if self.quoted_triples.is_empty() {
                        self.state = NQuadsState::ExpectSubject;
                    } else {
                        self.error(
                            context,
                            results,
                            errors,
                            token,
                            "line jumps are not allowed inside of quoted triples",
                        )
                    }
                    #[cfg(not(feature = "rdf-12"))]
                    {
                        self.state = NQuadsState::ExpectSubject;
                    }
                }
            },
            NQuadsState::ExpectPredicate => match token {
                TokenOrLineJump::Token(token) => match token {
                    N3Token::IriRef(p) => {
                        self.predicate = Some(self.output_builder.predicate(p));
                        self.state = NQuadsState::ExpectedObject;
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The predicate of a triple must be an IRI",
                    ),
                },
                TokenOrLineJump::LineJump => self.error(
                    context,
                    results,
                    errors,
                    token,
                    "line jumps are not allowed in the middle of triples",
                ),
            },
            NQuadsState::ExpectedObject => match token {
                TokenOrLineJump::Token(token) => match token {
                    N3Token::IriRef(o) => {
                        self.object = Some(self.output_builder.named_object(o));
                        self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                    }
                    N3Token::BlankNodeLabel(o) => {
                        self.object = Some(self.output_builder.blank_object(o));
                        self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                    }
                    N3Token::String(value) => {
                        let value = self.output_builder.literal_value(value);
                        self.state = NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value };
                    }
                    #[cfg(feature = "rdf-12")]
                    N3Token::Punctuation("<<(") => {
                        self.quoted_triples.push(QuotedTripleFrame {
                            subject: self.subject.take().unwrap(),
                            predicate: self.predicate.take().unwrap(),
                        });
                        self.state = NQuadsState::ExpectSubject;
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The object of a triple must be an IRI, a blank node or a literal",
                    ),
                },
                TokenOrLineJump::LineJump => self.error(
                    context,
                    results,
                    errors,
                    token,
                    "line jumps are not allowed in the middle of triples",
                ),
            },
            NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value } => match token {
                TokenOrLineJump::Token(N3Token::LangTag {
                    language,
                    #[cfg(feature = "rdf-12")]
                    direction,
                }) => {
                    self.object = Some(self.output_builder.language_tagged_literal(
                        value,
                        language,
                        #[cfg(feature = "rdf-12")]
                        direction,
                    ));
                    self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                }
                TokenOrLineJump::Token(N3Token::Punctuation("^^")) => {
                    self.state = NQuadsState::ExpectLiteralDatatype { value };
                }
                _ => {
                    self.object = Some(self.output_builder.simple_literal(value));
                    self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                    self.recognize_next(token, context, results, errors)
                }
            },
            NQuadsState::ExpectLiteralDatatype { value } => match token {
                TokenOrLineJump::Token(token) => match token {
                    N3Token::IriRef(d) => {
                        if !self.lenient && d.as_str() == rdf::LANG_STRING.as_str() {
                            errors.push("The datatype of a literal without a language tag must not be rdf:langString".into());
                        }
                        #[cfg(feature = "rdf-12")]
                        if !self.lenient && d.as_str() == rdf::DIR_LANG_STRING.as_str() {
                            errors.push("The datatype of a literal without a base direction must not be rdf:dirLangString".into());
                        }
                        self.object = Some(self.output_builder.typed_literal(value, d));
                        self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "A literal datatype must be an IRI",
                    ),
                },
                TokenOrLineJump::LineJump => self.error(
                    context,
                    results,
                    errors,
                    token,
                    "line jumps are not allowed in the middle of triples",
                ),
            },
            NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple => {
                #[cfg(feature = "rdf-12")]
                let inside_quoted_triple = !self.quoted_triples.is_empty();
                #[cfg(not(feature = "rdf-12"))]
                let inside_quoted_triple = false;
                if inside_quoted_triple {
                    #[cfg(feature = "rdf-12")]
                    if token == TokenOrLineJump::Token(N3Token::Punctuation(")>>")) {
                        self.state = NQuadsState::AfterQuotedTriple;
                    } else {
                        self.error(
                            context,
                            results,
                            errors,
                            token,
                            "Expecting the end of a quoted triple ')>>'",
                        )
                    }
                } else {
                    match token {
                        TokenOrLineJump::Token(N3Token::IriRef(g)) if context.with_graph_name => {
                            let graph_name = self.output_builder.named_graph(g);
                            self.emit_quad(results, graph_name);
                            self.state = NQuadsState::ExpectDot;
                        }
                        TokenOrLineJump::Token(N3Token::BlankNodeLabel(g))
                            if context.with_graph_name =>
                        {
                            let graph_name = self.output_builder.blank_graph(g);
                            self.emit_quad(results, graph_name);
                            self.state = NQuadsState::ExpectDot;
                        }
                        _ => {
                            let graph_name = self.output_builder.default_graph();
                            self.emit_quad(results, graph_name);
                            self.state = NQuadsState::ExpectDot;
                            self.recognize_next(token, context, results, errors)
                        }
                    }
                }
            }
            NQuadsState::ExpectDot => match token {
                TokenOrLineJump::Token(token) => {
                    if let N3Token::Punctuation(".") = token {
                        self.state = NQuadsState::ExpectLineJump;
                    } else {
                        errors.push("Quads must be followed by a dot".into());
                        self.state = NQuadsState::ExpectSubject;
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TokenOrLineJump::LineJump => {
                    self.error(
                        context,
                        results,
                        errors,
                        token,
                        "Quads must be followed by a dot",
                    );
                    self.recognize_next(TokenOrLineJump::LineJump, context, results, errors);
                }
            },
            NQuadsState::ExpectLineJump => match token {
                TokenOrLineJump::Token(token) => {
                    errors.push(
                        format!(
                            "Only a single triple or quad can be written in a line, found {token:?}"
                        )
                        .into(),
                    );
                    if B::BUFFER_OUTPUT_UNTIL_LINE_END {
                        self.set_error_recovery_state();
                    } else {
                        self.state = NQuadsState::ExpectSubject;
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TokenOrLineJump::LineJump => {
                    self.publish_pending_output(results);
                    self.state = NQuadsState::ExpectSubject;
                }
            },
            #[cfg(feature = "rdf-12")]
            NQuadsState::AfterQuotedTriple => {
                let triple = self.output_builder.triple(
                    self.subject.take().unwrap(),
                    self.predicate.take().unwrap(),
                    self.object.take().unwrap(),
                );
                let frame = self.quoted_triples.pop().unwrap();
                self.subject = Some(frame.subject);
                self.predicate = Some(frame.predicate);
                self.object = Some(triple);
                self.state = NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple;
                self.recognize_next(token, context, results, errors)
            }
            NQuadsState::RecoverToLineJump => {
                self.state = if token == TokenOrLineJump::LineJump {
                    NQuadsState::ExpectSubject
                } else {
                    NQuadsState::RecoverToLineJump
                };
            }
        }
    }

    fn recognize_end(
        mut self,
        _context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        #[cfg(feature = "rdf-12")]
        if !self.quoted_triples.is_empty() {
            errors.push("Unexpected end".into()); // TODO
            return;
        }
        let state = std::mem::replace(&mut self.state, NQuadsState::RecoverToLineJump);
        match state {
            NQuadsState::ExpectSubject => {}
            NQuadsState::ExpectLineJump => self.publish_pending_output(results),
            NQuadsState::ExpectDot => errors.push("Triples must be followed by a dot".into()),
            NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple => {
                let graph_name = self.output_builder.default_graph();
                self.emit_quad(results, graph_name);
                errors.push("Triples must be followed by a dot".into())
            }
            NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value } => {
                self.object = Some(self.output_builder.simple_literal(value));
                let graph_name = self.output_builder.default_graph();
                self.emit_quad(results, graph_name);
                errors.push("Triples must be followed by a dot".into())
            }
            _ => errors.push("Unexpected end".into()), // TODO
        }
    }

    fn lexer_options(context: &NQuadsRecognizerContext) -> &N3LexerOptions {
        &context.lexer_options
    }

    fn reuse_output(&mut self, output: Self::Output) {
        self.output_builder.reuse_output(output);
    }
}

impl NQuadsRecognizer {
    pub fn new_parser<B>(
        data: B,
        is_ending: bool,
        with_graph_name: bool,
        lenient: bool,
        max_buffer_size: usize,
    ) -> Parser<B, Self> {
        Self::new_parser_with_builder(
            data,
            is_ending,
            with_graph_name,
            lenient,
            max_buffer_size,
            OwnedNQuadsOutputBuilder,
        )
    }
}

impl BorrowedNQuadsRecognizer {
    pub(crate) fn new_borrowed_parser<B>(
        data: B,
        is_ending: bool,
        with_graph_name: bool,
        lenient: bool,
        max_buffer_size: usize,
    ) -> Parser<B, Self> {
        Self::new_parser_with_builder(
            data,
            is_ending,
            with_graph_name,
            lenient,
            max_buffer_size,
            BorrowedNQuadsOutputBuilder::default(),
        )
    }
}

impl<B: NQuadsOutputBuilder> NQuadsRecognizer<B> {
    fn new_parser_with_builder<D>(
        data: D,
        is_ending: bool,
        with_graph_name: bool,
        lenient: bool,
        max_buffer_size: usize,
        output_builder: B,
    ) -> Parser<D, Self> {
        Parser::new(
            Lexer::new(
                N3Lexer::new(N3LexerMode::NTriples, lenient),
                data,
                is_ending,
                MIN_BUFFER_SIZE,
                max_buffer_size,
                Some(b"#"),
            ),
            Self {
                state: NQuadsState::ExpectSubject,
                #[cfg(feature = "rdf-12")]
                quoted_triples: Vec::new(),
                subject: None,
                predicate: None,
                object: None,
                pending_graph_name: None,
                output_builder,
                lenient,
            },
            NQuadsRecognizerContext {
                with_graph_name,
                lexer_options: N3LexerOptions::default(),
            },
        )
    }

    fn error(
        &mut self,
        context: &mut NQuadsRecognizerContext,
        results: &mut Vec<B::Output>,
        errors: &mut Vec<RuleRecognizerError>,
        token: TokenOrLineJump<N3Token<'_>>,
        msg: impl Into<RuleRecognizerError>,
    ) {
        errors.push(msg.into());
        self.set_error_recovery_state();
        match token {
            TokenOrLineJump::Token(_) => (),
            TokenOrLineJump::LineJump => self.recognize_next(token, context, results, errors), /* We immediately recover */
        }
    }

    fn emit_quad(&mut self, results: &mut Vec<B::Output>, graph_name: B::GraphName) {
        if B::BUFFER_OUTPUT_UNTIL_LINE_END {
            debug_assert!(
                self.pending_graph_name.is_none(),
                "a line parser must publish or discard its previous output before emitting again"
            );
            self.pending_graph_name = Some(graph_name);
        } else {
            results.push(self.output_builder.quad(
                self.subject.take().unwrap(),
                self.predicate.take().unwrap(),
                self.object.take().unwrap(),
                graph_name,
            ));
        }
    }

    fn publish_pending_output(&mut self, results: &mut Vec<B::Output>) {
        if let Some(graph_name) = self.pending_graph_name.take() {
            results.push(self.output_builder.quad(
                self.subject.take().unwrap(),
                self.predicate.take().unwrap(),
                self.object.take().unwrap(),
                graph_name,
            ));
        }
    }
}
