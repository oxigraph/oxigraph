use crate::format_err;
use crate::utils::to_err;
use js_sys::{Reflect, UriError};
use oxigraph::model::*;
use std::convert::TryFrom;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = DataFactory)]
#[derive(Default)]
pub struct JsDataFactory {
    from_js: FromJsConverter,
}

#[wasm_bindgen(js_class = DataFactory)]
impl JsDataFactory {
    #[wasm_bindgen(js_name = namedNode)]
    pub fn named_node(&self, value: String) -> Result<JsNamedNode, JsValue> {
        NamedNode::new(value)
            .map(|v| v.into())
            .map_err(|v| UriError::new(&v.to_string()).into())
    }

    #[wasm_bindgen(js_name = blankNode)]
    pub fn blank_node(&self, value: Option<String>) -> Result<JsBlankNode, JsValue> {
        Ok(if let Some(value) = value {
            BlankNode::new(value).map_err(to_err)?
        } else {
            BlankNode::default()
        }
        .into())
    }

    #[wasm_bindgen]
    pub fn literal(
        &self,
        value: Option<String>,
        language_or_datatype: &JsValue,
    ) -> Result<JsLiteral, JsValue> {
        if language_or_datatype.is_null() || language_or_datatype.is_undefined() {
            Ok(Literal::new_simple_literal(value.unwrap_or_else(String::new)).into())
        } else if language_or_datatype.is_string() {
            Ok(Literal::new_language_tagged_literal(
                value.unwrap_or_else(String::new),
                language_or_datatype.as_string().unwrap_or_else(String::new),
            )
            .map_err(to_err)?
            .into())
        } else if let JsTerm::NamedNode(datatype) = self.from_js.to_term(language_or_datatype)? {
            Ok(Literal::new_typed_literal(value.unwrap_or_else(String::new), datatype).into())
        } else {
            Err(format_err!("The literal datatype should be a NamedNode"))
        }
    }

    #[wasm_bindgen(js_name = defaultGraph)]
    pub fn default_graph(&self) -> JsDefaultGraph {
        JsDefaultGraph {}
    }

    #[wasm_bindgen(js_name = triple)]
    pub fn triple(
        &self,
        subject: &JsValue,
        predicate: &JsValue,
        object: &JsValue,
    ) -> Result<JsQuad, JsValue> {
        Ok(JsQuad {
            subject: self.from_js.to_term(subject)?,
            predicate: self.from_js.to_term(predicate)?,
            object: self.from_js.to_term(object)?,
            graph_name: JsTerm::DefaultGraph(JsDefaultGraph {}),
        })
    }

    #[wasm_bindgen(js_name = quad)]
    pub fn quad(
        &self,
        subject: &JsValue,
        predicate: &JsValue,
        object: &JsValue,
        graph: &JsValue,
    ) -> Result<JsQuad, JsValue> {
        Ok(JsQuad {
            subject: self.from_js.to_term(subject)?,
            predicate: self.from_js.to_term(predicate)?,
            object: self.from_js.to_term(object)?,
            graph_name: if graph.is_undefined() || graph.is_null() {
                JsTerm::DefaultGraph(JsDefaultGraph {})
            } else {
                self.from_js.to_term(&graph)?
            },
        })
    }

    #[wasm_bindgen(js_name = fromTerm)]
    pub fn convert_term(&self, original: &JsValue) -> Result<JsValue, JsValue> {
        Ok(self.from_js.to_term(original)?.into())
    }

    #[wasm_bindgen(js_name = fromQuad)]
    pub fn convert_quad(&self, original: &JsValue) -> Result<JsQuad, JsValue> {
        self.from_js.to_quad(original)
    }
}

#[wasm_bindgen(js_name = NamedNode)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct JsNamedNode {
    inner: NamedNode,
}

