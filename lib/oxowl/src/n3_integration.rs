//! Integration between N3 and OWL 2.
//!
//! This module provides functionality to:
//! - Load OWL ontologies from N3 format
//! - Convert N3 formulas to OWL constructs where applicable
//! - Support N3-based reasoning rules

use crate::error::{OwlParseError, ParseErrorKind};
use crate::ontology::Ontology;
use crate::parser::{OntologyParser, ParserConfig};
use oxrdf::{Graph, Quad};
use oxttl::n3::{N3Parser, N3Quad};
use std::io::Read;

/// Parses an OWL ontology from N3 format.
///
/// # Example
///
/// ```
/// use oxowl::n3_integration::parse_n3_ontology;
///
/// let n3_data = r#"
/// @prefix owl: <http://www.w3.org/2002/07/owl#> .
/// @prefix ex: <http://example.org/> .
///
/// ex:Ontology a owl:Ontology .
/// ex:Dog a owl:Class .
/// ex:Animal a owl:Class .
/// ex:Dog rdfs:subClassOf ex:Animal .
/// "#;
///
/// let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
/// assert!(ontology.axiom_count() > 0);
/// ```
pub fn parse_n3_ontology<R: Read>(reader: R) -> Result<Ontology, OwlParseError> {
    parse_n3_ontology_with_config(reader, ParserConfig::new())
}

/// Parses an OWL ontology from N3 format with custom configuration.
///
/// # Example
///
/// ```
/// use oxowl::n3_integration::parse_n3_ontology_with_config;
/// use oxowl::parser::ParserConfig;
///
/// let n3_data = r#"
/// @prefix owl: <http://www.w3.org/2002/07/owl#> .
/// @prefix ex: <http://example.org/> .
///
/// ex:Dog a owl:Class .
/// "#;
///
/// let config = ParserConfig::new().lenient();
/// let ontology = parse_n3_ontology_with_config(n3_data.as_bytes(), config).unwrap();
/// ```
pub fn parse_n3_ontology_with_config<R: Read>(
    reader: R,
    config: ParserConfig,
) -> Result<Ontology, OwlParseError> {
    // Parse N3 into quads
    let quads = parse_n3_to_quads(reader)?;

    // Convert quads to graph
    let graph = quads_to_graph(&quads);

    // Parse OWL ontology from the graph
    let mut parser = OntologyParser::with_config(&graph, config);
    parser.parse()
}

/// Parses N3 input into a vector of quads.
///
/// This handles N3-specific features like formulas and variables,
/// converting them into RDF quads where possible.
fn parse_n3_to_quads<R: Read>(reader: R) -> Result<Vec<Quad>, OwlParseError> {
    let parser = N3Parser::new().for_reader(reader);
    let mut quads = Vec::new();

    for result in parser {
        match result {
            Ok(n3_quad) => {
                // Try to convert N3Quad to standard Quad
                if let Some(quad) = n3_quad_to_quad(n3_quad) {
                    quads.push(quad);
                }
                // Skip quads with variables or other N3-specific features that
                // don't translate directly to OWL
            }
            Err(e) => {
                return Err(OwlParseError::new(
                    ParseErrorKind::Syntax,
                    format!("N3 parsing error: {}", e),
                ));
            }
        }
    }

    Ok(quads)
}

/// Converts an N3Quad to a standard RDF Quad if possible.
///
/// Returns None if the N3Quad contains variables or other N3-specific
/// features that cannot be represented in standard RDF.
fn n3_quad_to_quad(n3_quad: N3Quad) -> Option<Quad> {
    use oxrdf::{NamedOrBlankNode, Subject, Term};
    use oxttl::n3::N3Term;

    // Convert subject
    #[allow(unreachable_patterns)]
    let subject = match n3_quad.subject {
        N3Term::NamedNode(n) => Subject::NamedNode(n),
        N3Term::BlankNode(b) => Subject::BlankNode(b),
        N3Term::Variable(_) => return None, // Skip variables
        N3Term::Literal(_) => return None,  // Invalid as subject in standard RDF
        #[cfg(feature = "rdf-12")]
        N3Term::Triple(_) => return None,   // RDF-star triples require special handling
        #[cfg(not(feature = "rdf-12"))]
        _ => return None, // Catch-all for any other N3-specific terms
    };

    // Convert predicate
    let predicate = match n3_quad.predicate {
        N3Term::NamedNode(n) => n,
        _ => return None, // Predicate must be a named node
    };

    // Convert object
    #[allow(unreachable_patterns)]
    let object = match n3_quad.object {
        N3Term::NamedNode(n) => Term::NamedNode(n),
        N3Term::BlankNode(b) => Term::BlankNode(b),
        N3Term::Literal(l) => Term::Literal(l),
        N3Term::Variable(_) => return None, // Skip variables
        #[cfg(feature = "rdf-12")]
        N3Term::Triple(_) => return None,   // RDF-star triples require special handling
        #[cfg(not(feature = "rdf-12"))]
        _ => return None, // Catch-all for any other N3-specific terms
    };

    Some(Quad {
        subject,
        predicate,
        object,
        graph_name: n3_quad.graph_name,
    })
}

