use crate::format_err;
use crate::reflect::*;
use crate::utils::to_option_ref;
#[cfg(feature = "rdf-12")]
use js_sys::Object;
use js_sys::UriError;
use oxigraph::model::vocab::xsd;
use oxigraph::model::*;
use wasm_bindgen::prelude::*;

pub fn default_data_factory() -> DataFactory {
    FACTORY.with(|f| <&JsValue>::from(f).clone().into())
}

pub fn from_named_node(factory: &DataFactory, value: NamedNodeRef<'_>) -> JsValue {
    factory.named_node(value.as_str())
}

pub fn from_blank_node(factory: &DataFactory, value: BlankNodeRef<'_>) -> JsValue {
    factory.blank_node(value.as_str())
}

pub fn from_literal(factory: &DataFactory, value: LiteralRef<'_>) -> JsValue {
    if let Some(language) = value.language() {
        #[cfg(feature = "rdf-12")]
        if let Some(direction) = value.direction() {
            let directional_language = Object::new();
            reflect_set(&directional_language, &LANGUAGE, &language.into()).unwrap();
            match direction {
                BaseDirection::Ltr => &LTR,
                BaseDirection::Rtl => &RTL,
            }
            .with(|direction| reflect_set(&directional_language, &DIRECTION, direction))
            .unwrap();
            return factory.typed_literal(value.value(), directional_language.into());
        }
        factory.language_tagged_literal(value.value(), language)
    } else {
        let datatype = value.datatype();
        if datatype == xsd::STRING {
            factory.simple_literal(value.value())
        } else {
            factory.typed_literal(value.value(), from_named_node(factory, datatype))
        }
    }
}

pub fn from_named_or_blank_node(factory: &DataFactory, value: NamedOrBlankNodeRef<'_>) -> JsValue {
    match value {
        NamedOrBlankNodeRef::NamedNode(value) => from_named_node(factory, value),
        NamedOrBlankNodeRef::BlankNode(value) => from_blank_node(factory, value),
    }
}

pub fn from_term(factory: &DataFactory, value: TermRef<'_>) -> JsValue {
    match value {
        TermRef::NamedNode(value) => from_named_node(factory, value),
        TermRef::BlankNode(value) => from_blank_node(factory, value),
        TermRef::Literal(value) => from_literal(factory, value),
        #[cfg(feature = "rdf-12")]
        TermRef::Triple(value) => from_triple(factory, value.as_ref()),
    }
}

pub fn from_graph_name(factory: &DataFactory, value: GraphNameRef<'_>) -> JsValue {
    match value {
        GraphNameRef::NamedNode(value) => from_named_node(factory, value),
        GraphNameRef::BlankNode(value) => from_blank_node(factory, value),
        GraphNameRef::DefaultGraph => factory.default_graph(),
    }
}

pub fn from_triple(factory: &DataFactory, value: TripleRef<'_>) -> JsValue {
    factory.triple(
        from_named_or_blank_node(factory, value.subject),
        from_named_node(factory, value.predicate),
        from_term(factory, value.object),
    )
}

pub fn from_quad(factory: &DataFactory, value: QuadRef<'_>) -> JsValue {
    factory.quad(
        from_named_or_blank_node(factory, value.subject),
        from_named_node(factory, value.predicate),
        from_term(factory, value.object),
        from_graph_name(factory, value.graph_name),
    )
}

#[wasm_bindgen(module = "/src/data_model.js")]
extern "C" {
    pub type DataFactory;

    #[wasm_bindgen(thread_local_v2, js_name = factory)]
    static FACTORY: DataFactory;

    #[wasm_bindgen(method, js_name = namedNode)]
    fn named_node(this: &DataFactory, value: &str) -> JsValue;

    #[wasm_bindgen(method, js_name = blankNode)]
    fn blank_node(this: &DataFactory, value: &str) -> JsValue;

    #[wasm_bindgen(method, js_name = literal)]
    fn simple_literal(this: &DataFactory, value: &str) -> JsValue;

    #[wasm_bindgen(method, js_name = literal)]
    fn language_tagged_literal(this: &DataFactory, value: &str, language: &str) -> JsValue;

    #[wasm_bindgen(method, js_name = literal)]
    fn typed_literal(this: &DataFactory, value: &str, datatype: JsValue) -> JsValue;

    #[wasm_bindgen(method, js_name = defaultGraph)]
    fn default_graph(this: &DataFactory) -> JsValue;

    #[wasm_bindgen(method, js_name = quad)]
    fn triple(this: &DataFactory, subject: JsValue, predicate: JsValue, object: JsValue)
    -> JsValue;

    #[wasm_bindgen(method, js_name = quad)]
    fn quad(
        this: &DataFactory,
        subject: JsValue,
        predicate: JsValue,
        object: JsValue,
        graph: JsValue,
    ) -> JsValue;
}

