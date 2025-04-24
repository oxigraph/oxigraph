use crate::context::{
    expand_iri, has_keyword_form, process_context, JsonLdContext, JsonLdProcessingMode, JsonNode,
};
use crate::error::JsonLdErrorCode;
use crate::JsonLdSyntaxError;
use json_event_parser::JsonEvent;
use oxiri::Iri;
use std::borrow::Cow;
use std::collections::HashMap;

pub enum JsonLdEvent {
    StartObject {
        types: Vec<String>,
    },
    EndObject,
    StartProperty(String),
    EndProperty,
    Id(String),
    Value {
        value: JsonLdValue,
        r#type: Option<String>,
        language: Option<String>,
    },
    StartGraph,
    EndGraph,
}

pub enum JsonLdValue {
    String(String),
    Number(String),
    Boolean(bool),
}

enum JsonLdExpansionState {
    Element {
        active_property: Option<String>,
        is_array: bool,
    },
    ObjectStart {
        types: Vec<String>,
        id: Option<String>,
        seen_type: bool,
    },
    ObjectType {
        types: Vec<String>,
        id: Option<String>,
        is_array: bool,
    },
    ObjectId {
        types: Vec<String>,
        id: Option<String>,
        from_start: bool,
    },
    Object {
        in_property: bool,
    },
    Value {
        r#type: Option<String>,
        value: Option<JsonLdValue>,
        language: Option<String>,
    },
    ValueValue {
        r#type: Option<String>,
        language: Option<String>,
    },
    ValueLanguage {
        r#type: Option<String>,
        value: Option<JsonLdValue>,
    },
    ValueType {
        value: Option<JsonLdValue>,
        language: Option<String>,
    },
    Graph,
    MaybeRootGraph {
        buffer: Vec<JsonEvent<'static>>,
        nesting: usize,
    },
    ToNode {
        stack: Vec<BuildingObjectOrArrayNode>,
        end_state: JsonLdExpansionStateAfterToNode,
    },
    Skip {
        is_array: bool,
    },
}

enum BuildingObjectOrArrayNode {
    Object(HashMap<String, JsonNode>),
    ObjectWithPendingKey(HashMap<String, JsonNode>, String),
    Array(Vec<JsonNode>),
}

#[derive(Clone, Copy)]
enum JsonLdExpansionStateAfterToNode {
    Context,
}

/// Applies the [Expansion Algorithm](https://www.w3.org/TR/json-ld-api/#expansion-algorithms)
pub struct JsonLdExpansionConverter {
    state: Vec<JsonLdExpansionState>,
    context: Vec<(JsonLdContext, usize)>,
    is_end: bool,
    lenient: bool,
    processing_mode: JsonLdProcessingMode,
}

