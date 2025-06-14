//! Interning of RDF elements using Rodeo

use crate::vocab::xsd;
use crate::*;
use std::collections::hash_map::{Entry, HashMap, RandomState};
use std::hash::{BuildHasher, Hasher};

#[derive(Debug, Default, Clone)]
pub struct Interner {
    hasher: RandomState,
    string_for_hash: HashMap<u64, String, IdentityHasherBuilder>,
    string_for_blank_node_id: HashMap<u128, String>,
    #[cfg(feature = "rdf-12")]
    triples: HashMap<InternedTriple, Triple>,
}

impl Interner {
    #[expect(clippy::never_loop)]
    fn get_or_intern(&mut self, value: &str) -> Key {
        let mut hash = self.hash(value);
        loop {
            match self.string_for_hash.entry(hash) {
                Entry::Vacant(e) => {
                    e.insert(value.into());
                    return Key(hash);
                }
                Entry::Occupied(e) => loop {
                    if e.get() == value {
                        return Key(hash);
                    } else if hash == u64::MAX - 1 {
                        hash = 0;
                    } else {
                        hash += 1;
                    }
                },
            }
        }
    }

    fn get(&self, value: &str) -> Option<Key> {
        let mut hash = self.hash(value);
        loop {
            let v = self.string_for_hash.get(&hash)?;
            if v == value {
                return Some(Key(hash));
            } else if hash == u64::MAX - 1 {
                hash = 0;
            } else {
                hash += 1;
            }
        }
    }

    fn hash(&self, value: &str) -> u64 {
        let hash = self.hasher.hash_one(value);
        if hash == u64::MAX { 0 } else { hash }
    }

