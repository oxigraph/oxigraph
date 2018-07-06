use errors::*;
use model::*;
use std::ops::Deref;
use std::str;
use std::str::FromStr;
use url::Url;
use uuid::Uuid;

pub const STRING_KEY_SIZE: usize = 8;

pub trait BytesStore {
    type BytesOutput: Deref<Target = [u8]>;

    fn put(&self, value: &[u8], id_buffer: &mut [u8]) -> Result<()>;
    fn get(&self, id: &[u8]) -> Result<Option<Self::BytesOutput>>;
}

const TYPE_KEY_SIZE: usize = 1;
const TYPE_NAMED_NODE_ID: u8 = 1;
const TYPE_BLANK_NODE_ID: u8 = 2;
const TYPE_LANG_STRING_LITERAL_ID: u8 = 3;
const TYPE_TYPED_LITERAL_ID: u8 = 4;
pub const TERM_ENCODING_SIZE: usize = TYPE_KEY_SIZE + 2 * STRING_KEY_SIZE;
const EMPTY_TERM: [u8; TERM_ENCODING_SIZE] = [0 as u8; TERM_ENCODING_SIZE];

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct EncodedTerm([u8; TERM_ENCODING_SIZE]);

impl EncodedTerm {
    pub fn new_from_buffer(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != TERM_ENCODING_SIZE {
            return Err("the term buffer has not the correct length".into());
        }
        let mut buf = [0 as u8; TERM_ENCODING_SIZE];
        buf.copy_from_slice(buffer);
        return Ok(EncodedTerm(buf));
    }
}

impl AsRef<[u8]> for EncodedTerm {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
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
    pub fn new_from_spog_buffer(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != 4 * TERM_ENCODING_SIZE {
            return Err("the spog buffer has not the correct length".into());
        }
        Ok(Self {
            subject: EncodedTerm::new_from_buffer(&buffer[0..TERM_ENCODING_SIZE])?,
            predicate: EncodedTerm::new_from_buffer(
                &buffer[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE],
            )?,
            object: EncodedTerm::new_from_buffer(
                &buffer[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE],
            )?,
            graph_name: EncodedTerm::new_from_buffer(
                &buffer[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE],
            )?,
        })
    }

    pub fn new_from_posg_buffer(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != 4 * TERM_ENCODING_SIZE {
            return Err("the posg buffer has not the correct length".into());
        }
        Ok(Self {
            subject: EncodedTerm::new_from_buffer(
                &buffer[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE],
            )?,
            predicate: EncodedTerm::new_from_buffer(&buffer[0..TERM_ENCODING_SIZE])?,
            object: EncodedTerm::new_from_buffer(
                &buffer[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE],
            )?,
            graph_name: EncodedTerm::new_from_buffer(
                &buffer[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE],
            )?,
        })
    }

    pub fn new_from_ospg_buffer(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != 4 * TERM_ENCODING_SIZE {
            return Err("the ospg buffer has not the correct length".into());
        }
        Ok(Self {
            subject: EncodedTerm::new_from_buffer(
                &buffer[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE],
            )?,
            predicate: EncodedTerm::new_from_buffer(
                &buffer[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE],
            )?,
            object: EncodedTerm::new_from_buffer(&buffer[0..TERM_ENCODING_SIZE])?,
            graph_name: EncodedTerm::new_from_buffer(
                &buffer[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE],
            )?,
        })
    }

    pub fn spog(&self) -> [u8; 4 * TERM_ENCODING_SIZE] {
        let mut spog = [0 as u8; 4 * TERM_ENCODING_SIZE];
        spog[0..TERM_ENCODING_SIZE].copy_from_slice(self.subject.as_ref());
        spog[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE].copy_from_slice(self.predicate.as_ref());
        spog[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE].copy_from_slice(self.object.as_ref());
        spog[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE]
            .copy_from_slice(self.graph_name.as_ref());
        spog
    }

    pub fn posg(&self) -> [u8; 4 * TERM_ENCODING_SIZE] {
        let mut posg = [0 as u8; 4 * TERM_ENCODING_SIZE];
        posg[0..TERM_ENCODING_SIZE].copy_from_slice(self.predicate.as_ref());
        posg[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE].copy_from_slice(self.object.as_ref());
        posg[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE].copy_from_slice(self.subject.as_ref());
        posg[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE]
            .copy_from_slice(self.graph_name.as_ref());
        posg
    }

