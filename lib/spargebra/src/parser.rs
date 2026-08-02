use crate::ast::*;
use crate::lexer::Token;
use chumsky::error::{Error, LabelError};
use chumsky::input::MappedInput;
use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::span::WrappingSpan;

type ParserInput<'src> = MappedInput<'src, Token<'src>, SimpleSpan, &'src [Spanned<Token<'src>>]>;

trait CParserError<'src>:
    Error<'src, ParserInput<'src>> + LabelError<'src, ParserInput<'src>, &'static str> + 'src
{
}

impl<
    'src,
    E: Error<'src, ParserInput<'src>> + LabelError<'src, ParserInput<'src>, &'static str> + 'src,
> CParserError<'src> for E
{
}

pub fn parse_sparql_query<'a>(
    tokens: &'a [Spanned<Token<'a>>],
    input_len: usize,
) -> Result<Query<'a>, Vec<Rich<'a, Token<'a>>>> {
    let input = || tokens.split_spanned((input_len..input_len).into());
    if let Ok(query) = query::<EmptyErr>().parse(input()).into_result() {
        return Ok(query);
    }
    query()
        .parse(tokens.split_spanned((input_len..input_len).into()))
        .into_result()
}

pub fn parse_sparql_update<'a>(
    tokens: &'a [Spanned<Token<'a>>],
    input_len: usize,
) -> Result<Update<'a>, Vec<Rich<'a, Token<'a>>>> {
    let input = || tokens.split_spanned((input_len..input_len).into());
    if let Ok(update) = update::<EmptyErr>().parse(input()).into_result() {
        return Ok(update);
    }
    update().parse(input()).into_result()
}

macro_rules! select_keyword {
    ($($(#[$attr:meta])? $p:literal => $out:expr),+) => ({
        select! {
            $($(#[$attr])? Token::Keyword(v) if v.eq_ignore_ascii_case($p) => $out),+
        }
    });
}

// Trait alias for chumsky::Parser with the relevant args
trait CParser<'src, O, E>: Parser<'src, ParserInput<'src>, O, extra::Err<E>> + Clone + 'src
where
    E: CParserError<'src>,
{
}

impl<
    'src,
    O,
    E: CParserError<'src>,
    P: Parser<'src, ParserInput<'src>, O, extra::Err<E>> + Clone + 'src,
> CParser<'src, O, E> for P
{
}

// [2]   	Query 	  ::=   	Prologue ( SelectQuery | ConstructQuery | DescribeQuery | AskQuery ) ValuesClause
fn query<'src, E: CParserError<'src>>() -> impl CParser<'src, Query<'src>, E> {
    let group_graph_pattern = group_graph_pattern(sub_select());
    let expression = expression(group_graph_pattern.clone());
    prologue()
        .then(choice((
            select_query(expression.clone(), group_graph_pattern.clone()).map(QueryQuery::Select),
            construct_query(expression.clone(), group_graph_pattern.clone())
                .map(QueryQuery::Construct),
            describe_query(expression.clone(), group_graph_pattern.clone())
                .map(QueryQuery::Describe),
            ask_query(expression, group_graph_pattern).map(QueryQuery::Ask),
        )))
        .then(values_clause())
        .map(|((prologue, query), values_clause)| Query {
            prologue,
            variant: query,
            values_clause,
        })
        .boxed()
}

// [4]   	Prologue 	  ::=   	( BaseDecl | PrefixDecl | VersionDecl )*
fn prologue<'src, E: CParserError<'src>>() -> impl CParser<'src, Vec<PrologueDecl<'src>>, E> {
    choice((
        base_decl(),
        prefix_decl(),
        #[cfg(feature = "sparql-12")]
        version_decl(),
    ))
    .repeated()
    .collect::<Vec<_>>()
    .boxed()
}

// [5]   	BaseDecl 	  ::=   	'BASE' IRIREF
fn base_decl<'src, E: CParserError<'src>>() -> impl CParser<'src, PrologueDecl<'src>, E> {
    keyword("BASE")
        .ignore_then(iriref())
        .map(PrologueDecl::Base)
        .boxed()
}

// [6]   	PrefixDecl 	  ::=   	'PREFIX' PNAME_NS IRIREF
fn prefix_decl<'src, E: CParserError<'src>>() -> impl CParser<'src, PrologueDecl<'src>, E> {
    keyword("PREFIX")
        .ignore_then(pname_ns())
        .then(iriref())
        .map(|(prefix, iri)| PrologueDecl::Prefix(prefix, iri))
        .boxed()
}

// [7]   	VersionDecl 	  ::=   	'VERSION' VersionSpecifier
#[cfg(feature = "sparql-12")]
fn version_decl<'src, E: CParserError<'src>>() -> impl CParser<'src, PrologueDecl<'src>, E> {
    keyword("VERSION")
        .ignore_then(version_specifier())
        .map(PrologueDecl::Version)
        .boxed()
}

// [8]   	VersionSpecifier 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2
#[cfg(feature = "sparql-12")]
fn version_specifier<'src, E: CParserError<'src>>() -> impl CParser<'src, &'src str, E> {
    select! {
        Token::StringLiteral1(v) | Token::StringLiteral2(v) => &v[1..v.len() -1]
    }
    .labelled("a string")
    .boxed()
}

// [9]   	SelectQuery 	  ::=   	SelectClause DatasetClause* WhereClause SolutionModifier
fn select_query<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, SelectQuery<'src>, E> {
    select_clause(expression.clone())
        .then(dataset_clause().repeated().collect())
        .then(where_clause(group_graph_pattern.clone()))
        .then(solution_modifier(expression, group_graph_pattern))
        .map(
            |(((select_clause, dataset_clause), where_clause), solution_modifier)| SelectQuery {
                select_clause,
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        )
        .boxed()
}

// [10]   	SubSelect 	  ::=   	SelectClause WhereClause SolutionModifier ValuesClause
fn sub_select<'src, E: CParserError<'src>>() -> impl CParser<'src, SubSelect<'src>, E> {
    recursive(|sub_select| {
        let group_graph_pattern = group_graph_pattern(sub_select);
        let expression = expression(group_graph_pattern.clone());
        select_clause(expression.clone())
            .then(where_clause(group_graph_pattern.clone()))
            .then(solution_modifier(expression, group_graph_pattern))
            .then(values_clause())
            .map(
                |(((select_clause, where_clause), solution_modifier), values_clause)| SubSelect {
                    select_clause,
                    where_clause,
                    solution_modifier,
                    values_clause,
                },
            )
    })
    .boxed()
}

// [11]   	SelectClause 	  ::=   	'SELECT' ( 'DISTINCT' | 'REDUCED' )? ( ( Var | ( '(' Expression 'AS' Var ')' ) )+ | '*' )
fn select_clause<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, SelectClause<'src>, E> {
    keyword("SELECT")
        .ignore_then(
            keyword("DISTINCT")
                .to(SelectionOption::Distinct)
                .or(keyword("REDUCED").to(SelectionOption::Reduced))
                .or_not()
                .map(|s| s.unwrap_or(SelectionOption::Default)),
        )
        .then(
            var()
                .map(|v| (None, v))
                .or(expression
                    .map(Some)
                    .then_ignore(keyword("AS"))
                    .then(var())
                    .delimited_by(operator("("), operator(")")))
                .spanned()
                .repeated()
                .at_least(1)
                .collect()
                .map(SelectVariables::Explicit)
                .or(operator("*").to(SelectVariables::Star))
                .spanned(),
        )
        .map(|(option, bindings)| SelectClause { option, bindings })
        .boxed()
}

// [12]   	ConstructQuery 	  ::=   	'CONSTRUCT' ( ConstructTemplate DatasetClause* WhereClause SolutionModifier | DatasetClause* 'WHERE' '{' TriplesTemplate? '}' SolutionModifier )
fn construct_query<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, ConstructQuery<'src>, E> {
    let dataset_clause = dataset_clause();
    let solution_modifier = solution_modifier(expression, group_graph_pattern.clone());
    keyword("CONSTRUCT")
        .ignore_then(
            construct_template()
                .then(dataset_clause.clone().repeated().collect())
                .then(where_clause(group_graph_pattern.clone()))
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
                )
                .or(dataset_clause
                    .repeated()
                    .collect()
                    .then_ignore(keyword("WHERE"))
                    .then(
                        triples_template()
                            .delimited_by(operator("{"), operator("}"))
                            .spanned(),
                    )
                    .then(solution_modifier)
                    .map(
                        |((dataset_clause, template), solution_modifier)| ConstructQuery {
                            template,
                            dataset_clause,
                            where_clause: None,
                            solution_modifier,
                        },
                    )),
        )
        .boxed()
}