pub fn to_named_node(value: &JsValue) -> Result<NamedNode, JsValue> {
    Ok(NamedNode::new(
        reflect_get(value, &VALUE)?
            .as_string()
            .ok_or_else(|| format_err!("NamedNode must have a string value"))?,
    )
    .map_err(|v| UriError::new(&v.to_string()))?)
}

pub fn to_blank_node(value: &JsValue) -> Result<BlankNode, JsValue> {
    Ok(BlankNode::new(
        reflect_get(value, &VALUE)?
            .as_string()
            .ok_or_else(|| format_err!("BlankNode must have a string value"))?,
    )
    .map_err(JsError::from)?)
}

pub fn to_literal(value: &JsValue) -> Result<Literal, JsValue> {
    let datatype = to_named_node(&reflect_get(value, &DATATYPE)?)?;
    let literal_value = reflect_get(value, &VALUE)?
        .as_string()
        .ok_or_else(|| format_err!("Literal must have a string value"))?;
    Ok(match datatype.as_str() {
        "http://www.w3.org/2001/XMLSchema#string" => Literal::new_simple_literal(literal_value),
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString" => {
            Literal::new_language_tagged_literal(
                literal_value,
                reflect_get(value, &LANGUAGE)?.as_string().ok_or_else(|| {
                    format_err!("Literal with rdf:langString datatype must have a language")
                })?,
            )
            .map_err(JsError::from)?
        }
        #[cfg(feature = "rdf-12")]
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#dirLangString" => {
            Literal::new_directional_language_tagged_literal(
                literal_value,
                reflect_get(value, &LANGUAGE)?.as_string().ok_or_else(|| {
                    format_err!("Literal with rdf:dirLangString datatype must have a language")
                })?,
                match reflect_get(value, &DIRECTION)?
                    .as_string()
                    .ok_or_else(|| {
                        format_err!("Literal with rdf:dirLangString datatype must have a direction")
                    })?
                    .as_str()
                {
                    "ltr" => BaseDirection::Ltr,
                    "rtl" => BaseDirection::Rtl,
                    dir => return Err(format_err!("Invalid direction: {dir}")),
                },
            )
            .map_err(JsError::from)?
        }
        _ => Literal::new_typed_literal(literal_value, datatype),
    })
}

pub fn to_named_or_blank_node(value: &JsValue) -> Result<NamedOrBlankNode, JsValue> {
    Ok(
        match reflect_get(value, &TERM_TYPE)?
            .as_string()
            .ok_or_else(|| format_err!("The object termType field must be a string"))?
            .as_str()
        {
            "NamedNode" => to_named_node(value)?.into(),
            "BlankNode" => to_blank_node(value)?.into(),
            term_type => {
                return Err(format_err!(
                    "The termType {term_type} must be NamedNode or BlankNode"
                ));
            }
        },
    )
}

pub fn to_term(value: &JsValue) -> Result<Term, JsValue> {
    Ok(
        match reflect_get(value, &TERM_TYPE)?
            .as_string()
            .ok_or_else(|| format_err!("The object termType field must be a string"))?
            .as_str()
        {
            "NamedNode" => to_named_node(value)?.into(),
            "BlankNode" => to_blank_node(value)?.into(),
            "Literal" => to_literal(value)?.into(),
            #[cfg(feature = "rdf-12")]
            "Quad" => Triple::from(to_quad(value)?).into(),
            term_type => {
                return Err(format_err!(
                    "The termType {term_type} is not a valid term type"
                ));
            }
        },
    )
}

pub fn to_graph_name(value: &JsValue) -> Result<GraphName, JsValue> {
    let Some(value) = to_option_ref(value) else {
        return Ok(GraphName::DefaultGraph);
    };
    Ok(
        match reflect_get(value, &TERM_TYPE)?
            .as_string()
            .ok_or_else(|| format_err!("The object termType field must be a string"))?
            .as_str()
        {
            "NamedNode" => to_named_node(value)?.into(),
            "BlankNode" => to_blank_node(value)?.into(),
            "DefaultGraph" => GraphName::DefaultGraph,
            term_type => {
                return Err(format_err!(
                    "The termType {term_type} is not a valid graph name type"
                ));
            }
        },
    )
}

pub fn to_quad(value: &JsValue) -> Result<Quad, JsValue> {
    Ok(Quad::new(
        to_named_or_blank_node(&reflect_get(value, &SUBJECT)?)?,
        to_named_node(&reflect_get(value, &PREDICATE)?)?,
        to_term(&reflect_get(value, &OBJECT)?)?,
        to_graph_name(&reflect_get(value, &GRAPH)?)?,
    ))
}