    pub fn ospg(&self) -> [u8; 4 * TERM_ENCODING_SIZE] {
        let mut ospg = [0 as u8; 4 * TERM_ENCODING_SIZE];
        ospg[0..TERM_ENCODING_SIZE].copy_from_slice(self.object.as_ref());
        ospg[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE].copy_from_slice(self.subject.as_ref());
        ospg[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE]
            .copy_from_slice(self.predicate.as_ref());
        ospg[3 * TERM_ENCODING_SIZE..4 * TERM_ENCODING_SIZE]
            .copy_from_slice(self.graph_name.as_ref());
        ospg
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
        let mut bytes = [0 as u8; TERM_ENCODING_SIZE];
        bytes[0] = TYPE_NAMED_NODE_ID;
        self.encode_str_value_to_lower_bytes(named_node.as_str(), &mut bytes)?;
        Ok(EncodedTerm(bytes))
    }

    pub fn encode_blank_node(&self, blank_node: &BlankNode) -> Result<EncodedTerm> {
        let mut bytes = [0 as u8; TERM_ENCODING_SIZE];
        bytes[0] = TYPE_BLANK_NODE_ID;
        bytes[TYPE_KEY_SIZE..TERM_ENCODING_SIZE].copy_from_slice(blank_node.as_bytes());
        Ok(EncodedTerm(bytes))
    }

    pub fn encode_literal(&self, literal: &Literal) -> Result<EncodedTerm> {
        let mut bytes = [0 as u8; TERM_ENCODING_SIZE];
        if let Some(language) = literal.language() {
            bytes[0] = TYPE_LANG_STRING_LITERAL_ID;
            self.encode_str_value_to_upper_bytes(language, &mut bytes)?;
        } else {
            bytes[0] = TYPE_TYPED_LITERAL_ID;
            self.encode_str_value_to_upper_bytes(literal.datatype().as_str(), &mut bytes)?;
        }
        self.encode_str_value_to_lower_bytes(literal.value().as_str(), &mut bytes)?;
        Ok(EncodedTerm(bytes))
    }

    pub fn encode_named_or_blank_node(&self, term: &NamedOrBlankNode) -> Result<EncodedTerm> {
        match term {
            NamedOrBlankNode::NamedNode(named_node) => self.encode_named_node(named_node),
            NamedOrBlankNode::BlankNode(blank_node) => self.encode_blank_node(blank_node),
        }
    }