// [13]   	DescribeQuery 	  ::=   	'DESCRIBE' ( VarOrIri+ | '*' ) DatasetClause* WhereClause? SolutionModifier
fn describe_query<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, DescribeQuery<'src>, E> {
    keyword("DESCRIBE")
        .ignore_then(
            var_or_iri()
                .spanned()
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>()
                .map(DescribeTargets::Explicit)
                .or(operator("*").to(DescribeTargets::Star))
                .spanned(),
        )
        .then(dataset_clause().repeated().collect())
        .then(where_clause(group_graph_pattern.clone()).or_not())
        .then(solution_modifier(expression, group_graph_pattern))
        .map(
            |(((targets, dataset_clause), where_clause), solution_modifier)| DescribeQuery {
                targets,
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        )
        .boxed()
}

// [14]   	AskQuery 	  ::=   	'ASK' DatasetClause* WhereClause SolutionModifier
fn ask_query<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, AskQuery<'src>, E> {
    keyword("ASK")
        .ignore_then(dataset_clause().repeated().collect())
        .then(where_clause(group_graph_pattern.clone()))
        .then(solution_modifier(expression, group_graph_pattern))
        .map(
            |((dataset_clause, where_clause), solution_modifier)| AskQuery {
                dataset_clause,
                where_clause,
                solution_modifier,
            },
        )
        .boxed()
}

// [15]   	DatasetClause 	  ::=   	'FROM' ( DefaultGraphClause | NamedGraphClause )
fn dataset_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphClause<'src>, E> {
    keyword("FROM")
        .ignore_then(default_graph_clause().or(named_graph_clause()))
        .boxed()
}

// [16]   	DefaultGraphClause 	  ::=   	SourceSelector
fn default_graph_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphClause<'src>, E> {
    iri().map(GraphClause::Default).boxed()
}

// [17]   	NamedGraphClause 	  ::=   	'NAMED' SourceSelector
fn named_graph_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphClause<'src>, E> {
    keyword("NAMED")
        .ignore_then(source_selector())
        .map(GraphClause::Named)
        .boxed()
}

// [18]   	SourceSelector 	  ::=   	iri
fn source_selector<'src, E: CParserError<'src>>() -> impl CParser<'src, Iri<'src>, E> {
    iri()
}

// [19]   	WhereClause 	  ::=   	'WHERE'? GroupGraphPattern
fn where_clause<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPattern<'src>, E> {
    keyword("WHERE")
        .or_not()
        .ignore_then(group_graph_pattern)
        .boxed()
}

// [20]   	SolutionModifier 	  ::=   	GroupClause? HavingClause? OrderClause? LimitOffsetClauses?
fn solution_modifier<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, SolutionModifier<'src>, E> {
    group_clause(expression.clone(), group_graph_pattern.clone())
        .or_not()
        .then(having_clause(expression.clone(), group_graph_pattern.clone()).or_not())
        .then(order_clause(expression, group_graph_pattern).or_not())
        .then(limit_offset_clauses().or_not())
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
        .boxed()
}

// [21]   	GroupClause 	  ::=   	'GROUP' 'BY' GroupCondition+
fn group_clause<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Vec<(Spanned<Expression<'src>>, Option<Spanned<Var<'src>>>)>, E> {
    keyword("GROUP")
        .ignore_then(keyword("BY"))
        .ignore_then(
            group_condition(expression, group_graph_pattern)
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .boxed()
}

// [22]   	GroupCondition 	  ::=   	BuiltInCall | FunctionCall | '(' Expression ( 'AS' Var )? ')' | Var
fn group_condition<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, (Spanned<Expression<'src>>, Option<Spanned<Var<'src>>>), E> {
    choice((
        built_in_call(expression.clone(), group_graph_pattern.clone()).map(|e| (e, None)),
        function_call(expression.clone()).map(|e| (e, None)),
        expression
            .then(keyword("AS").ignore_then(var()).or_not())
            .delimited_by(operator("("), operator(")")),
        var().map(Expression::Var).spanned().map(|v| (v, None)),
    ))
    .boxed()
}

// [23]   	HavingClause 	  ::=   	'HAVING' HavingCondition+
fn having_clause<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Vec<Spanned<Expression<'src>>>, E> {
    keyword("HAVING")
        .ignore_then(
            having_condition(expression, group_graph_pattern)
                .repeated()
                .at_least(1)
                .collect(),
        )
        .boxed()
}

// [24]   	HavingCondition 	  ::=   	Constraint
fn having_condition<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    constraint(expression, group_graph_pattern)
}
// [25]   	OrderClause 	  ::=   	'ORDER' 'BY' OrderCondition+
fn order_clause<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Vec<OrderCondition<'src>>, E> {
    keyword("ORDER")
        .ignore_then(keyword("BY"))
        .ignore_then(
            order_condition(expression, group_graph_pattern)
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .boxed()
}
// [26]   	OrderCondition 	  ::=   	( ( 'ASC' | 'DESC' ) BrackettedExpression ) | ( Constraint | Var )
fn order_condition<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, OrderCondition<'src>, E> {
    choice((
        keyword("ASC")
            .to(true)
            .or(keyword("DESC").to(false))
            .then(bracketted_expression(expression.clone()))
            .map(|(is_asc, e)| {
                if is_asc {
                    OrderCondition::Asc(e)
                } else {
                    OrderCondition::Desc(e)
                }
            }),
        constraint(expression, group_graph_pattern).map(OrderCondition::Asc),
        var()
            .map(Expression::Var)
            .spanned()
            .map(OrderCondition::Asc),
    ))
    .boxed()
}

// [27]   	LimitOffsetClauses 	  ::=   	LimitClause OffsetClause? | OffsetClause LimitClause?
fn limit_offset_clauses<'src, E: CParserError<'src>>()
-> impl CParser<'src, LimitOffsetClauses<'src>, E> {
    let limit_clause = limit_clause();
    let offset_clause = offset_clause();
    limit_clause
        .clone()
        .then(offset_clause.clone().or_not())
        .map(|(limit, offset)| LimitOffsetClauses {
            offset,
            limit: Some(limit),
        })
        .or(offset_clause
            .then(limit_clause.or_not())
            .map(|(offset, limit)| LimitOffsetClauses {
                offset: Some(offset),
                limit,
            }))
        .boxed()
}

// [28]   	LimitClause 	  ::=   	'LIMIT' INTEGER
fn limit_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<&'src str>, E> {
    keyword("LIMIT")
        .ignore_then(
            select! {
                Token::Integer(v) => v,
            }
            .labelled("an integer")
            .spanned(),
        )
        .boxed()
}

// [29]   	OffsetClause 	  ::=   	'OFFSET' INTEGER
fn offset_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<&'src str>, E> {
    keyword("OFFSET")
        .ignore_then(
            select! {
                Token::Integer(v) => v,
            }
            .labelled("an integer")
            .spanned(),
        )
        .boxed()
}

// [30]   	ValuesClause 	  ::=   	( 'VALUES' DataBlock )?
fn values_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, Option<ValuesClause<'src>>, E>
{
    keyword("VALUES").ignore_then(data_block()).or_not().boxed()
}

// [31]   	Update 	  ::=   	Prologue ( Update1 ( ';' Update )? )?
// or Update 	  ::=   	Prologue (Update1 ( ';' Prologue Update1 )* (';' Prologue)?)?
fn update<'src, E: CParserError<'src>>() -> impl CParser<'src, Update<'src>, E> {
    prologue()
        .then(
            update1()
                .then(
                    operator(";")
                        .ignore_then(prologue())
                        .then(update1())
                        .repeated()
                        .collect::<Vec<_>>()
                        .then(operator(";").ignore_then(prologue()).or_not()),
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
        })
        .boxed()
}

// [32]   	Update1 	  ::=   	Load | Clear | Drop | Add | Move | Copy | Create | DeleteWhere | Modify | InsertData | DeleteData
fn update1<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    choice((
        insert_data(),
        delete_data(),
        load(),
        clear(),
        drop(),
        add(),
        r#move(),
        copy(),
        create(),
        delete_where(),
        modify(),
    ))
    .boxed()
}

// [33]   	Load 	  ::=   	'LOAD' 'SILENT'? iri ( 'INTO' GraphRef )?
fn load<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("LOAD")
        .ignore_then(keyword("SILENT").or_not())
        .then(iri())
        .then(keyword("INTO").ignore_then(graph_ref()).or_not())
        .map(|((silent, from), to)| Update1::Load {
            silent: silent.is_some(),
            from,
            to,
        })
}

// [34]   	Clear 	  ::=   	'CLEAR' 'SILENT'? GraphRefAll
fn clear<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("CLEAR")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref_all())
        .map(|(silent, graph)| Update1::Clear {
            silent: silent.is_some(),
            graph,
        })
}

