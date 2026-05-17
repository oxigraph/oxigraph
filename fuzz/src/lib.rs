use oxrdf::{Quad, Term, Triple};

pub mod result_format;

pub fn count_triple_blank_nodes(triple: &Triple) -> usize {
    usize::from(triple.subject.is_blank_node())
        + match &triple.object {
            Term::BlankNode(_) => 1,
            Term::Triple(t) => count_triple_blank_nodes(t.as_ref()),
            _ => 0,
        }
}

pub fn count_quad_blank_nodes(quad: &Quad) -> usize {
    usize::from(quad.subject.is_blank_node())
        + match &quad.object {
            Term::BlankNode(_) => 1,
            Term::Triple(t) => count_triple_blank_nodes(t.as_ref()),
            _ => 0,
        }
        + usize::from(quad.graph_name.is_blank_node())
}
