//!  for the sparql grammar, i.e. intermediate representation between the parser and the algebra
//! Does not do things like resolving prefixes, relative IRIs...

use chumsky::span::Spanned;

pub struct Query<'a> {
    pub prologue: Vec<PrologueDecl<'a>>,
    pub variant: QueryQuery<'a>,
    pub values_clause: Option<ValuesClause<'a>>,
}

#[derive(Clone, Copy)]
pub enum PrologueDecl<'a> {
    Base(Spanned<IriRef<'a>>),
    Prefix(&'a str, Spanned<IriRef<'a>>),
    #[cfg(feature = "sparql-12")]
    #[expect(dead_code)]
    Version(&'a str),
}

pub enum QueryQuery<'a> {
    Select(SelectQuery<'a>),
    Construct(ConstructQuery<'a>),
    Describe(DescribeQuery<'a>),
    Ask(AskQuery<'a>),
}

pub struct SelectQuery<'a> {
    pub select_clause: SelectClause<'a>,
    pub dataset_clause: Vec<GraphClause<'a>>,
    pub where_clause: GraphPattern<'a>,
    pub solution_modifier: SolutionModifier<'a>,
}

pub struct SubSelect<'a> {
    pub select_clause: SelectClause<'a>,
    pub where_clause: GraphPattern<'a>,
    pub solution_modifier: SolutionModifier<'a>,
    pub values_clause: Option<ValuesClause<'a>>,
}

pub struct SelectClause<'a> {
    pub option: SelectionOption,
    pub bindings: Spanned<Vec<Spanned<(Option<Spanned<Expression<'a>>>, Var<'a>)>>>,
}

pub enum SelectionOption {
    Default,
    Distinct,
    Reduced,
}

pub struct ConstructQuery<'a> {
    pub template: Spanned<Vec<(GraphNode<'a>, PropertyList<'a>)>>,
    pub dataset_clause: Vec<GraphClause<'a>>,
    pub where_clause: Option<GraphPattern<'a>>,
    pub solution_modifier: SolutionModifier<'a>,
}

pub struct DescribeQuery<'a> {
    pub targets: Spanned<Vec<Spanned<VarOrIri<'a>>>>,
    pub dataset_clause: Vec<GraphClause<'a>>,
    pub where_clause: Option<GraphPattern<'a>>,
    pub solution_modifier: SolutionModifier<'a>,
}

pub struct AskQuery<'a> {
    pub dataset_clause: Vec<GraphClause<'a>>,
    pub where_clause: GraphPattern<'a>,
    pub solution_modifier: SolutionModifier<'a>,
}

pub enum GraphClause<'a> {
    Default(Iri<'a>),
    Named(Iri<'a>),
}

pub struct SolutionModifier<'a> {
    pub group_clause: Vec<(Spanned<Expression<'a>>, Option<Var<'a>>)>,
    pub having_clause: Vec<Spanned<Expression<'a>>>,
    pub order_clause: Vec<OrderCondition<'a>>,
    pub limit_offset_clauses: Option<LimitOffsetClauses>,
}

pub enum OrderCondition<'a> {
    Asc(Spanned<Expression<'a>>),
    Desc(Spanned<Expression<'a>>),
}

pub struct LimitOffsetClauses {
    pub offset: usize,
    pub limit: Option<usize>,
}

pub struct ValuesClause<'a> {
    pub variables: Spanned<Vec<Var<'a>>>,
    pub values: Spanned<Vec<Vec<DataBlockValue<'a>>>>,
}

pub enum DataBlockValue<'a> {
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    #[cfg(feature = "sparql-12")]
    TripleTerm(TripleTermData<'a>),
    Undef,
}