// [35]   	Drop 	  ::=   	'DROP' 'SILENT'? GraphRefAll
fn drop<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("DROP")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref_all())
        .map(|(silent, graph)| Update1::Drop {
            silent: silent.is_some(),
            graph,
        })
}

// [36]   	Create 	  ::=   	'CREATE' 'SILENT'? GraphRef
fn create<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("CREATE")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_ref())
        .map(|(silent, graph)| Update1::Create {
            silent: silent.is_some(),
            graph,
        })
}

// [37]   	Add 	  ::=   	'ADD' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
fn add<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("ADD")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default())
        .then_ignore(keyword("TO"))
        .then(graph_or_default())
        .map(|((silent, from), to)| Update1::Add {
            silent: silent.is_some(),
            from,
            to,
        })
}

// [38]   	Move 	  ::=   	'MOVE' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
fn r#move<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("MOVE")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default())
        .then_ignore(keyword("TO"))
        .then(graph_or_default())
        .map(|((silent, from), to)| Update1::Move {
            silent: silent.is_some(),
            from,
            to,
        })
}

// [39]   	Copy 	  ::=   	'COPY' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
fn copy<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("COPY")
        .ignore_then(keyword("SILENT").or_not())
        .then(graph_or_default())
        .then_ignore(keyword("TO"))
        .then(graph_or_default())
        .map(|((silent, from), to)| Update1::Copy {
            silent: silent.is_some(),
            from,
            to,
        })
}

// [40]   	InsertData 	  ::=   	'INSERT DATA' QuadData
fn insert_data<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("INSERT")
        .ignore_then(keyword("DATA"))
        .ignore_then(quad_data())
        .map(|quads| Update1::InsertData { quads })
}

// [41]   	DeleteData 	  ::=   	'DELETE DATA' QuadData
fn delete_data<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("DELETE")
        .ignore_then(keyword("DATA"))
        .ignore_then(quad_data())
        .map(|quads| Update1::DeleteData { quads })
}

// [42]   	DeleteWhere 	  ::=   	'DELETE WHERE' QuadPattern
fn delete_where<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    keyword("DELETE")
        .ignore_then(keyword("WHERE"))
        .ignore_then(quad_pattern())
        .map(|pattern| Update1::DeleteWhere { pattern })
}

// [43]   	Modify 	  ::=   	( 'WITH' iri )? ( DeleteClause InsertClause? | InsertClause ) UsingClause* 'WHERE' GroupGraphPattern
fn modify<'src, E: CParserError<'src>>() -> impl CParser<'src, Update1<'src>, E> {
    let insert_clause = insert_clause();
    keyword("WITH")
        .ignore_then(iri())
        .or_not()
        .then(
            delete_clause()
                .then(insert_clause.clone().or_not())
                .map(|(delete, insert)| (delete, insert.unwrap_or_default()))
                .or(insert_clause.map(|insert| (Vec::new(), insert))),
        )
        .then(using_clause().repeated().collect())
        .then_ignore(keyword("WHERE"))
        .then(group_graph_pattern(sub_select()))
        .map(
            |(((with, (delete, insert)), using), r#where)| Update1::Modify {
                with,
                delete,
                insert,
                using,
                r#where,
            },
        )
        .boxed()
}

// [44]   	DeleteClause 	  ::=   	'DELETE' QuadPattern
fn delete_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, QuadPatterns<'src>, E> {
    keyword("DELETE").ignore_then(quad_pattern())
}

// [45]   	InsertClause 	  ::=   	'INSERT' QuadPattern
fn insert_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, QuadPatterns<'src>, E> {
    keyword("INSERT").ignore_then(quad_pattern())
}

// [46]   	UsingClause 	  ::=   	'USING' ( iri | 'NAMED' iri )
fn using_clause<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphClause<'src>, E> {
    keyword("USING").ignore_then(
        iri()
            .map(GraphClause::Default)
            .or(keyword("NAMED").ignore_then(iri()).map(GraphClause::Named)),
    )
}

// [47]   	GraphOrDefault 	  ::=   	'DEFAULT' | 'GRAPH'? iri
fn graph_or_default<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphOrDefault<'src>, E> {
    keyword("DEFAULT")
        .to(GraphOrDefault::Default)
        .or(keyword("GRAPH")
            .or_not()
            .ignore_then(iri())
            .map(GraphOrDefault::Graph))
}

// [48]   	GraphRef 	  ::=   	'GRAPH' iri
fn graph_ref<'src, E: CParserError<'src>>() -> impl CParser<'src, Iri<'src>, E> {
    keyword("GRAPH").ignore_then(iri())
}

// [49]   	GraphRefAll 	  ::=   	GraphRef | 'DEFAULT' | 'NAMED' | 'ALL'
fn graph_ref_all<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphRefAll<'src>, E> {
    choice((
        graph_ref().map(GraphRefAll::Graph),
        keyword("DEFAULT").to(GraphRefAll::Default),
        keyword("NAMED").to(GraphRefAll::Named),
        keyword("ALL").to(GraphRefAll::All),
    ))
    .boxed()
}

// [50]   	QuadPattern 	  ::=   	'{' Quads '}'
fn quad_pattern<'src, E: CParserError<'src>>() -> impl CParser<'src, QuadPatterns<'src>, E> {
    quads().delimited_by(operator("{"), operator("}"))
}

// [51]   	QuadData 	  ::=   	'{' Quads '}'
fn quad_data<'src, E: CParserError<'src>>() -> impl CParser<'src, QuadPatterns<'src>, E> {
    quad_pattern()
}

// [52]   	Quads 	  ::=   	TriplesTemplate? ( QuadsNotTriples '.'? TriplesTemplate? )*
fn quads<'src, E: CParserError<'src>>() -> impl CParser<'src, QuadPatterns<'src>, E> {
    let triples_template = triples_template();
    triples_template
        .clone()
        .map(|t| vec![(None, t)])
        .foldl(
            quads_not_triples()
                .then_ignore(operator(".").or_not())
                .then(triples_template.map(|t| (None, t)))
                .repeated(),
            |mut a, (b, c)| {
                a.push(b);
                a.push(c);
                a
            },
        )
        .boxed()
}

// [53]   	QuadsNotTriples 	  ::=   	'GRAPH' VarOrIri '{' TriplesTemplate? '}'
fn quads_not_triples<'src, E: CParserError<'src>>() -> impl CParser<
    'src,
    (
        Option<VarOrIri<'src>>,
        Vec<(GraphNode<'src>, PropertyList<'src>)>,
    ),
    E,
> {
    keyword("GRAPH")
        .ignore_then(var_or_iri())
        .then(triples_template().delimited_by(operator("{"), operator("}")))
        .map(|(graph, triples)| (Some(graph), triples))
}

// [54]   	TriplesTemplate 	  ::=   	TriplesSameSubject ( '.' TriplesTemplate? )?
// TripleTemplate is always optional, we allow it to be empty
fn triples_template<'src, E: CParserError<'src>>()
-> impl CParser<'src, Vec<(GraphNode<'src>, PropertyList<'src>)>, E> {
    triples_same_subject()
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .boxed()
}

// [55]   	GroupGraphPattern 	  ::=   	'{' ( SubSelect | GroupGraphPatternSub ) '}'
fn group_graph_pattern<'src, E: CParserError<'src>>(
    sub_select: impl CParser<'src, SubSelect<'src>, E>,
) -> impl CParser<'src, GraphPattern<'src>, E> {
    recursive(|group_graph_pattern| {
        sub_select
            .map(|p| GraphPattern::SubSelect(Box::new(p)))
            .or(group_graph_pattern_sub(group_graph_pattern))
            .delimited_by(operator("{"), operator("}"))
    })
    .boxed()
}

// [56]   	GroupGraphPatternSub 	  ::=   	TriplesBlock? ( GraphPatternNotTriples '.'? TriplesBlock? )*
fn group_graph_pattern_sub<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPattern<'src>, E> {
    let triples_block = triples_block();
    triples_block
        .clone()
        .map(|group| vec![group])
        .foldl(
            graph_pattern_not_triples(group_graph_pattern)
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
        .map(GraphPattern::Group)
        .boxed()
}

// [57]   	TriplesBlock 	  ::=   	TriplesSameSubjectPath ( '.' TriplesBlock? )?
// also TriplesSameSubjectPath ( '.' TriplesSameSubjectPath? )*
// It is always optional, we allow it to be empty
fn triples_block<'src, E: CParserError<'src>>()
-> impl CParser<'src, Spanned<GraphPatternElement<'src>>, E> {
    triples_same_subject_path()
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .map(GraphPatternElement::Triples)
        .spanned()
        .boxed()
}

#[cfg(feature = "sep-0006")]
fn lateral_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("LATERAL")
        .ignore_then(group_graph_pattern)
        .map(|p| GraphPatternElement::Lateral(Box::new(p)))
}