#[wasm_bindgen(js_class = NamedNode)]
impl JsNamedNode {
    #[wasm_bindgen(getter = termType)]
    pub fn term_type(&self) -> String {
        "NamedNode".to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn value(&self) -> String {
        self.inner.as_str().to_owned()
    }

    pub fn equals(&self, other: &JsValue) -> bool {
        if let Ok(Some(JsTerm::NamedNode(other))) =
            FromJsConverter::default().to_optional_term(&other)
        {
            self == &other
        } else {
            false
        }
    }
}

impl From<NamedNode> for JsNamedNode {
    fn from(inner: NamedNode) -> Self {
        Self { inner }
    }
}

impl From<JsNamedNode> for NamedNode {
    fn from(node: JsNamedNode) -> Self {
        node.inner
    }
}

impl From<JsNamedNode> for NamedOrBlankNode {
    fn from(node: JsNamedNode) -> Self {
        node.inner.into()
    }
}

impl From<JsNamedNode> for Term {
    fn from(node: JsNamedNode) -> Self {
        node.inner.into()
    }
}

impl From<JsNamedNode> for GraphName {
    fn from(node: JsNamedNode) -> Self {
        node.inner.into()
    }
}

#[wasm_bindgen(js_name = BlankNode)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct JsBlankNode {
    inner: BlankNode,
}

#[wasm_bindgen(js_class = BlankNode)]
impl JsBlankNode {
    #[wasm_bindgen(getter = termType)]
    pub fn term_type(&self) -> String {
        "BlankNode".to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn value(&self) -> String {
        self.inner.as_str().to_owned()
    }

    pub fn equals(&self, other: &JsValue) -> bool {
        if let Ok(Some(JsTerm::BlankNode(other))) =
            FromJsConverter::default().to_optional_term(&other)
        {
            self == &other
        } else {
            false
        }
    }
}

impl From<BlankNode> for JsBlankNode {
    fn from(inner: BlankNode) -> Self {
        Self { inner }
    }
}

impl From<JsBlankNode> for BlankNode {
    fn from(node: JsBlankNode) -> Self {
        node.inner
    }
}

impl From<JsBlankNode> for NamedOrBlankNode {
    fn from(node: JsBlankNode) -> Self {
        node.inner.into()
    }
}

impl From<JsBlankNode> for Term {
    fn from(node: JsBlankNode) -> Self {
        node.inner.into()
    }
}

impl From<JsBlankNode> for GraphName {
    fn from(node: JsBlankNode) -> Self {
        node.inner.into()
    }
}

#[wasm_bindgen(js_name = Literal)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct JsLiteral {
    inner: Literal,
}

