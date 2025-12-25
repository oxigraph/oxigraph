use oxrdf::{GraphName, Literal, NamedNode, Variable, vocab::rdf, vocab::xsd};
use oxttl::n3::{N3Quad, N3Serializer, N3Term};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===== N3 Serializer Demo =====\n");

    // Demo 1: Simple triples with prefixes
    println!("1. Simple triple with @prefix declarations:");
    println!("-------------------------------------------");
    let mut serializer = N3Serializer::new()
        .with_prefix("ex", "http://example.com/")?
        .with_prefix("schema", "http://schema.org/")?
        .for_writer(Vec::new());

    serializer.serialize_quad(&N3Quad {
        subject: N3Term::NamedNode(NamedNode::new("http://example.com/alice")?),
        predicate: N3Term::NamedNode(rdf::TYPE.into_owned()),
        object: N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    println!("{}", String::from_utf8(serializer.finish()?)?);

    // Demo 2: Variables (N3-specific feature)
    println!("2. N3 with variables (formatted as ?name):");
    println!("-------------------------------------------");
    let mut serializer = N3Serializer::new().for_writer(Vec::new());

    serializer.serialize_quad(&N3Quad {
        subject: N3Term::Variable(Variable::new("x")?),
        predicate: N3Term::NamedNode(NamedNode::new("http://example.com/knows")?),
        object: N3Term::Variable(Variable::new("y")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    println!("{}", String::from_utf8(serializer.finish()?)?);

    // Demo 3: Grouping by subject with semicolons
    println!("3. Grouped triples (same subject, different predicates using ';'):");
    println!("------------------------------------------------------------------");
    let mut serializer = N3Serializer::new()
        .with_prefix("ex", "http://example.com/")?
        .for_writer(Vec::new());

    let alice = N3Term::NamedNode(NamedNode::new("http://example.com/alice")?);

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: N3Term::NamedNode(NamedNode::new("http://example.com/name")?),
        object: N3Term::Literal(Literal::new_simple_literal("Alice")),
        graph_name: GraphName::DefaultGraph,
    })?;

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: N3Term::NamedNode(NamedNode::new("http://example.com/age")?),
        object: N3Term::Literal(Literal::new_typed_literal("30", xsd::INTEGER)),
        graph_name: GraphName::DefaultGraph,
    })?;

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: N3Term::NamedNode(NamedNode::new("http://example.com/email")?),
        object: N3Term::Literal(Literal::new_simple_literal("alice@example.com")),
        graph_name: GraphName::DefaultGraph,
    })?;

    println!("{}", String::from_utf8(serializer.finish()?)?);

    // Demo 4: Grouping objects with commas
    println!("4. Grouped objects (same subject and predicate using ','):");
    println!("-----------------------------------------------------------");
    let mut serializer = N3Serializer::new()
        .with_prefix("ex", "http://example.com/")?
        .for_writer(Vec::new());

    let alice = N3Term::NamedNode(NamedNode::new("http://example.com/alice")?);
    let knows = N3Term::NamedNode(NamedNode::new("http://example.com/knows")?);

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: knows.clone(),
        object: N3Term::NamedNode(NamedNode::new("http://example.com/bob")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: knows.clone(),
        object: N3Term::NamedNode(NamedNode::new("http://example.com/charlie")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    serializer.serialize_quad(&N3Quad {
        subject: alice.clone(),
        predicate: knows.clone(),
        object: N3Term::NamedNode(NamedNode::new("http://example.com/diana")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    println!("{}", String::from_utf8(serializer.finish()?)?);

    // Demo 5: With @base declaration
    println!("5. With @base declaration (relative IRIs):");
    println!("-------------------------------------------");
    let mut serializer = N3Serializer::new()
        .with_base_iri("http://example.com/")?
        .for_writer(Vec::new());

    serializer.serialize_quad(&N3Quad {
        subject: N3Term::NamedNode(NamedNode::new("http://example.com/alice")?),
        predicate: N3Term::NamedNode(rdf::TYPE.into_owned()),
        object: N3Term::NamedNode(NamedNode::new("http://example.com/Person")?),
        graph_name: GraphName::DefaultGraph,
    })?;

    println!("{}", String::from_utf8(serializer.finish()?)?);

    Ok(())
}
