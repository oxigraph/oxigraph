//! Shared parser implementation for Turtle and TriG.

use crate::lexer::{resolve_local_name, N3Lexer, N3LexerMode, N3LexerOptions, N3Token};
use crate::toolkit::{Lexer, Parser, RuleRecognizer, RuleRecognizerError, TokenOrLineJump};
use crate::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE};
use oxiri::Iri;
use oxrdf::vocab::{rdf, xsd};
#[cfg(feature = "rdf-star")]
use oxrdf::Triple;
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Subject, Term};
use std::collections::hash_map::Iter;
use std::collections::HashMap;

pub struct TriGRecognizer {
    stack: Vec<TriGState>,
    cur_subject: Vec<Subject>,
    cur_predicate: Vec<NamedNode>,
    cur_object: Vec<Term>,
    cur_graph: GraphName,
}

#[allow(clippy::partial_pub_fields)]
pub struct TriGRecognizerContext {
    pub lexer_options: N3LexerOptions,
    pub with_graph_name: bool,
    #[cfg(feature = "rdf-star")]
    pub with_quoted_triples: bool,
    prefixes: HashMap<String, Iri<String>>,
}

impl TriGRecognizerContext {
    pub fn prefixes(&self) -> Iter<'_, String, Iri<String>> {
        self.prefixes.iter()
    }
}

impl RuleRecognizer for TriGRecognizer {
    type TokenRecognizer = N3Lexer;
    type Output = Quad;
    type Context = TriGRecognizerContext;

    fn error_recovery_state(mut self) -> Self {
        self.stack.clear();
        self.cur_subject.clear();
        self.cur_predicate.clear();
        self.cur_object.clear();
        self.cur_graph = GraphName::DefaultGraph;
        self
    }

