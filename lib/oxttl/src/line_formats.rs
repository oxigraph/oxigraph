//! Shared parser implementation for N-Triples and N-Quads.

use crate::lexer::{N3Lexer, N3LexerMode, N3LexerOptions, N3Token};
use crate::toolkit::{Lexer, Parser, RuleRecognizer, RuleRecognizerError, TokenOrLineJump};
use crate::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE};
#[cfg(feature = "rdf-12")]
use oxrdf::Triple;
use oxrdf::vocab::rdf;
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term};

pub struct NQuadsRecognizer {
    stack: Vec<NQuadsState>,
    subjects: Vec<NamedOrBlankNode>,
    predicates: Vec<NamedNode>,
    objects: Vec<Term>,
    lenient: bool,
}

pub struct NQuadsRecognizerContext {
    with_graph_name: bool,
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
    ExpectLineJump,
    RecoverToLineJump,
    #[cfg(feature = "rdf-12")]
    AfterQuotedTriple,
}

impl RuleRecognizer for NQuadsRecognizer {
    type TokenRecognizer = N3Lexer;
    type Output = Quad;
    type Context = NQuadsRecognizerContext;

    fn error_recovery_state(mut self) -> Self {
        self.stack.clear();
        self.stack.push(NQuadsState::RecoverToLineJump);
        self.subjects.clear();
        self.predicates.clear();
        self.objects.clear();
        self
    }

