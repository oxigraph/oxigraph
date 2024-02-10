use crate::algebra::*;
use crate::parser::{parse_update, SparqlSyntaxError};
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
    /// Parses a SPARQL update with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(update: &str, base_iri: Option<&str>) -> Result<Self, SparqlSyntaxError> {
        parse_update(update, base_iri)
    }

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
        Self::parse(update, None)
    }
}

impl<'a> TryFrom<&'a str> for Update {
    type Error = SparqlSyntaxError;

    fn try_from(update: &str) -> Result<Self, Self::Error> {
        Self::from_str(update)
    }
}

impl<'a> TryFrom<&'a String> for Update {
    type Error = SparqlSyntaxError;

    fn try_from(update: &String) -> Result<Self, Self::Error> {
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
            Self::InsertData { data } => {
                f.write_str("(insertData (")?;
                for (i, t) in data.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    t.fmt_sse(f)?;
                }
                f.write_str("))")
            }
            Self::DeleteData { data } => {
                f.write_str("(deleteData (")?;
                for (i, t) in data.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    t.fmt_sse(f)?;
                }
                f.write_str("))")
            }
            Self::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => {
                f.write_str("(modify ")?;
                if let Some(using) = using {
                    f.write_str(" (using ")?;
                    using.fmt_sse(f)?;
                    f.write_str(" ")?;
                    pattern.fmt_sse(f)?;
                    f.write_str(")")?;
                } else {
                    pattern.fmt_sse(f)?;
                }
                if !delete.is_empty() {
                    f.write_str(" (delete (")?;
                    for (i, t) in delete.iter().enumerate() {
                        if i > 0 {
                            f.write_str(" ")?;
                        }
                        t.fmt_sse(f)?;
                    }
                    f.write_str("))")?;
                }
                if !insert.is_empty() {
                    f.write_str(" (insert (")?;
                    for (i, t) in insert.iter().enumerate() {
                        if i > 0 {
                            f.write_str(" ")?;
                        }
                        t.fmt_sse(f)?;
                    }
                    f.write_str("))")?;
                }
                f.write_str(")")
            }
            Self::Load {
                silent,
                source,
                destination,
            } => {
                f.write_str("(load ")?;
                if *silent {
                    f.write_str("silent ")?;
                }
                write!(f, "{source} ")?;
                destination.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Clear { silent, graph } => {
                f.write_str("(clear ")?;
                if *silent {
                    f.write_str("silent ")?;
                }
                graph.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Create { silent, graph } => {
                f.write_str("(create ")?;
                if *silent {
                    f.write_str("silent ")?;
                }
                write!(f, "{graph})")
            }
            Self::Drop { silent, graph } => {
                f.write_str("(drop ")?;
                if *silent {
                    f.write_str("silent ")?;
                }
                graph.fmt_sse(f)?;
                f.write_str(")")
            }
        }
    }
}

impl fmt::Display for GraphUpdateOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsertData { data } => {
                writeln!(f, "INSERT DATA {{")?;
                write_quads(data, f)?;
                f.write_str("}")
            }
            Self::DeleteData { data } => {
                writeln!(f, "DELETE DATA {{")?;
                write_ground_quads(data, f)?;
                f.write_str("}")
            }
            Self::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => {
                if !delete.is_empty() {
                    writeln!(f, "DELETE {{")?;
                    for quad in delete {
                        writeln!(f, "\t{quad} .")?;
                    }
                    writeln!(f, "}}")?;
                }
                if !insert.is_empty() {
                    writeln!(f, "INSERT {{")?;
                    for quad in insert {
                        writeln!(f, "\t{quad} .")?;
                    }
                    writeln!(f, "}}")?;
                }
                if let Some(using) = using {
                    for g in &using.default {
                        writeln!(f, "USING {g}")?;
                    }
                    if let Some(named) = &using.named {
                        for g in named {
                            writeln!(f, "USING NAMED {g}")?;
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
            Self::Load {
                silent,
                source,
                destination,
            } => {
                f.write_str("LOAD ")?;
                if *silent {
                    f.write_str("SILENT ")?;
                }
                write!(f, "{source}")?;
                if destination != &GraphName::DefaultGraph {
                    write!(f, " INTO GRAPH {destination}")?;
                }
                Ok(())
            }
            Self::Clear { silent, graph } => {
                f.write_str("CLEAR ")?;
                if *silent {
                    f.write_str("SILENT ")?;
                }
                write!(f, "{graph}")
            }
            Self::Create { silent, graph } => {
                f.write_str("CREATE ")?;
                if *silent {
                    f.write_str("SILENT ")?;
                }
                write!(f, "GRAPH {graph}")
            }
            Self::Drop { silent, graph } => {
                f.write_str("DROP ")?;
                if *silent {
                    f.write_str("SILENT ")?;
                }
                write!(f, "{graph}")
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