// [58]   	ReifiedTripleBlock 	  ::=   	ReifiedTriple PropertyList
#[cfg(feature = "sparql-12")]
fn reified_triple_block<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, (GraphNode<'src>, PropertyList<'src>), E> {
    reified_triple()
        .map(GraphNode::ReifiedTriple)
        .then(property_list(property_list_not_empty))
}

// [59]   	ReifiedTripleBlockPath 	  ::=   	ReifiedTriple PropertyListPath
#[cfg(feature = "sparql-12")]
fn reified_triple_block_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, (GraphNodePath<'src>, PropertyListPath<'src>), E> {
    reified_triple()
        .map(GraphNodePath::ReifiedTriple)
        .then(property_list_path(property_list_path_not_empty))
}

// [60]   	GraphPatternNotTriples 	  ::=   	GroupOrUnionGraphPattern | OptionalGraphPattern | MinusGraphPattern | GraphGraphPattern | ServiceGraphPattern | Filter | Bind | InlineData
fn graph_pattern_not_triples<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    choice((
        group_or_union_graph_pattern(group_graph_pattern.clone()),
        optional_graph_pattern(group_graph_pattern.clone()),
        minus_graph_pattern(group_graph_pattern.clone()),
        graph_graph_pattern(group_graph_pattern.clone()),
        service_graph_pattern(group_graph_pattern.clone()),
        filter(group_graph_pattern.clone()),
        bind(group_graph_pattern.clone()),
        inline_data(),
        #[cfg(feature = "sep-0006")]
        lateral_graph_pattern(group_graph_pattern),
    ))
    .boxed()
}

// [73]   	GroupOrUnionGraphPattern 	  ::=   	GroupGraphPattern ( 'UNION' GroupGraphPattern )*
fn group_or_union_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    group_graph_pattern
        .separated_by(keyword("UNION"))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(GraphPatternElement::Union)
}

// [74]   	Filter 	  ::=   	'FILTER' Constraint
fn filter<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("FILTER")
        .ignore_then(constraint(
            expression(group_graph_pattern.clone()),
            group_graph_pattern,
        ))
        .map(GraphPatternElement::Filter)
}

// [61]   	OptionalGraphPattern 	  ::=   	'OPTIONAL' GroupGraphPattern
fn optional_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("OPTIONAL")
        .ignore_then(group_graph_pattern)
        .map(|p| GraphPatternElement::Optional(Box::new(p)))
}

// [62]   	GraphGraphPattern 	  ::=   	'GRAPH' VarOrIri GroupGraphPattern
fn graph_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("GRAPH")
        .ignore_then(var_or_iri())
        .then(group_graph_pattern)
        .map(|(name, pattern)| GraphPatternElement::Graph {
            name,
            pattern: Box::new(pattern),
        })
}

// [63]   	ServiceGraphPattern 	  ::=   	'SERVICE' 'SILENT'? VarOrIri GroupGraphPattern
fn service_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("SERVICE")
        .ignore_then(keyword("SILENT").or_not())
        .then(var_or_iri())
        .then(group_graph_pattern)
        .map(|((silent, name), pattern)| GraphPatternElement::Service {
            silent: silent.is_some(),
            name,
            pattern: Box::new(pattern),
        })
}

// [64]   	Bind 	  ::=   	'BIND' '(' Expression 'AS' Var ')'
fn bind<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("BIND")
        .ignore_then(
            expression(group_graph_pattern)
                .then_ignore(keyword("AS"))
                .then(var())
                .delimited_by(operator("("), operator(")")),
        )
        .map(|(e, v)| GraphPatternElement::Bind(e, v))
}

// [65]   	InlineData 	  ::=   	'VALUES' DataBlock
fn inline_data<'src, E: CParserError<'src>>() -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("VALUES")
        .ignore_then(data_block())
        .map(GraphPatternElement::Values)
}

// [66]   	DataBlock 	  ::=   	InlineDataOneVar | InlineDataFull
fn data_block<'src, E: CParserError<'src>>() -> impl CParser<'src, ValuesClause<'src>, E> {
    inline_data_one_var()
        .or(inline_data_full())
        .map(|(variables, values)| ValuesClause { variables, values })
}

// [67]   	InlineDataOneVar 	  ::=   	Var '{' DataBlockValue* '}'
fn inline_data_one_var<'src, E: CParserError<'src>>() -> impl CParser<
    'src,
    (
        Vec<Spanned<Var<'src>>>,
        Spanned<Vec<Vec<DataBlockValue<'src>>>>,
    ),
    E,
> {
    var()
        .map(|v| vec![v])
        .then(
            data_block_value()
                .map(|v| vec![v])
                .repeated()
                .collect()
                .delimited_by(operator("{"), operator("}"))
                .spanned(),
        )
        .boxed()
}

// [68]   	InlineDataFull 	  ::=   	( NIL | '(' Var* ')' ) '{' ( '(' DataBlockValue* ')' | NIL )* '}'
fn inline_data_full<'src, E: CParserError<'src>>() -> impl CParser<
    'src,
    (
        Vec<Spanned<Var<'src>>>,
        Spanned<Vec<Vec<DataBlockValue<'src>>>>,
    ),
    E,
> {
    var()
        .repeated()
        .collect()
        .delimited_by(operator("("), operator(")"))
        .then(
            data_block_value()
                .repeated()
                .collect()
                .delimited_by(operator("("), operator(")"))
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(operator("{"), operator("}"))
                .spanned(),
        )
        .boxed()
}

// [69]   	DataBlockValue 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | 'UNDEF' | TripleTermData
fn data_block_value<'src, E: CParserError<'src>>() -> impl CParser<'src, DataBlockValue<'src>, E> {
    choice((
        iri().map(DataBlockValue::Iri),
        rdf_literal().map(DataBlockValue::Literal),
        numeric_literal().map(DataBlockValue::Literal),
        boolean_literal().map(DataBlockValue::Literal),
        keyword("UNDEF").to(DataBlockValue::Undef),
        #[cfg(feature = "sparql-12")]
        triple_term_data().map(DataBlockValue::TripleTerm),
    ))
    .boxed()
}

// [70]   	Reifier 	  ::=   	'~' VarOrReifierId?
#[cfg(feature = "sparql-12")]
fn reifier<'src, E: CParserError<'src>>() -> impl CParser<'src, Option<VarOrReifierId<'src>>, E> {
    operator("~").ignore_then(var_or_reifier_id().or_not())
}

// [71]   	VarOrReifierId 	  ::=   	Var | iri | BlankNode
#[cfg(feature = "sparql-12")]
fn var_or_reifier_id<'src, E: CParserError<'src>>() -> impl CParser<'src, VarOrReifierId<'src>, E> {
    choice((
        var().map(VarOrReifierId::Var),
        iri().map(VarOrReifierId::Iri),
        blank_node().map(VarOrReifierId::BlankNode),
    ))
    .boxed()
}
// [72]   	MinusGraphPattern 	  ::=   	'MINUS' GroupGraphPattern
fn minus_graph_pattern<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, GraphPatternElement<'src>, E> {
    keyword("MINUS")
        .ignore_then(group_graph_pattern)
        .map(|p| GraphPatternElement::Minus(Box::new(p)))
        .boxed()
}

// [75]   	Constraint 	  ::=   	BrackettedExpression | BuiltInCall | FunctionCall
fn constraint<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    choice((
        bracketted_expression(expression.clone()),
        built_in_call(expression.clone(), group_graph_pattern),
        function_call(expression),
    ))
    .boxed()
}

// [76]   	FunctionCall 	  ::=   	iri ArgList
fn function_call<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    iri()
        .then(arg_list(expression))
        .map(|(name, args)| Expression::Function(name, args))
        .spanned()
        .boxed()
}

// [77]   	ArgList 	  ::=   	NIL | '(' 'DISTINCT'? Expression ( ',' Expression )* ')'
fn arg_list<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, ArgList<'src>, E> {
    keyword("DISTINCT")
        .or_not()
        .then(expression.separated_by(operator(",")).collect::<Vec<_>>())
        .delimited_by(operator("("), operator(")"))
        .map(|(distinct, args)| ArgList {
            distinct: distinct.is_some(),
            args,
        })
        .boxed()
}

// [78]   	ExpressionList 	  ::=   	NIL | '(' Expression ( ',' Expression )* ')'
fn expression_list<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, Vec<Spanned<Expression<'src>>>, E> {
    expression
        .separated_by(operator(","))
        .collect()
        .delimited_by(operator("("), operator(")"))
        .boxed()
}

// [79]   	ConstructTemplate 	  ::=   	'{' ConstructTriples? '}'
// [80]   	ConstructTriples 	  ::=   	TriplesSameSubject ( '.' ConstructTriples? )?
// also TriplesSameSubject ("." TriplesSameSubject?)*
fn construct_template<'src, E: CParserError<'src>>()
-> impl CParser<'src, Spanned<Vec<(GraphNode<'src>, PropertyList<'src>)>>, E> {
    triples_same_subject()
        .separated_by(operator("."))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(operator("{"), operator("}"))
        .spanned()
        .boxed()
}

