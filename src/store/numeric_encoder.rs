use errors::*;
use model::*;
use std::mem::size_of;
use std::ops::Deref;
use std::str;
use std::str::FromStr;
use url::Url;
use utils::from_bytes_slice;
use utils::to_bytes;
use uuid::Uuid;

pub trait BytesStore {
    type BytesOutput: Deref<Target = [u8]>;

    fn put(&self, value: &[u8]) -> Result<usize>;
    fn get(&self, id: usize) -> Result<Option<Self::BytesOutput>>;
}

const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_BLANK_NODE_ID: u8 = 2;
const TYPE_LANG_STRING_LITERAL_ID: u8 = 3;
const TYPE_TYPED_LITERAL_ID: u8 = 4;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum EncodedTerm {
    NamedNode { iri_id: usize },
    BlankNode(Uuid),
    LangStringLiteral { value_id: usize, language_id: usize },
    TypedLiteral { value_id: usize, datatype_id: usize },
}

impl EncodedTerm {
    pub fn new_from_buffer(buffer: &[u8]) -> Result<Self> {
        if buffer.is_empty() {
            return Err("the term buffer is empty.".into());
        }
        if buffer.len() < Self::type_length(buffer[0])? {
            return Err(format!(
                "the term buffer with id {} do not have at least {} bytes.",
                buffer[0],
                buffer.len()
            ).into());
        }
        match buffer[0] {
            TYPE_NAMED_NODE_ID => Ok(EncodedTerm::NamedNode {
                iri_id: from_bytes_slice(&buffer[1..1 + size_of::<usize>()]),
            }),
            TYPE_BLANK_NODE_ID => Ok(EncodedTerm::BlankNode(Uuid::from_bytes(&buffer[1..17])?)),
            TYPE_LANG_STRING_LITERAL_ID => Ok(EncodedTerm::LangStringLiteral {
                language_id: from_bytes_slice(&buffer[1..1 + size_of::<usize>()]),
                value_id: from_bytes_slice(
                    &buffer[1 + size_of::<usize>()..1 + 2 * size_of::<usize>()],
                ),
            }),
            TYPE_TYPED_LITERAL_ID => Ok(EncodedTerm::TypedLiteral {
                datatype_id: from_bytes_slice(&buffer[1..1 + size_of::<usize>()]),
                value_id: from_bytes_slice(
                    &buffer[1 + size_of::<usize>()..1 + 2 * size_of::<usize>()],
                ),
            }),
            _ => Err("the term buffer has an invalid type id".into()),
        }
    }

    pub fn encoding_size(&self) -> usize {
        Self::type_length(self.type_id()).unwrap() //It is not possible to fail here
    }

    fn type_id(&self) -> u8 {
        match self {
            EncodedTerm::NamedNode { .. } => TYPE_NAMED_NODE_ID,
            EncodedTerm::BlankNode(_) => TYPE_BLANK_NODE_ID,
            EncodedTerm::LangStringLiteral { .. } => TYPE_LANG_STRING_LITERAL_ID,
            EncodedTerm::TypedLiteral { .. } => TYPE_TYPED_LITERAL_ID,
        }
    }

    fn type_length(type_id: u8) -> Result<usize> {
        match type_id {
            TYPE_NAMED_NODE_ID => Ok(1 + size_of::<usize>()),
            TYPE_BLANK_NODE_ID => Ok(17), //TODO: guess
            TYPE_LANG_STRING_LITERAL_ID => Ok(1 + 2 * size_of::<usize>()),
            TYPE_TYPED_LITERAL_ID => Ok(1 + 2 * size_of::<usize>()),
            _ => Err(format!("{} is not a known type id", type_id).into()),
        }
    }

