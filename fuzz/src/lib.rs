use oxrdf::{GraphNameRef, QuadRef, SubjectRef, TermRef, TripleRef};

pub mod result_format;

pub fn count_triple_blank_nodes(triple: TripleRef<'_>) -> usize {
    (match &triple.subject {
        SubjectRef::BlankNode(_) => 1,
        SubjectRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
        _ => 0,
    }) + (match &triple.object {
        TermRef::BlankNode(_) => 1,
        TermRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
        _ => 0,
    })
}

pub fn count_quad_blank_nodes(quad: QuadRef<'_>) -> usize {
    (match &quad.subject {
        SubjectRef::BlankNode(_) => 1,
        SubjectRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
        _ => 0,
    }) + (match &quad.object {
        TermRef::BlankNode(_) => 1,
        TermRef::Triple(t) => count_triple_blank_nodes(t.as_ref()),
        _ => 0,
    }) + usize::from(matches!(quad.graph_name, GraphNameRef::BlankNode(_)))
}
