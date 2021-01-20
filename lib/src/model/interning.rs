//! Interning of RDF elements using Rodeo

use crate::model::*;
use lasso::{Key, Rodeo, Spur};
use std::convert::TryInto;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct InternedNamedNode {
    id: Spur,
}

impl InternedNamedNode {
    pub fn encoded_into(named_node: NamedNodeRef<'_>, interner: &mut Rodeo) -> Self {
        Self {
            id: interner.get_or_intern(named_node.as_str()),
        }
    }

    pub fn encoded_from(named_node: NamedNodeRef<'_>, interner: &Rodeo) -> Option<Self> {
        Some(Self {
            id: interner.get(named_node.as_str())?,
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> NamedNodeRef<'a> {
        NamedNodeRef::new_unchecked(interner.resolve(&self.id))
    }

    pub fn first() -> Self {
        Self { id: fist_spur() }
    }

    pub fn next(&self) -> Self {
        Self {
            id: next_spur(self.id),
        }
    }

    pub fn impossible() -> Self {
        Self {
            id: impossible_spur(),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct InternedBlankNode {
    id: Spur,
}

impl InternedBlankNode {
    pub fn encoded_into(blank_node: BlankNodeRef<'_>, interner: &mut Rodeo) -> Self {
        Self {
            id: interner.get_or_intern(blank_node.as_str()),
        }
    }

    pub fn encoded_from(blank_node: BlankNodeRef<'_>, interner: &Rodeo) -> Option<Self> {
        Some(Self {
            id: interner.get(blank_node.as_str())?,
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> BlankNodeRef<'a> {
        BlankNodeRef::new_unchecked(interner.resolve(&self.id))
    }

    pub fn next(&self) -> Self {
        Self {
            id: next_spur(self.id),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedLiteral {
    String {
        value_id: Spur,
    },
    LanguageTaggedString {
        value_id: Spur,
        language_id: Spur,
    },
    TypedLiteral {
        value_id: Spur,
        datatype: InternedNamedNode,
    },
}

impl InternedLiteral {
    pub fn encoded_into(literal: LiteralRef<'_>, interner: &mut Rodeo) -> Self {
        let value_id = interner.get_or_intern(literal.value());
        if literal.is_plain() {
            if let Some(language) = literal.language() {
                Self::LanguageTaggedString {
                    value_id,
                    language_id: interner.get_or_intern(language),
                }
            } else {
                Self::String { value_id }
            }
        } else {
            Self::TypedLiteral {
                value_id,
                datatype: InternedNamedNode::encoded_into(literal.datatype(), interner),
            }
        }
    }

    pub fn encoded_from(literal: LiteralRef<'_>, interner: &Rodeo) -> Option<Self> {
        let value_id = interner.get(literal.value())?;
        Some(if literal.is_plain() {
            if let Some(language) = literal.language() {
                Self::LanguageTaggedString {
                    value_id,
                    language_id: interner.get(language)?,
                }
            } else {
                Self::String { value_id }
            }
        } else {
            Self::TypedLiteral {
                value_id,
                datatype: InternedNamedNode::encoded_from(literal.datatype(), interner)?,
            }
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> LiteralRef<'a> {
        match self {
            InternedLiteral::String { value_id } => {
                LiteralRef::new_simple_literal(interner.resolve(value_id))
            }
            InternedLiteral::LanguageTaggedString {
                value_id,
                language_id,
            } => LiteralRef::new_language_tagged_literal_unchecked(
                interner.resolve(value_id),
                interner.resolve(language_id),
            ),
            InternedLiteral::TypedLiteral { value_id, datatype } => LiteralRef::new_typed_literal(
                interner.resolve(value_id),
                datatype.decode_from(interner),
            ),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::String { value_id } => Self::String {
                value_id: next_spur(*value_id),
            },
            Self::LanguageTaggedString {
                value_id,
                language_id,
            } => Self::LanguageTaggedString {
                value_id: *value_id,
                language_id: next_spur(*language_id),
            },
            Self::TypedLiteral { value_id, datatype } => Self::TypedLiteral {
                value_id: *value_id,
                datatype: datatype.next(),
            },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedNamedOrBlankNode {
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
}

impl InternedNamedOrBlankNode {
    pub fn encoded_into(node: NamedOrBlankNodeRef<'_>, interner: &mut Rodeo) -> Self {
        match node {
            NamedOrBlankNodeRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_into(node, interner))
            }
            NamedOrBlankNodeRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_into(node, interner))
            }
        }
    }

    pub fn encoded_from(node: NamedOrBlankNodeRef<'_>, interner: &Rodeo) -> Option<Self> {
        Some(match node {
            NamedOrBlankNodeRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_from(node, interner)?)
            }
            NamedOrBlankNodeRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_from(node, interner)?)
            }
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> NamedOrBlankNodeRef<'a> {
        match self {
            Self::NamedNode(node) => NamedOrBlankNodeRef::NamedNode(node.decode_from(interner)),
            Self::BlankNode(node) => NamedOrBlankNodeRef::BlankNode(node.decode_from(interner)),
        }
    }

    pub fn first() -> Self {
        Self::NamedNode(InternedNamedNode::first())
    }

    pub fn next(&self) -> Self {
        match self {
            Self::NamedNode(node) => Self::NamedNode(node.next()),
            Self::BlankNode(node) => Self::BlankNode(node.next()),
        }
    }

    pub fn impossible() -> Self {
        Self::NamedNode(InternedNamedNode::impossible())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedGraphName {
    DefaultGraph,
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
}

impl InternedGraphName {
    pub fn encoded_into(node: GraphNameRef<'_>, interner: &mut Rodeo) -> Self {
        match node {
            GraphNameRef::DefaultGraph => Self::DefaultGraph,
            GraphNameRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_into(node, interner))
            }
            GraphNameRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_into(node, interner))
            }
        }
    }

    pub fn encoded_from(node: GraphNameRef<'_>, interner: &Rodeo) -> Option<Self> {
        Some(match node {
            GraphNameRef::DefaultGraph => Self::DefaultGraph,
            GraphNameRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_from(node, interner)?)
            }
            GraphNameRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_from(node, interner)?)
            }
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> GraphNameRef<'a> {
        match self {
            Self::DefaultGraph => GraphNameRef::DefaultGraph,
            Self::NamedNode(node) => GraphNameRef::NamedNode(node.decode_from(interner)),
            Self::BlankNode(node) => GraphNameRef::BlankNode(node.decode_from(interner)),
        }
    }

    pub fn first() -> Self {
        Self::DefaultGraph
    }

    pub fn next(&self) -> Self {
        match self {
            Self::DefaultGraph => Self::NamedNode(InternedNamedNode::first()),
            Self::NamedNode(node) => Self::NamedNode(node.next()),
            Self::BlankNode(node) => Self::BlankNode(node.next()),
        }
    }

    pub fn impossible() -> Self {
        Self::NamedNode(InternedNamedNode::impossible())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedTerm {
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
    Literal(InternedLiteral),
}

impl InternedTerm {
    pub fn encoded_into(term: TermRef<'_>, interner: &mut Rodeo) -> Self {
        match term {
            TermRef::NamedNode(term) => {
                Self::NamedNode(InternedNamedNode::encoded_into(term, interner))
            }
            TermRef::BlankNode(term) => {
                Self::BlankNode(InternedBlankNode::encoded_into(term, interner))
            }
            TermRef::Literal(term) => Self::Literal(InternedLiteral::encoded_into(term, interner)),
        }
    }

    pub fn encoded_from(term: TermRef<'_>, interner: &Rodeo) -> Option<Self> {
        Some(match term {
            TermRef::NamedNode(term) => {
                Self::NamedNode(InternedNamedNode::encoded_from(term, interner)?)
            }
            TermRef::BlankNode(term) => {
                Self::BlankNode(InternedBlankNode::encoded_from(term, interner)?)
            }
            TermRef::Literal(term) => Self::Literal(InternedLiteral::encoded_from(term, interner)?),
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Rodeo) -> TermRef<'a> {
        match self {
            Self::NamedNode(term) => TermRef::NamedNode(term.decode_from(interner)),
            Self::BlankNode(term) => TermRef::BlankNode(term.decode_from(interner)),
            Self::Literal(term) => TermRef::Literal(term.decode_from(interner)),
        }
    }

    pub fn first() -> Self {
        Self::NamedNode(InternedNamedNode::first())
    }

    pub fn next(&self) -> Self {
        match self {
            Self::NamedNode(node) => Self::NamedNode(node.next()),
            Self::BlankNode(node) => Self::BlankNode(node.next()),
            Self::Literal(node) => Self::Literal(node.next()),
        }
    }

    pub fn impossible() -> Self {
        Self::NamedNode(InternedNamedNode::impossible())
    }
}

fn fist_spur() -> Spur {
    Spur::try_from_usize(0).unwrap()
}

fn next_spur(value: Spur) -> Spur {
    Spur::try_from_usize(value.into_usize() + 1).unwrap()
}

fn impossible_spur() -> Spur {
    Spur::try_from_usize((u32::max_value() - 10).try_into().unwrap()).unwrap()
}