pub enum GraphPattern<'a> {
    Group(Vec<Spanned<GraphPatternElement<'a>>>),
    SubSelect(Box<SubSelect<'a>>),
}

pub enum GraphPatternElement<'a> {
    Filter(Spanned<Expression<'a>>),
    Union(Vec<GraphPattern<'a>>),
    Minus(Box<GraphPattern<'a>>),
    Values(ValuesClause<'a>),
    Bind(Spanned<Expression<'a>>, Var<'a>),
    Service {
        silent: bool,
        name: VarOrIri<'a>,
        pattern: Box<GraphPattern<'a>>,
    },
    Graph {
        name: VarOrIri<'a>,
        pattern: Box<GraphPattern<'a>>,
    },
    Optional(Box<GraphPattern<'a>>),
    Triples(Vec<(GraphNodePath<'a>, PropertyListPath<'a>)>),
    #[cfg(feature = "sep-0006")]
    Lateral(Box<GraphPattern<'a>>),
}

#[cfg(feature = "sparql-12")]
#[derive(Clone, Copy)]
pub enum VarOrReifierId<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    BlankNode(Spanned<BlankNode<'a>>),
}

pub type PropertyListPath<'a> = Vec<(VarOrPath<'a>, Vec<ObjectPath<'a>>)>;
pub type PropertyList<'a> = Vec<(Verb<'a>, Vec<Object<'a>>)>;

#[derive(Clone, Copy)]
pub enum Verb<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    A,
}

pub enum VarOrPath<'a> {
    Var(Var<'a>),
    Path(Path<'a>),
}

pub struct ObjectPath<'a> {
    pub graph_node: GraphNodePath<'a>,
    #[cfg(feature = "sparql-12")]
    pub annotations: Vec<Spanned<AnnotationPath<'a>>>,
}

#[derive(Clone)]
pub struct Object<'a> {
    pub graph_node: GraphNode<'a>,
    #[cfg(feature = "sparql-12")]
    pub annotations: Vec<Spanned<Annotation<'a>>>,
}

pub enum Path<'a> {
    Alternative(Box<Self>, Box<Self>),
    Sequence(Box<Self>, Box<Self>),
    Inverse(Box<Self>),
    ZeroOrOne(Box<Self>),
    ZeroOrMore(Box<Self>),
    OneOrMore(Box<Self>),
    Iri(Iri<'a>),
    A,
    NegatedPropertySet(Vec<PathOneInPropertySet<'a>>),
}

pub enum PathOneInPropertySet<'a> {
    Iri(Iri<'a>),
    A,
    InverseIri(Iri<'a>),
    InverseA,
}

pub enum GraphNodePath<'a> {
    VarOrTerm(VarOrTerm<'a>),
    Collection(Vec<GraphNodePath<'a>>),
    BlankNodePropertyList(PropertyListPath<'a>),
    #[cfg(feature = "sparql-12")]
    ReifiedTriple(ReifiedTriple<'a>),
}

#[derive(Clone)]
pub enum GraphNode<'a> {
    VarOrTerm(VarOrTerm<'a>),
    Collection(Vec<GraphNode<'a>>),
    BlankNodePropertyList(PropertyList<'a>),
    #[cfg(feature = "sparql-12")]
    ReifiedTriple(ReifiedTriple<'a>),
}

#[cfg(feature = "sparql-12")]
pub enum AnnotationPath<'a> {
    Reifier(Option<VarOrReifierId<'a>>),
    AnnotationBlock(PropertyListPath<'a>),
}

#[cfg(feature = "sparql-12")]
#[derive(Clone)]
pub enum Annotation<'a> {
    Reifier(Option<VarOrReifierId<'a>>),
    AnnotationBlock(PropertyList<'a>),
}

#[derive(Clone, Copy)]
pub enum VarOrIri<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
}

#[derive(Clone)]
pub enum VarOrTerm<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    BlankNode(Spanned<BlankNode<'a>>),
    Nil,
    #[cfg(feature = "sparql-12")]
    TripleTerm(Box<TripleTerm<'a>>),
}

#[cfg(feature = "sparql-12")]
#[derive(Clone)]
pub struct TripleTerm<'a> {
    pub subject: VarOrTerm<'a>,
    pub predicate: Verb<'a>,
    pub object: VarOrTerm<'a>,
}

#[cfg(feature = "sparql-12")]
pub struct TripleTermData<'a> {
    pub subject: Iri<'a>,
    pub predicate: IriOrA<'a>,
    pub object: TripleTermDataObject<'a>,
}

#[cfg(feature = "sparql-12")]
pub enum TripleTermDataObject<'a> {
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    #[cfg(feature = "sparql-12")]
    TripleTerm(Box<TripleTermData<'a>>),
}

#[cfg(feature = "sparql-12")]
pub enum IriOrA<'a> {
    Iri(Iri<'a>),
    A,
}

#[cfg(feature = "sparql-12")]
#[derive(Clone)]
pub struct ReifiedTriple<'a> {
    pub subject: ReifiedTripleSubjectOrObject<'a>,
    pub predicate: Verb<'a>,
    pub object: ReifiedTripleSubjectOrObject<'a>,
    pub reifier: Option<VarOrReifierId<'a>>,
}

#[cfg(feature = "sparql-12")]
#[derive(Clone)]
pub enum ReifiedTripleSubjectOrObject<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    BlankNode(Spanned<BlankNode<'a>>),
    #[cfg(feature = "sparql-12")]
    ReifiedTriple(Box<ReifiedTriple<'a>>),
    #[cfg(feature = "sparql-12")]
    TripleTerm(Box<TripleTerm<'a>>),
}

pub enum Expression<'a> {
    Or(Box<Spanned<Self>>, Box<Spanned<Self>>),
    And(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Equal(Box<Spanned<Self>>, Box<Spanned<Self>>),
    NotEqual(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Less(Box<Spanned<Self>>, Box<Spanned<Self>>),
    LessOrEqual(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Greater(Box<Spanned<Self>>, Box<Spanned<Self>>),
    GreaterOrEqual(Box<Spanned<Self>>, Box<Spanned<Self>>),
    In(Box<Spanned<Self>>, Vec<Spanned<Self>>),
    NotIn(Box<Spanned<Self>>, Vec<Spanned<Self>>),
    Add(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Subtract(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Multiply(Box<Spanned<Self>>, Box<Spanned<Self>>),
    Divide(Box<Spanned<Self>>, Box<Spanned<Self>>),
    UnaryPlus(Box<Spanned<Self>>),
    UnaryMinus(Box<Spanned<Self>>),
    Not(Box<Spanned<Self>>),
    Bound(Var<'a>),
    Aggregate(Aggregate<'a>),
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    #[cfg(feature = "sparql-12")]
    TripleTerm(ExprTripleTerm<'a>),
    Var(Var<'a>),
    BuiltIn(BuiltInName, Vec<Spanned<Self>>),
    Function(Iri<'a>, ArgList<'a>),
    Exists(Box<GraphPattern<'a>>),
    NotExists(Box<GraphPattern<'a>>),
}

#[cfg(feature = "sparql-12")]
pub struct ExprTripleTerm<'a> {
    pub subject: ExprTripleTermSubject<'a>,
    pub predicate: Verb<'a>,
    pub object: ExprTripleTermObject<'a>,
}

#[cfg(feature = "sparql-12")]
pub enum ExprTripleTermSubject<'a> {
    Iri(Iri<'a>),
    Var(Var<'a>),
}

#[cfg(feature = "sparql-12")]
pub enum ExprTripleTermObject<'a> {
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    Var(Var<'a>),
    TripleTerm(Box<ExprTripleTerm<'a>>),
}

pub enum BuiltInName {
    Coalesce,
    If,
    SameTerm,
    Str,
    Lang,
    LangMatches,
    Datatype,
    Iri,
    Uri,
    BNode,
    Rand,
    Abs,
    Ceil,
    Floor,
    Round,
    Concat,
    SubStr,
    StrLen,
    Replace,
    UCase,
    LCase,
    EncodeForUri,
    Contains,
    StrStarts,
    StrEnds,
    StrBefore,
    StrAfter,
    Year,
    Month,
    Day,
    Hours,
    Minutes,
    Seconds,
    Timezone,
    Tz,
    Now,
    Uuid,
    StrUuid,
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
    StrLang,
    StrDt,
    IsIri,
    IsUri,
    IsBlank,
    IsLiteral,
    IsNumeric,
    Regex,
    #[cfg(feature = "sparql-12")]
    Triple,
    #[cfg(feature = "sparql-12")]
    Subject,
    #[cfg(feature = "sparql-12")]
    Predicate,
    #[cfg(feature = "sparql-12")]
    Object,
    #[cfg(feature = "sparql-12")]
    IsTriple,
    #[cfg(feature = "sparql-12")]
    LangDir,
    #[cfg(feature = "sparql-12")]
    HasLang,
    #[cfg(feature = "sparql-12")]
    HasLangDir,
    #[cfg(feature = "sparql-12")]
    StrLangDir,
    #[cfg(feature = "sep-0002")]
    Adjust,
}

pub enum Aggregate<'a> {
    Count(bool, Option<Box<Spanned<Expression<'a>>>>),
    Sum(bool, Box<Spanned<Expression<'a>>>),
    Min(bool, Box<Spanned<Expression<'a>>>),
    Max(bool, Box<Spanned<Expression<'a>>>),
    Avg(bool, Box<Spanned<Expression<'a>>>),
    Sample(bool, Box<Spanned<Expression<'a>>>),
    GroupConcat(bool, Box<Spanned<Expression<'a>>>, Option<String<'a>>),
}

pub struct ArgList<'a> {
    pub distinct: bool,
    pub args: Vec<Spanned<Expression<'a>>>,
}

#[derive(Clone, Copy)]
pub struct Var<'a>(pub &'a str);

#[derive(Clone, Copy)]
pub enum Literal<'a> {
    Boolean(bool),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    String(String<'a>),
    LangString(String<'a>, Spanned<&'a str>),
    #[cfg(feature = "sparql-12")]
    DirLangString(String<'a>, Spanned<(&'a str, &'a str)>),
    Typed(String<'a>, Iri<'a>),
}

#[derive(Clone, Copy)]
pub struct String<'a>(pub &'a str);

#[derive(Clone, Copy)]
pub enum Iri<'a> {
    IriRef(Spanned<IriRef<'a>>),
    PrefixedName(Spanned<PrefixedName<'a>>),
}

#[derive(Clone, Copy)]
pub struct PrefixedName<'a>(pub &'a str, pub &'a str);

#[derive(Clone, Copy)]
pub struct IriRef<'a>(pub &'a str);

#[derive(Clone, Copy)]
pub struct BlankNode<'a>(pub Option<&'a str>);

pub struct Update<'a>(pub Vec<(Vec<PrologueDecl<'a>>, Option<Update1<'a>>)>);

pub enum Update1<'a> {
    Load {
        silent: bool,
        from: Iri<'a>,
        to: Option<Iri<'a>>,
    },
    Clear {
        silent: bool,
        graph: GraphRefAll<'a>,
    },
    Drop {
        silent: bool,
        graph: GraphRefAll<'a>,
    },
    Create {
        silent: bool,
        graph: Iri<'a>,
    },
    Add {
        #[expect(dead_code)]
        silent: bool,
        from: GraphOrDefault<'a>,
        to: GraphOrDefault<'a>,
    },
    Move {
        silent: bool,
        from: GraphOrDefault<'a>,
        to: GraphOrDefault<'a>,
    },
    Copy {
        #[expect(dead_code)]
        silent: bool,
        from: GraphOrDefault<'a>,
        to: GraphOrDefault<'a>,
    },
    DeleteWhere {
        pattern: QuadPatterns<'a>,
    },
    Modify {
        with: Option<Iri<'a>>,
        delete: QuadPatterns<'a>,
        insert: QuadPatterns<'a>,
        using: Vec<GraphClause<'a>>,
        r#where: GraphPattern<'a>,
    },
    InsertData {
        quads: QuadPatterns<'a>,
    },
    DeleteData {
        quads: QuadPatterns<'a>,
    },
}

pub type QuadPatterns<'a> = Vec<(Option<VarOrIri<'a>>, Vec<(GraphNode<'a>, PropertyList<'a>)>)>;

#[derive(Clone, Copy)]
pub enum GraphRefAll<'a> {
    Graph(Iri<'a>),
    Default,
    Named,
    All,
}

#[derive(Clone, Copy)]
pub enum GraphOrDefault<'a> {
    Graph(Iri<'a>),
    Default,
}