// [81]   	TriplesSameSubject 	  ::=   	VarOrTerm PropertyListNotEmpty | TriplesNode PropertyList | ReifiedTripleBlock
fn triples_same_subject<'src, E: CParserError<'src>>()
-> impl CParser<'src, (GraphNode<'src>, PropertyList<'src>), E> {
    let property_list_not_empty = property_list_not_empty();
    choice((
        var_or_term()
            .map(GraphNode::VarOrTerm)
            .then(property_list_not_empty.clone()),
        triples_node(property_list_not_empty.clone())
            .then(property_list(property_list_not_empty.clone())),
        #[cfg(feature = "sparql-12")]
        reified_triple_block(property_list_not_empty),
    ))
    .boxed()
}

// [82]   	PropertyList 	  ::=   	PropertyListNotEmpty?
fn property_list<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, PropertyList<'src>, E> {
    property_list_not_empty
        .or_not()
        .map(Option::unwrap_or_default)
}

// [83]   	PropertyListNotEmpty 	  ::=   	Verb ObjectList ( ';' ( Verb ObjectList )? )*
fn property_list_not_empty<'src, E: CParserError<'src>>()
-> impl CParser<'src, PropertyList<'src>, E> {
    recursive(|property_list_not_empty| {
        let verb_object_list = verb().then(object_list(property_list_not_empty));
        verb_object_list.clone().map(|v| vec![v]).foldl(
            operator(";")
                .ignore_then(verb_object_list.or_not())
                .repeated(),
            |mut acc, val| {
                acc.extend(val);
                acc
            },
        )
    })
    .boxed()
}

// [84]   	Verb 	  ::=   	VarOrIri | 'a'
fn verb<'src, E: CParserError<'src>>() -> impl CParser<'src, Verb<'src>, E> {
    var_or_iri()
        .map(|v| match v {
            VarOrIri::Var(v) => Verb::Var(v),
            VarOrIri::Iri(v) => Verb::Iri(v),
        })
        .or(case_sensitive_keyword("a").to(Verb::A))
}

// [85]   	ObjectList 	  ::=   	Object ( ',' Object )*
fn object_list<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, Vec<Object<'src>>, E> {
    object(property_list_not_empty)
        .separated_by(operator(","))
        .at_least(1)
        .collect()
        .boxed()
}
// [86]   	Object 	  ::=   	GraphNode Annotation
#[cfg(feature = "sparql-12")]
fn object<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, Object<'src>, E> {
    graph_node(triples_node(property_list_not_empty.clone()))
        .then(annotation(property_list_not_empty))
        .map(|(graph_node, annotations)| Object {
            graph_node,
            annotations,
        })
}

#[cfg(not(feature = "sparql-12"))]
fn object<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, Object<'src>, E> {
    graph_node(triples_node(property_list_not_empty)).map(|graph_node| Object { graph_node })
}

// [87]   	TriplesSameSubjectPath 	  ::=   	VarOrTerm PropertyListPathNotEmpty | TriplesNodePath PropertyListPath | ReifiedTripleBlockPath
fn triples_same_subject_path<'src, E: CParserError<'src>>()
-> impl CParser<'src, (GraphNodePath<'src>, PropertyListPath<'src>), E> {
    let property_list_path_not_empty = property_list_path_not_empty();
    choice((
        var_or_term()
            .map(GraphNodePath::VarOrTerm)
            .then(property_list_path_not_empty.clone()),
        triples_node_path(property_list_path_not_empty.clone())
            .then(property_list_path(property_list_path_not_empty.clone())),
        #[cfg(feature = "sparql-12")]
        reified_triple_block_path(property_list_path_not_empty),
    ))
    .boxed()
}

// [88]   	PropertyListPath 	  ::=   	PropertyListPathNotEmpty?
fn property_list_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, PropertyListPath<'src>, E> {
    property_list_path_not_empty
        .or_not()
        .map(Option::unwrap_or_default)
}

// [89]   	PropertyListPathNotEmpty 	  ::=   	( VerbPath | VerbSimple ) ObjectListPath ( ';' ( ( VerbPath | VerbSimple ) ObjectListPath )? )*
fn property_list_path_not_empty<'src, E: CParserError<'src>>()
-> impl CParser<'src, PropertyListPath<'src>, E> {
    recursive(|property_list_path_not_empty| {
        let verb_object_list_path = verb_simple()
            .or(verb_path())
            .then(object_list_path(property_list_path_not_empty));
        verb_object_list_path.clone().map(|v| vec![v]).foldl(
            operator(";")
                .ignore_then(verb_object_list_path.or_not())
                .repeated(),
            |mut acc, val| {
                acc.extend(val);
                acc
            },
        )
    })
    .boxed()
}

// [90]   	VerbPath 	  ::=   	Path
fn verb_path<'src, E: CParserError<'src>>() -> impl CParser<'src, VarOrPath<'src>, E> {
    path().map(VarOrPath::Path)
}

// [91]   	VerbSimple 	  ::=   	Var
fn verb_simple<'src, E: CParserError<'src>>() -> impl CParser<'src, VarOrPath<'src>, E> {
    var().map(VarOrPath::Var)
}

// [92]   	ObjectListPath 	  ::=   	ObjectPath ( ',' ObjectPath )*
fn object_list_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, Vec<ObjectPath<'src>>, E> {
    object_path(property_list_path_not_empty)
        .separated_by(operator(","))
        .at_least(1)
        .collect()
        .boxed()
}

// [93]   	ObjectPath 	  ::=   	GraphNodePath AnnotationPath
#[cfg(feature = "sparql-12")]
fn object_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, ObjectPath<'src>, E> {
    graph_node_path(triples_node_path(property_list_path_not_empty.clone()))
        .then(annotation_path(property_list_path_not_empty))
        .map(|(graph_node, annotations)| ObjectPath {
            graph_node,
            annotations,
        })
}

#[cfg(not(feature = "sparql-12"))]
fn object_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, ObjectPath<'src>, E> {
    graph_node_path(triples_node_path(property_list_path_not_empty))
        .map(|graph_node| ObjectPath { graph_node })
}

// [94]   	Path 	  ::=   	PathAlternative
// [95]   	PathAlternative 	  ::=   	PathSequence ( '|' PathSequence )*
// [96]   	PathSequence 	  ::=   	PathEltOrInverse ( '/' PathEltOrInverse )*
// [97]   	PathElt 	  ::=   	PathPrimary PathMod?
// [98]   	PathEltOrInverse 	  ::=   	PathElt | '^' PathElt
// [99]   	PathMod 	  ::=   	'?' | '*' | '+'
fn path<'src, E: CParserError<'src>>() -> impl CParser<'src, Path<'src>, E> {
    recursive(|path| {
        path_primary(path).pratt((
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
    })
    .boxed()
}

// [100]   	PathPrimary 	  ::=   	iri | 'a' | '!' PathNegatedPropertySet | '(' Path ')'
fn path_primary<'src, E: CParserError<'src>>(
    path: impl CParser<'src, Path<'src>, E>,
) -> impl CParser<'src, Path<'src>, E> {
    choice((
        iri().map(Path::Iri),
        case_sensitive_keyword("a").to(Path::A),
        operator("!").ignore_then(path_negated_property_set()),
        path.delimited_by(operator("("), operator(")")),
    ))
    .boxed()
}

// [101]   	PathNegatedPropertySet 	  ::=   	PathOneInPropertySet | '(' ( PathOneInPropertySet ( '|' PathOneInPropertySet )* )? ')'
fn path_negated_property_set<'src, E: CParserError<'src>>() -> impl CParser<'src, Path<'src>, E> {
    path_one_in_property_set()
        .map(|p| vec![p])
        .or(path_one_in_property_set()
            .separated_by(operator("|"))
            .at_least(1)
            .collect::<Vec<_>>()
            .delimited_by(operator("("), operator(")")))
        .map(Path::NegatedPropertySet)
        .boxed()
}

// [102]   	PathOneInPropertySet 	  ::=   	iri | 'a' | '^' ( iri | 'a' )
fn path_one_in_property_set<'src, E: CParserError<'src>>()
-> impl CParser<'src, PathOneInPropertySet<'src>, E> {
    choice((
        iri().map(PathOneInPropertySet::Iri),
        case_sensitive_keyword("a").to(PathOneInPropertySet::A),
        operator("^").ignore_then(iri().map(PathOneInPropertySet::InverseIri)),
        operator("^").ignore_then(case_sensitive_keyword("a").to(PathOneInPropertySet::InverseA)),
    ))
    .boxed()
}

// [103]   	TriplesNode 	  ::=   	Collection | BlankNodePropertyList
fn triples_node<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, GraphNode<'src>, E> {
    recursive(|triples_node| {
        collection(triples_node).or(blank_node_property_list(property_list_not_empty))
    })
    .boxed()
}

// [104]   	BlankNodePropertyList 	  ::=   	'[' PropertyListNotEmpty ']'
fn blank_node_property_list<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, GraphNode<'src>, E> {
    property_list_not_empty
        .delimited_by(operator("["), operator("]"))
        .spanned()
        .map(GraphNode::BlankNodePropertyList)
}

