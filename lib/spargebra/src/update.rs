//! Data structures around SPARQL updates. The main type is [`Update`].
use crate::SparqlParser;
use crate::algebra::*;
use crate::error::SparqlSyntaxError;
use crate::term::*;
use oxiri::Iri;
use std::fmt;
use std::str::FromStr;

/// A parsed [SPARQL update](https://www.w3.org/TR/sparql11-update/).
///
/// ```
/// use spargebra::SparqlParser;
///
/// let update_str = "CLEAR ALL ;";
/// let update = SparqlParser::new().parse_update(update_str)?;
/// assert_eq!(update.to_string().trim(), update_str);
/// assert_eq!(update.to_sse(), "(update (clear all))");
/// # Ok::<_, spargebra::SparqlSyntaxError>(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Update {
    /// The update base IRI.
    pub base_iri: Option<Iri<String>>,
    /// The [update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate).
    pub operations: Vec<GraphUpdateOperation>,
}

impl Update {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{base_iri}> ")?;
        }
        f.write_str("(update")?;
        for op in &self.operations {
            f.write_str(" ")?;
            op.fmt_sse(f)?;
        }
        f.write_str(")")?;
        if self.base_iri.is_some() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{base_iri}>")?;
        }
        for update in &self.operations {
            writeln!(f, "{update} ;")?;
        }
        Ok(())
    }
}

impl FromStr for Update {
    type Err = SparqlSyntaxError;

    fn from_str(update: &str) -> Result<Self, Self::Err> {
        SparqlParser::new().parse_update(update)
    }
}

impl TryFrom<&str> for Update {
    type Error = SparqlSyntaxError;

    fn try_from(update: &str) -> Result<Self, Self::Error> {
        Self::from_str(update)
    }
}

impl TryFrom<&String> for Update {
    type Error = SparqlSyntaxError;

    fn try_from(update: &String) -> Result<Self, Self::Error> {
        Self::from_str(update)
    }
}

/// The [graph update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphUpdateOperation {
    /// [insert data](https://www.w3.org/TR/sparql11-update/#defn_insertDataOperation).
    InsertData(InsertDataOperation),
    /// [delete data](https://www.w3.org/TR/sparql11-update/#defn_deleteDataOperation).
    DeleteData(DeleteDataOperation),
    /// [delete insert](https://www.w3.org/TR/sparql11-update/#defn_deleteInsertOperation).
    DeleteInsert(DeleteInsertOperation),
    /// [load](https://www.w3.org/TR/sparql11-update/#defn_loadOperation).
    Load(LoadOperation),
    /// [clear](https://www.w3.org/TR/sparql11-update/#defn_clearOperation).
    Clear(ClearOperation),
    /// [create](https://www.w3.org/TR/sparql11-update/#defn_createOperation).
    Create(CreateOperation),
    /// [drop](https://www.w3.org/TR/sparql11-update/#defn_dropOperation).
    Drop(DropOperation),
}

impl GraphUpdateOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::InsertData(op) => op.fmt_sse(f),
            Self::DeleteData(op) => op.fmt_sse(f),
            Self::DeleteInsert(op) => op.fmt_sse(f),
            Self::Load(op) => op.fmt_sse(f),
            Self::Clear(op) => op.fmt_sse(f),
            Self::Create(op) => op.fmt_sse(f),
            Self::Drop(op) => op.fmt_sse(f),
        }
    }
}

impl fmt::Display for GraphUpdateOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsertData(op) => op.fmt(f),
            Self::DeleteData(op) => op.fmt(f),
            Self::DeleteInsert(op) => op.fmt(f),
            Self::Load(op) => op.fmt(f),
            Self::Clear(op) => op.fmt(f),
            Self::Create(op) => op.fmt(f),
            Self::Drop(op) => op.fmt(f),
        }
    }
}

/// The [insert data](https://www.w3.org/TR/sparql11-update/#defn_insertDataOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct InsertDataOperation {
    pub data: Vec<Quad>,
}

impl InsertDataOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(insertData (")?;
        for (i, t) in self.data.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            t.fmt_sse(f)?;
        }
        f.write_str("))")
    }
}

impl fmt::Display for InsertDataOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "INSERT DATA {{")?;
        serialize_quads(&self.data, f)?;
        f.write_str("}")
    }
}

impl From<InsertDataOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: InsertDataOperation) -> Self {
        Self::InsertData(op)
    }
}

/// The [delete data](https://www.w3.org/TR/sparql11-update/#defn_deleteDataOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct DeleteDataOperation {
    pub data: Vec<GroundQuad>,
}

impl DeleteDataOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(deleteData (")?;
        for (i, t) in self.data.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            t.fmt_sse(f)?;
        }
        f.write_str("))")
    }
}

impl fmt::Display for DeleteDataOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "DELETE DATA {{")?;
        write_ground_quads(&self.data, f)?;
        f.write_str("}")
    }
}

impl From<DeleteDataOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: DeleteDataOperation) -> Self {
        Self::DeleteData(op)
    }
}

/// The [delete insert](https://www.w3.org/TR/sparql11-update/#defn_deleteInsertOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct DeleteInsertOperation {
    pub delete: Vec<GroundQuadPattern>,
    pub insert: Vec<QuadPattern>,
    pub using: Option<QueryDataset>,
    pub pattern: Box<GraphPattern>,
}

