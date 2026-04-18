#![allow(clippy::unused_unit)]

use crate::ast::*;
use crate::lexer::Token;
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::span::WrappingSpan;
use std::str::FromStr;

pub fn parse_sparql_query<'a>(
    tokens: &'a [Spanned<Token<'a>>],
    input_len: usize,
) -> Result<Query<'a>, Vec<Rich<'a, Token<'a>>>> {
    build_parsers()
        .0
        .parse(tokens.split_spanned((input_len..input_len).into()))
        .into_result()
}

pub fn parse_sparql_update<'a>(
    tokens: &'a [Spanned<Token<'a>>],
    input_len: usize,
) -> Result<Update<'a>, Vec<Rich<'a, Token<'a>>>> {
    build_parsers()
        .1
        .parse(tokens.split_spanned((input_len..input_len).into()))
        .into_result()
}

// TODO: remove when bumping Chumsky
macro_rules! select_with_attr {
    ($($(#[$attr:meta])? $p:pat $(= $extra:ident)? $(if $guard:expr)? $(=> $out:expr)?),+ $(,)?) => ({
        chumsky::primitive::select(
            move |x, extra| match (x, extra) {
                $($(#[$attr])? ($p $(,$extra)?, ..) $(if $guard)? => ::core::option::Option::Some({ () $(;$out)? })),+,
                _ => ::core::option::Option::None,
            }
        )
    });
}

macro_rules! select_keyword {
    ($($(#[$attr:meta])? $p:literal => $out:expr),+) => ({
        select_with_attr! {
            $($(#[$attr])? Token::Keyword(v) if v.eq_ignore_ascii_case($p) => $out),+
        }
    });
}

fn build_parsers<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>() -> (
    Boxed<'src, 'src, I, Query<'src>, extra::Err<Rich<'src, Token<'src>>>>,
    Boxed<'src, 'src, I, Update<'src>, extra::Err<Rich<'src, Token<'src>>>>,
) {
    let iriref = select! { Token::IriRef(i) => IriRef(&i[1..i.len() - 1]) }
        .spanned()
        .labelled("an iri");
    let pname_ns = select! { Token::PnameNs(p) => &p[..p.len() - 1] }.labelled("a prefix");
    let nil = operator("(").then_ignore(operator(")"));
    let anon = operator("[").then_ignore(operator("]"));

    // [158]   	BlankNode 	  ::=   	BLANK_NODE_LABEL | ANON
    let blank_node = select! {
        Token::BlankNodeLabel(id) => BlankNode(Some(&id[2..])),
    }
    .or(anon.to(BlankNode(None)))
    .spanned()
    .labelled("a blank node");

    // [157]   	PrefixedName 	  ::=   	PNAME_LN | PNAME_NS
    let prefixed_name = select! {
        Token::PnameNs(p) => PrefixedName(&p[..p.len() - 1], ""),
        Token::PnameLn(p) => {
            #[expect(clippy::expect_used)]
            let (p, v) = p.split_once(':').expect("prefixed name must contain ':'");
            PrefixedName(p, v)
        }
    }
    .spanned()
    .labelled("a prefixed name");

    // [156]   	iri 	  ::=   	IRIREF | PrefixedName
    let iri = iriref
        .map(Iri::IriRef)
        .or(prefixed_name.map(Iri::PrefixedName));

    // [155]   	String 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2 | STRING_LITERAL_LONG1 | STRING_LITERAL_LONG2
    let string = select! {
        Token::StringLiteral1(s) | Token::StringLiteral2(s) => String(&s[1..s.len() - 1]),
        Token::StringLiteralLong1(s) | Token::StringLiteralLong2(s) => String(&s[3..s.len() - 3]),
    }
    .labelled("a string literal")
    .spanned();

    // [154]   	BooleanLiteral 	  ::=   	'true' | 'false'
    let boolean_literal = case_sensitive_keyword("true")
        .to(Literal::Boolean(true))
        .or(case_sensitive_keyword("false").to(Literal::Boolean(false)));

    // [153]   	NumericLiteralNegative 	  ::=   	INTEGER_NEGATIVE | DECIMAL_NEGATIVE | DOUBLE_NEGATIVE
    // [152]   	NumericLiteralPositive 	  ::=   	INTEGER_POSITIVE | DECIMAL_POSITIVE | DOUBLE_POSITIVE
    // [151]   	NumericLiteralUnsigned 	  ::=   	INTEGER | DECIMAL | DOUBLE
    // [150]   	NumericLiteral 	  ::=   	NumericLiteralUnsigned | NumericLiteralPositive | NumericLiteralNegative
    let numeric_literal = select! {
        Token::Integer(v) | Token::IntegerPositive(v) | Token::IntegerNegative(v) => Literal::Integer(v),
        Token::Decimal(v) | Token::DecimalPositive(v) | Token::DecimalNegative(v) => Literal::Decimal(v),
        Token::Double(v) | Token::DoublePositive(v) | Token::DoubleNegative(v) => Literal::Double(v),
    }
        .labelled("a number");

    let numeric_literal_positive_or_negative = select! {
        Token::IntegerPositive(v) | Token::IntegerNegative(v) => Literal::Integer(v),
        Token::DecimalPositive(v) | Token::DecimalNegative(v) => Literal::Decimal(v),
        Token::DoublePositive(v) | Token::DoubleNegative(v) => Literal::Double(v),
    }
    .labelled("a number");

    // [149]   	RDFLiteral 	  ::=   	String ( LANG_DIR | '^^' iri )?
    let rdf_literal = string
        .then(
            select! {
                Token::LangDir(l) => Either::Left(&l[1..]),
            }
            .labelled("a language tag")
            .or(operator("^^").ignore_then(iri).map(Either::Right))
            .spanned()
            .or_not(),
        )
        .map(|(string, extra): (_, Option<Spanned<Either<&str, _>>>)| {
            let Some(extra) = extra else {
                return Literal::String(string);
            };
            match extra.inner {
                #[cfg(feature = "sparql-12")]
                Either::Left(l) => {
                    if let Some((l, d)) = l.split_once("--") {
                        Literal::DirLangString(string, extra.span.make_wrapped((l, d)))
                    } else {
                        Literal::LangString(string, extra.span.make_wrapped(l))
                    }
                }
                #[cfg(not(feature = "sparql-12"))]
                Either::Left(l) => Literal::LangString(string, extra.span.make_wrapped(l)),
                Either::Right(t) => Literal::Typed(string, t),
            }
        });

    let mut expression = Recursive::declare();

    // [77]   	ArgList 	  ::=   	NIL | '(' 'DISTINCT'? Expression ( ',' Expression )* ')'
    let arg_list = keyword("DISTINCT")
        .or_not()
        .then(
            expression
                .clone()
                .separated_by(operator(","))
                .collect::<Vec<_>>(),
        )
        .delimited_by(operator("("), operator(")"))
        .try_map(|(distinct, args), span| {
            if distinct.is_some() && args.is_empty() {
                return Err(Rich::custom(
                    span,
                    "DISTINCT cannot be used without arguments",
                ));
            }
            Ok(ArgList {
                distinct: distinct.is_some(),
                args,
            })
        });

    // [148]   	iriOrFunction 	  ::=   	iri ArgList?
    let iri_or_function = iri
        .then(arg_list.clone().or_not())
        .map(|(name, args)| {
            if let Some(args) = args {
                Expression::Function(name, args)
            } else {
                Expression::Iri(name)
            }
        })
        .spanned();

    // [147]   	Aggregate 	  ::=   	  'COUNT' '(' 'DISTINCT'? ( '*' | Expression ) ')' | 'SUM' '(' 'DISTINCT'? Expression ')' | 'MIN' '(' 'DISTINCT'? Expression ')' | 'MAX' '(' 'DISTINCT'? Expression ')' | 'AVG' '(' 'DISTINCT'? Expression ')' | 'SAMPLE' '(' 'DISTINCT'? Expression ')' | 'GROUP_CONCAT' '(' 'DISTINCT'? Expression ( ';' 'SEPARATOR' '=' String )? ')'
    let aggregate = keyword("COUNT")
        .ignore_then(
            keyword("DISTINCT")
                .or_not()
                .then_ignore(operator("*"))
                .delimited_by(operator("("), operator(")")),
        )
        .map(|distinct| Aggregate::Count(distinct.is_some(), None))
        .or(select_keyword! {
            "COUNT" => AggregateFunction::Count,
            "SUM" => AggregateFunction::Sum,
            "MIN" => AggregateFunction::Min,
            "MAX" => AggregateFunction::Max,
            "AVG" => AggregateFunction::Avg,
            "SAMPLE" => AggregateFunction::Sample
        }
        .labelled("a built-in aggregate function name")
        .then(
            keyword("DISTINCT")
                .or_not()
                .then(expression.clone())
                .delimited_by(operator("("), operator(")")),
        )
        .map(|(name, (distinct, expr))| match name {
            AggregateFunction::Count => Aggregate::Count(distinct.is_some(), Some(Box::new(expr))),
            AggregateFunction::Sum => Aggregate::Sum(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Min => Aggregate::Min(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Max => Aggregate::Max(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Avg => Aggregate::Avg(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Sample => Aggregate::Sample(distinct.is_some(), Box::new(expr)),
        }))
        .or(keyword("GROUP_CONCAT")
            .ignore_then(
                keyword("DISTINCT")
                    .or_not()
                    .then(expression.clone())
                    .then(
                        operator(";")
                            .ignore_then(keyword("SEPARATOR"))
                            .ignore_then(operator("="))
                            .ignore_then(string)
                            .or_not(),
                    )
                    .delimited_by(operator("("), operator(")")),
            )
            .map(|((distinct, expr), separator)| {
                Aggregate::GroupConcat(distinct.is_some(), Box::new(expr), separator)
            }));

    // [146]   	NotExistsFunc 	  ::=   	'NOT' 'EXISTS' GroupGraphPattern
    // [145]   	ExistsFunc 	  ::=   	'EXISTS' GroupGraphPattern
    let mut group_graph_pattern = Recursive::declare();
    let exists = keyword("NOT")
        .ignored()
        .or_not()
        .then_ignore(keyword("EXISTS"))
        .then(group_graph_pattern.clone())
        .map(|(neg, e)| {
            if neg.is_some() {
                Expression::NotExists(Box::new(e))
            } else {
                Expression::Exists(Box::new(e))
            }
        });

    // [78]   	ExpressionList 	  ::=   	NIL | '(' Expression ( ',' Expression )* ')'
    let expression_list = expression
        .clone()
        .separated_by(operator(","))
        .collect()
        .delimited_by(operator("("), operator(")"));

    // [126]   	Var 	  ::=   	VAR1 | VAR2
    let var = select! {
        Token::Var1(v) => Var(&v[1..]),
        Token::Var2(v) => Var(&v[1..]),
    }
    .labelled("a variable");

    // [144]   	StrReplaceExpression 	  ::=   	'REPLACE' '(' Expression ',' Expression ',' Expression ( ',' Expression )? ')'
    // [143]   	SubstringExpression 	  ::=   	'SUBSTR' '(' Expression ',' Expression ( ',' Expression )? ')'
    // [142]   	RegexExpression 	  ::=   	'REGEX' '(' Expression ',' Expression ( ',' Expression )? ')'
    // [141]   	BuiltInCall 	  ::=   	  Aggregate | 'STR' '(' Expression ')' | 'LANG' | 'LANGMATCHES' '(' Expression ',' Expression ')' | 'LANGDIR' '(' Expression ')' | 'DATATYPE' '(' Expression ')' | 'BOUND' '(' Var ')' | 'IRI' '(' Expression ')' | 'URI' '(' Expression ')' | 'BNODE' ( '(' Expression ')' | NIL ) | 'RAND' NIL | 'ABS' '(' Expression ')' | 'CEIL' '(' Expression ')' | 'FLOOR' '(' Expression ')' | 'ROUND' '(' Expression ')' | 'CONCAT' ExpressionList | SubstringExpression | 'STRLEN' '(' Expression ')' | StrReplaceExpression | 'UCASE' '(' Expression ')' | 'LCASE' '(' Expression ')' | 'ENCODE_FOR_URI' '(' Expression ')' | 'CONTAINS' '(' Expression ',' Expression ')' | 'STRSTARTS' '(' Expression ',' Expression ')' | 'STRENDS' '(' Expression ',' Expression ')' | 'STRBEFORE' '(' Expression ',' Expression ')' | 'STRAFTER' '(' Expression ',' Expression ')' | 'YEAR' '(' Expression ')' | 'MONTH' '(' Expression ')' | 'DAY' '(' Expression ')' | 'HOURS' '(' Expression ')' | 'MINUTES' '(' Expression ')' | 'SECONDS' '(' Expression ')' | 'TIMEZONE' '(' Expression ')' | 'TZ' '(' Expression ')' | 'NOW' NIL | 'UUID' NIL | 'STRUUID' NIL | 'MD5' '(' Expression ')' | 'SHA1' '(' Expression ')' | 'SHA256' '(' Expression ')' | 'SHA384' '(' Expression ')' | 'SHA512' '(' Expression ')' | 'COALESCE' ExpressionList | 'IF' '(' Expression ',' Expression ',' Expression ')' | 'STRLANG' '(' Expression ',' Expression ')' | 'STRLANGDIR' '(' Expression ',' Expression ',' Expression ')' | 'STRDT' '(' Expression ',' Expression ')' | 'sameTerm' '(' Expression ',' Expression ')' | 'isIRI' '(' Expression ')' | 'isURI' '(' Expression ')' | 'isBLANK' '(' Expression ')' | 'isLITERAL' '(' Expression ')' | 'isNUMERIC' '(' Expression ')' | 'hasLANG' '(' Expression ')' | 'hasLANGDIR' '(' Expression ')' | RegexExpression | ExistsFunc | NotExistsFunc | 'isTRIPLE' '(' Expression ')' | 'TRIPLE' '(' Expression ',' Expression ',' Expression ')' | 'SUBJECT' '(' Expression ')' | 'PREDICATE' '(' Expression ')' | 'OBJECT' '(' Expression ')'
    let built_in_call = aggregate
        .map(Expression::Aggregate)
        .or(keyword("BOUND")
            .ignore_then(var.delimited_by(operator("("), operator(")")))
            .map(Expression::Bound))
        .or(select_keyword! {
            "COALESCE" => BuiltInName::Coalesce,
            "IF" => BuiltInName::If,
            "sameTerm" => BuiltInName::SameTerm,
            "STR" => BuiltInName::Str,
            "LANG" => BuiltInName::Lang,
            "LANGMATCHES" => BuiltInName::LangMatches,
            "DATATYPE" => BuiltInName::Datatype,
            "IRI" => BuiltInName::Iri,
            "URI" => BuiltInName::Uri,
            "BNODE" => BuiltInName::BNode,
            "RAND" => BuiltInName::Rand,
            "ABS" => BuiltInName::Abs,
            "CEIL" => BuiltInName::Ceil,
            "FLOOR" => BuiltInName::Floor,
            "ROUND" => BuiltInName::Round,
            "CONCAT" => BuiltInName::Concat,
            "SUBSTR" => BuiltInName::SubStr,
            "STRLEN" => BuiltInName::StrLen,
            "REPLACE" => BuiltInName::Replace,
            "UCASE" => BuiltInName::UCase,
            "LCASE" => BuiltInName::LCase,
            "ENCODE_FOR_URI" => BuiltInName::EncodeForUri,
            "CONTAINS" => BuiltInName::Contains,
            "STRSTARTS" => BuiltInName::StrStarts,
            "STRENDS" => BuiltInName::StrEnds,
            "STRBEFORE" => BuiltInName::StrBefore,
            "STRAFTER" => BuiltInName::StrAfter,
            "YEAR" => BuiltInName::Year,
            "MONTH" => BuiltInName::Month,
            "DAY" => BuiltInName::Day,
            "HOURS" => BuiltInName::Hours,
            "MINUTES" => BuiltInName::Minutes,
            "SECONDS" => BuiltInName::Seconds,
            "TIMEZONE" => BuiltInName::Timezone,
            "TZ" => BuiltInName::Tz,
            "NOW" => BuiltInName::Now,
            "UUID" => BuiltInName::Uuid,
            "STRUUID" => BuiltInName::StrUuid,
            "MD5" => BuiltInName::Md5,
            "SHA1" => BuiltInName::Sha1,
            "SHA256" => BuiltInName::Sha256,
            "SHA384" => BuiltInName::Sha384,
            "SHA512" => BuiltInName::Sha512,
            "STRLANG" => BuiltInName::StrLang,
            "STRDT" => BuiltInName::StrDt,
            "isIRI" => BuiltInName::IsIri,
            "isURI" => BuiltInName::IsUri,
            "isBLANK" => BuiltInName::IsBlank,
            "isLITERAL" => BuiltInName::IsLiteral,
            "isNUMERIC" => BuiltInName::IsNumeric,
            "REGEX" => BuiltInName::Regex,
            #[cfg(feature = "sparql-12")]
            "LANGDIR" => BuiltInName::LangDir,
            #[cfg(feature = "sparql-12")]
            "STRLANGDIR" => BuiltInName::StrLangDir,
            #[cfg(feature = "sparql-12")]
            "hasLANG" => BuiltInName::HasLang,
            #[cfg(feature = "sparql-12")]
            "hasLANGDIR" => BuiltInName::HasLangDir,
            #[cfg(feature = "sparql-12")]
            "isTRIPLE" => BuiltInName::IsTriple,
            #[cfg(feature = "sparql-12")]
            "TRIPLE" => BuiltInName::Triple,
            #[cfg(feature = "sparql-12")]
            "SUBJECT" => BuiltInName::Subject,
            #[cfg(feature = "sparql-12")]
            "PREDICATE" => BuiltInName::Predicate,
            #[cfg(feature = "sparql-12")]
            "OBJECT" => BuiltInName::Object,
            #[cfg(feature = "sep-0002")]
            "ADJUST" => BuiltInName::Adjust
        }
        .labelled("a built-in function name")
        .then(expression_list.clone())
        .map(|(name, args)| Expression::BuiltIn(name, args)))
        .or(exists)
        .spanned()
        .boxed();

    // [140]   	BrackettedExpression 	  ::=   	'(' Expression ')'
    let bracketted_expression = expression
        .clone()
        .delimited_by(operator("("), operator(")"));

    // [139]   	ExprTripleTermObject 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
    #[cfg(feature = "sparql-12")]
    let mut expr_triple_term = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let expr_triple_term_object = choice((
        iri.map(ExprTripleTermObject::Iri),
        rdf_literal.clone().map(ExprTripleTermObject::Literal),
        numeric_literal.map(ExprTripleTermObject::Literal),
        boolean_literal.clone().map(ExprTripleTermObject::Literal),
        var.map(ExprTripleTermObject::Var),
        expr_triple_term
            .clone()
            .map(|t| ExprTripleTermObject::TripleTerm(Box::new(t))),
    ));

    // [138]   	ExprTripleTermSubject 	  ::=   	iri | Var
    #[cfg(feature = "sparql-12")]
    let expr_triple_term_subject = iri
        .map(ExprTripleTermSubject::Iri)
        .or(var.map(ExprTripleTermSubject::Var));

    // [125]   	VarOrIri 	  ::=   	Var | iri
    let var_or_iri = var.map(VarOrIri::Var).or(iri.map(VarOrIri::Iri));

    // [84]   	Verb 	  ::=   	VarOrIri | 'a'
    let verb = var_or_iri
        .map(|v| match v {
            VarOrIri::Var(v) => Verb::Var(v),
            VarOrIri::Iri(v) => Verb::Iri(v),
        })
        .or(case_sensitive_keyword("a").to(Verb::A));

    // [137]   	ExprTripleTerm 	  ::=   	'<<(' ExprTripleTermSubject Verb ExprTripleTermObject ')>>'
    #[cfg(feature = "sparql-12")]
    expr_triple_term.define(
        expr_triple_term_subject
            .then(verb.clone())
            .then(expr_triple_term_object)
            .delimited_by(operator("<<("), operator(")>>"))
            .map(|((subject, predicate), object)| ExprTripleTerm {
                subject,
                predicate,
                object,
            }),
    );

    // [136]   	PrimaryExpression 	  ::=   	BrackettedExpression | BuiltInCall | iriOrFunction | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
    let primary_expression = choice((
        bracketted_expression.clone(),
        iri_or_function,
        rdf_literal.clone().map(Expression::Literal).spanned(),
        numeric_literal.map(Expression::Literal).spanned(),
        boolean_literal.clone().map(Expression::Literal).spanned(),
        var.map(Expression::Var).spanned(),
        #[cfg(feature = "sparql-12")]
        expr_triple_term.map(Expression::TripleTerm).spanned(),
        built_in_call.clone(),
    ))
    .boxed();

    expression.define(
        primary_expression
            .pratt((
                // [127]   	Expression 	  ::=   	ConditionalOrExpression

                // [128]   	ConditionalOrExpression 	  ::=   	ConditionalAndExpression ( '||' ConditionalAndExpression )*
                infix(left(1), operator("||"), |l, (), r, c| Spanned {
                    inner: Expression::Or(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                // [129]   	ConditionalAndExpression 	  ::=   	ValueLogical ( '&&' ValueLogical )*
                infix(left(2), operator("&&"), |l, (), r, c| Spanned {
                    inner: Expression::And(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                // [130]   	ValueLogical 	  ::=   	RelationalExpression
                // [131]   	RelationalExpression 	  ::=   	NumericExpression ( '=' NumericExpression | '!=' NumericExpression | '<' NumericExpression | '>' NumericExpression | '<=' NumericExpression | '>=' NumericExpression | 'IN' ExpressionList | 'NOT' 'IN' ExpressionList )?
                infix(left(3), operator("="), |l, (), r, c| Spanned {
                    inner: Expression::Equal(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(3), operator("!="), |l, (), r, c| Spanned {
                    inner: Expression::NotEqual(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(3), operator("<"), |l, (), r, c| Spanned {
                    inner: Expression::Less(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(3), operator(">"), |l, (), r, c| Spanned {
                    inner: Expression::Greater(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(3), operator("<="), |l, (), r, c| Spanned {
                    inner: Expression::LessOrEqual(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(3), operator(">="), |l, (), r, c| Spanned {
                    inner: Expression::GreaterOrEqual(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                postfix(
                    3,
                    keyword("IN").ignore_then(expression_list.clone()),
                    |l, r, c| Spanned {
                        inner: Expression::In(Box::new(l), r),
                        span: c.span(),
                    },
                ),
                postfix(
                    3,
                    keyword("NOT")
                        .ignore_then(keyword("IN"))
                        .ignore_then(expression_list.clone()),
                    |l, r, c| Spanned {
                        inner: Expression::NotIn(Box::new(l), r),
                        span: c.span(),
                    },
                ),
                // [132]   	NumericExpression 	  ::=   	AdditiveExpression
                // [133]   	AdditiveExpression 	  ::=   	MultiplicativeExpression ( '+' MultiplicativeExpression | '-' MultiplicativeExpression | ( NumericLiteralPositive | NumericLiteralNegative ) ( ( '*' UnaryExpression ) | ( '/' UnaryExpression ) )* )*
                infix(left(4), operator("+"), |l, (), r, c| Spanned {
                    inner: Expression::Add(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(4), operator("-"), |l, (), r, c| Spanned {
                    inner: Expression::Subtract(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                postfix(
                    4,
                    numeric_literal_positive_or_negative
                        .map(Expression::Literal)
                        .spanned(),
                    |l, r, c| Spanned {
                        inner: Expression::Add(Box::new(l), Box::new(r)),
                        span: c.span(),
                    },
                ),
                // [134]   	MultiplicativeExpression 	  ::=   	UnaryExpression ( '*' UnaryExpression | '/' UnaryExpression )*
                infix(left(5), operator("*"), |l, (), r, c| Spanned {
                    inner: Expression::Multiply(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                infix(left(5), operator("/"), |l, (), r, c| Spanned {
                    inner: Expression::Divide(Box::new(l), Box::new(r)),
                    span: c.span(),
                }),
                // [135]   	UnaryExpression 	  ::=   	  '!' UnaryExpression | '+' PrimaryExpression | '-' PrimaryExpression | PrimaryExpression
                prefix(6, operator("!"), |(), a, c| Spanned {
                    inner: Expression::Not(Box::new(a)),
                    span: c.span(),
                }),
                prefix(6, operator("+"), |(), a, c| Spanned {
                    inner: Expression::UnaryPlus(Box::new(a)),
                    span: c.span(),
                }),
                prefix(6, operator("-"), |(), a, c| Spanned {
                    inner: Expression::UnaryMinus(Box::new(a)),
                    span: c.span(),
                }),
            ))
            .boxed(),
    );

    // [124]   	TripleTermDataObject 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | TripleTermData
    #[cfg(feature = "sparql-12")]
    let mut triple_term_data = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let triple_term_data_object = choice((
        iri.map(TripleTermDataObject::Iri),
        rdf_literal.clone().map(TripleTermDataObject::Literal),
        numeric_literal.map(TripleTermDataObject::Literal),
        boolean_literal.clone().map(TripleTermDataObject::Literal),
        triple_term_data
            .clone()
            .map(|t| TripleTermDataObject::TripleTerm(Box::new(t))),
    ));

    // [123]   	TripleTermDataSubject 	  ::=   	iri
    // [122]   	TripleTermData 	  ::=   	'<<(' TripleTermDataSubject ( iri | 'a' ) TripleTermDataObject ')>>'
    #[cfg(feature = "sparql-12")]
    triple_term_data.define(
        iri.then(
            iri.map(IriOrA::Iri)
                .or(case_sensitive_keyword("a").to(IriOrA::A)),
        )
        .then(triple_term_data_object)
        .delimited_by(operator("<<("), operator(")>>"))
        .map(|((subject, predicate), object)| TripleTermData {
            subject,
            predicate,
            object,
        }),
    );

    // [121]   	TripleTermObject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | TripleTerm
    // [120]   	TripleTermSubject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | TripleTerm
    #[cfg(feature = "sparql-12")]
    let mut triple_term = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let triple_term_subject_or_object = choice((
        var.map(VarOrTerm::Var),
        iri.map(VarOrTerm::Iri),
        rdf_literal.clone().map(VarOrTerm::Literal),
        numeric_literal.map(VarOrTerm::Literal),
        boolean_literal.clone().map(VarOrTerm::Literal),
        blank_node.clone().map(VarOrTerm::BlankNode),
        triple_term
            .clone()
            .map(|t| VarOrTerm::TripleTerm(Box::new(t))),
    ))
    .boxed();

    // [119]   	TripleTerm 	  ::=   	'<<(' TripleTermSubject Verb TripleTermObject ')>>'
    #[cfg(feature = "sparql-12")]
    triple_term.define(
        triple_term_subject_or_object
            .clone()
            .then(verb.clone())
            .then(triple_term_subject_or_object)
            .delimited_by(operator("<<("), operator(")>>"))
            .map(|((subject, predicate), object)| TripleTerm {
                subject,
                predicate,
                object,
            }),
    );

    // [118]   	ReifiedTripleObject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | ReifiedTriple | TripleTerm
    // [117]   	ReifiedTripleSubject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | ReifiedTriple | TripleTerm
    #[cfg(feature = "sparql-12")]
    let mut reified_triple = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let reified_triple_subject_or_object = choice((
        var.map(ReifiedTripleSubjectOrObject::Var),
        iri.map(ReifiedTripleSubjectOrObject::Iri),
        rdf_literal
            .clone()
            .map(ReifiedTripleSubjectOrObject::Literal),
        numeric_literal.map(ReifiedTripleSubjectOrObject::Literal),
        boolean_literal
            .clone()
            .map(ReifiedTripleSubjectOrObject::Literal),
        blank_node
            .clone()
            .map(ReifiedTripleSubjectOrObject::BlankNode),
        reified_triple
            .clone()
            .map(|t| ReifiedTripleSubjectOrObject::ReifiedTriple(Box::new(t))),
        triple_term
            .clone()
            .map(|t| ReifiedTripleSubjectOrObject::TripleTerm(Box::new(t))),
    ))
    .boxed();

    // [71]   	VarOrReifierId 	  ::=   	Var | iri | BlankNode
    #[cfg(feature = "sparql-12")]
    let var_or_reifier_id = choice((
        var.map(VarOrReifierId::Var),
        iri.map(VarOrReifierId::Iri),
        blank_node.clone().map(VarOrReifierId::BlankNode),
    ));

    // [70]   	Reifier 	  ::=   	'~' VarOrReifierId?
    #[cfg(feature = "sparql-12")]
    let reifier = operator("~").ignore_then(var_or_reifier_id.or_not());

    // [116]   	ReifiedTriple 	  ::=   	'<<' ReifiedTripleSubject Verb ReifiedTripleObject Reifier? '>>'
    #[cfg(feature = "sparql-12")]
    reified_triple.define(
        reified_triple_subject_or_object
            .clone()
            .then(verb.clone())
            .then(reified_triple_subject_or_object)
            .then(reifier.clone().or_not())
            .delimited_by(operator("<<"), operator(">>"))
            .map(|(((subject, predicate), object), reifier)| ReifiedTriple {
                subject,
                predicate,
                object,
                reifier: reifier.flatten(),
            }),
    );

    // [115]   	VarOrTerm 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | NIL | TripleTerm
    let var_or_term = choice((
        var.map(VarOrTerm::Var),
        iri.map(VarOrTerm::Iri),
        rdf_literal.clone().map(VarOrTerm::Literal),
        numeric_literal.map(VarOrTerm::Literal),
        boolean_literal.clone().map(VarOrTerm::Literal),
        blank_node.clone().map(VarOrTerm::BlankNode),
        nil.clone().to(VarOrTerm::Nil),
        #[cfg(feature = "sparql-12")]
        triple_term.map(|t| VarOrTerm::TripleTerm(Box::new(t))),
    ));

    // [114]   	GraphNodePath 	  ::=   	VarOrTerm | TriplesNodePath | ReifiedTriple
    let mut triples_node_path = Recursive::declare();
    let graph_node_path = choice((
        var_or_term.clone().map(GraphNodePath::VarOrTerm),
        triples_node_path.clone(),
        #[cfg(feature = "sparql-12")]
        reified_triple.clone().map(GraphNodePath::ReifiedTriple),
    ));

    // [113]   	GraphNode 	  ::=   	VarOrTerm | TriplesNode | ReifiedTriple
    let mut triples_node = Recursive::declare();
    let graph_node = choice((
        var_or_term.clone().map(GraphNode::VarOrTerm),
        triples_node.clone(),
        #[cfg(feature = "sparql-12")]
        reified_triple.clone().map(GraphNode::ReifiedTriple),
    ));

    // [112]   	AnnotationBlock 	  ::=   	'{|' PropertyListNotEmpty '|}'
    let mut property_list_not_empty = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let annotation_block = property_list_not_empty
        .clone()
        .delimited_by(operator("{|"), operator("|}"));

    // [111]   	Annotation 	  ::=   	( Reifier | AnnotationBlock )*
    #[cfg(feature = "sparql-12")]
    let annotation = reifier
        .clone()
        .map(Annotation::Reifier)
        .or(annotation_block.map(Annotation::AnnotationBlock))
        .spanned()
        .repeated()
        .collect();

    // [110]   	AnnotationBlockPath 	  ::=   	'{|' PropertyListPathNotEmpty '|}'
    let mut property_list_path_not_empty = Recursive::declare();
    #[cfg(feature = "sparql-12")]
    let annotation_block_path = property_list_path_not_empty
        .clone()
        .delimited_by(operator("{|"), operator("|}"));

    // [109]   	AnnotationPath 	  ::=   	( Reifier | AnnotationBlockPath )*
    #[cfg(feature = "sparql-12")]
    let annotation_path = reifier
        .map(AnnotationPath::Reifier)
        .or(annotation_block_path.map(AnnotationPath::AnnotationBlock))
        .spanned()
        .repeated()
        .collect();

    // [108]   	CollectionPath 	  ::=   	'(' GraphNodePath+ ')'
    let collection_path = graph_node_path
        .clone()
        .repeated()
        .at_least(1)
        .collect()
        .delimited_by(operator("("), operator(")"))
        .map(GraphNodePath::Collection);

    // [107]   	Collection 	  ::=   	'(' GraphNode+ ')'
    let collection = graph_node
        .clone()
        .repeated()
        .at_least(1)
        .collect()
        .delimited_by(operator("("), operator(")"))
        .map(GraphNode::Collection);

    // [106]   	BlankNodePropertyListPath 	  ::=   	'[' PropertyListPathNotEmpty ']'
    let blank_node_property_list_path = property_list_path_not_empty
        .clone()
        .delimited_by(operator("["), operator("]"))
        .map(GraphNodePath::BlankNodePropertyList);

    // [105]   	TriplesNodePath 	  ::=   	CollectionPath | BlankNodePropertyListPath
    triples_node_path.define(collection_path.or(blank_node_property_list_path));

    // [104]   	BlankNodePropertyList 	  ::=   	'[' PropertyListNotEmpty ']'
    let blank_node_property_list = property_list_not_empty
        .clone()
        .delimited_by(operator("["), operator("]"))
        .map(GraphNode::BlankNodePropertyList);

    // [103]   	TriplesNode 	  ::=   	Collection | BlankNodePropertyList
    triples_node.define(collection.or(blank_node_property_list));

    let path = recursive(|path| {
        // [102]   	PathOneInPropertySet 	  ::=   	iri | 'a' | '^' ( iri | 'a' )
        let path_one_in_property_set = choice((
            iri.map(PathOneInPropertySet::Iri),
            case_sensitive_keyword("a").to(PathOneInPropertySet::A),
            operator("^").ignore_then(iri.map(PathOneInPropertySet::InverseIri)),
            operator("^")
                .ignore_then(case_sensitive_keyword("a").to(PathOneInPropertySet::InverseA)),
        ));

        // [101]   	PathNegatedPropertySet 	  ::=   	PathOneInPropertySet | '(' ( PathOneInPropertySet ( '|' PathOneInPropertySet )* )? ')'
        let path_negated_property_set = path_one_in_property_set
            .clone()
            .map(|p| vec![p])
            .or(path_one_in_property_set
                .separated_by(operator("|"))
                .at_least(1)
                .collect::<Vec<_>>()
                .delimited_by(operator("("), operator(")")))
            .map(Path::NegatedPropertySet);

        // [100]   	PathPrimary 	  ::=   	iri | 'a' | '!' PathNegatedPropertySet | '(' Path ')'
        let path_primary = choice((
            iri.map(Path::Iri),
            case_sensitive_keyword("a").to(Path::A),
            operator("!").ignore_then(path_negated_property_set),
            path.delimited_by(operator("("), operator(")")),
        ));

        // [94]   	Path 	  ::=   	PathAlternative
        // [95]   	PathAlternative 	  ::=   	PathSequence ( '|' PathSequence )*
        // [96]   	PathSequence 	  ::=   	PathEltOrInverse ( '/' PathEltOrInverse )*
        // [97]   	PathElt 	  ::=   	PathPrimary PathMod?
        // [98]   	PathEltOrInverse 	  ::=   	PathElt | '^' PathElt
        // [99]   	PathMod 	  ::=   	'?' | '*' | '+'
        path_primary
            .pratt((
                infix(left(1), operator("|"), |l, (), r, _| {
                    Path::Alternative(Box::new(l), Box::new(r))
                }),
                infix(left(2), operator("/"), |l, (), r, _| {
                    Path::Sequence(Box::new(l), Box::new(r))
                }),
                prefix(3, operator("^"), |(), e, _| Path::Inverse(Box::new(e))),
                postfix(4, operator("?"), |e, (), _| Path::ZeroOrOne(Box::new(e))),
                postfix(4, operator("*"), |e, (), _| Path::ZeroOrMore(Box::new(e))),
                postfix(4, operator("+"), |e, (), _| Path::OneOrMore(Box::new(e))),
            ))
            .boxed()
    });

    // [93]   	ObjectPath 	  ::=   	GraphNodePath AnnotationPath
    #[cfg(feature = "sparql-12")]
    let object_path = graph_node_path
        .then(annotation_path)
        .map(|(graph_node, annotations)| ObjectPath {
            graph_node,
            annotations,
        })
        .boxed();
    #[cfg(not(feature = "sparql-12"))]
    let object_path = graph_node_path
        .map(|graph_node| ObjectPath { graph_node })
        .boxed();

    // [92]   	ObjectListPath 	  ::=   	ObjectPath ( ',' ObjectPath )*
    let object_list_path = object_path
        .separated_by(operator(","))
        .at_least(1)
        .collect();

    // [91]   	VerbSimple 	  ::=   	Var
    let verb_simple = var.map(VarOrPath::Var);

    // [90]   	VerbPath 	  ::=   	Path
    let verb_path = path.map(VarOrPath::Path);

    // [89]   	PropertyListPathNotEmpty 	  ::=   	( VerbPath | VerbSimple ) ObjectListPath ( ';' ( ( VerbPath | VerbSimple ) ObjectListPath )? )*
    let property_list_path_element = verb_simple.or(verb_path).then(object_list_path);
    property_list_path_not_empty.define(
        property_list_path_element.clone().map(|v| vec![v]).foldl(
            operator(";")
                .ignore_then(property_list_path_element.clone().or_not())
                .repeated(),
            |mut acc, val| {
                acc.extend(val);
                acc
            },
        ),
    );

    // [88]   	PropertyListPath 	  ::=   	PropertyListPathNotEmpty?
    let property_list_path = property_list_path_not_empty
        .clone()
        .or_not()
        .map(Option::unwrap_or_default);

    // [59]   	ReifiedTripleBlockPath 	  ::=   	ReifiedTriple PropertyListPath
    #[cfg(feature = "sparql-12")]
    let reified_triple_block_path = reified_triple
        .clone()
        .map(GraphNodePath::ReifiedTriple)
        .then(property_list_path.clone());

    // [87]   	TriplesSameSubjectPath 	  ::=   	VarOrTerm PropertyListPathNotEmpty | TriplesNodePath PropertyListPath | ReifiedTripleBlockPath
    let triples_same_subject_path = choice((
        var_or_term
            .clone()
            .map(GraphNodePath::VarOrTerm)
            .then(property_list_path_not_empty),
        triples_node_path.then(property_list_path),
        #[cfg(feature = "sparql-12")]
        reified_triple_block_path,
    ));

    // [86]   	Object 	  ::=   	GraphNode Annotation
    #[cfg(feature = "sparql-12")]
    let object = graph_node
        .then(annotation)
        .map(|(graph_node, annotations)| Object {
            graph_node,
            annotations,
        })
        .boxed();
    #[cfg(not(feature = "sparql-12"))]
    let object = graph_node.map(|graph_node| Object { graph_node }).boxed();

    // [85]   	ObjectList 	  ::=   	Object ( ',' Object )*
    let object_list = object.separated_by(operator(",")).at_least(1).collect();

    // [83]   	PropertyListNotEmpty 	  ::=   	Verb ObjectList ( ';' ( Verb ObjectList )? )*
    let property_list_element = verb.then(object_list);
    property_list_not_empty.define(
        property_list_element.clone().map(|v| vec![v]).foldl(
            operator(";")
                .ignore_then(property_list_element.clone().or_not())
                .repeated(),
            |mut acc, val| {
                acc.extend(val);
                acc
            },
        ),
    );

    // [82]   	PropertyList 	  ::=   	PropertyListNotEmpty?
    let property_list = property_list_not_empty
        .clone()
        .or_not()
        .map(Option::unwrap_or_default);

    // [58]   	ReifiedTripleBlock 	  ::=   	ReifiedTriple PropertyList
    #[cfg(feature = "sparql-12")]
    let reified_triple_block = reified_triple
        .map(GraphNode::ReifiedTriple)
        .then(property_list.clone());

    // [81]   	TriplesSameSubject 	  ::=   	VarOrTerm PropertyListNotEmpty | TriplesNode PropertyList | ReifiedTripleBlock
    let triples_same_subject = choice((
        var_or_term
            .clone()
            .map(GraphNode::VarOrTerm)
            .then(property_list_not_empty),
        triples_node.then(property_list),
        #[cfg(feature = "sparql-12")]
        reified_triple_block,
    ));

    // [80]   	ConstructTriples 	  ::=   	TriplesSameSubject ( '.' ConstructTriples? )?
    // also TriplesSameSubject ("." TriplesSameSubject?)*
    // [79]   	ConstructTemplate 	  ::=   	'{' ConstructTriples? '}'
    let construct_template = triples_same_subject
        .clone()
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(operator("{"), operator("}"))
        .spanned();

    // [76]   	FunctionCall 	  ::=   	iri ArgList
    let function_call = iri
        .then(arg_list)
        .map(|(name, args)| Expression::Function(name, args))
        .spanned();

    // [75]   	Constraint 	  ::=   	BrackettedExpression | BuiltInCall | FunctionCall
    let constraint = choice((
        bracketted_expression.clone(),
        built_in_call.clone(),
        function_call.clone(),
    ));

    // [74]   	Filter 	  ::=   	'FILTER' Constraint
    let filter = keyword("FILTER")
        .ignore_then(constraint.clone())
        .map(GraphPatternElement::Filter);

    // [73]   	GroupOrUnionGraphPattern 	  ::=   	GroupGraphPattern ( 'UNION' GroupGraphPattern )*
    let group_or_union_graph_pattern = group_graph_pattern
        .clone()
        .separated_by(keyword("UNION"))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(GraphPatternElement::Union);

    // [72]   	MinusGraphPattern 	  ::=   	'MINUS' GroupGraphPattern
    let minus_graph_pattern = keyword("MINUS")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| GraphPatternElement::Minus(Box::new(p)));

    // [69]   	DataBlockValue 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | 'UNDEF' | TripleTermData
    let data_block_value = choice((
        iri.map(DataBlockValue::Iri),
        rdf_literal.clone().map(DataBlockValue::Literal),
        numeric_literal.map(DataBlockValue::Literal),
        boolean_literal.clone().map(DataBlockValue::Literal),
        keyword("UNDEF").to(DataBlockValue::Undef),
        #[cfg(feature = "sparql-12")]
        triple_term_data.map(DataBlockValue::TripleTerm),
    ));

    // [68]   	InlineDataFull 	  ::=   	( NIL | '(' Var* ')' ) '{' ( '(' DataBlockValue* ')' | NIL )* '}'
    let inline_data_full = var
        .repeated()
        .collect()
        .delimited_by(operator("("), operator(")"))
        .spanned()
        .then(
            data_block_value
                .clone()
                .repeated()
                .collect()
                .delimited_by(operator("("), operator(")"))
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(operator("{"), operator("}"))
                .spanned(),
        );

    // [67]   	InlineDataOneVar 	  ::=   	Var '{' DataBlockValue* '}'
    let inline_data_one_var = var.map(|v| vec![v]).spanned().then(
        data_block_value
            .map(|v| vec![v])
            .repeated()
            .collect()
            .delimited_by(operator("{"), operator("}"))
            .spanned(),
    );

    // [66]   	DataBlock 	  ::=   	InlineDataOneVar | InlineDataFull
    let data_block = inline_data_one_var
        .or(inline_data_full)
        .map(|(variables, values)| ValuesClause { variables, values });

    // [65]   	InlineData 	  ::=   	'VALUES' DataBlock
    let inline_data = keyword("VALUES")
        .ignore_then(data_block.clone())
        .map(GraphPatternElement::Values);

    // [64]   	Bind 	  ::=   	'BIND' '(' Expression 'AS' Var ')'
    let bind = keyword("BIND")
        .ignore_then(
            expression
                .clone()
                .then_ignore(keyword("AS"))
                .then(var)
                .delimited_by(operator("("), operator(")")),
        )
        .map(|(e, v)| GraphPatternElement::Bind(e, v));

    // [63]   	ServiceGraphPattern 	  ::=   	'SERVICE' 'SILENT'? VarOrIri GroupGraphPattern
    let service_graph_pattern = keyword("SERVICE")
        .ignore_then(keyword("SILENT").or_not())
        .then(var_or_iri)
        .then(group_graph_pattern.clone())
        .map(|((silent, name), pattern)| GraphPatternElement::Service {
            silent: silent.is_some(),
            name,
            pattern: Box::new(pattern),
        });

    // [62]   	GraphGraphPattern 	  ::=   	'GRAPH' VarOrIri GroupGraphPattern
    let graph_graph_pattern = keyword("GRAPH")
        .ignore_then(var_or_iri)
        .then(group_graph_pattern.clone())
        .map(|(name, pattern)| GraphPatternElement::Graph {
            name,
            pattern: Box::new(pattern),
        });

    // [61]   	OptionalGraphPattern 	  ::=   	'OPTIONAL' GroupGraphPattern
    let optional_graph_pattern = keyword("OPTIONAL")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| GraphPatternElement::Optional(Box::new(p)));

    #[cfg(feature = "sep-0006")]
    let lateral_graph_pattern = keyword("LATERAL")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| GraphPatternElement::Lateral(Box::new(p)));

    // [60]   	GraphPatternNotTriples 	  ::=   	GroupOrUnionGraphPattern | OptionalGraphPattern | MinusGraphPattern | GraphGraphPattern | ServiceGraphPattern | Filter | Bind | InlineData
    let graph_pattern_not_triples = choice((
        group_or_union_graph_pattern,
        optional_graph_pattern,
        minus_graph_pattern,
        graph_graph_pattern,
        service_graph_pattern,
        filter,
        bind,
        inline_data,
        #[cfg(feature = "sep-0006")]
        lateral_graph_pattern,
    ));

    // [57]   	TriplesBlock 	  ::=   	TriplesSameSubjectPath ( '.' TriplesBlock? )?
    // also TriplesSameSubjectPath ( '.' TriplesSameSubjectPath? )*
    // It is always optional, we allow it to be empty
    let triples_block = triples_same_subject_path
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .map(GraphPatternElement::Triples)
        .spanned()
        .boxed();

    // [56]   	GroupGraphPatternSub 	  ::=   	TriplesBlock? ( GraphPatternNotTriples '.'? TriplesBlock? )*
    let group_graph_pattern_sub = triples_block
        .clone()
        .map(|group| vec![group])
        .foldl(
            graph_pattern_not_triples
                .spanned()
                .then_ignore(operator(".").or_not())
                .then(triples_block)
                .repeated(),
            |mut a, (b, c)| {
                a.push(b);
                a.push(c);
                a
            },
        )
        .map(GraphPattern::Group);

    // [55]   	GroupGraphPattern 	  ::=   	'{' ( SubSelect | GroupGraphPatternSub ) '}'
    let mut sub_select = Recursive::declare();
    group_graph_pattern.define(
        sub_select
            .clone()
            .or(group_graph_pattern_sub)
            .delimited_by(operator("{"), operator("}"))
            .boxed(),
    );

    // [54]   	TriplesTemplate 	  ::=   	TriplesSameSubject ( '.' TriplesTemplate? )?
    // TripleTemplate is always option al, we allow it to be empty
    let triples_template = triples_same_subject
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .boxed();

    // [53]   	QuadsNotTriples 	  ::=   	'GRAPH' VarOrIri '{' TriplesTemplate? '}'
    let quads_not_triples = keyword("GRAPH")
        .ignore_then(var_or_iri)
        .then(
            triples_template
                .clone()
                .delimited_by(operator("{"), operator("}")),
        )
        .map(|(graph, triples)| (Some(graph), triples));

    // [52]   	Quads 	  ::=   	TriplesTemplate? ( QuadsNotTriples '.'? TriplesTemplate? )*
    let quads = triples_template
        .clone()
        .map(|t| vec![(None, t)])
        .foldl(
            quads_not_triples
                .then_ignore(operator(".").or_not())
                .then(triples_template.clone().map(|t| (None, t)))
                .repeated(),
            |mut a, (b, c)| {
                a.push(b);
                a.push(c);
                a
            },
        )
        .boxed();

    // [50]   	QuadPattern 	  ::=   	'{' Quads '}'
    let quad_pattern = quads.delimited_by(operator("{"), operator("}"));

    // [48]   	GraphRef 	  ::=   	'GRAPH' iri
    let graph_ref = keyword("GRAPH").ignore_then(iri);

    // [49]   	GraphRefAll 	  ::=   	GraphRef | 'DEFAULT' | 'NAMED' | 'ALL'
    let graph_ref_all = choice((
        graph_ref.clone().map(GraphRefAll::Graph),
        keyword("DEFAULT").to(GraphRefAll::Default),
        keyword("NAMED").to(GraphRefAll::Named),
        keyword("ALL").to(GraphRefAll::All),
    ));

    // [47]   	GraphOrDefault 	  ::=   	'DEFAULT' | 'GRAPH'? iri
    let graph_or_default = keyword("DEFAULT")
        .to(GraphOrDefault::Default)
        .or(keyword("GRAPH")
            .or_not()
            .ignore_then(iri)
            .map(GraphOrDefault::Graph));

    // [46]   	UsingClause 	  ::=   	'USING' ( iri | 'NAMED' iri )
    let using_clause = keyword("USING").ignore_then(
        iri.map(GraphClause::Default)
            .or(keyword("NAMED").ignore_then(iri).map(GraphClause::Named)),
    );

    // [45]   	InsertClause 	  ::=   	'INSERT' QuadPattern
    let insert_clause = keyword("INSERT").ignore_then(quad_pattern.clone());

    // [44]   	DeleteClause 	  ::=   	'DELETE' QuadPattern
    let delete_clause = keyword("DELETE").ignore_then(quad_pattern.clone());

    // [43]   	Modify 	  ::=   	( 'WITH' iri )? ( DeleteClause InsertClause? | InsertClause ) UsingClause* 'WHERE' GroupGraphPattern
    let modify = keyword("WITH")
        .ignore_then(iri)
        .or_not()
        .then(
            delete_clause
                .then(insert_clause.clone().or_not())
                .map(|(delete, insert)| (delete, insert.unwrap_or_default()))
                .or(insert_clause.map(|insert| (Vec::new(), insert))),
        )
        .then(using_clause.repeated().collect())
        .then_ignore(keyword("WHERE"))
        .then(group_graph_pattern.clone())
        .map(
            |(((with, (delete, insert)), using), r#where)| Update1::Modify {
                with,
                delete,
                insert,
                using,
                r#where,
            },
        );

    // [42]   	DeleteWhere 	  ::=   	'DELETE WHERE' QuadPattern
    let delete_where = keyword("DELETE")
        .ignore_then(keyword("WHERE"))
        .ignore_then(quad_pattern.clone())
        .map(|pattern| Update1::DeleteWhere { pattern });

    // [41]   	DeleteData 	  ::=   	'DELETE DATA' QuadData
    let delete_data = keyword("DELETE")
        .ignore_then(keyword("DATA"))
        .ignore_then(quad_pattern.clone())
        .map(|quads| Update1::DeleteData { quads });

    // [40]   	InsertData 	  ::=   	'INSERT DATA' QuadData
    let insert_data = keyword("INSERT")
        .ignore_then(keyword("DATA"))
        .ignore_then(quad_pattern)
        .map(|quads| Update1::InsertData { quads });

    // [39]   	Copy 	  ::=   	'COPY' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
    let copy = keyword("COPY")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default.clone())
        .then_ignore(keyword("TO"))
        .then(graph_or_default.clone())
        .map(|((silent, from), to)| Update1::Copy {
            silent: silent.is_some(),
            from,
            to,
        });

    // [38]   	Move 	  ::=   	'MOVE' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
    let r#move = keyword("MOVE")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default.clone())
        .then_ignore(keyword("TO"))
        .then(graph_or_default.clone())
        .map(|((silent, from), to)| Update1::Move {
            silent: silent.is_some(),
            from,
            to,
        });

    // [37]   	Add 	  ::=   	'ADD' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
    let add = keyword("ADD")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default.clone())
        .then_ignore(keyword("TO"))
        .then(graph_or_default)
        .map(|((silent, from), to)| Update1::Add {
            silent: silent.is_some(),
            from,
            to,
        });

    // [36]   	Create 	  ::=   	'CREATE' 'SILENT'? GraphRef
    let create = keyword("CREATE")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref.clone())
        .map(|(silent, graph)| Update1::Create {
            silent: silent.is_some(),
            graph,
        });

    // [35]   	Drop 	  ::=   	'DROP' 'SILENT'? GraphRefAll
    let drop = keyword("DROP")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref_all.clone())
        .map(|(silent, graph)| Update1::Drop {
            silent: silent.is_some(),
            graph,
        });

    // [34]   	Clear 	  ::=   	'CLEAR' 'SILENT'? GraphRefAll
    let clear = keyword("CLEAR")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref_all)
        .map(|(silent, graph)| Update1::Clear {
            silent: silent.is_some(),
            graph,
        });

    // [33]   	Load 	  ::=   	'LOAD' 'SILENT'? iri ( 'INTO' GraphRef )?
    let load = keyword("LOAD")
        .ignore_then(keyword("SILENT").or_not())
        .then(iri)
        .then(keyword("INTO").ignore_then(graph_ref).or_not())
        .map(|((silent, from), to)| Update1::Load {
            silent: silent.is_some(),
            from,
            to,
        });

    // [32]   	Update1 	  ::=   	Load | Clear | Drop | Add | Move | Copy | Create | DeleteWhere | Modify | InsertData | DeleteData
    let update1 = choice((
        load,
        clear,
        drop,
        add,
        r#move,
        copy,
        create,
        delete_where,
        modify,
        insert_data,
        delete_data,
    ));

    // [8]   	VersionSpecifier 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2
    #[cfg(feature = "sparql-12")]
    let version_specifier = select! {
        Token::StringLiteral1(v) | Token::StringLiteral2(v) => &v[1..v.len() -1]
    }
    .labelled("a string");

    // [7]   	VersionDecl 	  ::=   	'VERSION' VersionSpecifier
    #[cfg(feature = "sparql-12")]
    let version_decl = keyword("VERSION")
        .ignore_then(version_specifier)
        .map(PrologueDecl::Version);

    // [6]   	PrefixDecl 	  ::=   	'PREFIX' PNAME_NS IRIREF
    let prefix_decl = keyword("PREFIX")
        .ignore_then(pname_ns)
        .then(iriref)
        .map(|(prefix, iri)| PrologueDecl::Prefix(prefix, iri));

    // [5]   	BaseDecl 	  ::=   	'BASE' IRIREF
    let base_decl = keyword("BASE").ignore_then(iriref).map(PrologueDecl::Base);

    // [4]   	Prologue 	  ::=   	( BaseDecl | PrefixDecl | VersionDecl )*
    let prologue = choice((
        base_decl,
        prefix_decl,
        #[cfg(feature = "sparql-12")]
        version_decl,
    ))
    .repeated()
    .collect::<Vec<_>>();

    // [31]   	Update 	  ::=   	Prologue ( Update1 ( ';' Update )? )?
    // or Update 	  ::=   	Prologue (Update1 ( ';' Prologue Update1 )* (';' Prologue)?)?
    let update = prologue
        .clone()
        .then(
            update1
                .clone()
                .then(
                    operator(";")
                        .ignore_then(prologue.clone())
                        .then(update1)
                        .repeated()
                        .collect::<Vec<_>>()
                        .then(operator(";").ignore_then(prologue.clone()).or_not()),
                )
                .or_not(),
        )
        .map(|(first_prologue, rest)| {
            let Some((first_operation, (mut operations, trailing_prologue))) = rest else {
                return Update {
                    operations: Vec::new(),
                    trailing_prologue: first_prologue,
                };
            };
            operations.insert(0, (first_prologue, first_operation));
            Update {
                operations,
                trailing_prologue: trailing_prologue.unwrap_or_default(),
            }
        });

    // [30]   	ValuesClause 	  ::=   	( 'VALUES' DataBlock )?
    let values_clause = keyword("VALUES").ignore_then(data_block).or_not().boxed();

    // [29]   	OffsetClause 	  ::=   	'OFFSET' INTEGER
    let offset_clause = keyword("OFFSET")
        .ignore_then(
            select! {
                Token::Integer(v) => v,
            }
            .labelled("an integer"),
        )
        .try_map(|o, span| {
            usize::from_str(o).map_err(|_| {
                Rich::custom(
                    span,
                    format!("The query offset must be a non negative integer, found {o}"),
                )
            })
        });

    // [28]   	LimitClause 	  ::=   	'LIMIT' INTEGER
    let limit_clause = keyword("LIMIT")
        .ignore_then(
            select! {
                Token::Integer(v) => v,
            }
            .labelled("an integer"),
        )
        .try_map(|l, span| {
            usize::from_str(l).map_err(|_| {
                Rich::custom(
                    span,
                    format!("The query limit must be a non negative integer, found {l}"),
                )
            })
        });

    // [27]   	LimitOffsetClauses 	  ::=   	LimitClause OffsetClause? | OffsetClause LimitClause?
    let limit_offset_clauses = limit_clause
        .clone()
        .then(offset_clause.clone().or_not())
        .map(|(l, o)| LimitOffsetClauses {
            offset: o.unwrap_or(0),
            limit: Some(l),
        })
        .or(offset_clause
            .then(limit_clause.or_not())
            .map(|(offset, limit)| LimitOffsetClauses { offset, limit }));

    // [26]   	OrderCondition 	  ::=   	( ( 'ASC' | 'DESC' ) BrackettedExpression ) | ( Constraint | Var )
    let order_condition = choice((
        keyword("ASC")
            .to(true)
            .or(keyword("DESC").to(false))
            .then(bracketted_expression.clone())
            .map(|(is_asc, e)| {
                if is_asc {
                    OrderCondition::Asc(e)
                } else {
                    OrderCondition::Desc(e)
                }
            }),
        constraint.clone().map(OrderCondition::Asc),
        var.map(Expression::Var).spanned().map(OrderCondition::Asc),
    ));

    // [25]   	OrderClause 	  ::=   	'ORDER' 'BY' OrderCondition+
    let order_clause = keyword("ORDER")
        .ignore_then(keyword("BY"))
        .ignore_then(order_condition.repeated().at_least(1).collect::<Vec<_>>());

    // [24]   	HavingCondition 	  ::=   	Constraint
    let having_condition = constraint;

    // [23]   	HavingClause 	  ::=   	'HAVING' HavingCondition+
    let having_clause =
        keyword("HAVING").ignore_then(having_condition.repeated().at_least(1).collect());

    // [22]   	GroupCondition 	  ::=   	BuiltInCall | FunctionCall | '(' Expression ( 'AS' Var )? ')' | Var
    let group_condition = choice((
        built_in_call.map(|e| (e, None)),
        function_call.map(|e| (e, None)),
        expression
            .clone()
            .then(keyword("AS").ignore_then(var).or_not())
            .delimited_by(operator("("), operator(")")),
        var.map(Expression::Var).spanned().map(|v| (v, None)),
    ));

    // [21]   	GroupClause 	  ::=   	'GROUP' 'BY' GroupCondition+
    let group_clause = keyword("GROUP")
        .ignore_then(keyword("BY"))
        .ignore_then(group_condition.repeated().at_least(1).collect::<Vec<_>>());

    // [20]   	SolutionModifier 	  ::=   	GroupClause? HavingClause? OrderClause? LimitOffsetClauses?
    let solution_modifier = group_clause
        .or_not()
        .then(having_clause.or_not())
        .then(order_clause.or_not())
        .then(limit_offset_clauses.or_not())
        .map(
            |(((group_clause, having_clause), order_clause), limit_offset_clauses)| {
                SolutionModifier {
                    group_clause: group_clause.unwrap_or_default(),
                    having_clause: having_clause.unwrap_or_default(),
                    order_clause: order_clause.unwrap_or_default(),
                    limit_offset_clauses,
                }
            },
        )
        .boxed();

    // [19]   	WhereClause 	  ::=   	'WHERE'? GroupGraphPattern
    let where_clause = keyword("WHERE")
        .or_not()
        .ignore_then(group_graph_pattern)
        .boxed();

    // [18]   	SourceSelector 	  ::=   	iri
    let source_selector = iri;

    // [17]   	NamedGraphClause 	  ::=   	'NAMED' SourceSelector
    let named_graph_clause = keyword("NAMED")
        .ignore_then(source_selector)
        .map(GraphClause::Named);

    // [16]   	DefaultGraphClause 	  ::=   	SourceSelector
    let default_graph_clause = source_selector.map(GraphClause::Default);

    // [15]   	DatasetClause 	  ::=   	'FROM' ( DefaultGraphClause | NamedGraphClause )
    let dataset_clause = keyword("FROM").ignore_then(default_graph_clause.or(named_graph_clause));

    // [14]   	AskQuery 	  ::=   	'ASK' DatasetClause* WhereClause SolutionModifier
    let ask_query = keyword("ASK")
        .ignore_then(dataset_clause.clone().repeated().collect())
        .then(where_clause.clone())
        .then(solution_modifier.clone())
        .map(
            |((dataset_clause, where_clause), solution_modifier)| AskQuery {
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        );

    // [13]   	DescribeQuery 	  ::=   	'DESCRIBE' ( VarOrIri+ | '*' ) DatasetClause* WhereClause? SolutionModifier
    let describe_query = keyword("DESCRIBE")
        .ignore_then(
            var_or_iri
                .spanned()
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>()
                .map(DescribeTargets::Explicit)
                .or(operator("*").to(DescribeTargets::Star))
                .spanned(),
        )
        .then(dataset_clause.clone().repeated().collect())
        .then(where_clause.clone().or_not())
        .then(solution_modifier.clone())
        .map(
            |(((targets, dataset_clause), where_clause), solution_modifier)| DescribeQuery {
                targets,
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        );

    // [12]   	ConstructQuery 	  ::=   	'CONSTRUCT' ( ConstructTemplate DatasetClause* WhereClause SolutionModifier | DatasetClause* 'WHERE' '{' TriplesTemplate? '}' SolutionModifier )
    let construct_query = keyword("CONSTRUCT").ignore_then(
        construct_template
            .then(dataset_clause.clone().repeated().collect())
            .then(where_clause.clone())
            .then(solution_modifier.clone())
            .map(
                |(((template, dataset_clause), where_clause), solution_modifier)| ConstructQuery {
                    template,
                    dataset_clause,
                    where_clause: Some(where_clause),
                    solution_modifier,
                },
            )
            .or(dataset_clause
                .clone()
                .repeated()
                .collect()
                .then_ignore(keyword("WHERE"))
                .then(
                    triples_template
                        .delimited_by(operator("{"), operator("}"))
                        .spanned(),
                )
                .then(solution_modifier.clone())
                .map(
                    |((dataset_clause, template), solution_modifier)| ConstructQuery {
                        template,
                        dataset_clause,
                        where_clause: None,
                        solution_modifier,
                    },
                )),
    );

    // [11]   	SelectClause 	  ::=   	'SELECT' ( 'DISTINCT' | 'REDUCED' )? ( ( Var | ( '(' Expression 'AS' Var ')' ) )+ | '*' )
    let select_clause = keyword("SELECT")
        .ignore_then(
            keyword("DISTINCT")
                .to(SelectionOption::Distinct)
                .or(keyword("REDUCED").to(SelectionOption::Reduced))
                .or_not()
                .map(|s| s.unwrap_or(SelectionOption::Default)),
        )
        .then(
            var.map(|v| (None, v))
                .or(expression
                    .clone()
                    .map(Some)
                    .then_ignore(keyword("AS"))
                    .then(var)
                    .delimited_by(operator("("), operator(")")))
                .spanned()
                .repeated()
                .at_least(1)
                .collect()
                .map(SelectVariables::Explicit)
                .or(operator("*").to(SelectVariables::Star))
                .spanned(),
        )
        .map(|(option, bindings)| SelectClause { option, bindings });

    // [10]   	SubSelect 	  ::=   	SelectClause WhereClause SolutionModifier ValuesClause
    sub_select.define(
        select_clause
            .clone()
            .then(where_clause.clone())
            .then(solution_modifier.clone())
            .then(values_clause.clone())
            .map(
                |(((select_clause, where_clause), solution_modifier), values_clause)| {
                    GraphPattern::SubSelect(Box::new(SubSelect {
                        select_clause,
                        where_clause,
                        solution_modifier,
                        values_clause,
                    }))
                },
            ),
    );

    // [9]   	SelectQuery 	  ::=   	SelectClause DatasetClause* WhereClause SolutionModifier
    let select_query = select_clause
        .then(dataset_clause.repeated().collect())
        .then(where_clause)
        .then(solution_modifier)
        .map(
            |(((select_clause, dataset_clause), where_clause), solution_modifier)| SelectQuery {
                select_clause,
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        );

    // [2]   	Query 	  ::=   	Prologue ( SelectQuery | ConstructQuery | DescribeQuery | AskQuery ) ValuesClause
    let query = prologue
        .then(choice((
            select_query.map(QueryQuery::Select),
            construct_query.map(QueryQuery::Construct),
            describe_query.map(QueryQuery::Describe),
            ask_query.map(QueryQuery::Ask),
        )))
        .then(values_clause)
        .map(|((prologue, query), values_clause)| Query {
            prologue,
            variant: query,
            values_clause,
        });

    (query.boxed(), update.boxed())
}

fn keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>>>> + Clone {
    select! {
        Token::Keyword(v) if v.eq_ignore_ascii_case(keyword) => ()
    }
    .labelled(keyword)
}

fn case_sensitive_keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>>>> + Clone {
    just(Token::Keyword(keyword)).ignored()
}

fn operator<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    op: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>>>> + Clone {
    just(Token::Operator(op)).ignored()
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

enum AggregateFunction {
    Count,
    Sum,
    Min,
    Max,
    Avg,
    Sample,
}
