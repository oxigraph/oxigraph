use crate::context::{
    JsonLdContext, JsonLdContextProcessor, JsonLdLoadDocumentOptions, JsonLdRemoteDocument,
    has_keyword_form, json_node_from_events,
};
use crate::error::JsonLdErrorCode;
use crate::profile::JsonLdProcessingMode;
use crate::{JsonLdSyntaxError, MAX_CONTEXT_RECURSION};
use json_event_parser::JsonEvent;
use oxiri::Iri;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::{Arc, Mutex};

pub enum JsonLdEvent {
    StartObject {
        types: Vec<String>,
    },
    EndObject,
    StartProperty {
        name: String,
        reverse: bool,
    },
    EndProperty,
    Id(String),
    Value {
        value: JsonLdValue,
        r#type: Option<String>,
        language: Option<String>,
    },
    StartGraph,
    EndGraph,
    StartList,
    EndList,
    StartSet,
    EndSet,
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
        container: &'static [&'static str],
        reverse: bool,
    },
    ObjectOrContainerStart {
        buffer: Vec<(String, Vec<JsonEvent<'static>>)>,
        depth: usize,
        current_key: Option<String>,
        active_property: Option<String>,
        container: &'static [&'static str],
        reverse: bool,
    },
    ObjectOrContainerStartStreaming {
        active_property: Option<String>,
        container: &'static [&'static str],
        reverse: bool,
    },
    Context {
        buffer: Vec<JsonEvent<'static>>,
        depth: usize,
        active_property: Option<String>,
        container: &'static [&'static str],
        reverse: bool,
    },
    ObjectStart {
        types: Vec<String>,
        id: Option<String>,
        seen_id: bool,
        active_property: Option<String>,
        reverse: bool,
    },
    ObjectType {
        types: Vec<String>,
        id: Option<String>,
        is_array: bool,
        active_property: Option<String>,
        reverse: bool,
    },
    ObjectId {
        types: Vec<String>,
        id: Option<String>,
        from_start: bool,
        reverse: bool,
    },
    Object {
        in_property: bool,
        has_emitted_id: bool,
    },
    ReverseStart,
    Reverse {
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
    Index,
    Graph,
    RootGraph,
    ListOrSetContainer {
        needs_end_object: bool,
        end_event: Option<JsonLdEvent>,
    },
    IndexContainer {
        active_property: Option<String>,
    },
    LanguageContainer,
    LanguageContainerValue {
        language: String,
        is_array: bool,
    },
    Skip {
        is_array: bool,
    },
}

/// Applies the [Expansion Algorithm](https://www.w3.org/TR/json-ld-api/#expansion-algorithms)
pub struct JsonLdExpansionConverter {
    state: Vec<JsonLdExpansionState>,
    context: Vec<(JsonLdContext, usize)>,
    is_end: bool,
    streaming: bool,
    lenient: bool,
    base_url: Option<Iri<String>>,
    context_processor: JsonLdContextProcessor,
}

#[expect(clippy::expect_used, clippy::unwrap_in_result)]
impl JsonLdExpansionConverter {
    pub fn new(
        base_url: Option<Iri<String>>,
        streaming: bool,
        lenient: bool,
        processing_mode: JsonLdProcessingMode,
    ) -> Self {
        Self {
            state: vec![JsonLdExpansionState::Element {
                active_property: None,
                is_array: false,
                container: &[],
                reverse: false,
            }],
            context: vec![(JsonLdContext::new_empty(base_url.clone()), 0)],
            is_end: false,
            streaming,
            lenient,
            base_url,
            context_processor: JsonLdContextProcessor {
                processing_mode,
                lenient,
                max_context_recursion: MAX_CONTEXT_RECURSION,
                remote_context_cache: Arc::new(Mutex::new(HashMap::new())), /* TODO: share in the parser */
                load_document_callback: None,
            },
        }
    }

    pub fn is_end(&self) -> bool {
        self.is_end
    }

