use oxrdf::{QuadRef, TermRef, TripleRef};

pub mod result_format;

pub fn count_triple_blank_nodes(triple: TripleRef<'_>) -> usize {
    usize::from(triple.subject.is_blank_node())
        + match &triple.object {
            TermRef::BlankNode(_) => 1,
            TermRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
            _ => 0,
        }
}

pub fn count_quad_blank_nodes(quad: QuadRef<'_>) -> usize {
    usize::from(quad.subject.is_blank_node())
        + match &quad.object {
            TermRef::BlankNode(_) => 1,
            TermRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
            _ => 0,
        }
        + usize::from(quad.graph_name.is_blank_node())
}
