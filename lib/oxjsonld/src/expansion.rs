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
}

pub enum JsonLdValue {
    String(String),
    Number(String),
    Boolean(bool),
}

enum JsonLdExpansionState {
    Element,
    ElementArray,
    ObjectStart {
        types: Vec<String>,
        id: Option<String>,
    },
    ObjectType {
        types: Vec<String>,
        id: Option<String>,
    },
    ObjectTypeArray {
        types: Vec<String>,
        id: Option<String>,
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
    ToNode {
        stack: Vec<BuildingObjectOrArrayNode>,
        end_state: JsonLdExpansionStateAfterToNode,
    },
    Skip,
    SkipArray,
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

impl JsonLdExpansionConverter {
    pub fn new(
        base_url: Option<Iri<String>>,
        lenient: bool,
        processing_mode: JsonLdProcessingMode,
    ) -> Self {
        Self {
            state: vec![JsonLdExpansionState::Element],
            context: vec![(JsonLdContext::new_empty(base_url), 0)],
            is_end: false,
            lenient,
            processing_mode,
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn convert_event<'a>(
        &mut self,
        event: JsonEvent<'a>,
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
            JsonLdExpansionState::Element | JsonLdExpansionState::ElementArray => {
                match event {
                    JsonEvent::Null => {
                        // 1)
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                    }
                    JsonEvent::String(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                        self.expand_value(JsonLdValue::String(value.into()), results);
                    }
                    JsonEvent::Number(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                        self.expand_value(JsonLdValue::Number(value.into()), results);
                    }
                    JsonEvent::Boolean(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                        self.expand_value(JsonLdValue::Boolean(value), results);
                    }
                    JsonEvent::StartArray => {
                        // 5)
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                        self.state.push(JsonLdExpansionState::ElementArray);
                    }
                    JsonEvent::EndArray => (),
                    JsonEvent::StartObject => {
                        if matches!(state, JsonLdExpansionState::ElementArray) {
                            self.state.push(JsonLdExpansionState::ElementArray);
                        }
                        self.push_same_context();
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types: Vec::new(),
                            id: None,
                        });
                    }
                    JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::ObjectStart { types, id } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) = self.expand_iri(key, false, true, errors) {
                        match iri.as_ref() {
                            "@context" => self.state.push(JsonLdExpansionState::ToNode {
                                stack: Vec::new(),
                                end_state: JsonLdExpansionStateAfterToNode::Context,
                            }),
                            "@type" => {
                                self.state
                                    .push(JsonLdExpansionState::ObjectType { id, types });
                            }
                            "@value" => {
                                if types.len() > 1 {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Only a single @type is allowed when @value is present",
                                        JsonLdErrorCode::InvalidTypedValue,
                                    ));
                                }
                                self.state.push(JsonLdExpansionState::ValueValue {
                                    r#type: None,
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
                            _ if has_keyword_form(&iri) => {
                                errors.push(JsonLdSyntaxError::msg(format!(
                                    "Unsupported JSON-LD keyword: {iri}"
                                )));
                                self.state
                                    .push(JsonLdExpansionState::ObjectStart { types, id });
                                self.state.push(JsonLdExpansionState::Skip);
                            }
                            _ => {
                                results.push(JsonLdEvent::StartObject { types });
                                results.push(JsonLdEvent::StartProperty(iri.into()));
                                self.state
                                    .push(JsonLdExpansionState::Object { in_property: true });
                                self.state.push(JsonLdExpansionState::Element);
                            }
                        }
                    } else {
                        self.state
                            .push(JsonLdExpansionState::ObjectStart { types, id });
                        self.state.push(JsonLdExpansionState::Skip);
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
            JsonLdExpansionState::ObjectType { .. }
            | JsonLdExpansionState::ObjectTypeArray { .. } => {
                let (mut types, id, is_array) = match state {
                    JsonLdExpansionState::ObjectType { types, id } => (types, id, false),
                    JsonLdExpansionState::ObjectTypeArray { types, id } => (types, id, true),
                    _ => unreachable!(),
                };
                match event {
                    JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types, id });
                        } else {
                            self.state
                                .push(JsonLdExpansionState::ObjectStart { types, id });
                        }
                    }
                    JsonEvent::String(value) => {
                        // 13.4.4.4)
                        if let Some(iri) = self.expand_iri(value, false, true, errors) {
                            if has_keyword_form(&iri) {
                                errors.push(JsonLdSyntaxError::msg(format!(
                                    "{iri} is not a valid value for @type"
                                )));
                            } else {
                                types.push(iri.into());
                            }
                        }
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types, id });
                        } else {
                            self.state
                                .push(JsonLdExpansionState::ObjectStart { types, id });
                        }
                    }
                    JsonEvent::StartArray => {
                        self.state
                            .push(JsonLdExpansionState::ObjectTypeArray { types, id });
                        if is_array {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@type cannot contain a nested array",
                                JsonLdErrorCode::InvalidTypeValue,
                            ));
                            self.state.push(JsonLdExpansionState::SkipArray);
                        }
                    }
                    JsonEvent::EndArray => {
                        self.state
                            .push(JsonLdExpansionState::ObjectStart { types, id });
                    }
                    JsonEvent::StartObject => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types, id });
                        } else {
                            self.state
                                .push(JsonLdExpansionState::ObjectStart { types, id });
                        }
                        self.state.push(JsonLdExpansionState::Skip);
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
                        JsonLdExpansionState::ObjectStart { types, id }
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
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart { types, id }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    })
                }
                JsonEvent::StartArray => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart { types, id }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    });
                    self.state.push(JsonLdExpansionState::SkipArray);
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart { types, id }
                    } else {
                        JsonLdExpansionState::Object { in_property: false }
                    });
                    self.state.push(JsonLdExpansionState::Skip);
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
                        if let Some(iri) = self.expand_iri(key, false, true, errors) {
                            match iri.as_ref() {
                                "@id" => {
                                    self.state.push(JsonLdExpansionState::ObjectId {
                                        types: Vec::new(),
                                        id: None,
                                        from_start: false,
                                    });
                                }
                                _ if has_keyword_form(&iri) => {
                                    // TODO: we do not support any keyword
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: false });
                                    self.state.push(JsonLdExpansionState::Skip);
                                    errors.push(JsonLdSyntaxError::msg(format!(
                                        "Unsupported keyword: {iri}"
                                    )));
                                }
                                _ => {
                                    self.state
                                        .push(JsonLdExpansionState::Object { in_property: true });
                                    self.state.push(JsonLdExpansionState::Element);
                                    results.push(JsonLdEvent::StartProperty(iri.into()));
                                }
                            }
                        } else {
                            self.state
                                .push(JsonLdExpansionState::Object { in_property: false });
                            self.state.push(JsonLdExpansionState::Skip);
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
                                    self.state.push(JsonLdExpansionState::Skip);
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
                                    self.state.push(JsonLdExpansionState::Skip);
                                } else {
                                    self.state.push(JsonLdExpansionState::ValueLanguage {
                                        r#type,
                                        value,
                                    });
                                }
                            }
                            "@type" => {
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
                                    self.state.push(JsonLdExpansionState::Skip);
                                } else {
                                    self.state
                                        .push(JsonLdExpansionState::ValueType { value, language });
                                }
                            }
                            _ if has_keyword_form(&iri) => {
                                errors.push(JsonLdSyntaxError::msg(format!(
                                    "Unsupported JSON-Ld keyword inside of a @value: {iri}"
                                )));
                                self.state.push(JsonLdExpansionState::Value {
                                    r#type,
                                    value,
                                    language,
                                });
                                self.state.push(JsonLdExpansionState::Skip);
                            }
                            _ => {
                                errors.push(JsonLdSyntaxError::msg_and_code(format!("Objects with @value cannot contain properties, {iri} found"), JsonLdErrorCode::InvalidValueObject));
                                self.state.push(JsonLdExpansionState::Value {
                                    r#type,
                                    value,
                                    language,
                                });
                                self.state.push(JsonLdExpansionState::Skip);
                            }
                        }
                    } else {
                        self.state
                            .push(JsonLdExpansionState::Object { in_property: false });
                        self.state.push(JsonLdExpansionState::Skip);
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
                    self.state.push(JsonLdExpansionState::SkipArray);
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
                    self.state.push(JsonLdExpansionState::Skip);
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
                    self.state.push(JsonLdExpansionState::SkipArray);
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
                    self.state.push(JsonLdExpansionState::Skip);
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
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    });
                    self.state.push(JsonLdExpansionState::SkipArray);
                }
                JsonEvent::StartObject => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    });
                    self.state.push(JsonLdExpansionState::Skip);
                }
                JsonEvent::EndArray
                | JsonEvent::ObjectKey(_)
                | JsonEvent::EndObject
                | JsonEvent::Eof => {
                    unreachable!()
                }
            },
            JsonLdExpansionState::Skip | JsonLdExpansionState::SkipArray => match event {
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                }
                JsonEvent::EndArray | JsonEvent::EndObject => (),
                JsonEvent::StartArray => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                    self.state.push(JsonLdExpansionState::SkipArray);
                }
                JsonEvent::StartObject => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                    self.state.push(JsonLdExpansionState::Skip);
                }
                JsonEvent::ObjectKey(_) => {
                    self.state.push(JsonLdExpansionState::Skip);
                    self.state.push(JsonLdExpansionState::Skip);
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
                    &JsonLdContext::default(),
                    node,
                    None,
                    &mut Vec::new(),
                    false,
                    true,
                    JsonLdProcessingMode::JsonLd1_0, // TODO
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
    fn expand_value(&mut self, value: JsonLdValue, results: &mut Vec<JsonLdEvent>) {
        results.push(JsonLdEvent::Value {
            value,
            r#type: None,
            language: None,
        });
    }

    fn context(&self) -> &JsonLdContext {
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
        if last_context.1 > 0 {
            self.context.push(last_context);
        }
    }
}