    pub fn encode_optional_named_or_blank_node(
        &self,
        term: &Option<NamedOrBlankNode>,
    ) -> Result<EncodedTerm> {
        match term {
            Some(node) => self.encode_named_or_blank_node(node),
            None => Ok(EncodedTerm(EMPTY_TERM)),
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
            graph_name: self.encode_optional_named_or_blank_node(quad.graph_name())?,
        })
    }

    pub fn decode_term(&self, encoded: impl AsRef<[u8]>) -> Result<Term> {
        let encoding = encoded.as_ref();
        match encoding[0] {
            TYPE_NAMED_NODE_ID => {
                let iri = self.decode_url_value_from_lower_bytes(encoding)?;
                Ok(NamedNode::from(iri).into())
            }
            TYPE_BLANK_NODE_ID => Ok(BlankNode::from(Uuid::from_bytes(&encoding[1..])?).into()),
            TYPE_LANG_STRING_LITERAL_ID => {
                let value = self.decode_str_value_from_lower_bytes(encoding)?;
                let language = self.decode_str_value_from_upper_bytes(encoding)?;
                Ok(Literal::new_language_tagged_literal(value, language).into())
            }
            TYPE_TYPED_LITERAL_ID => {
                let value = self.decode_str_value_from_lower_bytes(encoding)?;
                let datatype = NamedNode::from(self.decode_url_value_from_upper_bytes(encoding)?);
                Ok(Literal::new_typed_literal(value, datatype).into())
            }
            _ => Err("invalid term type encoding".into()),
        }
    }

    pub fn decode_named_or_blank_node(
        &self,
        encoded: impl AsRef<[u8]>,
    ) -> Result<NamedOrBlankNode> {
        let encoding = encoded.as_ref();
        match self.decode_term(encoding)? {
            Term::NamedNode(named_node) => Ok(named_node.into()),
            Term::BlankNode(blank_node) => Ok(blank_node.into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_optional_named_or_blank_node(
        &self,
        encoded: impl AsRef<[u8]>,
    ) -> Result<Option<NamedOrBlankNode>> {
        let encoding = encoded.as_ref();
        if encoding == EMPTY_TERM {
            Ok(None)
        } else {
            Ok(Some(self.decode_named_or_blank_node(encoding)?))
        }
    }

    pub fn decode_named_node(&self, encoded: impl AsRef<[u8]>) -> Result<NamedNode> {
        let encoding = encoded.as_ref();
        match self.decode_term(encoding)? {
            Term::NamedNode(named_node) => Ok(named_node),
            Term::BlankNode(_) => Err("A blank node has been found instead of a named node".into()),
            Term::Literal(_) => Err("A literal has ben found instead of a named node".into()),
        }
    }

    pub fn decode_quad(&self, encoded: EncodedQuad) -> Result<Quad> {
        Ok(Quad::new(
            self.decode_named_or_blank_node(encoded.subject)?,
            self.decode_named_node(encoded.predicate)?,
            self.decode_term(encoded.object)?,
            self.decode_optional_named_or_blank_node(encoded.graph_name)?,
        ))
    }

    fn encode_str_value_to_upper_bytes(&self, text: &str, bytes: &mut [u8]) -> Result<()> {
        self.string_store.put(
            text.as_bytes(),
            &mut bytes[TYPE_KEY_SIZE..TYPE_KEY_SIZE + STRING_KEY_SIZE],
        )
    }
    fn encode_str_value_to_lower_bytes(&self, text: &str, bytes: &mut [u8]) -> Result<()> {
        self.string_store.put(
            text.as_bytes(),
            &mut bytes[TYPE_KEY_SIZE + STRING_KEY_SIZE..TYPE_KEY_SIZE + 2 * STRING_KEY_SIZE],
        )
    }

    fn decode_str_value_from_upper_bytes(&self, encoding: &[u8]) -> Result<String> {
        let bytes = self.decode_value_from_upper_bytes(encoding)?;
        Ok(str::from_utf8(&bytes)?.to_string())
    }

    fn decode_url_value_from_upper_bytes(&self, encoding: &[u8]) -> Result<Url> {
        let bytes = self.decode_value_from_upper_bytes(encoding)?;
        Ok(Url::from_str(str::from_utf8(&bytes)?)?)
    }

    fn decode_value_from_upper_bytes(&self, encoding: &[u8]) -> Result<S::BytesOutput> {
        self.string_store
            .get(&encoding[TYPE_KEY_SIZE..TYPE_KEY_SIZE + STRING_KEY_SIZE])?
            .ok_or(Error::from("value not found in the dictionary"))
    }

    fn decode_str_value_from_lower_bytes(&self, encoding: &[u8]) -> Result<String> {
        let bytes = self.decode_value_from_lower_bytes(encoding)?;
        Ok(str::from_utf8(&bytes)?.to_string())
    }

    fn decode_url_value_from_lower_bytes(&self, encoding: &[u8]) -> Result<Url> {
        let bytes = self.decode_value_from_lower_bytes(encoding)?;
        Ok(Url::from_str(str::from_utf8(&bytes)?)?)
    }

    fn decode_value_from_lower_bytes(&self, encoding: &[u8]) -> Result<S::BytesOutput> {
        self.string_store
            .get(&encoding[TYPE_KEY_SIZE + STRING_KEY_SIZE..TYPE_KEY_SIZE + 2 * STRING_KEY_SIZE])?
            .ok_or(Error::from("value not found in the dictionary"))
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
    use errors::*;
    use model::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use store::numeric_encoder::BytesStore;
    use store::numeric_encoder::Encoder;
    use store::numeric_encoder::STRING_KEY_SIZE;
    use store::numeric_encoder::TERM_ENCODING_SIZE;
    use utils::to_bytes;

    #[derive(Default)]
    struct MemoryBytesStore {
        id2str: RefCell<BTreeMap<[u8; STRING_KEY_SIZE], Vec<u8>>>,
        str2id: RefCell<BTreeMap<Vec<u8>, [u8; STRING_KEY_SIZE]>>,
    }

    impl BytesStore for MemoryBytesStore {
        type BytesOutput = Vec<u8>;

        fn put(&self, value: &[u8], id_buffer: &mut [u8]) -> Result<()> {
            let mut str2id = self.str2id.borrow_mut();
            let mut id2str = self.id2str.borrow_mut();
            let id = str2id.entry(value.to_vec()).or_insert_with(|| {
                let id = to_bytes(id2str.len());
                id2str.insert(id, value.to_vec());
                id
            });
            id_buffer.copy_from_slice(id);
            Ok(())
        }

        fn get(&self, id: &[u8]) -> Result<Option<Vec<u8>>> {
            Ok(self.id2str.borrow().get(id).map(|s| s.to_owned()))
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
            assert_eq!(term, encoder.decode_term(encoded).unwrap())
        }
    }

}
