#![doc = include_str!("../README.md")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(test(attr(deny(warnings))))]
#![allow(clippy::return_self_not_must_use)]

pub mod io;
pub mod sparql;
mod storage;
pub mod store;

pub mod model {
    //! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) using [OxRDF](https://crates.io/crates/oxrdf).

    pub use oxrdf::{
        dataset, graph, vocab, BlankNode, BlankNodeIdParseError, BlankNodeRef, Dataset, Graph,
        GraphName, GraphNameRef, IriParseError, LanguageTagParseError, Literal, LiteralRef,
        NamedNode, NamedNodeRef, NamedOrBlankNode, NamedOrBlankNodeRef, Quad, QuadRef, Subject,
        SubjectRef, Term, TermParseError, TermRef, Triple, TripleRef,
    };
}
