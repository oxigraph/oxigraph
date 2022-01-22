use crate::algebra::*;
use crate::parser::{parse_update, ParseError};
use crate::term::*;
use oxiri::Iri;
use std::fmt;
use std::str::FromStr;

/// A parsed [SPARQL update](https://www.w3.org/TR/sparql11-update/).
///
/// ```
/// use spargebra::Update;
///
/// let update_str = "CLEAR ALL ;";
/// let update = Update::parse(update_str, None)?;
/// assert_eq!(update.to_string().trim(), update_str);
/// assert_eq!(update.to_sse(), "(update (clear all))");
/// # Result::Ok::<_, spargebra::ParseError>(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Update {
    /// The update base IRI.
    pub base_iri: Option<Iri<String>>,
    /// The [update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate).
    pub operations: Vec<GraphUpdateOperation>,
}

impl Update {
    /// Parses a SPARQL update with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(update: &str, base_iri: Option<&str>) -> Result<Self, ParseError> {
        parse_update(update, base_iri)
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer)
            .expect("Unexpected error during SSE formatting");
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{}> ", base_iri)?;
        }
        write!(f, "(update")?;
        for op in &self.operations {
            write!(f, " ")?;
            op.fmt_sse(f)?;
        }
        write!(f, ")")?;
        if self.base_iri.is_some() {
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{}>", base_iri)?;
        }
        for update in &self.operations {
            writeln!(f, "{} ;", update)?;
        }
        Ok(())
    }
}

impl FromStr for Update {
    type Err = ParseError;

    fn from_str(update: &str) -> Result<Self, ParseError> {
        Self::parse(update, None)
    }
}

impl<'a> TryFrom<&'a str> for Update {
    type Error = ParseError;

    fn try_from(update: &str) -> Result<Self, ParseError> {
        Self::from_str(update)
    }
}

impl<'a> TryFrom<&'a String> for Update {
    type Error = ParseError;

    fn try_from(update: &String) -> Result<Self, ParseError> {
        Self::from_str(update)
    }
}

/// The [graph update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphUpdateOperation {
    /// [insert data](https://www.w3.org/TR/sparql11-update/#defn_insertDataOperation).
    InsertData { data: Vec<Quad> },
    /// [delete data](https://www.w3.org/TR/sparql11-update/#defn_deleteDataOperation).
    DeleteData { data: Vec<GroundQuad> },
    /// [delete insert](https://www.w3.org/TR/sparql11-update/#defn_deleteInsertOperation).
    DeleteInsert {
        delete: Vec<GroundQuadPattern>,
        insert: Vec<QuadPattern>,
        using: Option<QueryDataset>,
        pattern: Box<GraphPattern>,
    },
    /// [load](https://www.w3.org/TR/sparql11-update/#defn_loadOperation).
    Load {
        silent: bool,
        source: NamedNode,
        destination: GraphName,
    },
    /// [clear](https://www.w3.org/TR/sparql11-update/#defn_clearOperation).
    Clear { silent: bool, graph: GraphTarget },
    /// [create](https://www.w3.org/TR/sparql11-update/#defn_createOperation).
    Create { silent: bool, graph: NamedNode },
    /// [drop](https://www.w3.org/TR/sparql11-update/#defn_dropOperation).
    Drop { silent: bool, graph: GraphTarget },
}

impl GraphUpdateOperation {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            GraphUpdateOperation::InsertData { data } => {
                write!(f, "(insertData (")?;
                for (i, t) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    t.fmt_sse(f)?;
                }
                write!(f, "))")
            }
            GraphUpdateOperation::DeleteData { data } => {
                write!(f, "(deleteData (")?;
                for (i, t) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    t.fmt_sse(f)?;
                }
                write!(f, "))")
            }
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => {
                write!(f, "(modify ")?;
                if let Some(using) = using {
                    write!(f, " (using ")?;
                    using.fmt_sse(f)?;
                    write!(f, " ")?;
                    pattern.fmt_sse(f)?;
                    write!(f, ")")?;
                } else {
                    pattern.fmt_sse(f)?;
                }
                if !delete.is_empty() {
                    write!(f, " (delete (")?;
                    for (i, t) in delete.iter().enumerate() {
                        if i > 0 {
                            write!(f, " ")?;
                        }
                        t.fmt_sse(f)?;
                    }
                    write!(f, "))")?;
                }
                if !insert.is_empty() {
                    write!(f, " (insert (")?;
                    for (i, t) in insert.iter().enumerate() {
                        if i > 0 {
                            write!(f, " ")?;
                        }
                        t.fmt_sse(f)?;
                    }
                    write!(f, "))")?;
                }
                write!(f, ")")
            }
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                write!(f, "(load ")?;
                if *silent {
                    write!(f, "silent ")?;
                }
                write!(f, "{} ", source)?;
                destination.fmt_sse(f)?;
                write!(f, ")")
            }
            GraphUpdateOperation::Clear { silent, graph } => {
                write!(f, "(clear ")?;
                if *silent {
                    write!(f, "silent ")?;
                }
                graph.fmt_sse(f)?;
                write!(f, ")")
            }
            GraphUpdateOperation::Create { silent, graph } => {
                write!(f, "(create ")?;
                if *silent {
                    write!(f, "silent ")?;
                }
                write!(f, "{})", graph)
            }
            GraphUpdateOperation::Drop { silent, graph } => {
                write!(f, "(drop ")?;
                if *silent {
                    write!(f, "silent ")?;
                }
                graph.fmt_sse(f)?;
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for GraphUpdateOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphUpdateOperation::InsertData { data } => {
                writeln!(f, "INSERT DATA {{")?;
                write_quads(data, f)?;
                write!(f, "}}")
            }
            GraphUpdateOperation::DeleteData { data } => {
                writeln!(f, "DELETE DATA {{")?;
                write_ground_quads(data, f)?;
                write!(f, "}}")
            }
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => {
                if !delete.is_empty() {
                    writeln!(f, "DELETE {{")?;
                    for quad in delete {
                        writeln!(f, "\t{} .", quad)?;
                    }
                    writeln!(f, "}}")?;
                }
                if !insert.is_empty() {
                    writeln!(f, "INSERT {{")?;
                    for quad in insert {
                        writeln!(f, "\t{} .", quad)?;
                    }
                    writeln!(f, "}}")?;
                }
                if let Some(using) = using {
                    for g in &using.default {
                        writeln!(f, "USING {}", g)?;
                    }
                    if let Some(named) = &using.named {
                        for g in named {
                            writeln!(f, "USING NAMED {}", g)?;
                        }
                    }
                }
                write!(
                    f,
                    "WHERE {{ {} }}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: None
                    }
                )
            }
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                write!(f, "LOAD ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", source)?;
                if destination != &GraphName::DefaultGraph {
                    write!(f, " INTO GRAPH {}", destination)?;
                }
                Ok(())
            }
            GraphUpdateOperation::Clear { silent, graph } => {
                write!(f, "CLEAR ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", graph)
            }
            GraphUpdateOperation::Create { silent, graph } => {
                write!(f, "CREATE ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "GRAPH {}", graph)
            }
            GraphUpdateOperation::Drop { silent, graph } => {
                write!(f, "DROP ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", graph)
            }
        }
    }
}

fn write_quads(quads: &[Quad], f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