    fn recognize_next(
        mut self,
        token: TokenOrLineJump<N3Token<'_>>,
        context: &mut TriGRecognizerContext,
        results: &mut Vec<Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self {
        let TokenOrLineJump::Token(token) = token else {
            return self;
        };
        if let Some(rule) = self.stack.pop() {
            match rule {
                // [1g] trigDoc      ::=  (directive | block)*
                // [2g] block        ::=  triplesOrGraph | wrappedGraph | triples2 | "GRAPH" labelOrSubject wrappedGraph
                // [3]  directive    ::=  prefixID | base | sparqlPrefix | sparqlBase
                // [4]  prefixID     ::=  '@prefix' PNAME_NS IRIREF '.'
                // [5]  base         ::=  '@base' IRIREF '.'
                // [5s] sparqlPrefix ::=  "PREFIX" PNAME_NS IRIREF
                // [6s] sparqlBase   ::=  "BASE" IRIREF
                TriGState::TriGDoc => {
                    self.cur_graph = GraphName::DefaultGraph;
                    self.stack.push(TriGState::TriGDoc);
                    match token {
                        N3Token::PlainKeyword(k) if k.eq_ignore_ascii_case("base") => {
                            self.stack.push(TriGState::BaseExpectIri);
                            self
                        }
                        N3Token::PlainKeyword(k) if k.eq_ignore_ascii_case("prefix") => {
                            self.stack.push(TriGState::PrefixExpectPrefix);
                            self
                        }
                        N3Token::LangTag("prefix") => {
                            self.stack.push(TriGState::ExpectDot);
                            self.stack.push(TriGState::PrefixExpectPrefix);
                            self
                        }
                        N3Token::LangTag("base") => {
                            self.stack.push(TriGState::ExpectDot);
                            self.stack.push(TriGState::BaseExpectIri);
                            self
                        }
                        N3Token::PlainKeyword(k)
                            if k.eq_ignore_ascii_case("graph") && context.with_graph_name =>
                        {
                            self.stack.push(TriGState::WrappedGraph);
                            self.stack.push(TriGState::GraphName);
                            self
                        }
                        N3Token::Punctuation("{") if context.with_graph_name => {
                            self.stack.push(TriGState::WrappedGraph);
                            self.recognize_next(
                                TokenOrLineJump::Token(token),
                                context,
                                results,
                                errors,
                            )
                        }
                        _ => {
                            self.stack.push(TriGState::TriplesOrGraph);
                            self.recognize_next(
                                TokenOrLineJump::Token(token),
                                context,
                                results,
                                errors,
                            )
                        }
                    }
                }
                TriGState::ExpectDot => {
                    self.cur_subject.pop();
                    if token == N3Token::Punctuation(".") {
                        self
                    } else {
                        errors.push("A dot is expected at the end of statements".into());
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::BaseExpectIri => {
                    if let N3Token::IriRef(iri) = token {
                        context.lexer_options.base_iri = Some(Iri::parse_unchecked(iri));
                        self
                    } else {
                        self.error(errors, "The BASE keyword should be followed by an IRI")
                    }
                }
                TriGState::PrefixExpectPrefix => match token {
                    N3Token::PrefixedName { prefix, local, .. } if local.is_empty() => {
                        self.stack.push(TriGState::PrefixExpectIri {
                            name: prefix.to_owned(),
                        });
                        self
                    }
                    _ => self.error(
                        errors,
                        "The PREFIX keyword should be followed by a prefix like 'ex:'",
                    ),
                },
                TriGState::PrefixExpectIri { name } => {
                    if let N3Token::IriRef(iri) = token {
                        context.prefixes.insert(name, Iri::parse_unchecked(iri));
                        self
                    } else {
                        self.error(errors, "The PREFIX declaration should be followed by a prefix and its value as an IRI")
                    }
                }
                // [3g]  triplesOrGraph  ::=  labelOrSubject ( wrappedGraph | predicateObjectList '.' ) | quotedTriple predicateObjectList '.'
                // [4g]  triples2        ::=  blankNodePropertyList predicateObjectList? '.' | collection predicateObjectList '.'
                TriGState::TriplesOrGraph => match token {
                    N3Token::IriRef(iri) => {
                        self.stack
                            .push(TriGState::WrappedGraphOrPredicateObjectList {
                                term: NamedNode::new_unchecked(iri).into(),
                            });
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.stack
                                .push(TriGState::WrappedGraphOrPredicateObjectList {
                                    term: t.into(),
                                });
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.stack
                            .push(TriGState::WrappedGraphOrPredicateObjectList {
                                term: BlankNode::new_unchecked(label).into(),
                            });
                        self
                    }
                    N3Token::Punctuation("[") => {
                        self.stack
                            .push(TriGState::WrappedGraphBlankNodePropertyListCurrent);
                        self
                    }
                    N3Token::Punctuation("(") => {
                        self.stack.push(TriGState::ExpectDot);
                        self.stack.push(TriGState::PredicateObjectList);
                        self.stack.push(TriGState::SubjectCollectionBeginning);
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("<<") if context.with_quoted_triples => {
                        self.stack.push(TriGState::ExpectDot);
                        self.stack.push(TriGState::PredicateObjectList);
                        self.stack.push(TriGState::SubjectQuotedTripleEnd);
                        self.stack.push(TriGState::QuotedObject);
                        self.stack.push(TriGState::Verb);
                        self.stack.push(TriGState::QuotedSubject);
                        self
                    }
                    _ => self.error(errors, "TOKEN is not a valid subject or graph name"),
                },
                TriGState::WrappedGraphOrPredicateObjectList { term } => {
                    if token == N3Token::Punctuation("{") && context.with_graph_name {
                        self.cur_graph = term.into();
                        self.stack.push(TriGState::WrappedGraph);
                    } else {
                        self.cur_subject.push(term.into());
                        self.stack.push(TriGState::ExpectDot);
                        self.stack.push(TriGState::PredicateObjectList);
                    }
                    self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                }
                TriGState::WrappedGraphBlankNodePropertyListCurrent => {
                    if token == N3Token::Punctuation("]") {
                        self.stack
                            .push(TriGState::WrappedGraphOrPredicateObjectList {
                                term: BlankNode::default().into(),
                            });
                        self
                    } else {
                        self.cur_subject.push(BlankNode::default().into());
                        self.stack.push(TriGState::ExpectDot);
                        self.stack.push(TriGState::SubjectBlankNodePropertyListEnd);
                        self.stack.push(TriGState::PredicateObjectList);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::SubjectBlankNodePropertyListEnd => {
                    if token == N3Token::Punctuation("]") {
                        self.stack
                            .push(TriGState::SubjectBlankNodePropertyListAfter);
                        self
                    } else {
                        errors.push("blank node property lists should end with a ']'".into());
                        self.stack
                            .push(TriGState::SubjectBlankNodePropertyListAfter);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::SubjectBlankNodePropertyListAfter => {
                    if matches!(token, N3Token::Punctuation("." | "}")) {
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    } else {
                        self.stack.push(TriGState::PredicateObjectList);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::SubjectCollectionBeginning => {
                    if let N3Token::Punctuation(")") = token {
                        self.cur_subject.push(rdf::NIL.into());
                        self
                    } else {
                        let root = BlankNode::default();
                        self.cur_subject.push(root.clone().into());
                        self.cur_subject.push(root.into());
                        self.cur_predicate.push(rdf::FIRST.into());
                        self.stack.push(TriGState::SubjectCollectionPossibleEnd);
                        self.stack.push(TriGState::Object);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::SubjectCollectionPossibleEnd => {
                    let old = self.cur_subject.pop().unwrap();
                    self.cur_object.pop();
                    if let N3Token::Punctuation(")") = token {
                        self.cur_predicate.pop();
                        results.push(Quad::new(old, rdf::REST, rdf::NIL, self.cur_graph.clone()));
                        self
                    } else {
                        let new = BlankNode::default();
                        results.push(Quad::new(
                            old,
                            rdf::REST,
                            new.clone(),
                            self.cur_graph.clone(),
                        ));
                        self.cur_subject.push(new.into());
                        self.stack.push(TriGState::ObjectCollectionPossibleEnd);
                        self.stack.push(TriGState::Object);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                // [5g]  wrappedGraph  ::=  '{' triplesBlock? '}'
                // [6g]  triplesBlock  ::=  triples ('.' triplesBlock?)?
                TriGState::WrappedGraph => {
                    if token == N3Token::Punctuation("{") {
                        self.stack.push(TriGState::WrappedGraphPossibleEnd);
                        self.stack.push(TriGState::Triples);
                        self
                    } else {
                        self.error(errors, "The GRAPH keyword should be followed by a graph name and a value in '{'")
                    }
                }
                TriGState::WrappedGraphPossibleEnd => {
                    self.cur_subject.pop();
                    match token {
                        N3Token::Punctuation("}") => self,
                        N3Token::Punctuation(".") => {
                            self.stack.push(TriGState::WrappedGraphPossibleEnd);
                            self.stack.push(TriGState::Triples);
                            self
                        }
                        _ => {
                            errors.push(
                                "A '}' or a '.' is expected at the end of a graph block".into(),
                            );
                            self.recognize_next(
                                TokenOrLineJump::Token(token),
                                context,
                                results,
                                errors,
                            )
                        }
                    }
                }
                // [6]  triples   ::=  subject predicateObjectList | blankNodePropertyList predicateObjectList?
                // [10]  subject  ::=  iri | BlankNode | collection | quotedTriple
                TriGState::Triples => match token {
                    N3Token::Punctuation("}") => {
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                        // Early end
                    }
                    N3Token::Punctuation("[") => {
                        self.cur_subject.push(BlankNode::default().into());
                        self.stack
                            .push(TriGState::TriplesBlankNodePropertyListCurrent);
                        self
                    }
                    N3Token::IriRef(iri) => {
                        self.cur_subject.push(NamedNode::new_unchecked(iri).into());
                        self.stack.push(TriGState::PredicateObjectList);
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_subject.push(t.into());
                            self.stack.push(TriGState::PredicateObjectList);
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.cur_subject
                            .push(BlankNode::new_unchecked(label).into());
                        self.stack.push(TriGState::PredicateObjectList);
                        self
                    }
                    N3Token::Punctuation("(") => {
                        self.stack.push(TriGState::PredicateObjectList);
                        self.stack.push(TriGState::SubjectCollectionBeginning);
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("<<") if context.with_quoted_triples => {
                        self.stack.push(TriGState::PredicateObjectList);
                        self.stack.push(TriGState::SubjectQuotedTripleEnd);
                        self.stack.push(TriGState::QuotedObject);
                        self.stack.push(TriGState::Verb);
                        self.stack.push(TriGState::QuotedSubject);
                        self
                    }
                    _ => self.error(errors, "TOKEN is not a valid RDF subject"),
                },
                TriGState::TriplesBlankNodePropertyListCurrent => {
                    if token == N3Token::Punctuation("]") {
                        self.stack.push(TriGState::PredicateObjectList);
                        self
                    } else {
                        self.stack.push(TriGState::SubjectBlankNodePropertyListEnd);
                        self.stack.push(TriGState::PredicateObjectList);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                // [7g]  labelOrSubject  ::=  iri | BlankNode
                TriGState::GraphName => match token {
                    N3Token::IriRef(iri) => {
                        self.cur_graph = NamedNode::new_unchecked(iri).into();
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_graph = t.into();
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.cur_graph = BlankNode::new_unchecked(label).into();
                        self
                    }
                    N3Token::Punctuation("[") => {
                        self.stack.push(TriGState::GraphNameAnonEnd);
                        self
                    }
                    _ => self.error(errors, "TOKEN is not a valid graph name"),
                },
                TriGState::GraphNameAnonEnd => {
                    if token == N3Token::Punctuation("]") {
                        self.cur_graph = BlankNode::default().into();
                        self
                    } else {
                        self.error(errors, "Anonymous blank node with a property list are not allowed as graph name")
                    }
                }
                // [7]  predicateObjectList  ::=  verb objectList (';' (verb objectList)?)*
                TriGState::PredicateObjectList => {
                    self.stack.push(TriGState::PredicateObjectListEnd);
                    self.stack.push(TriGState::ObjectsList);
                    self.stack.push(TriGState::Verb);
                    self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                }
                TriGState::PredicateObjectListEnd => {
                    self.cur_predicate.pop();
                    if token == N3Token::Punctuation(";") {
                        self.stack
                            .push(TriGState::PredicateObjectListPossibleContinuation);
                        self
                    } else {
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::PredicateObjectListPossibleContinuation => {
                    if token == N3Token::Punctuation(";") {
                        self.stack
                            .push(TriGState::PredicateObjectListPossibleContinuation);
                        self
                    } else if matches!(token, N3Token::Punctuation("." | "}" | "]")) {
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    } else {
                        self.stack.push(TriGState::PredicateObjectListEnd);
                        self.stack.push(TriGState::ObjectsList);
                        self.stack.push(TriGState::Verb);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                // [8]   objectList  ::=  object annotation? ( ',' object annotation? )*
                // [30t] annotation  ::=  '{|' predicateObjectList '|}'
                TriGState::ObjectsList => {
                    self.stack.push(TriGState::ObjectsListEnd);
                    self.stack.push(TriGState::Object);
                    self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                }
                TriGState::ObjectsListEnd => match token {
                    N3Token::Punctuation(",") => {
                        self.cur_object.pop();
                        self.stack.push(TriGState::ObjectsListEnd);
                        self.stack.push(TriGState::Object);
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("{|") => {
                        let triple = Triple::new(
                            self.cur_subject.last().unwrap().clone(),
                            self.cur_predicate.last().unwrap().clone(),
                            self.cur_object.pop().unwrap(),
                        );
                        self.cur_subject.push(triple.into());
                        self.stack.push(TriGState::AnnotationEnd);
                        self.stack.push(TriGState::PredicateObjectList);
                        self
                    }
                    _ => {
                        self.cur_object.pop();
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                },
                #[cfg(feature = "rdf-star")]
                TriGState::AnnotationEnd => {
                    self.cur_subject.pop();
                    self.stack.push(TriGState::ObjectsListAfterAnnotation);
                    if token == N3Token::Punctuation("|}") {
                        self
                    } else {
                        self.error(errors, "Annotations should end with '|}'")
                    }
                }
                #[cfg(feature = "rdf-star")]
                TriGState::ObjectsListAfterAnnotation => {
                    if token == N3Token::Punctuation(",") {
                        self.stack.push(TriGState::ObjectsListEnd);
                        self.stack.push(TriGState::Object);
                        self
                    } else {
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                // [9]   verb       ::=  predicate | 'a'
                // [11]  predicate  ::=  iri
                TriGState::Verb => match token {
                    N3Token::PlainKeyword("a") => {
                        self.cur_predicate.push(rdf::TYPE.into());
                        self
                    }
                    N3Token::IriRef(iri) => {
                        self.cur_predicate.push(NamedNode::new_unchecked(iri));
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_predicate.push(t);
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    _ => self.error(errors, "TOKEN is not a valid predicate"),
                },
                // [12]    object                 ::=  iri | BlankNode | collection | blankNodePropertyList | literal | quotedTriple
                // [13]    literal                ::=  RDFLiteral | NumericLiteral | BooleanLiteral
                // [14]    blank                  ::=  BlankNode | collection
                // [15]    blankNodePropertyList  ::=  '[' predicateObjectList ']'
                // [16]    collection             ::=  '(' object* ')'
                // [17]    NumericLiteral         ::=  INTEGER | DECIMAL | DOUBLE
                // [128s]  RDFLiteral             ::=  String (LANGTAG | '^^' iri)?
                // [133s]  BooleanLiteral         ::=  'true' | 'false'
                // [18]    String                 ::=  STRING_LITERAL_QUOTE | STRING_LITERAL_SINGLE_QUOTE | STRING_LITERAL_LONG_SINGLE_QUOTE | STRING_LITERAL_LONG_QUOTE
                // [135s]  iri                    ::=  IRIREF | PrefixedName
                // [136s]  PrefixedName           ::=  PNAME_LN | PNAME_NS
                // [137s]  BlankNode              ::=  BLANK_NODE_LABEL | ANON
                TriGState::Object => match token {
                    N3Token::IriRef(iri) => {
                        self.cur_object.push(NamedNode::new_unchecked(iri).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_object.push(t.into());
                            self.emit_quad(results);
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.cur_object.push(BlankNode::new_unchecked(label).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::Punctuation("[") => {
                        self.stack
                            .push(TriGState::ObjectBlankNodePropertyListCurrent);
                        self
                    }
                    N3Token::Punctuation("(") => {
                        self.stack.push(TriGState::ObjectCollectionBeginning);
                        self
                    }
                    N3Token::String(value) => {
                        self.stack
                            .push(TriGState::LiteralPossibleSuffix { value, emit: true });
                        self
                    }
                    N3Token::Integer(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::INTEGER).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::Decimal(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::DECIMAL).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::Double(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::DOUBLE).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::PlainKeyword("true") => {
                        self.cur_object
                            .push(Literal::new_typed_literal("true", xsd::BOOLEAN).into());
                        self.emit_quad(results);
                        self
                    }
                    N3Token::PlainKeyword("false") => {
                        self.cur_object
                            .push(Literal::new_typed_literal("false", xsd::BOOLEAN).into());
                        self.emit_quad(results);
                        self
                    }
                    #[cfg(feature = "rdf-star")]
                    N3Token::Punctuation("<<") if context.with_quoted_triples => {
                        self.stack
                            .push(TriGState::ObjectQuotedTripleEnd { emit: true });
                        self.stack.push(TriGState::QuotedObject);
                        self.stack.push(TriGState::Verb);
                        self.stack.push(TriGState::QuotedSubject);
                        self
                    }
                    _ => self.error(errors, "TOKEN is not a valid RDF object"),
                },
                TriGState::ObjectBlankNodePropertyListCurrent => {
                    if token == N3Token::Punctuation("]") {
                        self.cur_object.push(BlankNode::default().into());
                        self.emit_quad(results);
                        self
                    } else {
                        self.cur_subject.push(BlankNode::default().into());
                        self.stack.push(TriGState::ObjectBlankNodePropertyListEnd);
                        self.stack.push(TriGState::PredicateObjectList);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::ObjectBlankNodePropertyListEnd => {
                    if token == N3Token::Punctuation("]") {
                        self.cur_object.push(self.cur_subject.pop().unwrap().into());
                        self.emit_quad(results);
                        self
                    } else {
                        self.error(errors, "blank node property lists should end with a ']'")
                    }
                }
                TriGState::ObjectCollectionBeginning => {
                    if let N3Token::Punctuation(")") = token {
                        self.cur_object.push(rdf::NIL.into());
                        self.emit_quad(results);
                        self
                    } else {
                        let root = BlankNode::default();
                        self.cur_object.push(root.clone().into());
                        self.emit_quad(results);
                        self.cur_subject.push(root.into());
                        self.cur_predicate.push(rdf::FIRST.into());
                        self.stack.push(TriGState::ObjectCollectionPossibleEnd);
                        self.stack.push(TriGState::Object);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::ObjectCollectionPossibleEnd => {
                    let old = self.cur_subject.pop().unwrap();
                    self.cur_object.pop();
                    if let N3Token::Punctuation(")") = token {
                        self.cur_predicate.pop();
                        results.push(Quad::new(old, rdf::REST, rdf::NIL, self.cur_graph.clone()));
                        self
                    } else {
                        let new = BlankNode::default();
                        results.push(Quad::new(
                            old,
                            rdf::REST,
                            new.clone(),
                            self.cur_graph.clone(),
                        ));
                        self.cur_subject.push(new.into());
                        self.stack.push(TriGState::ObjectCollectionPossibleEnd);
                        self.stack.push(TriGState::Object);
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                }
                TriGState::LiteralPossibleSuffix { value, emit } => match token {
                    N3Token::LangTag(lang) => {
                        self.cur_object.push(
                            Literal::new_language_tagged_literal_unchecked(
                                value,
                                lang.to_ascii_lowercase(),
                            )
                            .into(),
                        );
                        if emit {
                            self.emit_quad(results);
                        }
                        self
                    }
                    N3Token::Punctuation("^^") => {
                        self.stack
                            .push(TriGState::LiteralExpectDatatype { value, emit });
                        self
                    }
                    _ => {
                        self.cur_object
                            .push(Literal::new_simple_literal(value).into());
                        if emit {
                            self.emit_quad(results);
                        }
                        self.recognize_next(TokenOrLineJump::Token(token), context, results, errors)
                    }
                },
                TriGState::LiteralExpectDatatype { value, emit } => match token {
                    N3Token::IriRef(datatype) => {
                        self.cur_object.push(
                            Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype))
                                .into(),
                        );
                        if emit {
                            self.emit_quad(results);
                        }
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_object
                                .push(Literal::new_typed_literal(value, t).into());
                            if emit {
                                self.emit_quad(results);
                            }
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    _ => self
                        .error(errors, "Expecting a datatype IRI after ^^, found TOKEN")
                        .recognize_next(TokenOrLineJump::Token(token), context, results, errors),
                },
                // [27t]  quotedTriple  ::=  '<<' qtSubject verb qtObject '>>'
                #[cfg(feature = "rdf-star")]
                TriGState::SubjectQuotedTripleEnd => {
                    let triple = Triple::new(
                        self.cur_subject.pop().unwrap(),
                        self.cur_predicate.pop().unwrap(),
                        self.cur_object.pop().unwrap(),
                    );
                    self.cur_subject.push(triple.into());
                    if token == N3Token::Punctuation(">>") {
                        self
                    } else {
                        self.error(
                            errors,
                            "Expecting '>>' to close a quoted triple, found TOKEN",
                        )
                    }
                }
                #[cfg(feature = "rdf-star")]
                TriGState::ObjectQuotedTripleEnd { emit } => {
                    let triple = Triple::new(
                        self.cur_subject.pop().unwrap(),
                        self.cur_predicate.pop().unwrap(),
                        self.cur_object.pop().unwrap(),
                    );
                    self.cur_object.push(triple.into());
                    if emit {
                        self.emit_quad(results);
                    }
                    if token == N3Token::Punctuation(">>") {
                        self
                    } else {
                        self.error(
                            errors,
                            "Expecting '>>' to close a quoted triple, found TOKEN",
                        )
                    }
                }
                // [28t]  qtSubject  ::=  iri | BlankNode | quotedTriple
                #[cfg(feature = "rdf-star")]
                TriGState::QuotedSubject => match token {
                    N3Token::Punctuation("[") => {
                        self.cur_subject.push(BlankNode::default().into());
                        self.stack.push(TriGState::QuotedAnonEnd);
                        self
                    }
                    N3Token::IriRef(iri) => {
                        self.cur_subject.push(NamedNode::new_unchecked(iri).into());
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_subject.push(t.into());
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.cur_subject
                            .push(BlankNode::new_unchecked(label).into());
                        self
                    }
                    N3Token::Punctuation("<<") => {
                        self.stack.push(TriGState::SubjectQuotedTripleEnd);
                        self.stack.push(TriGState::QuotedObject);
                        self.stack.push(TriGState::Verb);
                        self.stack.push(TriGState::QuotedSubject);
                        self
                    }
                    _ => self.error(
                        errors,
                        "TOKEN is not a valid RDF quoted triple subject: TOKEN",
                    ),
                },
                // [29t]  qtObject  ::=  iri | BlankNode | literal | quotedTriple
                #[cfg(feature = "rdf-star")]
                TriGState::QuotedObject => match token {
                    N3Token::Punctuation("[") => {
                        self.cur_object.push(BlankNode::default().into());
                        self.stack.push(TriGState::QuotedAnonEnd);
                        self
                    }
                    N3Token::IriRef(iri) => {
                        self.cur_object.push(NamedNode::new_unchecked(iri).into());
                        self
                    }
                    N3Token::PrefixedName {
                        prefix,
                        local,
                        might_be_invalid_iri,
                    } => match resolve_local_name(
                        prefix,
                        &local,
                        might_be_invalid_iri,
                        &context.prefixes,
                    ) {
                        Ok(t) => {
                            self.cur_object.push(t.into());
                            self
                        }
                        Err(e) => self.error(errors, e),
                    },
                    N3Token::BlankNodeLabel(label) => {
                        self.cur_object.push(BlankNode::new_unchecked(label).into());
                        self
                    }
                    N3Token::String(value) => {
                        self.stack
                            .push(TriGState::LiteralPossibleSuffix { value, emit: false });
                        self
                    }
                    N3Token::Integer(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::INTEGER).into());
                        self
                    }
                    N3Token::Decimal(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::DECIMAL).into());
                        self
                    }
                    N3Token::Double(v) => {
                        self.cur_object
                            .push(Literal::new_typed_literal(v, xsd::DOUBLE).into());
                        self
                    }
                    N3Token::PlainKeyword("true") => {
                        self.cur_object
                            .push(Literal::new_typed_literal("true", xsd::BOOLEAN).into());
                        self
                    }
                    N3Token::PlainKeyword("false") => {
                        self.cur_object
                            .push(Literal::new_typed_literal("false", xsd::BOOLEAN).into());
                        self
                    }
                    N3Token::Punctuation("<<") => {
                        self.stack
                            .push(TriGState::ObjectQuotedTripleEnd { emit: false });
                        self.stack.push(TriGState::QuotedObject);
                        self.stack.push(TriGState::Verb);
                        self.stack.push(TriGState::QuotedSubject);
                        self
                    }
                    _ => self.error(errors, "TOKEN is not a valid RDF quoted triple object"),
                },
                #[cfg(feature = "rdf-star")]
                TriGState::QuotedAnonEnd => {
                    if token == N3Token::Punctuation("]") {
                        self
                    } else {
                        self.error(errors, "Anonymous blank node with a property list are not allowed in quoted triples")
                    }
                }
            }
        } else if token == N3Token::Punctuation(".") || token == N3Token::Punctuation("}") {
            // TODO: be smarter depending if we are in '{' or not
            self.stack.push(TriGState::TriGDoc);
            self
        } else {
            self
        }
    }

    fn recognize_end(
        mut self,
        _context: &mut TriGRecognizerContext,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        match &*self.stack {
            [] | [TriGState::TriGDoc] => {
                debug_assert!(
                    self.cur_subject.is_empty(),
                    "The cur_subject stack must be empty if the state stack is empty"
                );
                debug_assert!(
                    self.cur_predicate.is_empty(),
                    "The cur_predicate stack must be empty if the state stack is empty"
                );
                debug_assert!(
                    self.cur_object.is_empty(),
                    "The cur_object stack must be empty if the state stack is empty"
                );
            }
            [.., TriGState::LiteralPossibleSuffix { value, emit: true }] => {
                self.cur_object
                    .push(Literal::new_simple_literal(value).into());
                self.emit_quad(results);
                errors.push("Triples should be followed by a dot".into())
            }
            _ => errors.push("Unexpected end".into()), // TODO
        }
    }

    fn lexer_options(context: &TriGRecognizerContext) -> &N3LexerOptions {
        &context.lexer_options
    }
}

impl TriGRecognizer {
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn new_parser<B>(
        data: B,
        is_ending: bool,
        with_graph_name: bool,
        #[cfg(feature = "rdf-star")] with_quoted_triples: bool,
        unchecked: bool,
        base_iri: Option<Iri<String>>,
        prefixes: HashMap<String, Iri<String>>,
    ) -> Parser<B, Self> {
        Parser::new(
            Lexer::new(
                N3Lexer::new(N3LexerMode::Turtle, unchecked),
                data,
                is_ending,
                MIN_BUFFER_SIZE,
                MAX_BUFFER_SIZE,
                Some(b"#"),
            ),
            Self {
                stack: vec![TriGState::TriGDoc],
                cur_subject: Vec::new(),
                cur_predicate: Vec::new(),
                cur_object: Vec::new(),
                cur_graph: GraphName::DefaultGraph,
            },
            TriGRecognizerContext {
                with_graph_name,
                #[cfg(feature = "rdf-star")]
                with_quoted_triples,
                prefixes,
                lexer_options: N3LexerOptions { base_iri },
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
        self.cur_subject.clear();
        self.cur_predicate.clear();
        self.cur_object.clear();
        self.cur_graph = GraphName::DefaultGraph;
        self
    }

    fn emit_quad(&mut self, results: &mut Vec<Quad>) {
        results.push(Quad::new(
            self.cur_subject.last().unwrap().clone(),
            self.cur_predicate.last().unwrap().clone(),
            self.cur_object.last().unwrap().clone(),
            self.cur_graph.clone(),
        ));
    }
}

#[derive(Debug)]
enum TriGState {
    TriGDoc,
    ExpectDot,
    BaseExpectIri,
    PrefixExpectPrefix,
    PrefixExpectIri {
        name: String,
    },
    TriplesOrGraph,
    WrappedGraphBlankNodePropertyListCurrent,
    SubjectBlankNodePropertyListEnd,
    SubjectBlankNodePropertyListAfter,
    SubjectCollectionBeginning,
    SubjectCollectionPossibleEnd,
    WrappedGraphOrPredicateObjectList {
        term: NamedOrBlankNode,
    },
    WrappedGraph,
    WrappedGraphPossibleEnd,
    GraphName,
    GraphNameAnonEnd,
    Triples,
    TriplesBlankNodePropertyListCurrent,
    PredicateObjectList,
    PredicateObjectListEnd,
    PredicateObjectListPossibleContinuation,
    ObjectsList,
    ObjectsListEnd,
    #[cfg(feature = "rdf-star")]
    AnnotationEnd,
    #[cfg(feature = "rdf-star")]
    ObjectsListAfterAnnotation,
    Verb,
    Object,
    ObjectBlankNodePropertyListCurrent,
    ObjectBlankNodePropertyListEnd,
    ObjectCollectionBeginning,
    ObjectCollectionPossibleEnd,
    LiteralPossibleSuffix {
        value: String,
        emit: bool,
    },
    LiteralExpectDatatype {
        value: String,
        emit: bool,
    },
    #[cfg(feature = "rdf-star")]
    SubjectQuotedTripleEnd,
    #[cfg(feature = "rdf-star")]
    ObjectQuotedTripleEnd {
        emit: bool,
    },
    #[cfg(feature = "rdf-star")]
    QuotedSubject,
    #[cfg(feature = "rdf-star")]
    QuotedObject,
    #[cfg(feature = "rdf-star")]
    QuotedAnonEnd,
}