#[wasm_bindgen(js_class = Literal)]
impl JsLiteral {
    #[wasm_bindgen(getter = termType)]
    pub fn term_type(&self) -> String {
        "Literal".to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn value(&self) -> String {
        self.inner.value().to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn language(&self) -> String {
        self.inner.language().unwrap_or("").to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn datatype(&self) -> JsNamedNode {
        self.inner.datatype().into_owned().into()
    }

    pub fn equals(&self, other: &JsValue) -> bool {
        if let Ok(Some(JsTerm::Literal(other))) =
            FromJsConverter::default().to_optional_term(&other)
        {
            self == &other
        } else {
            false
        }
    }
}

impl From<Literal> for JsLiteral {
    fn from(inner: Literal) -> Self {
        Self { inner }
    }
}

impl From<JsLiteral> for Literal {
    fn from(node: JsLiteral) -> Self {
        node.inner
    }
}

impl From<JsLiteral> for Term {
    fn from(node: JsLiteral) -> Self {
        node.inner.into()
    }
}

#[wasm_bindgen(js_name = DefaultGraph)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct JsDefaultGraph {}

#[wasm_bindgen(js_class = DefaultGraph)]
impl JsDefaultGraph {
    #[wasm_bindgen(getter = termType)]
    pub fn term_type(&self) -> String {
        "DefaultGraph".to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn value(&self) -> String {
        "".to_owned()
    }

    pub fn equals(&self, other: &JsValue) -> bool {
        if let Ok(Some(JsTerm::DefaultGraph(other))) =
            FromJsConverter::default().to_optional_term(&other)
        {
            self == &other
        } else {
            false
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum JsTerm {
    NamedNode(JsNamedNode),
    BlankNode(JsBlankNode),
    Literal(JsLiteral),
    DefaultGraph(JsDefaultGraph),
}

impl From<JsTerm> for JsValue {
    fn from(value: JsTerm) -> Self {
        match value {
            JsTerm::NamedNode(v) => v.into(),
            JsTerm::BlankNode(v) => v.into(),
            JsTerm::Literal(v) => v.into(),
            JsTerm::DefaultGraph(v) => v.into(),
        }
    }
}

impl From<NamedNode> for JsTerm {
    fn from(node: NamedNode) -> Self {
        JsTerm::NamedNode(node.into())
    }
}

impl From<BlankNode> for JsTerm {
    fn from(node: BlankNode) -> Self {
        JsTerm::BlankNode(node.into())
    }
}

impl From<Literal> for JsTerm {
    fn from(literal: Literal) -> Self {
        JsTerm::Literal(literal.into())
    }
}

impl From<NamedOrBlankNode> for JsTerm {
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl From<Term> for JsTerm {
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            Term::Literal(literal) => literal.into(),
        }
    }
}

impl From<GraphName> for JsTerm {
    fn from(name: GraphName) -> Self {
        match name {
            GraphName::NamedNode(node) => node.into(),
            GraphName::BlankNode(node) => node.into(),
            GraphName::DefaultGraph => JsTerm::DefaultGraph(JsDefaultGraph {}),
        }
    }
}

impl TryFrom<JsTerm> for NamedNode {
    type Error = JsValue;

    fn try_from(value: JsTerm) -> Result<Self, JsValue> {
        match value {
            JsTerm::NamedNode(node) => Ok(node.into()),
            JsTerm::BlankNode(node) => Err(format_err!(
                "The blank node {} is not a named node",
                node.inner
            )),
            JsTerm::Literal(literal) => Err(format_err!(
                "The literal {} is not a named node",
                literal.inner
            )),
            JsTerm::DefaultGraph(_) => Err(format_err!("The default graph is not a named node")),
        }
    }
}

impl TryFrom<JsTerm> for NamedOrBlankNode {
    type Error = JsValue;

    fn try_from(value: JsTerm) -> Result<Self, JsValue> {
        match value {
            JsTerm::NamedNode(node) => Ok(node.into()),
            JsTerm::BlankNode(node) => Ok(node.into()),
            JsTerm::Literal(literal) => Err(format_err!(
                "The literal {} is not a possible named or blank node term",
                literal.inner
            )),
            JsTerm::DefaultGraph(_) => {
                Err(format_err!("The default graph is not a possible RDF term"))
            }
        }
    }
}

impl TryFrom<JsTerm> for Term {
    type Error = JsValue;

    fn try_from(value: JsTerm) -> Result<Self, JsValue> {
        match value {
            JsTerm::NamedNode(node) => Ok(node.into()),
            JsTerm::BlankNode(node) => Ok(node.into()),
            JsTerm::Literal(literal) => Ok(literal.into()),
            JsTerm::DefaultGraph(_) => {
                Err(format_err!("The default graph is not a possible RDF term"))
            }
        }
    }
}

impl TryFrom<JsTerm> for GraphName {
    type Error = JsValue;

    fn try_from(value: JsTerm) -> Result<Self, JsValue> {
        match value {
            JsTerm::NamedNode(node) => Ok(node.into()),
            JsTerm::BlankNode(node) => Ok(node.into()),
            JsTerm::Literal(literal) => Err(format_err!(
                "The literal {} is not a possible graph name",
                literal.inner
            )),
            JsTerm::DefaultGraph(_) => Ok(GraphName::DefaultGraph),
        }
    }
}

#[wasm_bindgen(js_name = Quad)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct JsQuad {
    subject: JsTerm,
    predicate: JsTerm,
    object: JsTerm,
    graph_name: JsTerm,
}

#[wasm_bindgen(js_class = Quad)]
impl JsQuad {
    #[wasm_bindgen(getter = subject)]
    pub fn subject(&self) -> JsValue {
        self.subject.clone().into()
    }

    #[wasm_bindgen(getter = predicate)]
    pub fn predicate(&self) -> JsValue {
        self.predicate.clone().into()
    }

    #[wasm_bindgen(getter = object)]
    pub fn object(&self) -> JsValue {
        self.object.clone().into()
    }

    #[wasm_bindgen(getter = graph)]
    pub fn graph(&self) -> JsValue {
        self.graph_name.clone().into()
    }

    pub fn equals(&self, other: &JsValue) -> bool {
        FromJsConverter::default()
            .to_quad(&other)
            .map_or(false, |other| self == &other)
    }
}

impl From<Quad> for JsQuad {
    fn from(quad: Quad) -> Self {
        Self {
            subject: quad.subject.into(),
            predicate: quad.predicate.into(),
            object: quad.object.into(),
            graph_name: quad.graph_name.into(),
        }
    }
}

impl TryFrom<JsQuad> for Quad {
    type Error = JsValue;

    fn try_from(quad: JsQuad) -> Result<Self, JsValue> {
        Ok(Quad {
            subject: NamedOrBlankNode::try_from(quad.subject)?,
            predicate: NamedNode::try_from(quad.predicate)?,
            object: Term::try_from(quad.object)?,
            graph_name: GraphName::try_from(quad.graph_name)?,
        })
    }
}

pub struct FromJsConverter {
    term_type: JsValue,
    value: JsValue,
    language: JsValue,
    datatype: JsValue,
    subject: JsValue,
    predicate: JsValue,
    object: JsValue,
    graph: JsValue,
}

impl Default for FromJsConverter {
    fn default() -> Self {
        Self {
            term_type: JsValue::from_str("termType"),
            value: JsValue::from_str("value"),
            language: JsValue::from_str("language"),
            datatype: JsValue::from_str("datatype"),
            subject: JsValue::from_str("subject"),
            predicate: JsValue::from_str("predicate"),
            object: JsValue::from_str("object"),
            graph: JsValue::from_str("graph"),
        }
    }
}

impl FromJsConverter {
    pub fn to_term(&self, value: &JsValue) -> Result<JsTerm, JsValue> {
        let term_type = Reflect::get(&value, &self.term_type)?;
        if let Some(term_type) = term_type.as_string() {
            match term_type.as_str() {
                "NamedNode" => Ok(NamedNode::new(
                    Reflect::get(&value, &self.value)?
                        .as_string()
                        .ok_or_else(|| format_err!("NamedNode should have a string value"))?,
                )
                .map_err(|v| UriError::new(&v.to_string()))?
                .into()),
                "BlankNode" => Ok(BlankNode::new(
                    &Reflect::get(&value, &self.value)?
                        .as_string()
                        .ok_or_else(|| format_err!("BlankNode should have a string value"))?,
                )
                .map_err(to_err)?
                .into()),
                "Literal" => {
                    if let JsTerm::NamedNode(datatype) =
                        self.to_term(&Reflect::get(&value, &self.datatype)?)?
                    {
                        let datatype = NamedNode::from(datatype);
                        let literal_value = Reflect::get(&value, &self.value)?
                            .as_string()
                            .ok_or_else(|| format_err!("Literal should have a string value"))?;
                        Ok(match datatype.as_str() {
                                    "http://www.w3.org/2001/XMLSchema#string" => Literal::new_simple_literal(literal_value),
                                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" => Literal::new_language_tagged_literal(literal_value, Reflect::get(&value, &self.language)?.as_string().ok_or_else(
                                        || format_err!("Literal with rdf:langString datatype should have a language"),
                                    )?).map_err(to_err)?,
                                    _ => Literal::new_typed_literal(literal_value, datatype)
                                }.into())
                    } else {
                        Err(format_err!(
                            "Literal should have a datatype that is a NamedNode"
                        ))
                    }
                }
                "DefaultGraph" => Ok(JsTerm::DefaultGraph(JsDefaultGraph {})),
                _ => Err(format_err!(
                    "The termType {} is not supported by Oxigraph",
                    term_type
                )),
            }
        } else {
            Err(format_err!("The object termType field should be a string"))
        }
    }

    pub fn to_optional_term(&self, value: &JsValue) -> Result<Option<JsTerm>, JsValue> {
        if value.is_null() || value.is_undefined() {
            Ok(None)
        } else {
            self.to_term(value).map(Some)
        }
    }

    pub fn to_quad(&self, value: &JsValue) -> Result<JsQuad, JsValue> {
        Ok(JsQuad {
            subject: self.to_term(&Reflect::get(&value, &self.subject)?)?,
            predicate: self.to_term(&Reflect::get(&value, &self.predicate)?)?,
            object: self.to_term(&Reflect::get(&value, &self.object)?)?,
            graph_name: self.to_term(&Reflect::get(&value, &self.graph)?)?,
        })
    }
}