/// Converts a collection of quads into a Graph.
///
/// Quads from named graphs are included in the default graph for
/// OWL ontology parsing purposes.
fn quads_to_graph(quads: &[Quad]) -> Graph {
    let mut graph = Graph::new();

    for quad in quads {
        // For OWL parsing, we typically work with the default graph
        // Named graphs in N3 can represent formulas, but for basic
        // OWL ontology loading, we merge everything into the default graph
        let triple_ref: oxrdf::TripleRef = quad.as_ref().into();
        graph.insert(triple_ref);
    }

    graph
}

/// N3 formula utilities for OWL integration.
pub mod formulas {
    use oxrdf::Formula;
    use oxrdf::Quad;

    /// Extracts formulas from a collection of quads.
    ///
    /// This identifies quads that belong to named graphs (which can represent
    /// N3 formulas) and groups them by their graph name.
    pub fn extract_formulas(quads: &[Quad]) -> Vec<Formula> {
        use oxrdf::GraphName;
        use std::collections::HashMap;

        let mut formula_map: HashMap<oxrdf::BlankNode, Vec<oxrdf::Triple>> = HashMap::new();

        for quad in quads {
            if let GraphName::BlankNode(bn) = &quad.graph_name {
                let triple_ref: oxrdf::TripleRef = quad.as_ref().into();
                formula_map
                    .entry(bn.clone())
                    .or_default()
                    .push(triple_ref.into_owned());
            }
        }

        formula_map
            .into_iter()
            .map(|(id, triples)| Formula::new(id, triples))
            .collect()
    }

    /// Checks if a formula represents an OWL class expression pattern.
    ///
    /// This is a simplified check - in practice, more sophisticated
    /// pattern matching would be needed for full N3-to-OWL conversion.
    pub fn is_class_expression_pattern(_formula: &Formula) -> bool {
        // Placeholder for future implementation
        // Could check for patterns like:
        // { ?x a ex:Dog } => indicates a class restriction
        // { ?x ex:hasAge ?age . ?age > 10 } => indicates a data range restriction
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_n3_ontology() {
        let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:Ontology a owl:Ontology .
ex:Dog a owl:Class .
ex:Animal a owl:Class .
ex:Dog rdfs:subClassOf ex:Animal .
"#;

        let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
        assert!(ontology.axiom_count() >= 3); // At least class declarations and subclass axiom
    }

    #[test]
    fn test_parse_n3_with_properties() {
        let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:hasOwner a owl:ObjectProperty .
ex:Person a owl:Class .
ex:Dog a owl:Class .
ex:hasOwner rdfs:domain ex:Dog .
ex:hasOwner rdfs:range ex:Person .
"#;

        let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
        assert!(ontology.axiom_count() > 0);
    }

    #[test]
    fn test_parse_n3_to_quads_with_variables() {
        let n3_data = r#"
@prefix ex: <http://example.org/> .

# This contains a variable and should be skipped in OWL conversion
{ ?x a ex:Dog } => { ?x a ex:Animal } .

# This is a regular triple and should be converted
ex:Fido a ex:Dog .
"#;

        let quads = parse_n3_to_quads(n3_data.as_bytes()).unwrap();
        // Should have at least the Fido triple
        // Variables in formulas are skipped
        assert!(!quads.is_empty());
    }

    #[test]
    fn test_n3_quad_to_quad_conversion() {
        use oxrdf::{NamedNode, BlankNode};
        use oxttl::n3::{N3Quad, N3Term};
        use oxrdf::GraphName;

        let ex = NamedNode::new("http://example.org/test").unwrap();
        let n3_quad = N3Quad {
            subject: N3Term::NamedNode(ex.clone()),
            predicate: N3Term::NamedNode(ex.clone()),
            object: N3Term::NamedNode(ex.clone()),
            graph_name: GraphName::DefaultGraph,
        };

        let quad = n3_quad_to_quad(n3_quad);
        assert!(quad.is_some());
    }

    #[test]
    fn test_n3_quad_with_variable_skipped() {
        use oxrdf::{NamedNode, Variable};
        use oxttl::n3::{N3Quad, N3Term};
        use oxrdf::GraphName;

        let ex = NamedNode::new("http://example.org/test").unwrap();
        let var = Variable::new("x").unwrap();

        let n3_quad = N3Quad {
            subject: N3Term::Variable(var),
            predicate: N3Term::NamedNode(ex.clone()),
            object: N3Term::NamedNode(ex),
            graph_name: GraphName::DefaultGraph,
        };

        let quad = n3_quad_to_quad(n3_quad);
        assert!(quad.is_none()); // Variables should be skipped
    }

    #[test]
    fn test_extract_formulas() {
        use oxrdf::{BlankNode, GraphName, NamedNode, Subject, Term};

        let bn = BlankNode::new("f1").unwrap();
        let ex = NamedNode::new("http://example.org/test").unwrap();

        let quad = Quad {
            subject: Subject::NamedNode(ex.clone()),
            predicate: ex.clone(),
            object: Term::NamedNode(ex),
            graph_name: GraphName::BlankNode(bn.clone()),
        };

        let formulas = formulas::extract_formulas(&[quad]);
        assert_eq!(formulas.len(), 1);
        assert_eq!(formulas[0].id(), &bn);
        assert_eq!(formulas[0].triples().len(), 1);
    }
}