impl DeleteInsertOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(modify ")?;
        if let Some(using) = &self.using {
            f.write_str(" (using ")?;
            using.fmt_sse(f)?;
            f.write_str(" ")?;
            self.pattern.fmt_sse(f)?;
            f.write_str(")")?;
        } else {
            self.pattern.fmt_sse(f)?;
        }
        if !self.delete.is_empty() {
            f.write_str(" (delete (")?;
            for (i, t) in self.delete.iter().enumerate() {
                if i > 0 {
                    f.write_str(" ")?;
                }
                t.fmt_sse(f)?;
            }
            f.write_str("))")?;
        }
        if !self.insert.is_empty() {
            f.write_str(" (insert (")?;
            for (i, t) in self.insert.iter().enumerate() {
                if i > 0 {
                    f.write_str(" ")?;
                }
                t.fmt_sse(f)?;
            }
            f.write_str("))")?;
        }
        f.write_str(")")
    }
}

impl fmt::Display for DeleteInsertOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.delete.is_empty() {
            writeln!(f, "DELETE {{")?;
            for quad in &self.delete {
                writeln!(f, "\t{quad} .")?;
            }
            writeln!(f, "}}")?;
        }
        if !self.insert.is_empty() || self.delete.is_empty() {
            writeln!(f, "INSERT {{")?;
            for quad in &self.insert {
                writeln!(f, "\t{quad} .")?;
            }
            writeln!(f, "}}")?;
        }
        if let Some(using) = &self.using {
            for g in &using.default {
                writeln!(f, "USING {g}")?;
            }
            if let Some(named) = &using.named {
                for g in named {
                    writeln!(f, "USING NAMED {g}")?;
                }
            }
        }
        let mut pattern = &*self.pattern;
        // We ignore the root projection, it's useless
        if let GraphPattern::Project { inner, .. } = pattern {
            pattern = inner;
        }
        write!(f, " WHERE {{ {pattern} }}")
    }
}

impl From<DeleteInsertOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: DeleteInsertOperation) -> Self {
        Self::DeleteInsert(op)
    }
}

/// The [load](https://www.w3.org/TR/sparql11-update/#defn_loadOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct LoadOperation {
    pub silent: bool,
    pub source: NamedNode,
    pub destination: GraphName,
}

impl LoadOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(load ")?;
        if self.silent {
            f.write_str("silent ")?;
        }
        write!(f, "{} ", self.source)?;
        self.destination.fmt_sse(f)?;
        f.write_str(")")
    }
}

impl fmt::Display for LoadOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LOAD ")?;
        if self.silent {
            f.write_str("SILENT ")?;
        }
        write!(f, "{}", self.source)?;
        if self.destination != GraphName::DefaultGraph {
            write!(f, " INTO GRAPH {}", self.destination)?;
        }
        Ok(())
    }
}

impl From<LoadOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: LoadOperation) -> Self {
        Self::Load(op)
    }
}

/// The [clear](https://www.w3.org/TR/sparql11-update/#defn_clearOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct ClearOperation {
    pub silent: bool,
    pub graph: GraphTarget,
}

impl ClearOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(clear ")?;
        if self.silent {
            f.write_str("silent ")?;
        }
        self.graph.fmt_sse(f)?;
        f.write_str(")")
    }
}

impl fmt::Display for ClearOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CLEAR ")?;
        if self.silent {
            f.write_str("SILENT ")?;
        }
        write!(f, "{}", self.graph)
    }
}

impl From<ClearOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: ClearOperation) -> Self {
        Self::Clear(op)
    }
}

/// The [create](https://www.w3.org/TR/sparql11-update/#defn_createOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct CreateOperation {
    pub silent: bool,
    pub graph: NamedNode,
}

impl CreateOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(create ")?;
        if self.silent {
            f.write_str("silent ")?;
        }
        write!(f, "{})", self.graph)
    }
}

impl fmt::Display for CreateOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if self.silent {
            f.write_str("SILENT ")?;
        }
        write!(f, "GRAPH {}", self.graph)
    }
}

impl From<CreateOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: CreateOperation) -> Self {
        Self::Create(op)
    }
}

/// The [drop](https://www.w3.org/TR/sparql11-update/#defn_dropOperation) update operation.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct DropOperation {
    pub silent: bool,
    pub graph: GraphTarget,
}

impl DropOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(drop ")?;
        if self.silent {
            f.write_str("silent ")?;
        }
        self.graph.fmt_sse(f)?;
        f.write_str(")")
    }
}

impl fmt::Display for DropOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP ")?;
        if self.silent {
            f.write_str("SILENT ")?;
        }
        write!(f, "{}", self.graph)
    }
}

impl From<DropOperation> for GraphUpdateOperation {
    #[inline]
    fn from(op: DropOperation) -> Self {
        Self::Drop(op)
    }
}

fn serialize_quads(quads: &[Quad], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for quad in quads {
        if quad.graph_name == GraphName::DefaultGraph {
            writeln!(f, "\t{} {} {} .", quad.subject, quad.predicate, quad.object)?;
        } else {
            writeln!(
                f,
                "\tGRAPH {} {{ {} {} {} }}",
                quad.graph_name, quad.subject, quad.predicate, quad.object
            )?;
        }
    }
    Ok(())
}

fn write_ground_quads(quads: &[GroundQuad], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for quad in quads {
        if quad.graph_name == GraphName::DefaultGraph {
            writeln!(f, "\t{} {} {} .", quad.subject, quad.predicate, quad.object)?;
        } else {
            writeln!(
                f,
                "\tGRAPH {} {{ {} {} {} }}",
                quad.graph_name, quad.subject, quad.predicate, quad.object
            )?;
        }
    }
    Ok(())
}