// [105]   	TriplesNodePath 	  ::=   	CollectionPath | BlankNodePropertyListPath
fn triples_node_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, GraphNodePath<'src>, E> {
    recursive(|triples_node_path| {
        collection_path(triples_node_path)
            .or(blank_node_property_list_path(property_list_path_not_empty))
    })
    .boxed()
}
// [106]   	BlankNodePropertyListPath 	  ::=   	'[' PropertyListPathNotEmpty ']'
fn blank_node_property_list_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, GraphNodePath<'src>, E> {
    property_list_path_not_empty
        .delimited_by(operator("["), operator("]"))
        .spanned()
        .map(GraphNodePath::BlankNodePropertyList)
}

// [107]   	Collection 	  ::=   	'(' GraphNode+ ')'
fn collection<'src, E: CParserError<'src>>(
    triples_node: impl CParser<'src, GraphNode<'src>, E>,
) -> impl CParser<'src, GraphNode<'src>, E> {
    graph_node(triples_node)
        .repeated()
        .at_least(1)
        .collect()
        .delimited_by(operator("("), operator(")"))
        .spanned()
        .map(GraphNode::Collection)
        .boxed()
}

// [108]   	CollectionPath 	  ::=   	'(' GraphNodePath+ ')'
fn collection_path<'src, E: CParserError<'src>>(
    triples_node_path: impl CParser<'src, GraphNodePath<'src>, E>,
) -> impl CParser<'src, GraphNodePath<'src>, E> {
    graph_node_path(triples_node_path)
        .repeated()
        .at_least(1)
        .collect()
        .delimited_by(operator("("), operator(")"))
        .spanned()
        .map(GraphNodePath::Collection)
        .boxed()
}

// [109]   	AnnotationPath 	  ::=   	( Reifier | AnnotationBlockPath )*
#[cfg(feature = "sparql-12")]
fn annotation_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, Vec<Spanned<AnnotationPath<'src>>>, E> {
    reifier()
        .map(AnnotationPath::Reifier)
        .or(annotation_block_path(property_list_path_not_empty)
            .map(AnnotationPath::AnnotationBlock))
        .spanned()
        .repeated()
        .collect()
        .boxed()
}

// [110]   	AnnotationBlockPath 	  ::=   	'{|' PropertyListPathNotEmpty '|}'
#[cfg(feature = "sparql-12")]
fn annotation_block_path<'src, E: CParserError<'src>>(
    property_list_path_not_empty: impl CParser<'src, PropertyListPath<'src>, E>,
) -> impl CParser<'src, PropertyListPath<'src>, E> {
    property_list_path_not_empty.delimited_by(operator("{|"), operator("|}"))
}

// [111]   	Annotation 	  ::=   	( Reifier | AnnotationBlock )*
#[cfg(feature = "sparql-12")]
fn annotation<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, Vec<Spanned<Annotation<'src>>>, E> {
    reifier()
        .map(Annotation::Reifier)
        .or(annotation_block(property_list_not_empty).map(Annotation::AnnotationBlock))
        .spanned()
        .repeated()
        .collect()
        .boxed()
}

// [112]   	AnnotationBlock 	  ::=   	'{|' PropertyListNotEmpty '|}'
#[cfg(feature = "sparql-12")]
fn annotation_block<'src, E: CParserError<'src>>(
    property_list_not_empty: impl CParser<'src, PropertyList<'src>, E>,
) -> impl CParser<'src, PropertyList<'src>, E> {
    property_list_not_empty.delimited_by(operator("{|"), operator("|}"))
}

// [113]   	GraphNode 	  ::=   	VarOrTerm | TriplesNode | ReifiedTriple
fn graph_node<'src, E: CParserError<'src>>(
    triples_node: impl CParser<'src, GraphNode<'src>, E>,
) -> impl CParser<'src, GraphNode<'src>, E> {
    choice((
        var_or_term().map(GraphNode::VarOrTerm),
        triples_node,
        #[cfg(feature = "sparql-12")]
        reified_triple().map(GraphNode::ReifiedTriple),
    ))
    .boxed()
}

// [114]   	GraphNodePath 	  ::=   	VarOrTerm | TriplesNodePath | ReifiedTriple
fn graph_node_path<'src, E: CParserError<'src>>(
    triples_node_path: impl CParser<'src, GraphNodePath<'src>, E>,
) -> impl CParser<'src, GraphNodePath<'src>, E> {
    choice((
        var_or_term().map(GraphNodePath::VarOrTerm),
        triples_node_path,
        #[cfg(feature = "sparql-12")]
        reified_triple().map(GraphNodePath::ReifiedTriple),
    ))
    .boxed()
}

// [115]   	VarOrTerm 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | NIL | TripleTerm
fn var_or_term<'src, E: CParserError<'src>>() -> impl CParser<'src, VarOrTerm<'src>, E> {
    choice((
        var().map(VarOrTerm::Var),
        iri().map(VarOrTerm::Iri),
        rdf_literal().map(VarOrTerm::Literal),
        numeric_literal().map(VarOrTerm::Literal),
        boolean_literal().map(VarOrTerm::Literal),
        blank_node().map(VarOrTerm::BlankNode),
        nil().to(VarOrTerm::Nil),
        #[cfg(feature = "sparql-12")]
        triple_term().map(|t| VarOrTerm::TripleTerm(Box::new(t))),
    ))
    .boxed()
}

// [116]   	ReifiedTriple 	  ::=   	'<<' ReifiedTripleSubject Verb ReifiedTripleObject Reifier? '>>'
#[cfg(feature = "sparql-12")]
fn reified_triple<'src, E: CParserError<'src>>() -> impl CParser<'src, ReifiedTriple<'src>, E> {
    recursive(|reified_triple| {
        reified_triple_subject_or_object(reified_triple.clone())
            .then(verb())
            .then(reified_triple_subject_or_object(reified_triple))
            .then(reifier().or_not())
            .delimited_by(operator("<<"), operator(">>"))
            .map(|(((subject, predicate), object), reifier)| ReifiedTriple {
                subject,
                predicate,
                object,
                reifier: reifier.flatten(),
            })
    })
}

// [117]   	ReifiedTripleSubject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | ReifiedTriple | TripleTerm
// [118]   	ReifiedTripleObject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | ReifiedTriple | TripleTerm
#[cfg(feature = "sparql-12")]
fn reified_triple_subject_or_object<'src, E: CParserError<'src>>(
    reified_triple: impl CParser<'src, ReifiedTriple<'src>, E>,
) -> impl CParser<'src, ReifiedTripleSubjectOrObject<'src>, E> {
    choice((
        var().map(ReifiedTripleSubjectOrObject::Var),
        iri().map(ReifiedTripleSubjectOrObject::Iri),
        rdf_literal().map(ReifiedTripleSubjectOrObject::Literal),
        numeric_literal().map(ReifiedTripleSubjectOrObject::Literal),
        boolean_literal().map(ReifiedTripleSubjectOrObject::Literal),
        blank_node().map(ReifiedTripleSubjectOrObject::BlankNode),
        reified_triple.map(|t| ReifiedTripleSubjectOrObject::ReifiedTriple(Box::new(t))),
        triple_term()
            .clone()
            .map(|t| ReifiedTripleSubjectOrObject::TripleTerm(Box::new(t))),
    ))
    .boxed()
}

// [119]   	TripleTerm 	  ::=   	'<<(' TripleTermSubject Verb TripleTermObject ')>>'
#[cfg(feature = "sparql-12")]
fn triple_term<'src, E: CParserError<'src>>() -> impl CParser<'src, TripleTerm<'src>, E> {
    recursive(|triple_term| {
        triple_term_subject_or_object(triple_term.clone())
            .then(verb())
            .then(triple_term_subject_or_object(triple_term))
            .delimited_by(operator("<<("), operator(")>>"))
            .map(|((subject, predicate), object)| TripleTerm {
                subject,
                predicate,
                object,
            })
    })
    .boxed()
}