    pub fn add_to_vec(&self, vec: &mut Vec<u8>) {
        vec.push(self.type_id());
        match self {
            EncodedTerm::NamedNode { iri_id } => vec.extend_from_slice(&to_bytes(*iri_id)),
            EncodedTerm::BlankNode(id) => vec.extend_from_slice(id.as_bytes()),
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => {
                vec.extend_from_slice(&to_bytes(*language_id));
                vec.extend_from_slice(&to_bytes(*value_id));
            }
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => {
                vec.extend_from_slice(&to_bytes(*datatype_id));
                vec.extend_from_slice(&to_bytes(*value_id));
            }
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct EncodedQuad {
    pub subject: EncodedTerm,
    pub predicate: EncodedTerm,
    pub object: EncodedTerm,
    pub graph_name: Option<EncodedTerm>,
}

impl EncodedQuad {
    pub fn new_from_spog_buffer(buffer: &[u8]) -> Result<Self> {
        let mut start = 0 as usize;
        let subject = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += subject.encoding_size();
        let predicate = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += predicate.encoding_size();
        let object = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += object.encoding_size();
        let graph_name = if start < buffer.len() {
            Some(EncodedTerm::new_from_buffer(&buffer[start..])?)
        } else {
            None
        };
        Ok(Self {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    pub fn new_from_posg_buffer(buffer: &[u8]) -> Result<Self> {
        let mut start = 0 as usize;
        let predicate = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += predicate.encoding_size();
        let object = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += object.encoding_size();
        let subject = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += subject.encoding_size();
        let graph_name = if start < buffer.len() {
            Some(EncodedTerm::new_from_buffer(&buffer[start..])?)
        } else {
            None
        };
        Ok(Self {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    pub fn new_from_ospg_buffer(buffer: &[u8]) -> Result<Self> {
        let mut start = 0 as usize;
        let object = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += object.encoding_size();
        let subject = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += subject.encoding_size();
        let predicate = EncodedTerm::new_from_buffer(&buffer[start..])?;
        start += predicate.encoding_size();
        let graph_name = if start < buffer.len() {
            Some(EncodedTerm::new_from_buffer(&buffer[start..])?)
        } else {
            None
        };
        Ok(Self {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    pub fn spog(&self) -> Vec<u8> {
        let mut spog = Vec::with_capacity(self.encoding_size());
        self.subject.add_to_vec(&mut spog);
        self.predicate.add_to_vec(&mut spog);
        self.object.add_to_vec(&mut spog);
        if let Some(ref graph_name) = self.graph_name {
            graph_name.add_to_vec(&mut spog);
        }
        spog
    }

    pub fn posg(&self) -> Vec<u8> {
        let mut posg = Vec::with_capacity(self.encoding_size());
        self.predicate.add_to_vec(&mut posg);
        self.object.add_to_vec(&mut posg);
        self.subject.add_to_vec(&mut posg);
        if let Some(ref graph_name) = self.graph_name {
            graph_name.add_to_vec(&mut posg);
        }
        posg
    }

    pub fn ospg(&self) -> Vec<u8> {
        let mut ospg = Vec::with_capacity(self.encoding_size());
        self.object.add_to_vec(&mut ospg);
        self.subject.add_to_vec(&mut ospg);
        self.predicate.add_to_vec(&mut ospg);
        if let Some(ref graph_name) = self.graph_name {
            graph_name.add_to_vec(&mut ospg);
        }
        ospg
    }

    fn encoding_size(&self) -> usize {
        self.subject.encoding_size() + self.predicate.encoding_size() + self.object.encoding_size()
            + match self.graph_name {
                Some(ref graph_name) => graph_name.encoding_size(),
                None => 0,
            }
    }
}

pub struct Encoder<S: BytesStore> {
    string_store: S,
}

impl<S: BytesStore> Encoder<S> {
    pub fn new(string_store: S) -> Self {
        Self { string_store }
    }

    pub fn encode_named_node(&self, named_node: &NamedNode) -> Result<EncodedTerm> {
        Ok(EncodedTerm::NamedNode {
            iri_id: self.encode_str_value(named_node.as_str())?,
        })
    }

    pub fn encode_blank_node(&self, blank_node: &BlankNode) -> Result<EncodedTerm> {
        Ok(EncodedTerm::BlankNode(blank_node.deref().clone()))
    }

    pub fn encode_literal(&self, literal: &Literal) -> Result<EncodedTerm> {
        if let Some(language) = literal.language() {
            Ok(EncodedTerm::LangStringLiteral {
                value_id: self.encode_str_value(&literal.value())?,
                language_id: self.encode_str_value(language)?,
            })
        } else {
            Ok(EncodedTerm::TypedLiteral {
                value_id: self.encode_str_value(&literal.value())?,
                datatype_id: self.encode_str_value(literal.datatype().as_ref())?,
            })
        }
    }

    pub fn encode_named_or_blank_node(&self, term: &NamedOrBlankNode) -> Result<EncodedTerm> {
        match term {
            NamedOrBlankNode::NamedNode(named_node) => self.encode_named_node(named_node),
            NamedOrBlankNode::BlankNode(blank_node) => self.encode_blank_node(blank_node),
        }
    }

    pub fn encode_term(&self, term: &Term) -> Result<EncodedTerm> {
        match term {
            Term::NamedNode(named_node) => self.encode_named_node(named_node),
            Term::BlankNode(blank_node) => self.encode_blank_node(blank_node),
            Term::Literal(literal) => self.encode_literal(literal),
        }
    }

    pub fn encode_quad(&self, quad: &Quad) -> Result<EncodedQuad> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(quad.subject())?,
            predicate: self.encode_named_node(quad.predicate())?,
            object: self.encode_term(quad.object())?,
            graph_name: match quad.graph_name() {
                Some(graph_name) => Some(self.encode_named_or_blank_node(&graph_name)?),
                None => None,
            },
        })
    }

    pub fn decode_term(&self, encoded: &EncodedTerm) -> Result<Term> {
        match encoded {
            EncodedTerm::NamedNode { iri_id } => {
                Ok(NamedNode::from(self.decode_url_value(*iri_id)?).into())
            }
            EncodedTerm::BlankNode(id) => Ok(BlankNode::from(*id).into()),
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => Ok(Literal::new_language_tagged_literal(
                self.decode_str_value(*value_id)?,
                self.decode_str_value(*language_id)?,
            ).into()),
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => Ok(Literal::new_typed_literal(
                self.decode_str_value(*value_id)?,
                NamedNode::from(self.decode_url_value(*datatype_id)?),
            ).into()),
        }
    }

    pub fn decode_named_or_blank_node(&self, encoded: &EncodedTerm) -> Result<NamedOrBlankNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_named_node(&self, encoded: &EncodedTerm) -> Result<NamedNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err("A blank node has been found instead of a named node".into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_quad(&self, encoded: &EncodedQuad) -> Result<Quad> {
        Ok(Quad::new(
            self.decode_named_or_blank_node(&encoded.subject)?,
            self.decode_named_node(&encoded.predicate)?,
            self.decode_term(&encoded.object)?,
            match encoded.graph_name {
                Some(ref graph_name) => Some(self.decode_named_or_blank_node(&graph_name)?),
                None => None,
            },
        ))
    }

    fn encode_str_value(&self, text: &str) -> Result<usize> {
        self.string_store.put(text.as_bytes())
    }

    fn decode_url_value(&self, id: usize) -> Result<Url> {
        let bytes = self.decode_value(id)?;
        Ok(Url::from_str(str::from_utf8(&bytes)?)?)
    }

    fn decode_str_value(&self, id: usize) -> Result<String> {
        let bytes = self.decode_value(id)?;
        Ok(str::from_utf8(&bytes)?.to_owned())
    }

    fn decode_value(&self, id: usize) -> Result<S::BytesOutput> {
        self.string_store
            .get(id)?
            .ok_or("value not found in the dictionary".into())
    }
}

impl<S: BytesStore + Default> Default for Encoder<S> {
    fn default() -> Self {
        Self {
            string_store: S::default(),
        }
    }
}

mod test {
    use model::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use store::numeric_encoder::*;
    use utils::to_bytes;

    #[derive(Default)]
    struct MemoryBytesStore {
        id2str: RefCell<BTreeMap<usize, Vec<u8>>>,
        str2id: RefCell<BTreeMap<Vec<u8>, usize>>,
    }

    impl BytesStore for MemoryBytesStore {
        type BytesOutput = Vec<u8>;

        fn put(&self, value: &[u8]) -> Result<usize> {
            let mut str2id = self.str2id.borrow_mut();
            let mut id2str = self.id2str.borrow_mut();
            let id = str2id.entry(value.to_vec()).or_insert_with(|| {
                let id = id2str.len();
                id2str.insert(id, value.to_vec());
                id
            });
            Ok(*id)
        }

        fn get(&self, id: usize) -> Result<Option<Vec<u8>>> {
            Ok(self.id2str.borrow().get(&id).map(|s| s.to_owned()))
        }
    }

    #[test]
    fn test_encoding() {
        let encoder: Encoder<MemoryBytesStore> = Encoder::default();
        let terms: Vec<Term> = vec![
            NamedNode::from_str("http://foo.com").unwrap().into(),
            NamedNode::from_str("http://bar.com").unwrap().into(),
            NamedNode::from_str("http://foo.com").unwrap().into(),
            BlankNode::default().into(),
            Literal::from(true).into(),
            Literal::from(1.2).into(),
            Literal::from("foo").into(),
            Literal::new_language_tagged_literal("foo", "fr").into(),
        ];
        for term in terms {
            let encoded = encoder.encode_term(&term).unwrap();
            assert_eq!(term, encoder.decode_term(&encoded).unwrap())
        }
    }

}