#[allow(clippy::expect_used, clippy::unwrap_in_result)]
impl JsonLdExpansionConverter {
    pub fn new(
        base_url: Option<Iri<String>>,
        lenient: bool,
        processing_mode: JsonLdProcessingMode,
    ) -> Self {
        Self {
            state: vec![JsonLdExpansionState::Element {
                active_property: None,
                is_array: false,
            }],
            context: vec![(JsonLdContext::new_empty(base_url), 0)],
            is_end: false,
            lenient,
            processing_mode,
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn convert_event(
        &mut self,
        event: JsonEvent<'_>,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        if self.state.len() > 4096 {
            errors.push(JsonLdSyntaxError::msg("Too large state stack"));
            return;
        }
        if event == JsonEvent::Eof {
            self.is_end = true;
            return;
        }

        // Large hack to fetch the last state but keep it if we are in an array
        let state = self.state.pop().expect("Empty stack");
        match state {
            JsonLdExpansionState::Element {
                active_property,
                is_array,
            } => {
                match event {
                    JsonEvent::Null => {
                        // 1)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                            });
                        }
                    }
                    JsonEvent::String(value) => {
                        // 4)
                        if let Some(active_property) = &active_property {
                            self.expand_value(
                                active_property,
                                JsonLdValue::String(value.into()),
                                results,
                                errors,
                            );
                        }
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                            });
                        }
                    }
                    JsonEvent::Number(value) => {
                        // 4)
                        if let Some(active_property) = &active_property {
                            self.expand_value(
                                active_property,
                                JsonLdValue::Number(value.into()),
                                results,
                                errors,
                            );
                        }
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                            });
                        }
                    }
                    JsonEvent::Boolean(value) => {
                        // 4)
                        if let Some(active_property) = &active_property {
                            self.expand_value(
                                active_property,
                                JsonLdValue::Boolean(value),
                                results,
                                errors,
                            );
                        }
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                            });
                        }
                    }
                    JsonEvent::StartArray => {
                        // 5)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property: active_property.clone(),
                                is_array,
                            });
                        }
                        self.state.push(JsonLdExpansionState::Element {
                            active_property,
                            is_array: true,
                        });
                    }
                    JsonEvent::EndArray => (),
                    JsonEvent::StartObject => {
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                            });
                        }
                        self.push_same_context();
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types: Vec::new(),
                            id: None,
                            seen_type: false,
                        });
                    }
                    JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::ObjectStart {
                types,
                id,
                seen_type,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) = self.expand_iri(key.as_ref().into(), false, true, errors) {
                        match iri.as_ref() {
                            "@context" => {
                                if seen_type {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@context must be the first key of an object",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ))
                                }
                                self.state.push(JsonLdExpansionState::ToNode {
                                    stack: Vec::new(),
                                    end_state: JsonLdExpansionStateAfterToNode::Context,
                                })
                            }
                            "@type" => {
                                if seen_type && !self.lenient {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type must be the first key of an object or right after @context",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ))
                                }
                                self.state.push(JsonLdExpansionState::ObjectType {
                                    id,
                                    types,
                                    is_array: false,
                                });
                            }
                            "@value" => {
                                if types.len() > 1 {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Only a single @type is allowed when @value is present",
                                        JsonLdErrorCode::InvalidTypedValue,
                                    ));
                                }
                                self.state.push(JsonLdExpansionState::ValueValue {
                                    r#type: types.into_iter().next(),
                                    language: None,
                                });
                            }
                            "@language" => {
                                if types.len() > 1 {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Only a single @language is allowed",
                                        JsonLdErrorCode::CollidingKeywords,
                                    ));
                                }
                                self.state.push(JsonLdExpansionState::ValueLanguage {
                                    r#type: None,
                                    value: None,
                                });
                            }
                            "@id" => {
                                if id.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Only a single @id is allowed",
                                        JsonLdErrorCode::CollidingKeywords,
                                    ));
                                }
                                self.state.push(JsonLdExpansionState::ObjectId {
                                    types,
                                    id,
                                    from_start: true,
                                });
                            }
                            "@graph" => {
                                if id.is_none() && types.is_empty() && self.state.is_empty() {
                                    // Likely graph only for @context, we ignore it
                                    self.state.push(JsonLdExpansionState::MaybeRootGraph {
                                        buffer: Vec::new(),
                                        nesting: 1,
                                    });
                                } else {
                                    results.push(JsonLdEvent::StartObject { types });
                                    if let Some(id) = id {
                                        results.push(JsonLdEvent::Id(id));
                                    }
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: false });
                                    self.state.push(JsonLdExpansionState::Graph);
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property: None,
                                    });
                                    results.push(JsonLdEvent::StartGraph);
                                }
                            }
                            _ if has_keyword_form(&iri) => {
                                errors.push(JsonLdSyntaxError::msg(format!(
                                    "Unsupported JSON-LD keyword: {iri}"
                                )));
                                self.state.push(JsonLdExpansionState::ObjectStart {
                                    types,
                                    id,
                                    seen_type: true,
                                });
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            }
                            _ => {
                                results.push(JsonLdEvent::StartObject { types });
                                if let Some(id) = id {
                                    results.push(JsonLdEvent::Id(id));
                                }
                                results.push(JsonLdEvent::StartProperty(iri.clone().into()));
                                self.state
                                    .push(JsonLdExpansionState::Object { in_property: true });
                                self.state.push(JsonLdExpansionState::Element {
                                    active_property: Some(key.into()),
                                    is_array: false,
                                });
                            }
                        }
                    } else {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        });
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: false });
                    }
                }
                JsonEvent::EndObject => {
                    results.push(JsonLdEvent::StartObject { types });
                    if let Some(id) = id {
                        results.push(JsonLdEvent::Id(id));
                    }
                    results.push(JsonLdEvent::EndObject);
                    self.pop_context();
                }
                _ => unreachable!("Inside of an object"),
            },
            JsonLdExpansionState::ObjectType {
                mut types,
                id,
                is_array,
            } => {
                match event {
                    JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state.push(JsonLdExpansionState::ObjectType {
                                types,
                                id,
                                is_array,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_type: true,
                            });
                        }
                    }
                    JsonEvent::String(value) => {
                        // 13.4.4.4)
                        if let Some(iri) = self.expand_iri(value, true, true, errors) {
                            if has_keyword_form(&iri) {
                                errors.push(JsonLdSyntaxError::msg(format!(
                                    "{iri} is not a valid value for @type"
                                )));
                            } else {
                                types.push(iri.into());
                            }
                        }
                        if is_array {
                            self.state.push(JsonLdExpansionState::ObjectType {
                                types,
                                id,
                                is_array,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_type: true,
                            });
                        }
                    }
                    JsonEvent::StartArray => {
                        self.state.push(JsonLdExpansionState::ObjectType {
                            types,
                            id,
                            is_array: true,
                        });
                        if is_array {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@type cannot contain a nested array",
                                JsonLdErrorCode::InvalidTypeValue,
                            ));
                            self.state
                                .push(JsonLdExpansionState::Skip { is_array: true });
                        }
                    }
                    JsonEvent::EndArray => {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        });
                    }
                    JsonEvent::StartObject => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state.push(JsonLdExpansionState::ObjectType {
                                types,
                                id,
                                is_array: true,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_type: true,
                            });
                        }
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: false });
                    }
                    JsonEvent::ObjectKey(_) | JsonEvent::EndObject | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::ObjectId {
                types,
                mut id,
                from_start,
            } => match event {
                JsonEvent::String(new_id) => {
                    if let Some(new_id) = self.expand_iri(new_id, true, false, errors) {
                        if has_keyword_form(&new_id) {
                            errors.push(JsonLdSyntaxError::msg(
                                "@id value must be an IRI or a blank node",
                            ));
                        } else {
                            id = Some(new_id.into());
                        }
                    }
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        }
                    } else {
                        if let Some(id) = id {
                            results.push(JsonLdEvent::Id(id));
                        }
                        JsonLdExpansionState::Object { in_property: false }
                    })
                }
                JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidIdValue,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    })
                }
                JsonEvent::StartArray => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidIdValue,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: true });
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidIdValue,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_type: true,
                        }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::EndArray
                | JsonEvent::ObjectKey(_)
                | JsonEvent::EndObject
                | JsonEvent::Eof => {
                    unreachable!()
                }
            },
            JsonLdExpansionState::Object { in_property } => {
                if in_property {
                    results.push(JsonLdEvent::EndProperty);
                }
                match event {
                    JsonEvent::EndObject => {
                        results.push(JsonLdEvent::EndObject);
                        self.pop_context();
                    }
                    JsonEvent::ObjectKey(key) => {
                        if let Some(iri) = self.expand_iri(key.as_ref().into(), false, true, errors)
                        {
                            match iri.as_ref() {
                                "@id" => {
                                    self.state.push(JsonLdExpansionState::ObjectId {
                                        types: Vec::new(),
                                        id: None,
                                        from_start: false,
                                    });
                                }
                                "@graph" => {
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: false });
                                    self.state.push(JsonLdExpansionState::Graph);
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property: None,
                                    });
                                    results.push(JsonLdEvent::StartGraph);
                                }
                                "@context" => errors.push(JsonLdSyntaxError::msg_and_code(
                                    "@context must be the first key of an object",
                                    JsonLdErrorCode::InvalidStreamingKeyOrder,
                                )),
                                "@type" => {
                                    // TODO: be nice and allow this if lenient
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: false });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type must be the first key of an object or right after @context",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ))
                                }
                                _ if has_keyword_form(&iri) => {
                                    // TODO: we do not support any keyword
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: false });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                    errors.push(JsonLdSyntaxError::msg(format!(
                                        "Unsupported JSON-LD keyword: {iri}"
                                    )));
                                }
                                _ => {
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: true });
                                    self.state.push(JsonLdExpansionState::Element {
                                        active_property: Some(key.clone().into()),
                                        is_array: false,
                                    });
                                    results.push(JsonLdEvent::StartProperty(iri.into()));
                                }
                            }
                        } else {
                            self.state
                                .push(JsonLdExpansionState::Object { in_property: false });
                            self.state
                                .push(JsonLdExpansionState::Skip { is_array: false });
                        }
                    }
                    JsonEvent::Null
                    | JsonEvent::String(_)
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::StartArray
                    | JsonEvent::EndArray
                    | JsonEvent::StartObject
                    | JsonEvent::Eof => unreachable!(),
                }
            }
            JsonLdExpansionState::Value {
                r#type,
                value,
                language,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) = self.expand_iri(key, false, true, errors) {
                        match iri.as_ref() {
                            "@value" => {
                                if value.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@value cannot be set multiple times",
                                        JsonLdErrorCode::InvalidValueObject,
                                    ));
                                    self.state.push(JsonLdExpansionState::Value {
                                        r#type,
                                        value,
                                        language,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                } else {
                                    self.state.push(JsonLdExpansionState::ValueValue {
                                        r#type,
                                        language,
                                    });
                                }
                            }
                            "@language" => {
                                if language.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@language cannot be set multiple times",
                                        JsonLdErrorCode::CollidingKeywords,
                                    ));
                                    self.state.push(JsonLdExpansionState::Value {
                                        r#type,
                                        value,
                                        language,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                } else {
                                    self.state.push(JsonLdExpansionState::ValueLanguage {
                                        r#type,
                                        value,
                                    });
                                }
                            }
                            "@type" => {
                                if !self.lenient {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type must be the first key of an object or right after @context",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ))
                                }
                                if r#type.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type cannot be set multiple times",
                                        JsonLdErrorCode::CollidingKeywords,
                                    ));
                                    self.state.push(JsonLdExpansionState::Value {
                                        r#type,
                                        value,
                                        language,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                } else {
                                    self.state
                                        .push(JsonLdExpansionState::ValueType { value, language });
                                }
                            }
                            "@context" => errors.push(JsonLdSyntaxError::msg_and_code(
                                "@context must be the first key of an object",
                                JsonLdErrorCode::InvalidStreamingKeyOrder,
                            )),
                            _ if has_keyword_form(&iri) => {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!(
                                        "Unsupported JSON-Ld keyword inside of a @value: {iri}",
                                    ),
                                    JsonLdErrorCode::InvalidValueObject,
                                ));
                                self.state.push(JsonLdExpansionState::Value {
                                    r#type,
                                    value,
                                    language,
                                });
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            }
                            _ => {
                                errors.push(JsonLdSyntaxError::msg_and_code(format!("Objects with @value cannot contain properties, {iri} found"), JsonLdErrorCode::InvalidValueObject));
                                self.state.push(JsonLdExpansionState::Value {
                                    r#type,
                                    value,
                                    language,
                                });
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            }
                        }
                    } else {
                        self.state
                            .push(JsonLdExpansionState::Object { in_property: false });
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: false });
                    }
                }
                JsonEvent::EndObject => {
                    if let Some(value) = value {
                        if language.is_some() && r#type.is_some() {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@type and @language cannot be used together",
                                JsonLdErrorCode::InvalidValueObject,
                            ))
                        }
                        if language.is_some() && !matches!(value, JsonLdValue::String(_)) {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@language can be used only on a string @value",
                                JsonLdErrorCode::InvalidLanguageTaggedValue,
                            ))
                        }
                        results.push(JsonLdEvent::Value {
                            value,
                            r#type,
                            language,
                        })
                    }
                    self.pop_context();
                }
                JsonEvent::Null
                | JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::StartArray
                | JsonEvent::EndArray
                | JsonEvent::StartObject
                | JsonEvent::Eof => unreachable!(),
            },
            JsonLdExpansionState::ValueValue { r#type, language } => match event {
                JsonEvent::Null => self.state.push(JsonLdExpansionState::Value {
                    r#type,
                    value: None,
                    language,
                }),
                JsonEvent::Number(value) => self.state.push(JsonLdExpansionState::Value {
                    r#type,
                    value: Some(JsonLdValue::Number(value.into())),
                    language,
                }),
                JsonEvent::Boolean(value) => self.state.push(JsonLdExpansionState::Value {
                    r#type,
                    value: Some(JsonLdValue::Boolean(value)),
                    language,
                }),
                JsonEvent::String(value) => self.state.push(JsonLdExpansionState::Value {
                    r#type,
                    value: Some(JsonLdValue::String(value.into())),
                    language,
                }),
                JsonEvent::StartArray => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type cannot contain an array",
                        JsonLdErrorCode::InvalidValueObjectValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value: None,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: true });
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type cannot contain an object",
                        JsonLdErrorCode::InvalidValueObjectValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value: None,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::EndArray
                | JsonEvent::ObjectKey(_)
                | JsonEvent::EndObject
                | JsonEvent::Eof => {
                    unreachable!()
                }
            },
            JsonLdExpansionState::ValueLanguage { value, r#type } => match event {
                JsonEvent::String(language) => self.state.push(JsonLdExpansionState::Value {
                    r#type,
                    value,
                    language: Some(language.into()),
                }),
                JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value,
                        language: None,
                    })
                }
                JsonEvent::StartArray => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value,
                        language: None,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: true });
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value,
                        language: None,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::EndArray
                | JsonEvent::ObjectKey(_)
                | JsonEvent::EndObject
                | JsonEvent::Eof => {
                    unreachable!()
                }
            },
            JsonLdExpansionState::ValueType { value, language } => match event {
                JsonEvent::String(t) => self.state.push(JsonLdExpansionState::Value {
                    r#type: Some(t.into()),
                    value,
                    language,
                }),
                JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type value must be a string when @value is present",
                        JsonLdErrorCode::InvalidTypedValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    })
                }
                JsonEvent::StartArray => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: true });
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::EndArray
                | JsonEvent::ObjectKey(_)
                | JsonEvent::EndObject
                | JsonEvent::Eof => {
                    unreachable!()
                }
            },
            JsonLdExpansionState::Graph => {
                results.push(JsonLdEvent::EndGraph);
                self.convert_event(event, results, errors)
            }
            JsonLdExpansionState::MaybeRootGraph {
                mut buffer,
                mut nesting,
            } => {
                let event = to_owned_event(event);
                match event {
                    JsonEvent::String(_)
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null => {
                        buffer.push(event);
                        self.state
                            .push(JsonLdExpansionState::MaybeRootGraph { buffer, nesting });
                    }
                    JsonEvent::StartArray | JsonEvent::StartObject => {
                        buffer.push(event);
                        nesting += 1;
                        self.state
                            .push(JsonLdExpansionState::MaybeRootGraph { buffer, nesting });
                    }
                    JsonEvent::EndArray | JsonEvent::EndObject => {
                        nesting -= 1;
                        if nesting == 0 {
                            // We are out of the root object without seeing other keys, we emit the graph as a default graph
                            self.state.push(JsonLdExpansionState::Element {
                                is_array: false,
                                active_property: None,
                            });
                            for event in buffer {
                                self.convert_event(event, results, errors);
                            }
                        } else {
                            buffer.push(event);
                            self.state
                                .push(JsonLdExpansionState::MaybeRootGraph { buffer, nesting });
                        }
                    }
                    JsonEvent::ObjectKey(_) => {
                        if nesting == 1 {
                            // Other key in the object, we know we are seeing an object
                            results.push(JsonLdEvent::StartObject { types: Vec::new() });
                            self.state
                                .push(JsonLdExpansionState::Object { in_property: false });
                            self.state.push(JsonLdExpansionState::Graph);
                            self.state.push(JsonLdExpansionState::Element {
                                is_array: false,
                                active_property: None,
                            });
                            results.push(JsonLdEvent::StartGraph);
                            for event in buffer {
                                self.convert_event(event, results, errors);
                            }
                            self.convert_event(event, results, errors);
                        } else {
                            buffer.push(event);
                            self.state
                                .push(JsonLdExpansionState::MaybeRootGraph { buffer, nesting });
                        }
                    }
                    JsonEvent::Eof => unreachable!(),
                }
            }
            JsonLdExpansionState::Skip { is_array } => match event {
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    if is_array {
                        self.state.push(JsonLdExpansionState::Skip { is_array });
                    }
                }
                JsonEvent::EndArray | JsonEvent::EndObject => (),
                JsonEvent::StartArray => {
                    if is_array {
                        self.state.push(JsonLdExpansionState::Skip { is_array });
                    }
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: true });
                }
                JsonEvent::StartObject => {
                    if is_array {
                        self.state.push(JsonLdExpansionState::Skip { is_array });
                    }
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::ObjectKey(_) => {
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::Eof => unreachable!(),
            },
            JsonLdExpansionState::ToNode {
                mut stack,
                end_state,
            } => match event {
                JsonEvent::String(value) => self.after_to_node_event(
                    stack,
                    end_state,
                    JsonNode::String(value.into()),
                    errors,
                ),
                JsonEvent::Number(value) => self.after_to_node_event(
                    stack,
                    end_state,
                    JsonNode::Number(value.into()),
                    errors,
                ),
                JsonEvent::Boolean(value) => {
                    self.after_to_node_event(stack, end_state, JsonNode::Boolean(value), errors)
                }
                JsonEvent::Null => {
                    self.after_to_node_event(stack, end_state, JsonNode::Null, errors)
                }
                JsonEvent::EndArray | JsonEvent::EndObject => {
                    let value = match stack.pop() {
                        Some(BuildingObjectOrArrayNode::Object(object)) => JsonNode::Map(object),
                        Some(BuildingObjectOrArrayNode::Array(array)) => JsonNode::Array(array),
                        _ => unreachable!(),
                    };
                    self.after_to_node_event(stack, end_state, value, errors)
                }
                JsonEvent::StartArray => {
                    stack.push(BuildingObjectOrArrayNode::Array(Vec::new()));
                    self.state
                        .push(JsonLdExpansionState::ToNode { stack, end_state })
                }
                JsonEvent::StartObject => {
                    stack.push(BuildingObjectOrArrayNode::Object(HashMap::new()));
                    self.state
                        .push(JsonLdExpansionState::ToNode { stack, end_state })
                }
                JsonEvent::ObjectKey(key) => {
                    if let Some(BuildingObjectOrArrayNode::Object(object)) = stack.pop() {
                        stack.push(BuildingObjectOrArrayNode::ObjectWithPendingKey(
                            object,
                            key.into(),
                        ));
                    }
                    self.state
                        .push(JsonLdExpansionState::ToNode { stack, end_state })
                }
                JsonEvent::Eof => unreachable!(),
            },
        }
    }

    fn after_to_node_event(
        &mut self,
        mut stack: Vec<BuildingObjectOrArrayNode>,
        end_state: JsonLdExpansionStateAfterToNode,
        new_value: JsonNode,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        match stack.pop() {
            Some(BuildingObjectOrArrayNode::ObjectWithPendingKey(mut object, key)) => {
                object.insert(key, new_value);
                stack.push(BuildingObjectOrArrayNode::Object(object));
                self.state
                    .push(JsonLdExpansionState::ToNode { stack, end_state });
            }
            Some(BuildingObjectOrArrayNode::Object(object)) => {
                stack.push(BuildingObjectOrArrayNode::Object(object));
                self.state
                    .push(JsonLdExpansionState::ToNode { stack, end_state });
            }
            Some(BuildingObjectOrArrayNode::Array(mut array)) => {
                array.push(new_value);
                stack.push(BuildingObjectOrArrayNode::Array(array));
                self.state
                    .push(JsonLdExpansionState::ToNode { stack, end_state });
            }
            None => self.after_buffering(new_value, end_state, errors),
        }
    }

    fn after_buffering(
        &mut self,
        node: JsonNode,
        state: JsonLdExpansionStateAfterToNode,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        match state {
            JsonLdExpansionStateAfterToNode::Context => {
                let context = process_context(
                    self.context(),
                    node,
                    None,
                    &mut Vec::new(),
                    false,
                    true,
                    self.processing_mode, // TODO
                    self.lenient,
                    errors,
                );
                self.context
                    .last_mut()
                    .expect("Context stack must not be empty")
                    .1 -= 1;
                self.context.push((context, 1));
                self.state.push(JsonLdExpansionState::ObjectStart {
                    types: Vec::new(),
                    id: None,
                    seen_type: false,
                })
            }
        }
    }

    /// [IRI Expansion](https://www.w3.org/TR/json-ld-api/#iri-expansion)
    fn expand_iri<'a>(
        &mut self,
        value: Cow<'a, str>,
        document_relative: bool,
        vocab: bool,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<Cow<'a, str>> {
        expand_iri(
            &mut self
                .context
                .last_mut()
                .expect("The context stack must not be empty")
                .0,
            value,
            document_relative,
            vocab,
            None,
            &mut HashMap::new(),
            self.processing_mode,
            self.lenient,
            errors,
        )
    }

    /// [Value Expansion](https://www.w3.org/TR/json-ld-api/#value-expansion)
    fn expand_value(
        &mut self,
        active_property: &str,
        value: JsonLdValue,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        let active_context = self.context();
        let mut r#type = None;
        let mut language = None;
        if let Some(term_definition) = active_context.term_definitions.get(active_property) {
            if let Some(type_mapping) = &term_definition.type_mapping {
                match type_mapping.as_ref() {
                    // 1)
                    "@id" => {
                        if let JsonLdValue::String(value) = value {
                            if let Some(id) = self.expand_iri(value.into(), true, false, errors) {
                                results.push(JsonLdEvent::StartObject { types: Vec::new() });
                                results.push(JsonLdEvent::Id(id.into()));
                                results.push(JsonLdEvent::EndObject);
                            }
                            return;
                        }
                    }
                    // 2)
                    "@vocab" => {
                        if let JsonLdValue::String(value) = value {
                            if let Some(id) = self.expand_iri(value.into(), true, true, errors) {
                                results.push(JsonLdEvent::StartObject { types: Vec::new() });
                                results.push(JsonLdEvent::Id(id.into()));
                                results.push(JsonLdEvent::EndObject);
                            }
                            return;
                        }
                    }
                    // 4)
                    "@none" => (),
                    _ => {
                        r#type = Some(type_mapping.clone());
                    }
                }
            }
            // 5)
            if matches!(value, JsonLdValue::String(_)) {
                language.clone_from(&term_definition.language_mapping);
            }
        }
        // 5)
        if matches!(value, JsonLdValue::String(_)) && language.is_none() {
            language.clone_from(&active_context.default_language);
        }
        results.push(JsonLdEvent::Value {
            value,
            r#type,
            language,
        });
    }

    pub fn context(&self) -> &JsonLdContext {
        &self
            .context
            .last()
            .expect("The context stack must not be empty")
            .0
    }

    fn push_same_context(&mut self) {
        self.context
            .last_mut()
            .expect("The context stack must not be empty")
            .1 += 1;
    }

    fn pop_context(&mut self) {
        let mut last_context = self
            .context
            .pop()
            .expect("The context stack must not be empty");
        last_context.1 -= 1;
        if last_context.1 > 0 || self.context.is_empty() {
            // We always keep a context to allow reading the root context at the end of the document
            self.context.push(last_context);
        }
    }
}

fn to_owned_event(event: JsonEvent<'_>) -> JsonEvent<'static> {
    match event {
        JsonEvent::String(s) => JsonEvent::String(Cow::Owned(s.into())),
        JsonEvent::Number(n) => JsonEvent::Number(Cow::Owned(n.into())),
        JsonEvent::Boolean(b) => JsonEvent::Boolean(b),
        JsonEvent::Null => JsonEvent::Null,
        JsonEvent::StartArray => JsonEvent::StartArray,
        JsonEvent::EndArray => JsonEvent::EndArray,
        JsonEvent::StartObject => JsonEvent::StartObject,
        JsonEvent::EndObject => JsonEvent::EndObject,
        JsonEvent::ObjectKey(k) => JsonEvent::ObjectKey(Cow::Owned(k.into())),
        JsonEvent::Eof => JsonEvent::Eof,
    }
}
