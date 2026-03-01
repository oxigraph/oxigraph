//!  for the sparql grammar, i.e. intermediate representation between the parser and the algebra
//! Does not do things like resolving prefixes, relative IRIs...

pub struct Query<'a> {
    pub prologue: Vec<PrologueDecl<'a>>,
    pub query: QueryQuery<'a>,
    pub values_clause: Option<ValuesClause<'a>>,
}

pub enum PrologueDecl<'a> {
    Base(IriRef<'a>),
    Prefix(&'a str, IriRef<'a>),
    #[cfg(feature = "sparql-12")]
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

pub struct SelectClause<'a> {
    pub option: SelectionOption,
    pub bindings: Vec<(Option<Expression<'a>>, Var<'a>)>,
}

pub enum SelectionOption {
    Default,
    Distinct,
    Reduced,
}

pub struct ConstructQuery<'a> {
    pub template: Vec<(GraphNode<'a>, Vec<(Verb<'a>, Vec<Object<'a>>)>)>,
    pub dataset_clause: Vec<GraphClause<'a>>,
    pub where_clause: Option<GraphPattern<'a>>,
    pub solution_modifier: SolutionModifier<'a>,
}

pub struct DescribeQuery<'a> {
    pub targets: Vec<VarOrIri<'a>>,
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
    pub group_clause: Vec<(Expression<'a>, Option<Var<'a>>)>,
    pub having_clause: Vec<Expression<'a>>,
    pub order_clause: Vec<OrderCondition<'a>>,
    pub limit_offset_clauses: Option<LimitOffsetClauses>,
}

pub enum OrderCondition<'a> {
    Asc(Expression<'a>),
    Desc(Expression<'a>),
}

pub struct LimitOffsetClauses {
    pub offset: usize,
    pub limit: Option<usize>,
}

pub struct ValuesClause<'a> {
    pub variables: Vec<Var<'a>>,
    pub values: Vec<Vec<DataBlockValue<'a>>>,
}

pub enum DataBlockValue<'a> {
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    Undef,
}

pub enum GraphPattern<'a> {
    Filter(Expression<'a>),
    Union(Vec<Self>),
    Minus(Box<Self>),
    Values {
        variables: Vec<Var<'a>>,
        values: Vec<Vec<DataBlockValue<'a>>>,
    },
    Bind(Expression<'a>, Var<'a>),
    Service {
        silent: bool,
        name: VarOrIri<'a>,
        pattern: Box<Self>,
    },
    Graph {
        name: VarOrIri<'a>,
        pattern: Box<Self>,
    },
    Optional(Box<Self>),
    Triples(Vec<(GraphNodePath<'a>, Vec<(VarOrPath<'a>, Vec<ObjectPath<'a>>)>)>),
    Group(Vec<Self>),
}

#[cfg(feature = "sparql-12")]
pub enum VarOrReifierId<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    BlankNode(BlankNode<'a>),
}

pub type PropertyListPath<'a> = Vec<(VarOrPath<'a>, Vec<ObjectPath<'a>>)>;
pub type PropertyList<'a> = Vec<(Verb<'a>, Vec<Object<'a>>)>;

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
    pub annotation: Vec<AnnotationPath<'a>>,
}

pub struct Object<'a> {
    pub graph_node: GraphNode<'a>,
    #[cfg(feature = "sparql-12")]
    pub annotation: Vec<Annotation<'a>>,
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
}

pub enum GraphNode<'a> {
    VarOrTerm(VarOrTerm<'a>),
    Collection(Vec<GraphNode<'a>>),
    BlankNodePropertyList(PropertyList<'a>),
}

#[cfg(feature = "sparql-12")]
pub enum AnnotationPath<'a> {
    Reifier(Option<VarOrReifierId<'a>>),
    AnnotationBlock(PropertyListPath<'a>),
}

#[cfg(feature = "sparql-12")]
pub enum Annotation<'a> {
    Reifier(Option<VarOrReifierId<'a>>),
    AnnotationBlock(PropertyList<'a>),
}

pub enum VarOrIri<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
}

pub enum VarOrTerm<'a> {
    Var(Var<'a>),
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    BlankNode(BlankNode<'a>),
    Nil,
}

pub enum Expression<'a> {
    Or(Box<Self>, Box<Self>),
    And(Box<Self>, Box<Self>),
    Equal(Box<Self>, Box<Self>),
    NotEqual(Box<Self>, Box<Self>),
    Less(Box<Self>, Box<Self>),
    LessOrEqual(Box<Self>, Box<Self>),
    Greater(Box<Self>, Box<Self>),
    GreaterOrEqual(Box<Self>, Box<Self>),
    In(Box<Self>, Vec<Self>),
    NotIn(Box<Self>, Vec<Self>),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    UnaryPlus(Box<Self>),
    UnaryMinus(Box<Self>),
    Not(Box<Self>),
    Bound(Var<'a>),
    Aggregate(Aggregate<'a>),
    Iri(Iri<'a>),
    Literal(Literal<'a>),
    Var(Var<'a>),
    BuiltIn(BuiltInName, Vec<Expression<'a>>),
    Function(Iri<'a>, ArgList<'a>),
    Exists(Box<GraphPattern<'a>>),
    NotExists(Box<GraphPattern<'a>>),
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
    Count(bool, Option<Box<Expression<'a>>>),
    Sum(bool, Box<Expression<'a>>),
    Min(bool, Box<Expression<'a>>),
    Max(bool, Box<Expression<'a>>),
    Avg(bool, Box<Expression<'a>>),
    Sample(bool, Box<Expression<'a>>),
    GroupConcat(bool, Box<Expression<'a>>, Option<String<'a>>),
}

pub struct ArgList<'a> {
    pub distinct: bool,
    pub args: Vec<Expression<'a>>,
}

pub struct Var<'a>(pub &'a str);

pub enum Literal<'a> {
    Boolean(bool),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    String(String<'a>),
    LangString(String<'a>, &'a str),
    #[cfg(feature = "sparql-12")]
    DirLangString(String<'a>, &'a str, &'a str),
    Typed(String<'a>, Iri<'a>),
}

pub struct String<'a>(pub &'a str);

pub enum Iri<'a> {
    IriRef(IriRef<'a>),
    PrefixedName(PrefixedName<'a>),
}

pub struct PrefixedName<'a>(pub &'a str, pub &'a str);

pub struct PName<'a>(pub &'a str);

pub struct IriRef<'a>(pub &'a str);

pub struct BlankNode<'a>(pub Option<&'a str>);
