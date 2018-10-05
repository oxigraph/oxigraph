use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use model::*;
use std::io::Read;
use std::io::Write;
use std::ops::Deref;
use std::str;
use std::str::FromStr;
use url::Url;
use uuid::Uuid;
use Error;
use Result;

pub trait BytesStore {
    type BytesOutput: Deref<Target = [u8]>;

    fn insert_bytes(&self, value: &[u8]) -> Result<u64>;
    fn get_bytes(&self, id: u64) -> Result<Option<Self::BytesOutput>>;
}

const TYPE_DEFAULT_GRAPH_ID: u8 = 0;
const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_BLANK_NODE_ID: u8 = 2;
const TYPE_SIMPLE_LITERAL_ID: u8 = 3;
const TYPE_LANG_STRING_LITERAL_ID: u8 = 4;
const TYPE_TYPED_LITERAL_ID: u8 = 5;
const TYPE_STRING_LITERAL: u8 = 6;
const TYPE_BOOLEAN_LITERAL_TRUE: u8 = 6;
const TYPE_BOOLEAN_LITERAL_FALSE: u8 = 7;

pub static ENCODED_DEFAULT_GRAPH: EncodedTerm = EncodedTerm::DefaultGraph {};

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Hash)]
pub enum EncodedTerm {
    DefaultGraph {},
    NamedNode { iri_id: u64 },
    BlankNode(Uuid),
    SimpleLiteral { value_id: u64 },
    LangStringLiteral { value_id: u64, language_id: u64 },
    TypedLiteral { value_id: u64, datatype_id: u64 },
    StringLiteral { value_id: u64 },
    BooleanLiteral(bool),
}

impl EncodedTerm {
    fn type_id(&self) -> u8 {
        match self {
            EncodedTerm::DefaultGraph { .. } => TYPE_DEFAULT_GRAPH_ID,
            EncodedTerm::NamedNode { .. } => TYPE_NAMED_NODE_ID,
            EncodedTerm::BlankNode(_) => TYPE_BLANK_NODE_ID,
            EncodedTerm::SimpleLiteral { .. } => TYPE_SIMPLE_LITERAL_ID,
            EncodedTerm::LangStringLiteral { .. } => TYPE_LANG_STRING_LITERAL_ID,
            EncodedTerm::TypedLiteral { .. } => TYPE_TYPED_LITERAL_ID,
            EncodedTerm::StringLiteral { .. } => TYPE_STRING_LITERAL,
            EncodedTerm::BooleanLiteral(true) => TYPE_BOOLEAN_LITERAL_TRUE,
            EncodedTerm::BooleanLiteral(false) => TYPE_BOOLEAN_LITERAL_FALSE,
        }
    }
}