    fn recognize_next(
        mut self,
        token: TokenOrLineJump<N3Token<'_>>,
        context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self {
        match self.stack.pop().unwrap_or(NQuadsState::ExpectSubject) {
            NQuadsState::ExpectSubject => {
                let TokenOrLineJump::Token(token) = token else {
                    return if self.stack.is_empty() {
                        self
                    } else {
                        self.error(
                            context,
                            results,
                            errors,
                            token,
                            "line jumps are not allowed inside of quoted triples",
                        )
                    };
                };
                match token {
                    N3Token::IriRef(s) => {
                        self.subjects.push(NamedNode::new_unchecked(s).into());
                        self.stack.push(NQuadsState::ExpectPredicate);
                        self
                    }
                    N3Token::BlankNodeLabel(s) => {
                        self.subjects.push(BlankNode::new_unchecked(s).into());
                        self.stack.push(NQuadsState::ExpectPredicate);
                        self
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The subject of a triple must be an IRI or a blank node",
                    ),
                }
            }
            NQuadsState::ExpectPredicate => {
                let TokenOrLineJump::Token(token) = token else {
                    return self.error(
                        context,
                        results,
                        errors,
                        token,
                        "line jumps are not allowed in the middle of triples",
                    );
                };
                match token {
                    N3Token::IriRef(p) => {
                        self.predicates.push(NamedNode::new_unchecked(p));
                        self.stack.push(NQuadsState::ExpectedObject);
                        self
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The predicate of a triple must be an IRI",
                    ),
                }
            }
            NQuadsState::ExpectedObject => {
                let TokenOrLineJump::Token(token) = token else {
                    return self.error(
                        context,
                        results,
                        errors,
                        token,
                        "line jumps are not allowed in the middle of triples",
                    );
                };
                match token {
                    N3Token::IriRef(o) => {
                        self.objects.push(NamedNode::new_unchecked(o).into());
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
                    #[cfg(feature = "rdf-12")]
                    N3Token::Punctuation("<<(") => {
                        self.stack.push(NQuadsState::AfterQuotedTriple);
                        self.stack.push(NQuadsState::ExpectSubject);
                        self
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "The object of a triple must be an IRI, a blank node or a literal",
                    ),
                }
            }
            NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value } => match token {
                #[cfg(feature = "rdf-12")]
                TokenOrLineJump::Token(N3Token::LangTag {
                    language,
                    direction,
                }) => {
                    self.objects.push(
                        if let Some(direction) = direction {
                            Literal::new_directional_language_tagged_literal_unchecked(
                                value,
                                language.to_ascii_lowercase(),
                                direction,
                            )
                        } else {
                            Literal::new_language_tagged_literal_unchecked(
                                value,
                                language.to_ascii_lowercase(),
                            )
                        }
                        .into(),
                    );
                    self.stack
                        .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                    self
                }
                #[cfg(not(feature = "rdf-12"))]
                TokenOrLineJump::Token(N3Token::LangTag { language }) => {
                    self.objects.push(
                        Literal::new_language_tagged_literal_unchecked(
                            value,
                            language.to_ascii_lowercase(),
                        )
                        .into(),
                    );
                    self.stack
                        .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                    self
                }
                TokenOrLineJump::Token(N3Token::Punctuation("^^")) => {
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
            NQuadsState::ExpectLiteralDatatype { value } => {
                let TokenOrLineJump::Token(token) = token else {
                    return self.error(
                        context,
                        results,
                        errors,
                        token,
                        "line jumps are not allowed in the middle of triples",
                    );
                };
                match token {
                    N3Token::IriRef(d) => {
                        if !self.lenient && d == rdf::LANG_STRING.as_str() {
                            errors.push("The datatype of a literal without a language tag must not be rdf:langString".into());
                        }
                        #[cfg(feature = "rdf-12")]
                        if !self.lenient && d == rdf::DIR_LANG_STRING.as_str() {
                            errors.push("The datatype of a literal without a base direction must not be rdf:dirLangString".into());
                        }
                        self.objects.push(
                            Literal::new_typed_literal(value, NamedNode::new_unchecked(d)).into(),
                        );
                        self.stack
                            .push(NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple);
                        self
                    }
                    _ => self.error(
                        context,
                        results,
                        errors,
                        TokenOrLineJump::Token(token),
                        "A literal datatype must be an IRI",
                    ),
                }
            }
            NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple => {
                if self.stack.is_empty() {
                    match token {
                        TokenOrLineJump::Token(N3Token::IriRef(g)) if context.with_graph_name => {
                            self.emit_quad(results, NamedNode::new_unchecked(g).into());
                            self.stack.push(NQuadsState::ExpectDot);
                            self
                        }
                        TokenOrLineJump::Token(N3Token::BlankNodeLabel(g))
                            if context.with_graph_name =>
                        {
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
                } else if token == TokenOrLineJump::Token(N3Token::Punctuation(")>>")) {
                    self
                } else {
                    self.error(
                        context,
                        results,
                        errors,
                        token,
                        "Expecting the end of a quoted triple ')>>'",
                    )
                }
            }
            NQuadsState::ExpectDot => {
                let TokenOrLineJump::Token(token) = token else {
                    return self
                        .error(
                            context,
                            results,
                            errors,
                            token,
                            "Quads must be followed by a dot",
                        )
                        .recognize_next(TokenOrLineJump::LineJump, context, results, errors);
                };
                if let N3Token::Punctuation(".") = token {
                    self.stack.push(NQuadsState::ExpectLineJump);
                    self
                } else {
                    errors.push("Quads must be followed by a dot".into());
                    self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                }
            }
            NQuadsState::ExpectLineJump => {
                let TokenOrLineJump::Token(token) = token else {
                    return self;
                };
                errors.push(
                    format!(
                        "Only a single triple or quad can be written in a line, found {token:?}"
                    )
                    .into(),
                );
                self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
            }
            #[cfg(feature = "rdf-12")]
            NQuadsState::AfterQuotedTriple => {
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
            NQuadsState::RecoverToLineJump => {
                if token != TokenOrLineJump::LineJump {
                    self.stack.push(NQuadsState::RecoverToLineJump);
                }
                self
            }
        }
    }

    fn recognize_end(
        mut self,
        _context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        match &*self.stack {
            [NQuadsState::ExpectSubject | NQuadsState::ExpectLineJump] | [] => (),
            [NQuadsState::ExpectDot] => errors.push("Triples must be followed by a dot".into()),
            [NQuadsState::ExpectPossibleGraphOrEndOfQuotedTriple] => {
                self.emit_quad(results, GraphName::DefaultGraph);
                errors.push("Triples must be followed by a dot".into())
            }
            [NQuadsState::ExpectLiteralAnnotationOrGraphNameOrDot { value }] => {
                self.objects.push(Literal::new_simple_literal(value).into());
                self.emit_quad(results, GraphName::DefaultGraph);
                errors.push("Triples must be followed by a dot".into())
            }
            _ => errors.push("Unexpected end".into()), // TODO
        }
    }

    fn lexer_options(context: &NQuadsRecognizerContext) -> &N3LexerOptions {
        &context.lexer_options
    }
}

impl NQuadsRecognizer {
    pub fn new_parser<B>(
        data: B,
        is_ending: bool,
        with_graph_name: bool,
        lenient: bool,
    ) -> Parser<B, Self> {
        Parser::new(
            Lexer::new(
                N3Lexer::new(N3LexerMode::NTriples, lenient),
                data,
                is_ending,
                MIN_BUFFER_SIZE,
                MAX_BUFFER_SIZE,
                Some(b"#"),
            ),
            Self {
                stack: vec![NQuadsState::ExpectSubject],
                subjects: Vec::new(),
                predicates: Vec::new(),
                objects: Vec::new(),
                lenient,
            },
            NQuadsRecognizerContext {
                with_graph_name,
                lexer_options: N3LexerOptions::default(),
            },
        )
    }

    #[must_use]
    fn error(
        self,
        context: &mut NQuadsRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
        token: TokenOrLineJump<N3Token<'_>>,
        msg: impl Into<RuleRecognizerError>,
    ) -> Self {
        errors.push(msg.into());
        let this = self.error_recovery_state();
        match token {
            TokenOrLineJump::Token(_) => this,
            TokenOrLineJump::LineJump => this.recognize_next(token, context, results, errors), /* We immediately recover */
        }
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