// [120]   	TripleTermSubject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | TripleTerm
// [121]   	TripleTermObject 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | TripleTerm
#[cfg(feature = "sparql-12")]
fn triple_term_subject_or_object<'src, E: CParserError<'src>>(
    triple_term: impl CParser<'src, TripleTerm<'src>, E>,
) -> impl CParser<'src, VarOrTerm<'src>, E> {
    choice((
        var().map(VarOrTerm::Var),
        iri().map(VarOrTerm::Iri),
        rdf_literal().map(VarOrTerm::Literal),
        numeric_literal().map(VarOrTerm::Literal),
        boolean_literal().map(VarOrTerm::Literal),
        blank_node().map(VarOrTerm::BlankNode),
        triple_term.map(|t| VarOrTerm::TripleTerm(Box::new(t))),
    ))
    .boxed()
}

// [122]   	TripleTermData 	  ::=   	'<<(' TripleTermDataSubject ( iri | 'a' ) TripleTermDataObject ')>>'
// [123]   	TripleTermDataSubject 	  ::=   	iri
#[cfg(feature = "sparql-12")]
fn triple_term_data<'src, E: CParserError<'src>>() -> impl CParser<'src, TripleTermData<'src>, E> {
    recursive(|triple_term_data| {
        iri()
            .then(
                iri()
                    .map(IriOrA::Iri)
                    .or(case_sensitive_keyword("a").to(IriOrA::A)),
            )
            .then(triple_term_data_object(triple_term_data))
            .delimited_by(operator("<<("), operator(")>>"))
            .map(|((subject, predicate), object)| TripleTermData {
                subject,
                predicate,
                object,
            })
    })
    .boxed()
}

// [124]   	TripleTermDataObject 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | TripleTermData
#[cfg(feature = "sparql-12")]
fn triple_term_data_object<'src, E: CParserError<'src>>(
    triple_term_data: impl CParser<'src, TripleTermData<'src>, E>,
) -> impl CParser<'src, TripleTermDataObject<'src>, E> {
    choice((
        iri().map(TripleTermDataObject::Iri),
        rdf_literal().map(TripleTermDataObject::Literal),
        numeric_literal().map(TripleTermDataObject::Literal),
        boolean_literal().map(TripleTermDataObject::Literal),
        triple_term_data.map(|t| TripleTermDataObject::TripleTerm(Box::new(t))),
    ))
    .boxed()
}

// [125]   	VarOrIri 	  ::=   	Var | iri
fn var_or_iri<'src, E: CParserError<'src>>() -> impl CParser<'src, VarOrIri<'src>, E> {
    var().map(VarOrIri::Var).or(iri().map(VarOrIri::Iri))
}

// [126]   	Var 	  ::=   	VAR1 | VAR2
fn var<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<Var<'src>>, E> {
    select! {
        Token::Var1(v) => Var(&v[1..]),
        Token::Var2(v) => Var(&v[1..]),
    }
    .labelled("a variable")
    .spanned()
}

// [127]   	Expression 	  ::=   	ConditionalOrExpression
fn expression<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    recursive(|expression| {
        primary_expression(expression.clone(), group_graph_pattern).pratt((
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
                keyword("IN").ignore_then(expression_list(expression.clone())),
                |l, r, c| Spanned {
                    inner: Expression::In(Box::new(l), r),
                    span: c.span(),
                },
            ),
            postfix(
                3,
                keyword("NOT")
                    .ignore_then(keyword("IN"))
                    .ignore_then(expression_list(expression.clone())),
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
                numeric_literal_positive_or_negative()
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
    })
    .boxed()
}

// [136]   	PrimaryExpression 	  ::=   	BrackettedExpression | BuiltInCall | iriOrFunction | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
fn primary_expression<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    choice((
        bracketted_expression(expression.clone()),
        iri_or_function(expression.clone()),
        rdf_literal().map(Expression::Literal).spanned(),
        numeric_literal().map(Expression::Literal).spanned(),
        boolean_literal().map(Expression::Literal).spanned(),
        var().map(Expression::Var).spanned(),
        #[cfg(feature = "sparql-12")]
        expr_triple_term().map(Expression::TripleTerm).spanned(),
        built_in_call(expression.clone(), group_graph_pattern),
    ))
    .boxed()
}

// [137]   	ExprTripleTerm 	  ::=   	'<<(' ExprTripleTermSubject Verb ExprTripleTermObject ')>>'
#[cfg(feature = "sparql-12")]
fn expr_triple_term<'src, E: CParserError<'src>>() -> impl CParser<'src, ExprTripleTerm<'src>, E> {
    recursive(|expr_triple_term| {
        expr_triple_term_subject()
            .then(verb())
            .then(expr_triple_term_object(expr_triple_term))
            .delimited_by(operator("<<("), operator(")>>"))
            .map(|((subject, predicate), object)| ExprTripleTerm {
                subject,
                predicate,
                object,
            })
    })
    .boxed()
}

// [138]   	ExprTripleTermSubject 	  ::=   	iri | Var
#[cfg(feature = "sparql-12")]
fn expr_triple_term_subject<'src, E: CParserError<'src>>()
-> impl CParser<'src, ExprTripleTermSubject<'src>, E> {
    iri()
        .map(ExprTripleTermSubject::Iri)
        .or(var().map(ExprTripleTermSubject::Var))
}

// [139]   	ExprTripleTermObject 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
#[cfg(feature = "sparql-12")]
fn expr_triple_term_object<'src, E: CParserError<'src>>(
    expr_triple_term: impl CParser<'src, ExprTripleTerm<'src>, E>,
) -> impl CParser<'src, ExprTripleTermObject<'src>, E> {
    choice((
        iri().map(ExprTripleTermObject::Iri),
        rdf_literal().map(ExprTripleTermObject::Literal),
        numeric_literal().map(ExprTripleTermObject::Literal),
        boolean_literal().map(ExprTripleTermObject::Literal),
        var().map(ExprTripleTermObject::Var),
        expr_triple_term.map(|t| ExprTripleTermObject::TripleTerm(Box::new(t))),
    ))
    .boxed()
}

// [140]   	BrackettedExpression 	  ::=   	'(' Expression ')'
fn bracketted_expression<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    expression.delimited_by(operator("("), operator(")"))
}

// [141]   	BuiltInCall 	  ::=   	  Aggregate | 'STR' '(' Expression ')' | 'LANG' | 'LANGMATCHES' '(' Expression ',' Expression ')' | 'LANGDIR' '(' Expression ')' | 'DATATYPE' '(' Expression ')' | 'BOUND' '(' Var ')' | 'IRI' '(' Expression ')' | 'URI' '(' Expression ')' | 'BNODE' ( '(' Expression ')' | NIL ) | 'RAND' NIL | 'ABS' '(' Expression ')' | 'CEIL' '(' Expression ')' | 'FLOOR' '(' Expression ')' | 'ROUND' '(' Expression ')' | 'CONCAT' ExpressionList | SubstringExpression | 'STRLEN' '(' Expression ')' | StrReplaceExpression | 'UCASE' '(' Expression ')' | 'LCASE' '(' Expression ')' | 'ENCODE_FOR_URI' '(' Expression ')' | 'CONTAINS' '(' Expression ',' Expression ')' | 'STRSTARTS' '(' Expression ',' Expression ')' | 'STRENDS' '(' Expression ',' Expression ')' | 'STRBEFORE' '(' Expression ',' Expression ')' | 'STRAFTER' '(' Expression ',' Expression ')' | 'YEAR' '(' Expression ')' | 'MONTH' '(' Expression ')' | 'DAY' '(' Expression ')' | 'HOURS' '(' Expression ')' | 'MINUTES' '(' Expression ')' | 'SECONDS' '(' Expression ')' | 'TIMEZONE' '(' Expression ')' | 'TZ' '(' Expression ')' | 'NOW' NIL | 'UUID' NIL | 'STRUUID' NIL | 'MD5' '(' Expression ')' | 'SHA1' '(' Expression ')' | 'SHA256' '(' Expression ')' | 'SHA384' '(' Expression ')' | 'SHA512' '(' Expression ')' | 'COALESCE' ExpressionList | 'IF' '(' Expression ',' Expression ',' Expression ')' | 'STRLANG' '(' Expression ',' Expression ')' | 'STRLANGDIR' '(' Expression ',' Expression ',' Expression ')' | 'STRDT' '(' Expression ',' Expression ')' | 'sameTerm' '(' Expression ',' Expression ')' | 'isIRI' '(' Expression ')' | 'isURI' '(' Expression ')' | 'isBLANK' '(' Expression ')' | 'isLITERAL' '(' Expression ')' | 'isNUMERIC' '(' Expression ')' | 'hasLANG' '(' Expression ')' | 'hasLANGDIR' '(' Expression ')' | RegexExpression | ExistsFunc | NotExistsFunc | 'isTRIPLE' '(' Expression ')' | 'TRIPLE' '(' Expression ',' Expression ',' Expression ')' | 'SUBJECT' '(' Expression ')' | 'PREDICATE' '(' Expression ')' | 'OBJECT' '(' Expression ')'
// [142]   	RegexExpression 	  ::=   	'REGEX' '(' Expression ',' Expression ( ',' Expression )? ')'
// [143]   	SubstringExpression 	  ::=   	'SUBSTR' '(' Expression ',' Expression ( ',' Expression )? ')'
// [144]   	StrReplaceExpression 	  ::=   	'REPLACE' '(' Expression ',' Expression ',' Expression ( ',' Expression )? ')'
fn built_in_call<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    aggregate(expression.clone())
        .map(Expression::Aggregate)
        .or(keyword("BOUND")
            .ignore_then(var().delimited_by(operator("("), operator(")")))
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
        .then(expression_list(expression))
        .map(|(name, args)| Expression::BuiltIn(name, args)))
        .or(exists(group_graph_pattern))
        .spanned()
        .boxed()
}