impl From<bool> for EncodedTerm {
    fn from(val: bool) -> Self {
        EncodedTerm::BooleanLiteral(val)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct EncodedQuad {
    pub subject: EncodedTerm,
    pub predicate: EncodedTerm,
    pub object: EncodedTerm,
    pub graph_name: EncodedTerm,
}

impl EncodedQuad {
    pub fn new(
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Self {
        Self {
            subject,
            predicate,
            object,
            graph_name,
        }
    }
}

pub trait TermReader {
    fn read_term(&mut self) -> Result<EncodedTerm>;
    fn read_spog_quad(&mut self) -> Result<EncodedQuad>;
    fn read_posg_quad(&mut self) -> Result<EncodedQuad>;
    fn read_ospg_quad(&mut self) -> Result<EncodedQuad>;
}

impl<R: Read> TermReader for R {
    fn read_term(&mut self) -> Result<EncodedTerm> {
        match self.read_u8()? {
            TYPE_DEFAULT_GRAPH_ID => Ok(EncodedTerm::DefaultGraph {}),
            TYPE_NAMED_NODE_ID => Ok(EncodedTerm::NamedNode {
                iri_id: self.read_u64::<NetworkEndian>()?,
            }),
            TYPE_BLANK_NODE_ID => {
                let mut uuid_buffer = [0 as u8; 16];
                self.read_exact(&mut uuid_buffer)?;
                Ok(EncodedTerm::BlankNode(Uuid::from_bytes(uuid_buffer)))
            }
            TYPE_SIMPLE_LITERAL_ID => Ok(EncodedTerm::SimpleLiteral {
                value_id: self.read_u64::<NetworkEndian>()?,
            }),
            TYPE_LANG_STRING_LITERAL_ID => Ok(EncodedTerm::LangStringLiteral {
                language_id: self.read_u64::<NetworkEndian>()?,
                value_id: self.read_u64::<NetworkEndian>()?,
            }),
            TYPE_TYPED_LITERAL_ID => Ok(EncodedTerm::TypedLiteral {
                datatype_id: self.read_u64::<NetworkEndian>()?,
                value_id: self.read_u64::<NetworkEndian>()?,
            }),
            TYPE_STRING_LITERAL => Ok(EncodedTerm::SimpleLiteral {
                value_id: self.read_u64::<NetworkEndian>()?,
            }),
            TYPE_BOOLEAN_LITERAL_TRUE => Ok(EncodedTerm::BooleanLiteral(true)),
            TYPE_BOOLEAN_LITERAL_FALSE => Ok(EncodedTerm::BooleanLiteral(false)),
            _ => Err("the term buffer has an invalid type id".into()),
        }
    }

    fn read_spog_quad(&mut self) -> Result<EncodedQuad> {
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_posg_quad(&mut self) -> Result<EncodedQuad> {
        let predicate = self.read_term()?;
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }

    fn read_ospg_quad(&mut self) -> Result<EncodedQuad> {
        let object = self.read_term()?;
        let subject = self.read_term()?;
        let predicate = self.read_term()?;
        let graph_name = self.read_term()?;
        Ok(EncodedQuad {
            subject,
            predicate,
            object,
            graph_name,
        })
    }
}

pub trait TermWriter {
    fn write_term(&mut self, term: EncodedTerm) -> Result<()>;
    fn write_spog_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
    fn write_posg_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
    fn write_ospg_quad(&mut self, quad: &EncodedQuad) -> Result<()>;
}

impl<R: Write> TermWriter for R {
    fn write_term(&mut self, term: EncodedTerm) -> Result<()> {
        self.write_u8(term.type_id())?;
        match term {
            EncodedTerm::DefaultGraph {} => {}
            EncodedTerm::NamedNode { iri_id } => self.write_u64::<NetworkEndian>(iri_id)?,
            EncodedTerm::BlankNode(id) => self.write_all(id.as_bytes())?,
            EncodedTerm::SimpleLiteral { value_id } => {
                self.write_u64::<NetworkEndian>(value_id)?;
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => {
                self.write_u64::<NetworkEndian>(language_id)?;
                self.write_u64::<NetworkEndian>(value_id)?;
            }
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => {
                self.write_u64::<NetworkEndian>(datatype_id)?;
                self.write_u64::<NetworkEndian>(value_id)?;
            }

            EncodedTerm::StringLiteral { value_id } => {
                self.write_u64::<NetworkEndian>(value_id)?;
            }
            EncodedTerm::BooleanLiteral(_) => {}
        }
        Ok(())
    }

    fn write_spog_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.subject)?;
        self.write_term(quad.predicate)?;
        self.write_term(quad.object)?;
        self.write_term(quad.graph_name)?;
        Ok(())
    }

    fn write_posg_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.predicate)?;
        self.write_term(quad.object)?;
        self.write_term(quad.subject)?;
        self.write_term(quad.graph_name)?;
        Ok(())
    }

    fn write_ospg_quad(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.write_term(quad.object)?;
        self.write_term(quad.subject)?;
        self.write_term(quad.predicate)?;
        self.write_term(quad.graph_name)?;
        Ok(())
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
        Ok(EncodedTerm::BlankNode(*blank_node.as_uuid()))
    }

    pub fn encode_literal(&self, literal: &Literal) -> Result<EncodedTerm> {
        Ok(if literal.is_plain() {
            if let Some(language) = literal.language() {
                EncodedTerm::LangStringLiteral {
                    value_id: self.encode_str_value(&literal.value())?,
                    language_id: self.encode_str_value(language)?,
                }
            } else {
                EncodedTerm::SimpleLiteral {
                    value_id: self.encode_str_value(&literal.value())?,
                }
            }
        } else if literal.is_string() {
            EncodedTerm::StringLiteral {
                value_id: self.encode_str_value(&literal.value())?,
            }
        } else if literal.is_boolean() {
            EncodedTerm::BooleanLiteral(
                literal
                    .to_bool()
                    .ok_or_else(|| Error::from("boolean literal without boolean value"))?,
            )
        } else {
            EncodedTerm::TypedLiteral {
                value_id: self.encode_str_value(&literal.value())?,
                datatype_id: self.encode_str_value(literal.datatype().as_str())?,
            }
        })
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
                Some(graph_name) => self.encode_named_or_blank_node(&graph_name)?,
                None => ENCODED_DEFAULT_GRAPH,
            },
        })
    }

    pub fn encode_triple_in_graph(
        &self,
        triple: &Triple,
        graph_name: EncodedTerm,
    ) -> Result<EncodedQuad> {
        Ok(EncodedQuad {
            subject: self.encode_named_or_blank_node(triple.subject())?,
            predicate: self.encode_named_node(triple.predicate())?,
            object: self.encode_term(triple.object())?,
            graph_name,
        })
    }

    pub fn decode_term(&self, encoded: EncodedTerm) -> Result<Term> {
        match encoded {
            EncodedTerm::DefaultGraph {} => Err("The default graph tag is not a valid term".into()),
            EncodedTerm::NamedNode { iri_id } => {
                Ok(NamedNode::from(self.decode_url_value(iri_id)?).into())
            }
            EncodedTerm::BlankNode(id) => Ok(BlankNode::from(id).into()),
            EncodedTerm::SimpleLiteral { value_id } => {
                Ok(Literal::new_simple_literal(self.decode_str_value(value_id)?).into())
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => Ok(Literal::new_language_tagged_literal(
                self.decode_str_value(value_id)?,
                self.decode_str_value(language_id)?,
            ).into()),
            EncodedTerm::TypedLiteral {
                value_id,
                datatype_id,
            } => Ok(Literal::new_typed_literal(
                self.decode_str_value(value_id)?,
                NamedNode::from(self.decode_url_value(datatype_id)?),
            ).into()),
            EncodedTerm::StringLiteral { value_id } => {
                Ok(Literal::from(self.decode_str_value(value_id)?).into())
            }
            EncodedTerm::BooleanLiteral(value) => Ok(Literal::from(value).into()),
        }
    }

    pub fn decode_named_or_blank_node(&self, encoded: EncodedTerm) -> Result<NamedOrBlankNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_named_node(&self, encoded: EncodedTerm) -> Result<NamedNode> {
        match self.decode_term(encoded)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err("A blank node has been found instead of a named node".into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_triple(&self, encoded: &EncodedQuad) -> Result<Triple> {
        Ok(Triple::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
        ))
    }

    pub fn decode_quad(&self, encoded: &EncodedQuad) -> Result<Quad> {
        Ok(Quad::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
            match encoded.graph_name {
                EncodedTerm::DefaultGraph {} => None,
                graph_name => Some(self.decode_named_or_blank_node(graph_name)?),
            },
        ))
    }

    fn encode_str_value(&self, text: &str) -> Result<u64> {
        self.string_store.insert_bytes(text.as_bytes())
    }

    fn decode_url_value(&self, id: u64) -> Result<Url> {
        let bytes = self.decode_value(id)?;
        Ok(Url::from_str(str::from_utf8(&bytes)?)?)
    }

    fn decode_str_value(&self, id: u64) -> Result<String> {
        let bytes = self.decode_value(id)?;
        Ok(str::from_utf8(&bytes)?.to_owned())
    }

    fn decode_value(&self, id: u64) -> Result<S::BytesOutput> {
        self.string_store
            .get_bytes(id)?
            .ok_or_else(|| "value not found in the dictionary".into())
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
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use store::numeric_encoder::*;

    #[derive(Default)]
    struct MemoryBytesStore {
        id2str: RefCell<BTreeMap<u64, Vec<u8>>>,
        str2id: RefCell<BTreeMap<Vec<u8>, u64>>,
    }

    impl BytesStore for MemoryBytesStore {
        type BytesOutput = Vec<u8>;

        fn insert_bytes(&self, value: &[u8]) -> Result<u64> {
            let mut str2id = self.str2id.borrow_mut();
            let mut id2str = self.id2str.borrow_mut();
            let id = str2id.entry(value.to_vec()).or_insert_with(|| {
                let id = id2str.len() as u64;
                id2str.insert(id, value.to_vec());
                id
            });
            Ok(*id)
        }

        fn get_bytes(&self, id: u64) -> Result<Option<Vec<u8>>> {
            Ok(self.id2str.borrow().get(&id).map(|s| s.to_owned()))
        }
    }

    #[test]
    fn test_encoding() {
        use model::*;

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
            assert_eq!(term, encoder.decode_term(encoded).unwrap())
        }
    }

    #[test]
    fn test_encoded_term_size() {
        use std::mem::size_of;

        assert_eq!(size_of::<EncodedTerm>(), 24);
    }

}
