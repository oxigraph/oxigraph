//! Shared parser implementation for N-Triples and N-Quads.

use crate::lexer::{N3Lexer, N3LexerMode, N3LexerOptions, N3Token};
use crate::toolkit::{Lexer, Parser, RuleRecognizer, RuleRecognizerError};
use crate::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE};
#[cfg(feature = "rdf-star")]
use oxrdf::Triple;
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, Quad, Subject, Term};

pub struct NQuadsRecognizer {
    stack: Vec<NQuadsState>,
    subjects: Vec<Subject>,
    predicates: Vec<NamedNode>,
    objects: Vec<Term>,
}
pub struct NQuadsRecognizerContext {
    with_graph_name: bool,
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
    lexer_options: N3LexerOptions,
}

enum NQuadsState {
    ExpectSubject,
    ExpectPredicate,
    ExpectedObject,
    ExpectPossibleGraphOrEndOfQuotedTriple,
    ExpectDot,
    ExpectLiteralAnnotationOrGraphNameOrDot {
        value: String,
    },
    ExpectLiteralDatatype {
        value: String,
    },
    #[cfg(feature = "rdf-star")]
    AfterQuotedSubject,
    #[cfg(feature = "rdf-star")]
    AfterQuotedObject,
}

impl RuleRecognizer for NQuadsRecognizer {
    type TokenRecognizer = N3Lexer;
    type Output = Quad;
    type Context = NQuadsRecognizerContext;

    fn error_recovery_state(mut self) -> Self {
        self.stack.clear();
        self.subjects.clear();
        self.predicates.clear();
        self.objects.clear();
        self
    }

    fn recognize_next(
        mut self,
        token: N3Token<'_>,
        context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self {
        if let Some(state) = self.stack.pop() {
            match state {
                NQuadsState::ExpectSubject => match token {
                    N3Token::IriRef(s) => {
                        self.subjects
                            .push(NamedNode::new_unchecked(s).into());
                        self.stack.push(NQuadsState::ExpectPredicate);
                        self
                    }
                    N3Token::BlankNodeLabel(s) => {
                        self.subjects.push(BlankNode::new_unchecked(s).into());
                        self.stack.push(NQuadsState::ExpectPredicate);
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("<<") if context.with_quoted_triples => {
                        self.stack.push(NQuadsState::AfterQuotedSubject);
                        self.stack.push(NQuadsState::ExpectSubject);
                        self
                    }
                    _ => self.error(
                        errors,
                        "The subject of a triple should be an IRI or a blank node, TOKEN found",
                    ),
                },
                NQuadsState::ExpectPredicate => match token {
                    N3Token::IriRef(p) => {
                        self.predicates
                            .push(NamedNode::new_unchecked(p));
                        self.stack.push(NQuadsState::ExpectedObject);
                        self
                    }
                    _ => self.error(
                        errors,
                        "The predicate of a triple should be an IRI, TOKEN found",
                    ),
                },
                NQuadsState::ExpectedObject => match token {
                    N3Token::IriRef(o) => {
                        self.objects
                            .push(NamedNode::new_unchecked(o).into());
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self
                    }
                    N3Token::BlankNodeLabel(o) => {
                        self.objects.push(BlankNode::new_unchecked(o).into());
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self
                    }
                    N3Token::String(value) => {
                        self.stack
                            .push(NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value });
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("<<") if context.with_quoted_triples => {
                        self.stack.push(NQuadsState::AfterQuotedObject);
                        self.stack.push(NQuadsState::ExpectSubject);
                        self
                    }
                    _ => self.error(
                        errors,
                        "The object of a triple should be an IRI, a blank node or a literal, TOKEN found",
                    ),
                },
                NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value } => match token {
                    N3Token::LangTag(lang_tag) => {
                        self.objects.push(
                            Literal::new_language_tagged_literal_unchecked(
                                value,
                                lang_tag.to_ascii_lowercase(),
                            )
                            .into(),
                        );
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self
                    }
                    N3Token::Punctuation("^^") => {
                        self.stack
                            .push(NQuadsState::ExpectLiteralDatatype { value });
                        self
                    }
                    _ => {
                        self.objects.push(Literal::new_simple_literal(value).into());
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self.recognize_next(token, context, results, errors)
                    }
                },
                NQuadsState::ExpectLiteralDatatype { value } => match token {
                    N3Token::IriRef(d) => {
                        self.objects.push(
                            Literal::new_typed_literal(
                                value,
                                NamedNode::new_unchecked(d)
                            )
                            .into(),
                        );
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self
                    }
                    _ => self.error(errors, "A literal datatype must be an IRI, found TOKEN"),
                },
                NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple => {
                    if self.stack.is_empty() {
                        match token {
                            N3Token::IriRef(g) if context.with_graph_name => {
                                self.emit_quad(
                                    results,
                                    NamedNode::new_unchecked(g).into(),
                                );
                                self.stack.push(NQuadsState::ExpectDot);
                                self
                            }
                            N3Token::BlankNodeLabel(g) if context.with_graph_name => {
                                self.emit_quad(results, BlankNode::new_unchecked(g).into());
                                self.stack.push(NQuadsState::ExpectDot);
                                self
                            }
                            _ => {
                                self.emit_quad(results, GraphName::DefaultGraph);
                                self.stack.push(NQuadsState::ExpectDot);
                                self.recognize_next(token, context, results, errors)
                            }
                        }
                    } else if token == N3Token::Punctuation(">>") {
                        self
                    } else {
                        self.error(errors, "Expecting the end of a quoted triple '>>'")
                    }
                }
                NQuadsState::ExpectDot => if let N3Token::Punctuation(".") = token {
                    self.stack.push(NQuadsState::ExpectSubject);
                    self
                } else {
                    errors.push("Quads should be followed by a dot".into());
                    self.stack.push(NQuadsState::ExpectSubject);
                    self.recognize_next(token, context, results, errors)
                },
                #[cfg(feature = "rdf-star")]
                NQuadsState::AfterQuotedSubject => {
                    let triple = Triple {
                        subject: self.subjects.pop().unwrap(),
                        predicate: self.predicates.pop().unwrap(),
                        object: self.objects.pop().unwrap(),
                    };
                    self.subjects.push(triple.into());
                    self.stack.push(NQuadsState::ExpectPredicate);
                    self.recognize_next(token,context,  results, errors)
                }
                #[cfg(feature = "rdf-star")]
                NQuadsState::AfterQuotedObject => {
                    let triple = Triple {
                        subject: self.subjects.pop().unwrap(),
                        predicate: self.predicates.pop().unwrap(),
                        object: self.objects.pop().unwrap(),
                    };
                    self.objects.push(triple.into());
                    self.stack
                        .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                    self.recognize_next(token, context, results, errors)
                }
            }
        } else if token == N3Token::Punctuation(".") {
            self.stack.push(NQuadsState::ExpectSubject);
            self
        } else {
            self
        }
    }

    fn recognize_end(
        mut self,
        _context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        match &*self.stack {
            [NQuadsState::ExpectSubject] | [] => (),
            [NQuadsState::ExpectDot] => errors.push("Triples should be followed by a dot".into()),
            [NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple] => {
                self.emit_quad(results, GraphName::DefaultGraph);
                errors.push("Triples should be followed by a dot".into())
            }
            [NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value }] => {
                self.objects.push(Literal::new_simple_literal(value).into());
                self.emit_quad(results, GraphName::DefaultGraph);
                errors.push("Triples should be followed by a dot".into())
            }
            _ => errors.push("Unexpected end".into()), // TODO
        }
    }