// [145]   	ExistsFunc 	  ::=   	'EXISTS' GroupGraphPattern
// [146]   	NotExistsFunc 	  ::=   	'NOT' 'EXISTS' GroupGraphPattern
fn exists<'src, E: CParserError<'src>>(
    group_graph_pattern: impl CParser<'src, GraphPattern<'src>, E>,
) -> impl CParser<'src, Expression<'src>, E> {
    keyword("NOT")
        .ignored()
        .or_not()
        .then_ignore(keyword("EXISTS"))
        .then(group_graph_pattern)
        .map(|(neg, e)| {
            if neg.is_some() {
                Expression::NotExists(Box::new(e))
            } else {
                Expression::Exists(Box::new(e))
            }
        })
        .boxed()
}

// [147]   	Aggregate 	  ::=   	  'COUNT' '(' 'DISTINCT'? ( '*' | Expression ) ')' | 'SUM' '(' 'DISTINCT'? Expression ')' | 'MIN' '(' 'DISTINCT'? Expression ')' | 'MAX' '(' 'DISTINCT'? Expression ')' | 'AVG' '(' 'DISTINCT'? Expression ')' | 'SAMPLE' '(' 'DISTINCT'? Expression ')' | 'GROUP_CONCAT' '(' 'DISTINCT'? Expression ( ';' 'SEPARATOR' '=' String )? ')'
fn aggregate<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, Aggregate<'src>, E> {
    keyword("COUNT")
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
                    .then(expression)
                    .then(
                        operator(";")
                            .ignore_then(keyword("SEPARATOR"))
                            .ignore_then(operator("="))
                            .ignore_then(string())
                            .or_not(),
                    )
                    .delimited_by(operator("("), operator(")")),
            )
            .map(|((distinct, expr), separator)| {
                Aggregate::GroupConcat(distinct.is_some(), Box::new(expr), separator)
            }))
        .boxed()
}

// [148]   	iriOrFunction 	  ::=   	iri ArgList?
fn iri_or_function<'src, E: CParserError<'src>>(
    expression: impl CParser<'src, Spanned<Expression<'src>>, E>,
) -> impl CParser<'src, Spanned<Expression<'src>>, E> {
    iri()
        .then(arg_list(expression).or_not())
        .map(|(name, args)| {
            if let Some(args) = args {
                Expression::Function(name, args)
            } else {
                Expression::Iri(name)
            }
        })
        .spanned()
        .boxed()
}

// [149]   	RDFLiteral 	  ::=   	String ( LANG_DIR | '^^' iri )?
fn rdf_literal<'src, E: CParserError<'src>>() -> impl CParser<'src, Literal<'src>, E> {
    string()
        .then(
            select! {
                Token::LangDir(l) => Either::Left(&l[1..]),
            }
            .labelled("a language tag")
            .or(operator("^^").ignore_then(iri()).map(Either::Right))
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
        })
}

// [150]   	NumericLiteral 	  ::=   	NumericLiteralUnsigned | NumericLiteralPositive | NumericLiteralNegative
// [151]   	NumericLiteralUnsigned 	  ::=   	INTEGER | DECIMAL | DOUBLE
// [152]   	NumericLiteralPositive 	  ::=   	INTEGER_POSITIVE | DECIMAL_POSITIVE | DOUBLE_POSITIVE
// [153]   	NumericLiteralNegative 	  ::=   	INTEGER_NEGATIVE | DECIMAL_NEGATIVE | DOUBLE_NEGATIVE
fn numeric_literal<'src, E: CParserError<'src>>() -> impl CParser<'src, Literal<'src>, E> {
    select! {
        Token::Integer(v) | Token::IntegerPositive(v) | Token::IntegerNegative(v) => Literal::Integer(v),
        Token::Decimal(v) | Token::DecimalPositive(v) | Token::DecimalNegative(v) => Literal::Decimal(v),
        Token::Double(v) | Token::DoublePositive(v) | Token::DoubleNegative(v) => Literal::Double(v),
    }
        .labelled("a number")
}

fn numeric_literal_positive_or_negative<'src, E: CParserError<'src>>()
-> impl CParser<'src, Literal<'src>, E> {
    select! {
        Token::IntegerPositive(v) | Token::IntegerNegative(v) => Literal::Integer(v),
        Token::DecimalPositive(v) | Token::DecimalNegative(v) => Literal::Decimal(v),
        Token::DoublePositive(v) | Token::DoubleNegative(v) => Literal::Double(v),
    }
    .labelled("a number")
}

// [154]   	BooleanLiteral 	  ::=   	'true' | 'false'
fn boolean_literal<'src, E: CParserError<'src>>() -> impl CParser<'src, Literal<'src>, E> {
    keyword("true")
        .to(Literal::Boolean(true))
        .or(keyword("false").to(Literal::Boolean(false)))
}

// [155]   	String 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2 | STRING_LITERAL_LONG1 | STRING_LITERAL_LONG2
fn string<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<String<'src>>, E> {
    select! {
        Token::StringLiteral1(s) | Token::StringLiteral2(s) => String(&s[1..s.len() - 1]),
        Token::StringLiteralLong1(s) | Token::StringLiteralLong2(s) => String(&s[3..s.len() - 3]),
    }
    .labelled("a string literal")
    .spanned()
}

// [156]   	iri 	  ::=   	IRIREF | PrefixedName
fn iri<'src, E: CParserError<'src>>() -> impl CParser<'src, Iri<'src>, E> {
    iriref()
        .map(Iri::IriRef)
        .or(prefixed_name().map(Iri::PrefixedName))
}

// [157]   	PrefixedName 	  ::=   	PNAME_LN | PNAME_NS
fn prefixed_name<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<PrefixedName<'src>>, E>
{
    select! {
        Token::PnameNs(p) => PrefixedName(&p[..p.len() - 1], ""),
        Token::PnameLn(p) => {
            #[expect(clippy::expect_used)]
            let (p, v) = p.split_once(':').expect("prefixed name must contain ':'");
            PrefixedName(p, v)
        }
    }
    .spanned()
    .labelled("a prefixed name")
}

// [158]   	BlankNode 	  ::=   	BLANK_NODE_LABEL | ANON
fn blank_node<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<BlankNode<'src>>, E> {
    select! {
        Token::BlankNodeLabel(id) => BlankNode(Some(&id[2..])),
    }
    .or(anon().to(BlankNode(None)))
    .spanned()
    .labelled("a blank node")
}

fn iriref<'src, E: CParserError<'src>>() -> impl CParser<'src, Spanned<IriRef<'src>>, E> {
    select! { Token::IriRef(i) => IriRef(&i[1..i.len() - 1]) }
        .spanned()
        .labelled("an iri")
}

fn pname_ns<'src, E: CParserError<'src>>() -> impl CParser<'src, &'src str, E> {
    select! { Token::PnameNs(p) => &p[..p.len() - 1] }.labelled("a prefix")
}

fn nil<'src, E: CParserError<'src>>() -> impl CParser<'src, (), E> {
    empty().delimited_by(operator("("), operator(")"))
}

fn anon<'src, E: CParserError<'src>>() -> impl CParser<'src, (), E> {
    empty().delimited_by(operator("["), operator("]"))
}

fn keyword<'src, E: CParserError<'src>>(
    keyword: &'static str,
) -> impl Parser<'src, ParserInput<'src>, (), extra::Err<E>> + Clone {
    select! {
        Token::Keyword(v) if v.eq_ignore_ascii_case(keyword) => ()
    }
    .labelled(keyword)
}

fn case_sensitive_keyword<'src, E: CParserError<'src>>(
    keyword: &'static str,
) -> impl Parser<'src, ParserInput<'src>, (), extra::Err<E>> + Clone {
    just(Token::Keyword(keyword)).ignored()
}

fn operator<'src, E: CParserError<'src>>(
    op: &'static str,
) -> impl Parser<'src, ParserInput<'src>, (), extra::Err<E>> + Clone {
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
