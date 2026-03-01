use crate::ast::*;
use crate::lexer::Token;
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use std::str::FromStr;

pub fn parse_sparql_query<'a>(
    tokens: &'a [Token<'a>],
) -> Result<Query<'a>, Vec<Rich<'a, Token<'a>>>> {
    build_parser().parse(tokens).into_result()
}

// TODO: upstream
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
    ($($p:expr => $out:expr),+) => ({
        select_with_attr! {
            $(Token::Keyword(v) if v.eq_ignore_ascii_case($p) => $out),+
        }
    });
}

fn build_parser<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>()
-> impl Parser<'src, I, Query<'src>, extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> {
    let iriref = select! { Token::IriRef(i) => IriRef(i) }.labelled("an iri");
    let pname_ns = select! { Token::PnameNs(p) => p }.labelled("a prefix");
    let nil = operator("(").then_ignore(operator(")"));
    let anon = operator("[").then_ignore(operator("]"));

    // [158]   	BlankNode 	  ::=   	BLANK_NODE_LABEL | ANON
    let blank_node = select! {
        Token::BlankNodeLabel(id) => BlankNode(Some(id)),
    }
    .or(anon.map(|()| BlankNode(None)))
    .labelled("a blank node");

    // [157]   	PrefixedName 	  ::=   	PNAME_LN | PNAME_NS
    let prefixed_name = select! {
        Token::PnameNs(p) => PrefixedName(p, ""),
        Token::PnameLn(p, v) => PrefixedName(p, v)
    }
    .labelled("a prefixed name");

    // [156]   	iri 	  ::=   	IRIREF | PrefixedName
    let iri = iriref
        .map(Iri::IriRef)
        .or(prefixed_name.map(Iri::PrefixedName));

    // [155]   	String 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2 | STRING_LITERAL_LONG1 | STRING_LITERAL_LONG2
    let string = select! {
        Token::StringLiteral1(s) | Token::StringLiteral2(s) | Token::StringLiteralLong1(s) | Token::StringLiteralLong2(s) => String(s),
    }
    .labelled("a string literal");

    // [154]   	BooleanLiteral 	  ::=   	'true' | 'false'
    let boolean_literal = case_sensitive_keyword("true")
        .map(|()| Literal::Boolean(true))
        .or(case_sensitive_keyword("false").map(|()| Literal::Boolean(false)));

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

    // [149]   	RDFLiteral 	  ::=   	String ( LANG_DIR | '^^' iri )?
    let rdf_literal = string
        .then(
            {
                #[cfg(feature = "sparql-12")]
                {
                    select! {
                        Token::LangDir(l, r) => Either::Left((l, r)),
                    }
                }
                #[cfg(not(feature = "sparql-12"))]
                {
                    select! {
                        Token::LangDir(l) => Either::Left(l),
                    }
                }
            }
            .labelled("a language tag")
            .or(operator("^^").ignore_then(iri.clone()).map(Either::Right))
            .or_not(),
        )
        .map(|(string, extra)| match extra {
            #[cfg(feature = "sparql-12")]
            Some(Either::Left((l, d))) => {
                if let Some(d) = d {
                    Literal::DirLangString(string, l, d)
                } else {
                    Literal::LangString(string, l)
                }
            }
            #[cfg(not(feature = "sparql-12"))]
            Some(Either::Left(l)) => Literal::LangString(string, l),
            Some(Either::Right(t)) => Literal::Typed(string, t),
            None => Literal::String(string),
        });

    let mut expression = Recursive::declare();

    // [77]   	ArgList 	  ::=   	NIL | '(' 'DISTINCT'? Expression ( ',' Expression )* ')'
    let arg_list = keyword("DISTINCT")
        .or_not()
        .then(
            expression
                .clone()
                .separated_by(operator(","))
                .at_least(1)
                .collect(),
        )
        .delimited_by(operator("("), operator(")"))
        .map(|(distinct, args)| ArgList {
            distinct: distinct.is_some(),
            args,
        });

    // [148]   	iriOrFunction 	  ::=   	iri ArgList?
    let iri_or_function = iri
        .clone()
        .then(arg_list.clone().or_not())
        .map(|(name, args)| {
            if let Some(args) = args {
                Expression::Function(name, args)
            } else {
                Expression::Iri(name)
            }
        });

    // [147]   	Aggregate 	  ::=   	  'COUNT' '(' 'DISTINCT'? ( '*' | Expression ) ')' | 'SUM' '(' 'DISTINCT'? Expression ')' | 'MIN' '(' 'DISTINCT'? Expression ')' | 'MAX' '(' 'DISTINCT'? Expression ')' | 'AVG' '(' 'DISTINCT'? Expression ')' | 'SAMPLE' '(' 'DISTINCT'? Expression ')' | 'GROUP_CONCAT' '(' 'DISTINCT'? Expression ( ';' 'SEPARATOR' '=' String )? ')'
    enum AggregateFunction {
        Count,
        Sum,
        Min,
        Max,
        Avg,
        Sample,
    }
    let aggregate = keyword("COUNT")
        .ignore_then(keyword("DISTINCT").or_not())
        .then_ignore(operator("*"))
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
        .then(keyword("DISTINCT").or_not())
        .then(
            expression
                .clone()
                .delimited_by(operator("("), operator(")")),
        )
        .map(|((name, distinct), expr)| match name {
            AggregateFunction::Count => Aggregate::Count(distinct.is_some(), Some(Box::new(expr))),
            AggregateFunction::Sum => Aggregate::Sum(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Min => Aggregate::Min(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Max => Aggregate::Max(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Avg => Aggregate::Avg(distinct.is_some(), Box::new(expr)),
            AggregateFunction::Sample => Aggregate::Sample(distinct.is_some(), Box::new(expr)),
        }))
        .or(keyword("GROUP_CONCAT")
            .ignore_then(keyword("DISTINCT").or_not())
            .then(
                expression
                    .clone()
                    .then(
                        operator(";")
                            .ignore_then(keyword("SEPARATOR"))
                            .ignore_then(operator("="))
                            .ignore_then(string.clone())
                            .or_not(),
                    )
                    .delimited_by(operator("("), operator(")")),
            )
            .map(|(distinct, (expr, separator))| {
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
        Token::Var1(v) => Var(v),
        Token::Var2(v) => Var(v),
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
            "URI" => BuiltInName::Iri,
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
            "isURI" => BuiltInName::IsIri,
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
        .boxed();

    // [140]   	BrackettedExpression 	  ::=   	'(' Expression ')'
    let bracketted_expression = expression
        .clone()
        .delimited_by(operator("("), operator(")"));

    // [136]   	PrimaryExpression 	  ::=   	BrackettedExpression | BuiltInCall | iriOrFunction | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
    let primary_expression = bracketted_expression
        .clone()
        .or(built_in_call.clone())
        .or(iri_or_function)
        .or(rdf_literal.clone().map(Expression::Literal))
        .or(numeric_literal.clone().map(Expression::Literal))
        .or(boolean_literal.clone().map(Expression::Literal))
        .or(var.map(Expression::Var))
        .boxed(); // TODO ExprTripleTerm

    expression.define(
        primary_expression
            .boxed()
            .pratt((
                // [127]   	Expression 	  ::=   	ConditionalOrExpression

                // [128]   	ConditionalOrExpression 	  ::=   	ConditionalAndExpression ( '||' ConditionalAndExpression )*
                infix(left(1), operator("||"), |l, _, r, _| {
                    Expression::Or(Box::new(l), Box::new(r))
                }),
                // [129]   	ConditionalAndExpression 	  ::=   	ValueLogical ( '&&' ValueLogical )*
                infix(left(2), operator("&&"), |l, _, r, _| {
                    Expression::And(Box::new(l), Box::new(r))
                }),
                // [130]   	ValueLogical 	  ::=   	RelationalExpression
                // [131]   	RelationalExpression 	  ::=   	NumericExpression ( '=' NumericExpression | '!=' NumericExpression | '<' NumericExpression | '>' NumericExpression | '<=' NumericExpression | '>=' NumericExpression | 'IN' ExpressionList | 'NOT' 'IN' ExpressionList )?
                infix(left(3), operator("="), |l, _, r, _| {
                    Expression::Equal(Box::new(l), Box::new(r))
                }),
                infix(left(3), operator("!="), |l, _, r, _| {
                    Expression::Not(Box::new(Expression::Equal(Box::new(l), Box::new(r))))
                }),
                infix(left(3), operator("<"), |l, _, r, _| {
                    Expression::Less(Box::new(l), Box::new(r))
                }),
                infix(left(3), operator(">"), |l, _, r, _| {
                    Expression::Greater(Box::new(l), Box::new(r))
                }),
                infix(left(3), operator("<="), |l, _, r, _| {
                    Expression::LessOrEqual(Box::new(l), Box::new(r))
                }),
                infix(left(3), operator("=>"), |l, _, r, _| {
                    Expression::GreaterOrEqual(Box::new(l), Box::new(r))
                }),
                postfix(
                    3,
                    keyword("IN").ignore_then(expression_list.clone()),
                    |l, r, _| Expression::In(Box::new(l), r),
                ),
                postfix(
                    3,
                    keyword("NOT")
                        .ignore_then(keyword("IN"))
                        .ignore_then(expression_list.clone()),
                    |l, r, _| Expression::NotIn(Box::new(l), r),
                ),
                // [132]   	NumericExpression 	  ::=   	AdditiveExpression
                // [133]   	AdditiveExpression 	  ::=   	MultiplicativeExpression ( '+' MultiplicativeExpression | '-' MultiplicativeExpression | ( NumericLiteralPositive | NumericLiteralNegative ) ( ( '*' UnaryExpression ) | ( '/' UnaryExpression ) )* )*
                infix(left(4), operator("+"), |l, _, r, _| {
                    Expression::Add(Box::new(l), Box::new(r))
                }),
                infix(left(4), operator("-"), |l, _, r, _| {
                    Expression::Subtract(Box::new(l), Box::new(r))
                }),
                // [134]   	MultiplicativeExpression 	  ::=   	UnaryExpression ( '*' UnaryExpression | '/' UnaryExpression )*
                infix(left(5), operator("*"), |l, _, r, _| {
                    Expression::Multiply(Box::new(l), Box::new(r))
                }),
                infix(left(5), operator("/"), |l, _, r, _| {
                    Expression::Divide(Box::new(l), Box::new(r))
                }),
                // [135]   	UnaryExpression 	  ::=   	  '!' UnaryExpression | '+' PrimaryExpression | '-' PrimaryExpression | PrimaryExpression
                prefix(
                    6,
                    one_of([
                        Token::Operator("!"),
                        Token::Operator("+"),
                        Token::Operator("-"),
                    ]),
                    |o, a, _| match o {
                        Token::Operator("!") => Expression::Not(Box::new(a)),
                        Token::Operator("+") => Expression::UnaryPlus(Box::new(a)),
                        Token::Operator("-") => Expression::UnaryMinus(Box::new(a)),
                        _ => unreachable!(),
                    },
                ),
            ))
            .boxed(),
    );

    // [125]   	VarOrIri 	  ::=   	Var | iri
    let var_or_iri = var.map(VarOrIri::Var).or(iri.clone().map(VarOrIri::Iri));

    // [115]   	VarOrTerm 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | NIL | TripleTerm
    let var_or_term = var
        .map(VarOrTerm::Var)
        .or(iri.clone().map(VarOrTerm::Iri))
        .or(rdf_literal.clone().map(VarOrTerm::Literal))
        .or(numeric_literal.map(VarOrTerm::Literal))
        .or(boolean_literal.clone().map(VarOrTerm::Literal))
        .or(blank_node.clone().map(VarOrTerm::BlankNode))
        .or(nil.clone().map(|()| VarOrTerm::Nil))
        .boxed();

    // [114]   	GraphNodePath 	  ::=   	VarOrTerm | TriplesNodePath | ReifiedTriple
    let mut triples_node_path = Recursive::declare();
    let graph_node_path = var_or_term
        .clone()
        .map(GraphNodePath::VarOrTerm)
        .or(triples_node_path.clone()); // TODO: ReifiedTriple

    // [113]   	GraphNode 	  ::=   	VarOrTerm | TriplesNode | ReifiedTriple
    let mut triples_node = Recursive::declare();
    let graph_node = var_or_term
        .clone()
        .map(GraphNode::VarOrTerm)
        .or(triples_node.clone()); // TODO: ReifiedTriple

    // [71]   	VarOrReifierId 	  ::=   	Var | iri | BlankNode
    #[cfg(feature = "sparql-12")]
    let var_or_reifier_id = var
        .map(VarOrReifierId::Var)
        .or(iri.map(VarOrReifierId::Iri))
        .or(blank_node.map(VarOrReifierId::BlankNode));

    // [70]   	Reifier 	  ::=   	'~' VarOrReifierId?
    #[cfg(feature = "sparql-12")]
    let reifier = operator("~").ignore_then(var_or_reifier_id.or_not());

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
        let path_one_in_property_set = iri
            .clone()
            .map(PathOneInPropertySet::Iri)
            .or(case_sensitive_keyword("a").map(|()| PathOneInPropertySet::A))
            .or(operator("^").ignore_then(
                iri.clone()
                    .map(PathOneInPropertySet::InverseIri)
                    .or(case_sensitive_keyword("a").map(|()| PathOneInPropertySet::InverseA)),
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
        let path_primary = iri
            .clone()
            .map(Path::Iri)
            .or(case_sensitive_keyword("a").map(|()| Path::A))
            .or(operator("!").ignore_then(path_negated_property_set))
            .or(path.delimited_by(operator("("), operator(")")));

        // [94]   	Path 	  ::=   	PathAlternative
        // [95]   	PathAlternative 	  ::=   	PathSequence ( '|' PathSequence )*
        // [96]   	PathSequence 	  ::=   	PathEltOrInverse ( '/' PathEltOrInverse )*
        // [97]   	PathElt 	  ::=   	PathPrimary PathMod?
        // [98]   	PathEltOrInverse 	  ::=   	PathElt | '^' PathElt
        // [99]   	PathMod 	  ::=   	'?' | '*' | '+'
        path_primary
            .pratt((
                infix(left(1), operator("|"), |l, _, r, _| {
                    Path::Alternative(Box::new(l), Box::new(r))
                }),
                infix(left(2), operator("/"), |l, _, r, _| {
                    Path::Sequence(Box::new(l), Box::new(r))
                }),
                prefix(3, operator("^"), |_, e, _| Path::Inverse(Box::new(e))),
                postfix(4, operator("?"), |e, _, _| Path::ZeroOrOne(Box::new(e))),
                postfix(4, operator("*"), |e, _, _| Path::ZeroOrMore(Box::new(e))),
                postfix(4, operator("+"), |e, _, _| Path::OneOrMore(Box::new(e))),
            ))
            .boxed()
    });

    // [93]   	ObjectPath 	  ::=   	GraphNodePath AnnotationPath
    #[cfg(feature = "sparql-12")]
    let object_path = graph_node_path
        .then(annotation_path)
        .map(|(graph_node, annotation)| ObjectPath {
            graph_node,
            annotation,
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
    property_list_path_not_empty.define(
        verb_simple
            .or(verb_path)
            .then(object_list_path)
            .separated_by(operator(";"))
            .allow_trailing()
            .at_least(1)
            .collect(),
    );

    // [88]   	PropertyListPath 	  ::=   	PropertyListPathNotEmpty?
    let property_list_path = property_list_path_not_empty
        .clone()
        .or_not()
        .map(Option::unwrap_or_default);

    // [87]   	TriplesSameSubjectPath 	  ::=   	VarOrTerm PropertyListPathNotEmpty | TriplesNodePath PropertyListPath | ReifiedTripleBlockPath
    let triples_same_subject_path = var_or_term
        .clone()
        .map(GraphNodePath::VarOrTerm)
        .then(property_list_path_not_empty)
        .or(triples_node_path.then(property_list_path)); // TODO ReifiedTripleBlockPath

    // [86]   	Object 	  ::=   	GraphNode Annotation
    #[cfg(feature = "sparql-12")]
    let object = graph_node
        .then(annotation)
        .map(|(graph_node, annotation)| Object {
            graph_node,
            annotation,
        })
        .boxed();
    #[cfg(not(feature = "sparql-12"))]
    let object = graph_node.map(|graph_node| Object { graph_node }).boxed();

    // [85]   	ObjectList 	  ::=   	Object ( ',' Object )*
    let object_list = object.separated_by(operator(",")).at_least(1).collect();

    // [84]   	Verb 	  ::=   	VarOrIri | 'a'
    let verb = var_or_iri
        .clone()
        .map(|v| match v {
            VarOrIri::Var(v) => Verb::Var(v),
            VarOrIri::Iri(v) => Verb::Iri(v),
        })
        .or(case_sensitive_keyword("a").map(|()| Verb::A));

    // [83]   	PropertyListNotEmpty 	  ::=   	Verb ObjectList ( ';' ( Verb ObjectList )? )*
    property_list_not_empty.define(
        verb.then(object_list)
            .separated_by(operator(";"))
            .allow_trailing()
            .at_least(1)
            .collect(),
    );

    // [82]   	PropertyList 	  ::=   	PropertyListNotEmpty?
    let property_list = property_list_not_empty
        .clone()
        .or_not()
        .map(Option::unwrap_or_default);

    // [81]   	TriplesSameSubject 	  ::=   	VarOrTerm PropertyListNotEmpty | TriplesNode PropertyList | ReifiedTripleBlock
    let triples_same_subject = var_or_term
        .clone()
        .map(GraphNode::VarOrTerm)
        .then(property_list_not_empty)
        .or(triples_node.then(property_list)); // TODO ReifiedTripleBlockPath

    // [80]   	ConstructTriples 	  ::=   	TriplesSameSubject ( '.' ConstructTriples? )?
    // also TriplesSameSubject ("." TriplesSameSubject?)*
    // [79]   	ConstructTemplate 	  ::=   	'{' ConstructTriples? '}'
    let construct_template = triples_same_subject
        .clone()
        .separated_by(operator("."))
        .allow_trailing()
        .collect()
        .delimited_by(operator("{"), operator("}"));

    // [76]   	FunctionCall 	  ::=   	iri ArgList
    let function_call = iri
        .clone()
        .then(arg_list)
        .map(|(name, args)| Expression::Function(name, args));

    // [75]   	Constraint 	  ::=   	BrackettedExpression | BuiltInCall | FunctionCall
    let constraint = bracketted_expression
        .clone()
        .or(built_in_call.clone())
        .or(function_call.clone());

    // [74]   	Filter 	  ::=   	'FILTER' Constraint
    let filter = keyword("filter")
        .ignore_then(constraint.clone())
        .map(GraphPattern::Filter);

    // [73]   	GroupOrUnionGraphPattern 	  ::=   	GroupGraphPattern ( 'UNION' GroupGraphPattern )*
    let group_or_union_graph_pattern = group_graph_pattern
        .clone()
        .separated_by(keyword("union"))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|p| {
            if p.len() == 1 {
                p.into_iter().next().unwrap()
            } else {
                GraphPattern::Union(p)
            }
        });

    // [72]   	MinusGraphPattern 	  ::=   	'MINUS' GroupGraphPattern
    let minus_graph_pattern = keyword("minus")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| GraphPattern::Minus(Box::new(p)));

    // [69]   	DataBlockValue 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | 'UNDEF' | TripleTermData
    let data_block_value = iri
        .clone()
        .map(DataBlockValue::Iri)
        .or(rdf_literal.clone().map(DataBlockValue::Literal))
        .or(numeric_literal.map(DataBlockValue::Literal))
        .or(boolean_literal.clone().map(DataBlockValue::Literal))
        .or(keyword("undef").map(|()| DataBlockValue::Undef)); // TODO: TripleTermData

    // [68]   	InlineDataFull 	  ::=   	( NIL | '(' Var* ')' ) '{' ( '(' DataBlockValue* ')' | NIL )* '}'
    let inline_data_full = nil
        .clone()
        .map(|()| Vec::new())
        .or(var
            .clone()
            .repeated()
            .collect()
            .delimited_by(operator("("), operator(")")))
        .then(
            nil.map(|()| Vec::new())
                .or(data_block_value
                    .clone()
                    .repeated()
                    .collect()
                    .delimited_by(operator("("), operator(")")))
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(operator("{"), operator("}")),
        );

    // [67]   	InlineDataOneVar 	  ::=   	Var '{' DataBlockValue* '}'
    let inline_data_one_var = var.map(|v| vec![v]).then(
        data_block_value
            .map(|v| vec![v])
            .repeated()
            .collect()
            .delimited_by(operator("{"), operator("}")),
    );

    // [66]   	DataBlock 	  ::=   	InlineDataOneVar | InlineDataFull
    let data_block = inline_data_one_var.or(inline_data_full);

    // [65]   	InlineData 	  ::=   	'VALUES' DataBlock
    let inline_data = keyword("values")
        .ignore_then(data_block.clone())
        .map(|(variables, values)| GraphPattern::Values { variables, values });

    // [64]   	Bind 	  ::=   	'BIND' '(' Expression 'AS' Var ')'
    let bind = keyword("bind")
        .ignore_then(
            expression
                .clone()
                .then_ignore(keyword("as"))
                .then(var)
                .delimited_by(operator("("), operator(")")),
        )
        .map(|(e, v)| GraphPattern::Bind(e, v));

    // [63]   	ServiceGraphPattern 	  ::=   	'SERVICE' 'SILENT'? VarOrIri GroupGraphPattern
    let service_graph_pattern = keyword("service")
        .ignore_then(keyword("silent").or_not())
        .then(var_or_iri.clone())
        .then(group_graph_pattern.clone())
        .map(|((silent, name), pattern)| GraphPattern::Service {
            silent: silent.is_some(),
            name,
            pattern: Box::new(pattern),
        });

    // [62]   	GraphGraphPattern 	  ::=   	'GRAPH' VarOrIri GroupGraphPattern
    let graph_graph_pattern = keyword("graph")
        .ignore_then(var_or_iri.clone())
        .then(group_graph_pattern.clone())
        .map(|(name, pattern)| GraphPattern::Graph {
            name,
            pattern: Box::new(pattern),
        });

    // [61]   	OptionalGraphPattern 	  ::=   	'OPTIONAL' GroupGraphPattern
    let optional_graph_pattern = keyword("optional")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| GraphPattern::Optional(Box::new(p)));

    // [60]   	GraphPatternNotTriples 	  ::=   	GroupOrUnionGraphPattern | OptionalGraphPattern | MinusGraphPattern | GraphGraphPattern | ServiceGraphPattern | Filter | Bind | InlineData
    let graph_pattern_not_triples = group_or_union_graph_pattern
        .or(optional_graph_pattern)
        .or(minus_graph_pattern)
        .or(graph_graph_pattern)
        .or(service_graph_pattern)
        .or(filter)
        .or(bind)
        .or(inline_data);

    // [57]   	TriplesBlock 	  ::=   	TriplesSameSubjectPath ( '.' TriplesBlock? )?
    // also TriplesSameSubjectPath ( '.' TriplesSameSubjectPath? )*
    let triples_block = triples_same_subject_path
        .separated_by(operator("."))
        .allow_trailing()
        .at_least(1)
        .collect()
        .map(GraphPattern::Triples)
        .boxed();

    // [56]   	GroupGraphPatternSub 	  ::=   	TriplesBlock? ( GraphPatternNotTriples '.'? TriplesBlock? )*
    let group_graph_pattern_sub = triples_block
        .clone()
        .or_not()
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .foldl(
            graph_pattern_not_triples
                .then_ignore(operator(".").or_not())
                .then(triples_block.or_not())
                .repeated(),
            |mut a, (b, c)| {
                a.push(b);
                if let Some(c) = c {
                    a.push(c);
                }
                a
            },
        )
        .map(GraphPattern::Group);

    // [55]   	GroupGraphPattern 	  ::=   	'{' ( SubSelect | GroupGraphPatternSub ) '}'
    group_graph_pattern.define(
        group_graph_pattern_sub.delimited_by(operator("{"), operator("}")), // TODO: SubSelect
    );

    // [54]   	TriplesTemplate 	  ::=   	TriplesSameSubject ( '.' TriplesTemplate? )?
    let triples_template = triples_same_subject
        .separated_by(operator("."))
        .allow_trailing()
        .at_least(1)
        .collect();

    // [30]   	ValuesClause 	  ::=   	( 'VALUES' DataBlock )?
    let values_clause = keyword("values")
        .ignore_then(data_block)
        .map(|(variables, values)| ValuesClause { variables, values })
        .or_not()
        .boxed();

    // [29]   	OffsetClause 	  ::=   	'OFFSET' INTEGER
    let offset_clause = keyword("offset")
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
    let limit_clause = keyword("limit")
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
    let order_condition = keyword("asc")
        .ignore_then(bracketted_expression.clone())
        .map(OrderCondition::Asc)
        .or(keyword("desc")
            .ignore_then(bracketted_expression)
            .map(OrderCondition::Desc))
        .or(constraint.clone().map(OrderCondition::Asc))
        .or(var.clone().map(|v| OrderCondition::Asc(Expression::Var(v))));

    // [25]   	OrderClause 	  ::=   	'ORDER' 'BY' OrderCondition+
    let order_clause = keyword("order")
        .ignore_then(keyword("by"))
        .ignore_then(order_condition.repeated().at_least(1).collect::<Vec<_>>());

    // [24]   	HavingCondition 	  ::=   	Constraint
    let having_condition = constraint;

    // [23]   	HavingClause 	  ::=   	'HAVING' HavingCondition+
    let having_clause =
        keyword("having").ignore_then(having_condition.repeated().at_least(1).collect());

    // [22]   	GroupCondition 	  ::=   	BuiltInCall | FunctionCall | '(' Expression ( 'AS' Var )? ')' | Var
    let group_condition = built_in_call
        .map(|e| (e, None))
        .or(function_call.map(|e| (e, None)))
        .or(expression
            .clone()
            .then(keyword("as").ignore_then(var).or_not()))
        .or(var.map(|e| (Expression::Var(e), None)));

    // [21]   	GroupClause 	  ::=   	'GROUP' 'BY' GroupCondition+
    let group_clause = keyword("group")
        .ignore_then(keyword("by"))
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
    let where_clause = keyword("where")
        .or_not()
        .ignore_then(group_graph_pattern)
        .boxed();

    // [18]   	SourceSelector 	  ::=   	iri
    let source_selector = iri;

    // [17]   	NamedGraphClause 	  ::=   	'NAMED' SourceSelector
    let named_graph_clause = keyword("named")
        .ignore_then(source_selector.clone())
        .map(GraphClause::Named);

    // [16]   	DefaultGraphClause 	  ::=   	SourceSelector
    let default_graph_clause = source_selector.map(GraphClause::Default);

    // [15]   	DatasetClause 	  ::=   	'FROM' ( DefaultGraphClause | NamedGraphClause )
    let dataset_clause = keyword("from").ignore_then(default_graph_clause.or(named_graph_clause));

    // [14]   	AskQuery 	  ::=   	'ASK' DatasetClause* WhereClause SolutionModifier
    let ask_query = keyword("ask")
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
    let describe_query = keyword("describe")
        .ignore_then(
            var_or_iri
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>()
                .or(operator("*").map(|()| Vec::new())),
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
    let construct_query = keyword("construct")
        .ignore_then(
            construct_template
                .then(dataset_clause.clone().repeated().collect())
                .then(where_clause.clone())
                .then(solution_modifier.clone())
                .map(
                    |(((template, dataset_clause), where_clause), solution_modifier)| {
                        ConstructQuery {
                            template,
                            dataset_clause,
                            where_clause: Some(where_clause),
                            solution_modifier,
                        }
                    },
                ),
        )
        .or(dataset_clause
            .clone()
            .repeated()
            .collect()
            .then_ignore(keyword("where"))
            .then(
                triples_template
                    .or_not()
                    .delimited_by(operator("{"), operator("}")),
            )
            .then(solution_modifier.clone())
            .map(
                |((dataset_clause, template), solution_modifier)| ConstructQuery {
                    template: template.unwrap_or_default(),
                    dataset_clause,
                    where_clause: None,
                    solution_modifier,
                },
            ));

    // [11]   	SelectClause 	  ::=   	'SELECT' ( 'DISTINCT' | 'REDUCED' )? ( ( Var | ( '(' Expression 'AS' Var ')' ) )+ | '*' )
    let select_clause = keyword("SELECT")
        .ignore_then(
            keyword("DISTINCT")
                .map(|()| SelectionOption::Distinct)
                .or(keyword("REDUCED").map(|()| SelectionOption::Reduced))
                .or_not()
                .map(|s| s.unwrap_or(SelectionOption::Default)),
        )
        .then(
            var.clone()
                .map(|v| (None, v))
                .or(expression
                    .clone()
                    .map(Some)
                    .then_ignore(keyword("AS"))
                    .then(var.clone())
                    .delimited_by(operator("("), operator(")")))
                .repeated()
                .at_least(1)
                .collect()
                .or(operator("*").map(|()| Vec::new())),
        )
        .map(|(option, bindings)| SelectClause { option, bindings });

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

    // [8]   	VersionSpecifier 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2
    #[cfg(feature = "sparql-12")]
    let version_specifier = select! {
        Token::StringLiteral1(v) | Token::StringLiteral2(v) => v
    }
    .labelled("a string");

    // [7]   	VersionDecl 	  ::=   	'VERSION' VersionSpecifier
    #[cfg(feature = "sparql-12")]
    let version_decl = keyword("version")
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
    let prologue_decl = base_decl.or(prefix_decl);
    #[cfg(feature = "sparql-12")]
    let prologue_decl = prologue_decl.or(version_decl);
    let prologue = prologue_decl.repeated().collect();

    // [2]   	Query 	  ::=   	Prologue ( SelectQuery | ConstructQuery | DescribeQuery | AskQuery ) ValuesClause
    let query = prologue
        .then(
            select_query
                .map(QueryQuery::Select)
                .or(construct_query.map(QueryQuery::Construct))
                .or(describe_query.map(QueryQuery::Describe))
                .or(ask_query.map(QueryQuery::Ask)),
        )
        .then(values_clause)
        .map(|((prologue, query), values_clause)| Query {
            prologue,
            query,
            values_clause,
        });

    query
}

fn keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    select! {
        Token::Keyword(v) if v.eq_ignore_ascii_case(keyword) => ()
    }
    .labelled(keyword)
}

fn case_sensitive_keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    select! {
        Token::Keyword(v) if v == keyword => ()
    }
    .labelled(keyword)
}

fn operator<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    op: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    just(Token::Operator(op)).ignored().labelled(op)
}

#[cfg(feature = "sparql-12")]
enum Either<L, R> {
    Left(L),
    Right(R),
}