    fn lexer_options(context: &NQuadsRecognizerContext) -> &N3LexerOptions {
        &context.lexer_options
    }
}

impl NQuadsRecognizer {
    pub fn new_parser(
        with_graph_name: bool,
        #[cfg(feature = "rdf-star")] with_quoted_triples: bool,
        unchecked: bool,
    ) -> Parser<Self> {
        Parser::new(
            Lexer::new(
                N3Lexer::new(N3LexerMode::NTriples, unchecked),
                MIN_BUFFER_SIZE,
                MAX_BUFFER_SIZE,
                true,
                Some(b"#"),
            ),
            Self {
                stack: vec![NQuadsState::ExpectSubject],
                subjects: Vec::new(),
                predicates: Vec::new(),
                objects: Vec::new(),
            },
            NQuadsRecognizerContext {
                with_graph_name,
                #[cfg(feature = "rdf-star")]
                with_quoted_triples,
                lexer_options: N3LexerOptions::default(),
            },
        )
    }

    #[must_use]
    fn error(
        mut self,
        errors: &mut Vec<RuleRecognizerError>,
        msg: impl Into<RuleRecognizerError>,
    ) -> Self {
        errors.push(msg.into());
        self.stack.clear();
        self.subjects.clear();
        self.predicates.clear();
        self.objects.clear();
        self
    }

    fn emit_quad(&mut self, results: &mut Vec<Quad>, graph_name: GraphName) {
        results.push(Quad {
            subject: self.subjects.pop().unwrap(),
            predicate: self.predicates.pop().unwrap(),
            object: self.objects.pop().unwrap(),
            graph_name,
        })
    }
}
