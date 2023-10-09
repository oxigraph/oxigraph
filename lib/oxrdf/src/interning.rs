//! Interning of RDF elements using Rodeo

use crate::*;
use std::collections::hash_map::{Entry, HashMap, RandomState};
use std::hash::{BuildHasher, Hasher};

#[derive(Debug, Default)]
pub struct Interner {
    hasher: RandomState,
    string_for_hash: HashMap<u64, String, IdentityHasherBuilder>,
    #[cfg(feature = "rdf-star")]
    triples: HashMap<InternedTriple, Triple>,
}

impl Interner {
    #[allow(clippy::never_loop)]
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
        let mut hasher = self.hasher.build_hasher();
        hasher.write(value.as_bytes());
        let hash = hasher.finish();
        if hash == u64::MAX {
            0
        } else {
            hash
        }
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

    pub fn decode_from(self, interner: &Interner) -> NamedNodeRef {
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
pub struct InternedBlankNode {
    id: Key,
}

impl InternedBlankNode {
    pub fn encoded_into(blank_node: BlankNodeRef<'_>, interner: &mut Interner) -> Self {
        Self {
            id: interner.get_or_intern(blank_node.as_str()),
        }
    }

    pub fn encoded_from(blank_node: BlankNodeRef<'_>, interner: &Interner) -> Option<Self> {
        Some(Self {
            id: interner.get(blank_node.as_str())?,
        })
    }

    pub fn decode_from(self, interner: &Interner) -> BlankNodeRef {
        BlankNodeRef::new_unchecked(interner.resolve(self.id))
    }

    pub fn next(self) -> Self {
        Self { id: self.id.next() }
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
    TypedLiteral {
        value_id: Key,
        datatype: InternedNamedNode,
    },
}

impl InternedLiteral {
    pub fn encoded_into(literal: LiteralRef<'_>, interner: &mut Interner) -> Self {
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

    pub fn encoded_from(literal: LiteralRef<'_>, interner: &Interner) -> Option<Self> {
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
            Self::TypedLiteral { value_id, datatype } => Self::TypedLiteral {
                value_id: *value_id,
                datatype: datatype.next(),
            },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum InternedSubject {
    NamedNode(InternedNamedNode),
    BlankNode(InternedBlankNode),
    #[cfg(feature = "rdf-star")]
    Triple(Box<InternedTriple>),
}

impl InternedSubject {
    pub fn encoded_into(node: SubjectRef<'_>, interner: &mut Interner) -> Self {
        match node {
            SubjectRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_into(node, interner))
            }
            SubjectRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_into(node, interner))
            }
            #[cfg(feature = "rdf-star")]
            SubjectRef::Triple(triple) => Self::Triple(Box::new(InternedTriple::encoded_into(
                triple.as_ref(),
                interner,
            ))),
        }
    }

    pub fn encoded_from(node: SubjectRef<'_>, interner: &Interner) -> Option<Self> {
        Some(match node {
            SubjectRef::NamedNode(node) => {
                Self::NamedNode(InternedNamedNode::encoded_from(node, interner)?)
            }
            SubjectRef::BlankNode(node) => {
                Self::BlankNode(InternedBlankNode::encoded_from(node, interner)?)
            }
            #[cfg(feature = "rdf-star")]
            SubjectRef::Triple(triple) => Self::Triple(Box::new(InternedTriple::encoded_from(
                triple.as_ref(),
                interner,
            )?)),
        })
    }

    pub fn decode_from<'a>(&self, interner: &'a Interner) -> SubjectRef<'a> {
        match self {
            Self::NamedNode(node) => SubjectRef::NamedNode(node.decode_from(interner)),
            Self::BlankNode(node) => SubjectRef::BlankNode(node.decode_from(interner)),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => SubjectRef::Triple(&interner.triples[triple.as_ref()]),
        }
    }

    pub fn first() -> Self {
        Self::NamedNode(InternedNamedNode::first())
    }

    pub fn next(&self) -> Self {
        match self {
            Self::NamedNode(node) => Self::NamedNode(node.next()),
            Self::BlankNode(node) => Self::BlankNode(node.next()),
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => Self::Triple(Box::new(triple.next())),
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
    #[cfg(feature = "rdf-star")]
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
            #[cfg(feature = "rdf-star")]
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
            #[cfg(feature = "rdf-star")]
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
            #[cfg(feature = "rdf-star")]
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
            #[cfg(feature = "rdf-star")]
            Self::Triple(triple) => Self::Triple(Box::new(triple.next())),
        }
    }

    pub fn impossible() -> Self {
        Self::NamedNode(InternedNamedNode::impossible())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct InternedTriple {
    pub subject: InternedSubject,
    pub predicate: InternedNamedNode,
    pub object: InternedTerm,
}

#[cfg(feature = "rdf-star")]
impl InternedTriple {
    pub fn encoded_into(triple: TripleRef<'_>, interner: &mut Interner) -> Self {
        let interned_triple = Self {
            subject: InternedSubject::encoded_into(triple.subject, interner),
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
            subject: InternedSubject::encoded_from(triple.subject, interner)?,
            predicate: InternedNamedNode::encoded_from(triple.predicate, interner)?,
            object: InternedTerm::encoded_from(triple.object, interner)?,
        };
        interner
            .triples
            .contains_key(&interned_triple)
            .then(|| interned_triple)
    }

    pub fn next(&self) -> Self {
        Self {
            subject: self.subject.clone(),
            predicate: self.predicate,
            object: self.object.next(),
        }
    }
}

#[derive(Default)]
struct IdentityHasherBuilder {}

impl BuildHasher for IdentityHasherBuilder {
    type Hasher = IdentityHasher;

    fn build_hasher(&self) -> IdentityHasher {
        IdentityHasher::default()
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