    fn resolve(&self, key: Key) -> &str {
        &self.string_for_hash[&key.0]
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct Key(u64);

impl Key {
    fn first() -> Self {
        Self(0)
    }

    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    fn impossible() -> Self {
        Self(u64::MAX)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub struct InternedNamedNode {
    id: Key,
}

impl InternedNamedNode {
    pub fn encoded_into(named_node: NamedNodeRef<'_>, interner: &mut Interner) -> Self {
        Self {
            id: interner.get_or_intern(named_node.as_str()),
        }
    }

    pub fn encoded_from(named_node: NamedNodeRef<'_>, interner: &Interner) -> Option<Self> {
        Some(Self {
            id: interner.get(named_node.as_str())?,
        })
    }

    pub fn decode_from(self, interner: &Interner) -> NamedNodeRef<'_> {
        NamedNodeRef::new_unchecked(interner.resolve(self.id))
    }

    pub fn first() -> Self {
        Self { id: Key::first() }
    }

    pub fn next(self) -> Self {
        Self { id: self.id.next() }
    }

    pub fn impossible() -> Self {
        Self {
            id: Key::impossible(),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedBlankNode {
    Number { id: u128 },
    Other { id: Key },
}

impl InternedBlankNode {
    pub fn encoded_into(blank_node: BlankNodeRef<'_>, interner: &mut Interner) -> Self {
        if let Some(id) = blank_node.unique_id() {
            interner
                .string_for_blank_node_id
                .entry(id)
                .or_insert_with(|| blank_node.as_str().into());
            Self::Number { id }
        } else {
            Self::Other {
                id: interner.get_or_intern(blank_node.as_str()),
            }
        }
    }

    pub fn encoded_from(blank_node: BlankNodeRef<'_>, interner: &Interner) -> Option<Self> {
        if let Some(id) = blank_node.unique_id() {
            interner
                .string_for_blank_node_id
                .contains_key(&id)
                .then_some(Self::Number { id })
        } else {
            Some(Self::Other {
                id: interner.get(blank_node.as_str())?,
            })
        }
    }

    pub fn decode_from(self, interner: &Interner) -> BlankNodeRef<'_> {
        BlankNodeRef::new_unchecked(match self {
            Self::Number { id } => &interner.string_for_blank_node_id[&id],
            Self::Other { id } => interner.resolve(id),
        })
    }

    pub fn next(self) -> Self {
        match self {
            Self::Number { id } => Self::Number {
                id: id.saturating_add(1),
            },
            Self::Other { id } => Self::Other { id: id.next() },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum InternedLiteral {
    String {
        value_id: Key,
    },
    LanguageTaggedString {
        value_id: Key,
        language_id: Key,
    },
    #[cfg(feature = "rdf-12")]
    DirectionalLanguageTaggedString {
        value_id: Key,
        language_id: Key,
        is_ltr: bool,
    },
    TypedLiteral {
        value_id: Key,
        datatype: InternedNamedNode,
    },
}

impl InternedLiteral {
    pub fn encoded_into(literal: LiteralRef<'_>, interner: &mut Interner) -> Self {
        let value_id = interner.get_or_intern(literal.value());
        if let Some(language) = literal.language() {
            let language_id = interner.get_or_intern(language);
            #[cfg(feature = "rdf-12")]
            if let Some(direction) = literal.direction() {
                return Self::DirectionalLanguageTaggedString {
                    value_id,
                    language_id,
                    is_ltr: direction == BaseDirection::Ltr,
                };
            }
            Self::LanguageTaggedString {
                value_id,
                language_id,
            }
        } else if literal.datatype() == xsd::STRING {
            Self::String { value_id }
        } else {
            Self::TypedLiteral {
                value_id,
                datatype: InternedNamedNode::encoded_into(literal.datatype(), interner),
            }
        }
    }

    pub fn encoded_from(literal: LiteralRef<'_>, interner: &Interner) -> Option<Self> {
        let value_id = interner.get(literal.value())?;
        Some(if let Some(language) = literal.language() {
            let language_id = interner.get(language)?;
            #[cfg(feature = "rdf-12")]
            if let Some(direction) = literal.direction() {
                Self::DirectionalLanguageTaggedString {
                    value_id,
                    language_id,
                    is_ltr: direction == BaseDirection::Ltr,
                }
            } else {
                Self::LanguageTaggedString {
                    value_id,
                    language_id,
                }
            }
            #[cfg(not(feature = "rdf-12"))]
            Self::LanguageTaggedString {
                value_id,
                language_id,
            }
        } else if literal.datatype() == xsd::STRING {
            Self::String { value_id }
        } else {
            Self::TypedLiteral {
                value_id,
                datatype: InternedNamedNode::encoded_from(literal.datatype(), interner)?,
            }
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Interner) -> LiteralRef<'a> {
        match self {
            Self::String { value_id } => {
                LiteralRef::new_simple_literal(interner.resolve(*value_id))
            }
            Self::LanguageTaggedString {
                value_id,
                language_id,
            } => LiteralRef::new_language_tagged_literal_unchecked(
                interner.resolve(*value_id),
                interner.resolve(*language_id),
            ),
            #[cfg(feature = "rdf-12")]
            Self::DirectionalLanguageTaggedString {
                value_id,
                language_id,
                is_ltr,
            } => LiteralRef::new_directional_language_tagged_literal_unchecked(
                interner.resolve(*value_id),
                interner.resolve(*language_id),
                if *is_ltr {
                    BaseDirection::Ltr
                } else {
                    BaseDirection::Rtl
                },
            ),
            Self::TypedLiteral { value_id, datatype } => LiteralRef::new_typed_literal(
                interner.resolve(*value_id),
                datatype.decode_from(interner),
            ),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::String { value_id } => Self::String {
                value_id: value_id.next(),
            },
            Self::LanguageTaggedString {
                value_id,
                language_id,
            } => Self::LanguageTaggedString {
                value_id: *value_id,
                language_id: language_id.next(),
            },
            #[cfg(feature = "rdf-12")]
            Self::DirectionalLanguageTaggedString {
                value_id,
                language_id,
                is_ltr,
            } => Self::DirectionalLanguageTaggedString {
                value_id: *value_id,
                language_id: if *is_ltr {
                    language_id.next()
                } else {
                    *language_id
                },
                is_ltr: !*is_ltr,
            },
            Self::TypedLiteral { value_id, datatype } => Self::TypedLiteral {
                value_id: *value_id,
                datatype: datatype.next(),
            },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum InternedNamedOrBlankNode {
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
}

impl InternedNamedOrBlankNode {
    pub fn encoded_into(node: NamedOrBlankNodeRef<'_>, interner: &mut Interner) -> Self {
        match node {
            NamedOrBlankNodeRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_into(node, interner))
            }
            NamedOrBlankNodeRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_into(node, interner))
            }
        }
    }

    pub fn encoded_from(node: NamedOrBlankNodeRef<'_>, interner: &Interner) -> Option<Self> {
        Some(match node {
            NamedOrBlankNodeRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_from(node, interner)?)
            }
            NamedOrBlankNodeRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_from(node, interner)?)
            }
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Interner) -> NamedOrBlankNodeRef<'a> {
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum InternedGraphName {
    DefaultGraph,
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
}

impl InternedGraphName {
    pub fn encoded_into(node: GraphNameRef<'_>, interner: &mut Interner) -> Self {
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

    pub fn encoded_from(node: GraphNameRef<'_>, interner: &Interner) -> Option<Self> {
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

    pub fn decode_from<'a>(&self, interner: &'a Interner) -> GraphNameRef<'a> {
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum InternedTerm {
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
    Literal(InternedLiteral),
    #[cfg(feature = "rdf-12")]
    Triple(Box<InternedTriple>),
}

impl InternedTerm {
    pub fn encoded_into(term: TermRef<'_>, interner: &mut Interner) -> Self {
        match term {
            TermRef::NamedNode(term) => {
                Self::NamedNode(InternedNamedNode::encoded_into(term, interner))
            }
            TermRef::BlankNode(term) => {
                Self::BlankNode(InternedBlankNode::encoded_into(term, interner))
            }
            TermRef::Literal(term) => Self::Literal(InternedLiteral::encoded_into(term, interner)),
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(triple) => Self::Triple(Box::new(InternedTriple::encoded_into(
                triple.as_ref(),
                interner,
            ))),
        }
    }

    pub fn encoded_from(term: TermRef<'_>, interner: &Interner) -> Option<Self> {
        Some(match term {
            TermRef::NamedNode(term) => {
                Self::NamedNode(InternedNamedNode::encoded_from(term, interner)?)
            }
            TermRef::BlankNode(term) => {
                Self::BlankNode(InternedBlankNode::encoded_from(term, interner)?)
            }
            TermRef::Literal(term) => Self::Literal(InternedLiteral::encoded_from(term, interner)?),
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(triple) => Self::Triple(Box::new(InternedTriple::encoded_from(
                triple.as_ref(),
                interner,
            )?)),
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Interner) -> TermRef<'a> {
        match self {
            Self::NamedNode(term) => TermRef::NamedNode(term.decode_from(interner)),
            Self::BlankNode(term) => TermRef::BlankNode(term.decode_from(interner)),
            Self::Literal(term) => TermRef::Literal(term.decode_from(interner)),
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => TermRef::Triple(&interner.triples[triple.as_ref()]),
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
            #[cfg(feature = "rdf-12")]
            Self::Triple(triple) => Self::Triple(Box::new(triple.next())),
        }
    }

    pub fn impossible() -> Self {
        Self::NamedNode(InternedNamedNode::impossible())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct InternedTriple {
    pub subject: InternedNamedOrBlankNode,
    pub predicate: InternedNamedNode,
    pub object: InternedTerm,
}

#[cfg(feature = "rdf-12")]
impl InternedTriple {
    pub fn encoded_into(triple: TripleRef<'_>, interner: &mut Interner) -> Self {
        let interned_triple = Self {
            subject: InternedNamedOrBlankNode::encoded_into(triple.subject, interner),
            predicate: InternedNamedNode::encoded_into(triple.predicate, interner),
            object: InternedTerm::encoded_into(triple.object, interner),
        };
        interner
            .triples
            .insert(interned_triple.clone(), triple.into_owned());
        interned_triple
    }

    pub fn encoded_from(triple: TripleRef<'_>, interner: &Interner) -> Option<Self> {
        let interned_triple = Self {
            subject: InternedNamedOrBlankNode::encoded_from(triple.subject, interner)?,
            predicate: InternedNamedNode::encoded_from(triple.predicate, interner)?,
            object: InternedTerm::encoded_from(triple.object, interner)?,
        };
        interner
            .triples
            .contains_key(&interned_triple)
            .then_some(interned_triple)
    }

    pub fn next(&self) -> Self {
        Self {
            subject: self.subject.clone(),
            predicate: self.predicate,
            object: self.object.next(),
        }
    }
}

#[derive(Default, Clone)]
struct IdentityHasherBuilder;

impl BuildHasher for IdentityHasherBuilder {
    type Hasher = IdentityHasher;

    fn build_hasher(&self) -> Self::Hasher {
        Self::Hasher::default()
    }
}

#[derive(Default)]
struct IdentityHasher {
    value: u64,
}

impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.value
    }

    fn write(&mut self, _bytes: &[u8]) {
        unreachable!("Should only be used on u64 values")
    }

    fn write_u64(&mut self, i: u64) {
        self.value = i
    }
}