    pub fn with_load_document_callback(
        mut self,
        callback: impl Fn(
            &str,
            &JsonLdLoadDocumentOptions,
        ) -> Result<JsonLdRemoteDocument, Box<dyn Error + Send + Sync>>
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
        + 'static,
    ) -> Self {
        self.context_processor.load_document_callback = Some(Arc::new(callback));
        self
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
                container,
                reverse,
            } => {
                match event {
                    JsonEvent::Null => {
                        // 1)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                is_array,
                                container,
                                reverse,
                            });
                        }
                    }
                    JsonEvent::String(value) => self.on_literal_value(
                        JsonLdValue::String(value.into()),
                        active_property,
                        is_array,
                        container,
                        reverse,
                        results,
                        errors,
                    ),
                    JsonEvent::Number(value) => self.on_literal_value(
                        JsonLdValue::Number(value.into()),
                        active_property,
                        is_array,
                        container,
                        reverse,
                        results,
                        errors,
                    ),
                    JsonEvent::Boolean(value) => self.on_literal_value(
                        JsonLdValue::Boolean(value),
                        active_property,
                        is_array,
                        container,
                        reverse,
                        results,
                        errors,
                    ),
                    JsonEvent::StartArray => {
                        // 5)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property: active_property.clone(),
                                is_array,
                                container,
                                reverse,
                            });
                        }
                        if container.contains(&"@list") {
                            if reverse {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    "Lists are not allowed inside of reverse properties",
                                    JsonLdErrorCode::InvalidReversePropertyValue,
                                ))
                            }
                            results.push(JsonLdEvent::StartList);
                            self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                needs_end_object: false,
                                end_event: Some(JsonLdEvent::EndList),
                            })
                        }
                        if container.contains(&"@set") && !is_array {
                            results.push(JsonLdEvent::StartSet);
                            self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                needs_end_object: false,
                                end_event: Some(JsonLdEvent::EndSet),
                            })
                        }
                        self.state.push(JsonLdExpansionState::Element {
                            active_property,
                            is_array: true,
                            container,
                            reverse,
                        });
                    }
                    JsonEvent::EndArray => (),
                    JsonEvent::StartObject => {
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property: active_property.clone(),
                                is_array,
                                container,
                                reverse,
                            });
                        } else if container.contains(&"@index") {
                            self.state
                                .push(JsonLdExpansionState::IndexContainer { active_property });
                            return;
                        } else if container.contains(&"@language") {
                            self.state.push(JsonLdExpansionState::LanguageContainer);
                            return;
                        }
                        self.push_same_context();
                        self.state.push(if self.streaming {
                            JsonLdExpansionState::ObjectOrContainerStartStreaming {
                                active_property,
                                container: if is_array { &[] } else { container },
                                reverse,
                            }
                        } else {
                            JsonLdExpansionState::ObjectOrContainerStart {
                                buffer: Vec::new(),
                                depth: 1,
                                current_key: None,
                                active_property,
                                container: if is_array { &[] } else { container },
                                reverse,
                            }
                        });
                    }
                    JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::ObjectOrContainerStart {
                mut buffer,
                mut depth,
                mut current_key,
                active_property,
                container,
                reverse,
            } => {
                // We have to buffer everything to make sure we get the @context key even if it's at the end
                match event {
                    JsonEvent::String(_)
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null => {
                        buffer.last_mut().unwrap().1.push(to_owned_event(event));
                    }
                    JsonEvent::ObjectKey(key) => {
                        if depth == 1 {
                            buffer.push((key.clone().into(), Vec::new()));
                            current_key = Some(key.into());
                        } else {
                            buffer
                                .last_mut()
                                .unwrap()
                                .1
                                .push(to_owned_event(JsonEvent::ObjectKey(key)));
                        }
                    }
                    JsonEvent::EndArray | JsonEvent::EndObject => {
                        if depth > 1 {
                            buffer.last_mut().unwrap().1.push(to_owned_event(event));
                        }
                        depth -= 1;
                    }
                    JsonEvent::StartArray | JsonEvent::StartObject => {
                        buffer.last_mut().unwrap().1.push(to_owned_event(event));
                        depth += 1;
                    }
                    JsonEvent::Eof => unreachable!(),
                }
                if depth == 0 {
                    // We look for @context @type, @id and @graph
                    let mut context_value = None;
                    let mut type_data = None;
                    let mut id_data = None;
                    let mut graph_data = Vec::new();
                    let mut other_data = Vec::with_capacity(buffer.len());
                    for (key, value) in buffer {
                        let expanded = self.expand_iri(key.as_str().into(), false, true, errors);
                        match expanded.as_deref() {
                            Some("@context") => {
                                if context_value.is_some() {
                                    errors.push(JsonLdSyntaxError::msg("@context is defined twice"))
                                }
                                context_value = Some(value);
                            }
                            Some("@type") => {
                                if type_data.is_some() {
                                    errors.push(JsonLdSyntaxError::msg("@type is defined twice"))
                                }
                                type_data = Some((key, value));
                            }
                            Some("@id") => {
                                if id_data.is_some() {
                                    errors.push(JsonLdSyntaxError::msg("@id is defined twice"))
                                }
                                id_data = Some((key, value));
                            }
                            Some("@graph") => {
                                graph_data.push((key, value));
                            }
                            _ => other_data.push((key, value)),
                        }
                    }
                    self.state
                        .push(JsonLdExpansionState::ObjectOrContainerStartStreaming {
                            active_property,
                            container,
                            reverse,
                        });

                    // We first process @context, @type and @id then other then graph
                    if let Some(context) = context_value {
                        self.push_new_context(context, errors);
                    }
                    for (key, value) in type_data
                        .into_iter()
                        .chain(id_data)
                        .chain(other_data)
                        .chain(graph_data)
                    {
                        self.convert_event(JsonEvent::ObjectKey(key.into()), results, errors);
                        for event in value {
                            self.convert_event(event, results, errors);
                        }
                    }
                    self.convert_event(JsonEvent::EndObject, results, errors);
                } else {
                    self.state
                        .push(JsonLdExpansionState::ObjectOrContainerStart {
                            buffer,
                            depth,
                            current_key,
                            active_property,
                            container,
                            reverse,
                        });
                }
            }
            JsonLdExpansionState::ObjectOrContainerStartStreaming {
                active_property,
                container,
                reverse,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) = self.expand_iri(key.as_ref().into(), false, true, errors) {
                        match iri.as_ref() {
                            "@context" => self.state.push(JsonLdExpansionState::Context {
                                buffer: Vec::new(),
                                depth: 0,
                                active_property,
                                container,
                                reverse,
                            }),
                            "@index" => {
                                self.state.push(
                                    JsonLdExpansionState::ObjectOrContainerStartStreaming {
                                        active_property,
                                        container,
                                        reverse,
                                    },
                                );
                                self.state.push(JsonLdExpansionState::Index);
                            }
                            "@list" => {
                                if active_property.is_some() {
                                    if reverse {
                                        errors.push(JsonLdSyntaxError::msg_and_code(
                                            "Lists are not allowed inside of reverse properties",
                                            JsonLdErrorCode::InvalidReversePropertyValue,
                                        ))
                                    }
                                    self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                        needs_end_object: true,
                                        end_event: Some(JsonLdEvent::EndList),
                                    });
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property,
                                        container: &[],
                                        reverse: false,
                                    });
                                    results.push(JsonLdEvent::StartList);
                                } else {
                                    // We don't have an active property, we skip the list
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                            }
                            "@set" => {
                                let has_property = active_property.is_some();
                                self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                    needs_end_object: true,
                                    end_event: has_property.then_some(JsonLdEvent::EndSet),
                                });
                                self.state.push(JsonLdExpansionState::Element {
                                    is_array: false,
                                    active_property,
                                    container: &[],
                                    reverse: false,
                                });
                                if has_property {
                                    results.push(JsonLdEvent::StartSet);
                                }
                            }
                            _ => {
                                if container.contains(&"@list") {
                                    results.push(JsonLdEvent::StartList);
                                    self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                        needs_end_object: false,
                                        end_event: Some(JsonLdEvent::EndList),
                                    });
                                } else if container.contains(&"@set") {
                                    results.push(JsonLdEvent::StartSet);
                                    self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                        needs_end_object: false,
                                        end_event: Some(JsonLdEvent::EndSet),
                                    });
                                }
                                self.state.push(JsonLdExpansionState::ObjectStart {
                                    types: Vec::new(),
                                    id: None,
                                    seen_id: false,
                                    active_property,
                                    reverse,
                                });
                                self.convert_event(JsonEvent::ObjectKey(key), results, errors)
                            }
                        }
                    } else {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types: Vec::new(),
                            id: None,
                            seen_id: false,
                            active_property,
                            reverse,
                        });
                        self.convert_event(JsonEvent::ObjectKey(key), results, errors)
                    }
                }
                JsonEvent::EndObject => {
                    self.state.push(JsonLdExpansionState::ObjectStart {
                        types: Vec::new(),
                        id: None,
                        seen_id: false,
                        active_property,
                        reverse,
                    });
                    self.convert_event(JsonEvent::EndObject, results, errors)
                }
                _ => unreachable!("Inside of an object"),
            },
            JsonLdExpansionState::Context {
                mut buffer,
                mut depth,
                active_property,
                container,
                reverse,
            } => {
                match event {
                    JsonEvent::String(_)
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null
                    | JsonEvent::ObjectKey(_) => buffer.push(to_owned_event(event)),
                    JsonEvent::EndArray | JsonEvent::EndObject => {
                        buffer.push(to_owned_event(event));
                        depth -= 1;
                    }
                    JsonEvent::StartArray | JsonEvent::StartObject => {
                        buffer.push(to_owned_event(event));
                        depth += 1;
                    }
                    JsonEvent::Eof => unreachable!(),
                }
                if depth == 0 {
                    self.push_new_context(buffer, errors);
                    self.state
                        .push(JsonLdExpansionState::ObjectOrContainerStartStreaming {
                            active_property,
                            container,
                            reverse,
                        });
                } else {
                    self.state.push(JsonLdExpansionState::Context {
                        buffer,
                        depth,
                        active_property,
                        container,
                        reverse,
                    });
                }
            }
            JsonLdExpansionState::ObjectStart {
                types,
                id,
                seen_id,
                active_property,
                reverse,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) = self.expand_iri(key.as_ref().into(), false, true, errors) {
                        match iri.as_ref() {
                            "@type" => {
                                if seen_id && !self.lenient {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type must be the first key of an object or right after @context",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ))
                                }
                                self.state.push(JsonLdExpansionState::ObjectType {
                                    id,
                                    types,
                                    is_array: false,
                                    active_property,
                                    reverse,
                                });
                            }
                            "@value" | "@language" => {
                                if types.len() > 1 {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Only a single @type is allowed when @value is present",
                                        JsonLdErrorCode::InvalidTypedValue,
                                    ));
                                }
                                if id.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@value and @id are incompatible",
                                        JsonLdErrorCode::InvalidValueObject,
                                    ));
                                }
                                if reverse {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Literals are not allowed inside of reverse properties",
                                        JsonLdErrorCode::InvalidReversePropertyValue,
                                    ))
                                }
                                self.state.push(JsonLdExpansionState::Value {
                                    r#type: types.into_iter().next(),
                                    value: None,
                                    language: None,
                                });
                                self.convert_event(JsonEvent::ObjectKey(key), results, errors);
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
                                    reverse,
                                });
                            }
                            "@graph"
                                if id.is_none() && types.is_empty() && self.state.is_empty() =>
                            {
                                // Graph only for @context
                                self.state.push(JsonLdExpansionState::RootGraph);
                                self.state.push(JsonLdExpansionState::Element {
                                    active_property: None,
                                    is_array: false,
                                    container: &[],
                                    reverse: false,
                                })
                            }
                            "@index" => {
                                self.state.push(JsonLdExpansionState::ObjectStart {
                                    types,
                                    id,
                                    seen_id,
                                    active_property,
                                    reverse,
                                });
                                self.state.push(JsonLdExpansionState::Index);
                            }
                            _ => {
                                results.push(JsonLdEvent::StartObject { types });
                                let has_emitted_id = id.is_some();
                                if let Some(id) = id {
                                    results.push(JsonLdEvent::Id(id));
                                }
                                self.state.push(JsonLdExpansionState::Object {
                                    in_property: false,
                                    has_emitted_id,
                                });
                                self.convert_event(JsonEvent::ObjectKey(key), results, errors);
                            }
                        }
                    } else {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_id,
                            active_property,
                            reverse,
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
                active_property,
                reverse,
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
                                active_property,
                                reverse,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_id: false,
                                active_property,
                                reverse,
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
                                active_property,
                                reverse,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_id: false,
                                active_property,
                                reverse,
                            });
                        }
                    }
                    JsonEvent::StartArray => {
                        self.state.push(JsonLdExpansionState::ObjectType {
                            types,
                            id,
                            is_array: true,
                            active_property,
                            reverse,
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
                            seen_id: false,
                            active_property,
                            reverse,
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
                                active_property,
                                reverse,
                            });
                        } else {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_id: false,
                                active_property,
                                reverse,
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
                reverse,
            } => {
                if let JsonEvent::String(new_id) = event {
                    if let Some(new_id) = self.expand_iri(new_id, true, false, errors) {
                        if has_keyword_form(&new_id) {
                            errors.push(JsonLdSyntaxError::msg(
                                "@id value must be an IRI or a blank node",
                            ));
                        } else {
                            id = Some(new_id.into());
                        }
                        self.state.push(if from_start {
                            JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_id: true,
                                active_property: None,
                                reverse,
                            }
                        } else {
                            if let Some(id) = id {
                                results.push(JsonLdEvent::Id(id));
                            }
                            JsonLdExpansionState::Object {
                                in_property: false,
                                has_emitted_id: true,
                            }
                        })
                    } else {
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: false });
                    }
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidIdValue,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            types,
                            id,
                            seen_id: true,
                            active_property: None,
                            reverse,
                        }
                    } else {
                        JsonLdExpansionState::Object {
                            in_property: false,
                            has_emitted_id: true,
                        }
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::Object {
                in_property,
                has_emitted_id,
            } => {
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
                                    if has_emitted_id {
                                        errors.push(JsonLdSyntaxError::msg("Duplicated @id key"));
                                        self.state.push(JsonLdExpansionState::Object {
                                            in_property: false,
                                            has_emitted_id: true,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ObjectId {
                                            types: Vec::new(),
                                            id: None,
                                            from_start: false,
                                            reverse: false,
                                        });
                                    }
                                }
                                "@graph" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state.push(JsonLdExpansionState::Graph);
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property: None,
                                        container: &[],
                                        reverse: false,
                                    });
                                    results.push(JsonLdEvent::StartGraph);
                                }
                                "@context" => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@context must be the first key of an object",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ));
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                "@type" => {
                                    // TODO: be nice and allow this if lenient
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type must be the first key of an object or right after @context",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ));
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                "@index" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state.push(JsonLdExpansionState::Index);
                                }
                                "@reverse" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state.push(JsonLdExpansionState::ReverseStart);
                                }
                                _ if has_keyword_form(&iri) => {
                                    errors.push(if iri == "@list" || iri == "@set" {
                                        JsonLdSyntaxError::msg_and_code(
                                            "@list and @set must be the only keys of an object",
                                            JsonLdErrorCode::InvalidSetOrListObject,
                                        )
                                    } else if iri == "@context" {
                                        JsonLdSyntaxError::msg_and_code(
                                            "@context must be the first key of an object",
                                            JsonLdErrorCode::InvalidStreamingKeyOrder,
                                        )
                                    } else {
                                        JsonLdSyntaxError::msg(format!(
                                            "Unsupported JSON-LD keyword: {iri}"
                                        ))
                                    });
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: false,
                                        has_emitted_id,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                _ => {
                                    let (container, reverse) = self
                                        .context()
                                        .term_definitions
                                        .get(key.as_ref())
                                        .map_or(([].as_slice(), false), |term_definition| {
                                            (
                                                term_definition.container_mapping,
                                                term_definition.reverse_property,
                                            )
                                        });
                                    self.state.push(JsonLdExpansionState::Object {
                                        in_property: true,
                                        has_emitted_id,
                                    });
                                    self.state.push(JsonLdExpansionState::Element {
                                        active_property: Some(key.clone().into()),
                                        is_array: false,
                                        container,
                                        reverse,
                                    });
                                    results.push(JsonLdEvent::StartProperty {
                                        name: iri.into(),
                                        reverse,
                                    });
                                }
                            }
                        } else {
                            self.state.push(JsonLdExpansionState::Object {
                                in_property: false,
                                has_emitted_id,
                            });
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
            JsonLdExpansionState::ReverseStart => {
                if matches!(event, JsonEvent::StartObject) {
                    self.state
                        .push(JsonLdExpansionState::Reverse { in_property: false });
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@reverse value must be a JSON object",
                        JsonLdErrorCode::InvalidReverseValue,
                    ));
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::Reverse { in_property } => {
                if in_property {
                    results.push(JsonLdEvent::EndProperty);
                }
                match event {
                    JsonEvent::EndObject => (),
                    JsonEvent::ObjectKey(key) => {
                        if let Some(iri) = self.expand_iri(key.as_ref().into(), false, true, errors)
                        {
                            if has_keyword_form(&iri) {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!(
                                        "@reverse object value cannot contain any keyword, found {iri}",
                                    ),
                                    JsonLdErrorCode::InvalidReversePropertyMap,
                                ));
                                self.state
                                    .push(JsonLdExpansionState::Reverse { in_property: false });
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            } else {
                                let (container, reverse) = self
                                    .context()
                                    .term_definitions
                                    .get(key.as_ref())
                                    .map_or(([].as_slice(), false), |term_definition| {
                                        (
                                            term_definition.container_mapping,
                                            term_definition.reverse_property,
                                        )
                                    });
                                let reverse = !reverse; // We are in @reverse
                                self.state
                                    .push(JsonLdExpansionState::Reverse { in_property: true });
                                self.state.push(JsonLdExpansionState::Element {
                                    active_property: Some(key.clone().into()),
                                    is_array: false,
                                    container,
                                    reverse,
                                });
                                results.push(JsonLdEvent::StartProperty {
                                    name: iri.into(),
                                    reverse,
                                });
                            }
                        } else {
                            self.state
                                .push(JsonLdExpansionState::Reverse { in_property: false });
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
            } => {
                match event {
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
                                        self.state.push(JsonLdExpansionState::ValueType {
                                            value,
                                            language,
                                        });
                                    }
                                }
                                "@context" => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@context must be the first key of an object",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ));
                                    self.state.push(JsonLdExpansionState::Value {
                                        r#type,
                                        value,
                                        language,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                "@index" => {
                                    self.state.push(JsonLdExpansionState::Value {
                                        r#type,
                                        value,
                                        language,
                                    });
                                    self.state.push(JsonLdExpansionState::Index);
                                }
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
                            self.state.push(JsonLdExpansionState::Value {
                                r#type,
                                value,
                                language,
                            });
                            self.state
                                .push(JsonLdExpansionState::Skip { is_array: false });
                        }
                    }
                    JsonEvent::EndObject => {
                        if let Some(value) = value {
                            let mut is_valid = true;
                            if language.is_some() && r#type.is_some() {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    "@type and @language cannot be used together",
                                    JsonLdErrorCode::InvalidValueObject,
                                ));
                                is_valid = false;
                            }
                            if language.is_some() && !matches!(value, JsonLdValue::String(_)) {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    "@language can be used only on a string @value",
                                    JsonLdErrorCode::InvalidLanguageTaggedValue,
                                ));
                                is_valid = false;
                            }
                            if let Some(r#type) = &r#type {
                                if r#type.starts_with("_:") {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@type cannot be a blank node",
                                        JsonLdErrorCode::InvalidTypedValue,
                                    ));
                                    is_valid = false;
                                } else if !self.lenient {
                                    if let Err(e) = Iri::parse(r#type.as_str()) {
                                        errors.push(JsonLdSyntaxError::msg_and_code(
                                            format!("@type value '{type}' must be an IRI: {e}"),
                                            JsonLdErrorCode::InvalidTypedValue,
                                        ));
                                        is_valid = false;
                                    }
                                }
                            }
                            if is_valid {
                                results.push(JsonLdEvent::Value {
                                    value,
                                    r#type,
                                    language,
                                })
                            }
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
                }
            }
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
                _ => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@value value must be a string, number, boolean or null",
                        JsonLdErrorCode::InvalidValueObjectValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value: None,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            },
            JsonLdExpansionState::ValueLanguage { value, r#type } => {
                if let JsonEvent::String(language) = event {
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value,
                        language: Some(language.into()),
                    })
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@value value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type,
                        value,
                        language: None,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::ValueType { value, language } => {
                if let JsonEvent::String(t) = event {
                    let mut r#type = self.expand_iri(t, true, true, errors);
                    if let Some(iri) = &r#type {
                        if has_keyword_form(iri) {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                format!("{iri} is not a valid value for @type"),
                                JsonLdErrorCode::InvalidTypedValue,
                            ));
                            r#type = None
                        }
                    }
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: r#type.map(Into::into),
                        value,
                        language,
                    })
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type value must be a string when @value is present",
                        JsonLdErrorCode::InvalidTypedValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        r#type: None,
                        value,
                        language,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::Index => {
                if let JsonEvent::String(_) = event {
                    // TODO: properly emit if we implement expansion output
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@index value must be a string",
                        JsonLdErrorCode::InvalidIndexValue,
                    ));
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::Graph => {
                results.push(JsonLdEvent::EndGraph);
                self.convert_event(event, results, errors)
            }
            JsonLdExpansionState::RootGraph => match event {
                JsonEvent::ObjectKey(key) => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!(
                            "@graph must be the last property of the object, found {key} after it"
                        ),
                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                    ));
                    self.state.push(JsonLdExpansionState::RootGraph);
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                }
                JsonEvent::EndObject => (),
                _ => unreachable!(),
            },
            JsonLdExpansionState::ListOrSetContainer {
                needs_end_object,
                end_event,
            } => {
                if needs_end_object {
                    match event {
                        JsonEvent::EndObject => {
                            results.extend(end_event);
                            self.pop_context();
                        }
                        JsonEvent::ObjectKey(key) => {
                            self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                needs_end_object,
                                end_event,
                            });
                            if let Some(iri) =
                                self.expand_iri(key.as_ref().into(), false, true, errors)
                            {
                                if iri == "@index" {
                                    self.state.push(JsonLdExpansionState::Index);
                                } else {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        format!(
                                            "@list must be the only key of an object, {key} found"
                                        ),
                                        JsonLdErrorCode::InvalidSetOrListObject,
                                    ));
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                            } else {
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    results.extend(end_event);
                    self.convert_event(event, results, errors)
                }
            }
            JsonLdExpansionState::IndexContainer { active_property } => match event {
                JsonEvent::EndObject => (),
                JsonEvent::ObjectKey(_) => {
                    // TODO: emit @index
                    self.state.push(JsonLdExpansionState::IndexContainer {
                        active_property: active_property.clone(),
                    });
                    self.state.push(JsonLdExpansionState::Element {
                        active_property,
                        is_array: false,
                        container: &[],
                        reverse: false,
                    })
                }
                _ => unreachable!(),
            },
            JsonLdExpansionState::LanguageContainer => match event {
                JsonEvent::EndObject => (),
                JsonEvent::ObjectKey(language) => {
                    self.state.push(JsonLdExpansionState::LanguageContainer);
                    self.state
                        .push(JsonLdExpansionState::LanguageContainerValue {
                            language: language.into(),
                            is_array: false,
                        })
                }
                _ => unreachable!(),
            },
            JsonLdExpansionState::LanguageContainerValue { language, is_array } => match event {
                JsonEvent::Null => {
                    if is_array {
                        self.state
                            .push(JsonLdExpansionState::LanguageContainerValue {
                                language,
                                is_array,
                            });
                    }
                }
                JsonEvent::String(value) => {
                    if is_array {
                        self.state
                            .push(JsonLdExpansionState::LanguageContainerValue {
                                language: language.clone(),
                                is_array,
                            });
                    }
                    results.push(JsonLdEvent::Value {
                        value: JsonLdValue::String(value.into()),
                        r#type: None,
                        language: (language != "@none"
                            && self.expand_iri(language.as_str().into(), false, false, errors)
                                != Some("@none".into()))
                        .then_some(language),
                    })
                }
                JsonEvent::StartArray => {
                    self.state
                        .push(JsonLdExpansionState::LanguageContainerValue {
                            language,
                            is_array: true,
                        });
                    if is_array {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "The values in a @language map must be null or strings",
                            JsonLdErrorCode::InvalidLanguageMapValue,
                        ));
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: true })
                    }
                }
                JsonEvent::EndArray => (),
                _ => {
                    if is_array {
                        self.state
                            .push(JsonLdExpansionState::LanguageContainerValue {
                                language,
                                is_array,
                            });
                    }
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "The values in a @language map must be null or strings",
                        JsonLdErrorCode::InvalidLanguageMapValue,
                    ));
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            },
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
        self.context_processor.expand_iri(
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
            errors,
        )
    }

    fn on_literal_value(
        &mut self,
        value: JsonLdValue,
        active_property: Option<String>,
        is_array: bool,
        container: &'static [&'static str],
        reverse: bool,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        if !is_array {
            if container.contains(&"@list") {
                if reverse {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "Lists are not allowed inside of reverse properties",
                        JsonLdErrorCode::InvalidReversePropertyValue,
                    ))
                }
                results.push(JsonLdEvent::StartList);
            } else if container.contains(&"@set") {
                results.push(JsonLdEvent::StartSet);
            }
        }
        if let Some(active_property) = &active_property {
            self.expand_value(active_property, value, reverse, results, errors);
        }
        if is_array {
            self.state.push(JsonLdExpansionState::Element {
                active_property,
                is_array,
                container,
                reverse,
            });
        } else if container.contains(&"@list") {
            results.push(JsonLdEvent::EndList);
        } else if container.contains(&"@set") {
            results.push(JsonLdEvent::EndSet);
        }
    }

    /// [Value Expansion](https://www.w3.org/TR/json-ld-api/#value-expansion)
    fn expand_value(
        &mut self,
        active_property: &str,
        value: JsonLdValue,
        reverse: bool,
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
                language = term_definition
                    .language_mapping
                    .clone()
                    .unwrap_or_else(|| active_context.default_language.clone());
            }
        } else {
            // 5)
            if matches!(value, JsonLdValue::String(_)) && language.is_none() {
                language.clone_from(&active_context.default_language);
            }
        }
        if reverse {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "Literals are not allowed inside of reverse properties",
                JsonLdErrorCode::InvalidReversePropertyValue,
            ))
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

    fn push_new_context(
        &mut self,
        context: Vec<JsonEvent<'static>>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        let context = self.context_processor.process_context(
            self.context(),
            json_node_from_events(context.into_iter().map(Ok)).unwrap(),
            self.base_url.as_ref(),
            &mut Vec::new(),
            false,
            true,
            true,
            errors,
        );
        if let Some((last_context, last_count)) = self.context.pop() {
            if last_count > 1 {
                self.context.push((last_context, last_count - 1));
            }
        }
        self.context.push((context, 1));
    }

    fn pop_context(&mut self) {
        let Some((last_context, mut last_count)) = self.context.pop() else {
            return;
        };
        last_count -= 1;
        if last_count > 0 || self.context.is_empty() {
            // We always keep a context to allow reading the root context at the end of the document
            self.context.push((last_context, last_count));
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
