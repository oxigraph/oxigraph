use crate::context::{
    JsonLdContext, JsonLdContextProcessor, JsonLdLoadDocumentOptions, JsonLdRemoteDocument,
    has_keyword_form, is_keyword, json_node_from_events,
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
    StartObject,
    EndObject,
    StartProperty {
        name: String,
        reverse: bool,
    },
    EndProperty,
    Id(String),
    Type(String),
    Value {
        value: JsonLdValue,
        r#type: Option<String>,
        language: Option<String>,
        direction: Option<&'static str>,
    },
    StartGraph,
    EndGraph,
    StartList,
    EndList,
    StartSet,
    EndSet,
    StartIncluded,
    EndIncluded,
}

pub enum JsonLdValue {
    String(String),
    Number(String),
    Boolean(bool),
}

enum JsonLdExpansionState {
    Element {
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        is_array: bool,
        container: &'static [&'static str],
        reverse: bool,
        in_included: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    ObjectOrContainerStart {
        buffer: Vec<(String, Vec<JsonEvent<'static>>)>,
        depth: usize,
        current_key: Option<String>,
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        container: &'static [&'static str],
        reverse: bool,
        in_included: bool,
    },
    ObjectOrContainerStartStreaming {
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        container: &'static [&'static str],
        reverse: bool,
        in_included: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    Context {
        buffer: Vec<JsonEvent<'static>>,
        depth: usize,
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        container: &'static [&'static str],
        reverse: bool,
        in_included: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    ObjectStartIsSingleIdOrValue {
        buffer: Vec<JsonEvent<'static>>,
        depth: usize,
        seen_type: bool,
        seen_id: bool,
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        reverse: bool,
        in_included: bool,
        is_array: bool,
        container: &'static [&'static str],
    },
    ObjectStart {
        types: Vec<String>,
        id: Option<String>,
        seen_id: bool,
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        reverse: bool,
        in_included: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    ObjectType {
        types: Vec<String>,
        new_types: Vec<String>,
        id: Option<String>,
        is_array: bool,
        active_property: Option<String>,
        active_context: Arc<JsonLdContext>,
        from_start: bool,
        reverse: bool,
        in_included: bool,
        nesting: usize,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    ObjectId {
        active_context: Arc<JsonLdContext>,
        types: Vec<String>,
        id: Option<String>,
        from_start: bool,
        reverse: bool,
        nesting: usize,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
    },
    Object {
        active_context: Arc<JsonLdContext>,
        in_property: bool,
        has_emitted_id: bool,
        nesting: usize,
    },
    ReverseStart {
        active_context: Arc<JsonLdContext>,
    },
    Reverse {
        active_context: Arc<JsonLdContext>,
        in_property: bool,
    },
    Value {
        active_context: Arc<JsonLdContext>,
        r#type: Option<String>,
        value: Option<JsonLdValue>,
        language: Option<String>,
        direction: Option<&'static str>,
    },
    ValueValue {
        active_context: Arc<JsonLdContext>,
        r#type: Option<String>,
        language: Option<String>,
        direction: Option<&'static str>,
    },
    ValueLanguage {
        active_context: Arc<JsonLdContext>,
        r#type: Option<String>,
        value: Option<JsonLdValue>,
        direction: Option<&'static str>,
    },
    ValueDirection {
        active_context: Arc<JsonLdContext>,
        r#type: Option<String>,
        value: Option<JsonLdValue>,
        language: Option<String>,
    },
    ValueType {
        active_context: Arc<JsonLdContext>,
        value: Option<JsonLdValue>,
        language: Option<String>,
        direction: Option<&'static str>,
    },
    Index,
    Graph,
    RootGraph,
    ListOrSetContainer {
        needs_end_object: bool,
        end_event: Option<JsonLdEvent>,
        active_context: Arc<JsonLdContext>,
    },
    IndexContainer {
        active_context: Arc<JsonLdContext>,
        active_property: Option<String>,
    },
    LanguageContainer {
        active_context: Arc<JsonLdContext>,
        direction: Option<&'static str>,
    },
    LanguageContainerValue {
        active_context: Arc<JsonLdContext>,
        language: String,
        direction: Option<&'static str>,
        is_array: bool,
    },
    Included,
    NestStart {
        active_context: Arc<JsonLdContext>,
        parent_active_context: Arc<JsonLdContext>,
        has_emitted_id: bool,
        nesting: usize,
        array_count: usize,
    },
    Skip {
        is_array: bool,
    },
}

/// Applies the [Expansion Algorithm](https://www.w3.org/TR/json-ld-api/#expansion-algorithms)
pub struct JsonLdExpansionConverter {
    state: Vec<JsonLdExpansionState>,
    is_end: bool,
    streaming: bool,
    lenient: bool,
    base_url: Option<Iri<String>>,
    context_processor: JsonLdContextProcessor,
    root_context: Arc<JsonLdContext>,
}

#[expect(clippy::expect_used)]
impl JsonLdExpansionConverter {
    pub fn new(
        base_url: Option<Iri<String>>,
        streaming: bool,
        lenient: bool,
        processing_mode: JsonLdProcessingMode,
    ) -> Self {
        let root_context = Arc::new(JsonLdContext::new_empty(base_url.clone()));
        Self {
            state: vec![JsonLdExpansionState::Element {
                active_property: None,
                active_context: Arc::clone(&root_context),
                is_array: false,
                container: &[],
                reverse: false,
                in_included: false,
                extra_nodes: Vec::new(),
            }],
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
            root_context,
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
                mut active_context,
                is_array,
                container,
                reverse,
                in_included,
                extra_nodes,
            } => {
                match event {
                    JsonEvent::Null => {
                        // 1)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property,
                                active_context,
                                is_array,
                                container,
                                reverse,
                                in_included,
                                extra_nodes,
                            });
                        }
                    }
                    JsonEvent::String(value) => self.on_literal_value(
                        JsonLdValue::String(value.into()),
                        active_context,
                        active_property,
                        is_array,
                        container,
                        reverse,
                        in_included,
                        extra_nodes,
                        results,
                        errors,
                    ),
                    JsonEvent::Number(value) => self.on_literal_value(
                        JsonLdValue::Number(value.into()),
                        active_context,
                        active_property,
                        is_array,
                        container,
                        reverse,
                        in_included,
                        extra_nodes,
                        results,
                        errors,
                    ),
                    JsonEvent::Boolean(value) => self.on_literal_value(
                        JsonLdValue::Boolean(value),
                        active_context,
                        active_property,
                        is_array,
                        container,
                        reverse,
                        in_included,
                        extra_nodes,
                        results,
                        errors,
                    ),
                    JsonEvent::StartArray => {
                        // 5)
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property: active_property.clone(),
                                active_context: Arc::clone(&active_context),
                                is_array,
                                container,
                                reverse,
                                in_included,
                                extra_nodes: extra_nodes.clone(),
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
                                active_context: Arc::clone(&active_context),
                            })
                        }
                        if container.contains(&"@set") && !is_array {
                            results.push(JsonLdEvent::StartSet);
                            self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                needs_end_object: false,
                                end_event: Some(JsonLdEvent::EndSet),
                                active_context: Arc::clone(&active_context),
                            })
                        }
                        self.state.push(JsonLdExpansionState::Element {
                            active_property,
                            active_context,
                            is_array: true,
                            container,
                            reverse,
                            in_included,
                            extra_nodes,
                        });
                    }
                    JsonEvent::EndArray => (),
                    JsonEvent::StartObject => {
                        if is_array {
                            self.state.push(JsonLdExpansionState::Element {
                                active_property: active_property.clone(),
                                active_context: Arc::clone(&active_context),
                                is_array,
                                container,
                                reverse,
                                in_included,
                                extra_nodes: extra_nodes.clone(),
                            });
                        } else if container.contains(&"@index") {
                            self.state.push(JsonLdExpansionState::IndexContainer {
                                active_context,
                                active_property,
                            });
                            return;
                        } else if container.contains(&"@language") {
                            // 13.7.2)
                            let mut direction = active_context.default_direction;
                            // 13.7.3)
                            if let Some(active_property) = &active_property {
                                if let Some(term_definition) =
                                    active_context.term_definitions.get(active_property)
                                {
                                    if let Some(direction_mapping) =
                                        term_definition.direction_mapping
                                    {
                                        direction = direction_mapping;
                                    }
                                }
                            }
                            self.state.push(JsonLdExpansionState::LanguageContainer {
                                active_context,
                                direction,
                            });
                            return;
                        }
                        if active_context.previous_context.is_some() {
                            // We need to decide if we go back to the previous context or not
                            self.state
                                .push(JsonLdExpansionState::ObjectStartIsSingleIdOrValue {
                                    buffer: Vec::new(),
                                    depth: 1,
                                    seen_type: false,
                                    seen_id: false,
                                    active_property,
                                    active_context: Arc::clone(&active_context),
                                    reverse,
                                    in_included,
                                    is_array,
                                    container,
                                });
                            for (k, v) in extra_nodes {
                                self.convert_event(JsonEvent::ObjectKey(k.into()), results, errors);
                                for e in v {
                                    self.convert_event(e, results, errors);
                                }
                            }
                        } else {
                            if let Some(active_property) = &active_property {
                                if let Some(property_scoped_context) = self.new_scoped_context(
                                    &active_context,
                                    active_property,
                                    true,
                                    true,
                                    errors,
                                ) {
                                    active_context = Arc::new(property_scoped_context);
                                }
                            }
                            self.state.push(if self.streaming {
                                JsonLdExpansionState::ObjectOrContainerStartStreaming {
                                    extra_nodes,
                                    active_property,
                                    active_context,
                                    container: if is_array { &[] } else { container },
                                    reverse,
                                    in_included,
                                }
                            } else {
                                JsonLdExpansionState::ObjectOrContainerStart {
                                    buffer: extra_nodes,
                                    depth: 1,
                                    current_key: None,
                                    active_property,
                                    active_context,
                                    container: if is_array { &[] } else { container },
                                    reverse,
                                    in_included,
                                }
                            });
                        }
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
                mut active_context,
                container,
                reverse,
                in_included,
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
                    // We look for @context
                    if let Some((idx, _)) = buffer
                        .iter()
                        .enumerate()
                        .find(|(_, (key, _))| key == "@context")
                    {
                        let (_, events) = buffer.remove(idx);
                        active_context = self.new_context(&active_context, events, errors);
                        if self.state.is_empty() {
                            self.root_context = Arc::clone(&active_context);
                        }
                    }
                    // We look for @type, @id and @graph
                    let mut type_data = Vec::new();
                    let mut id_data = None;
                    let mut graph_data = Vec::new();
                    let mut other_data = Vec::with_capacity(buffer.len());
                    for (key, value) in buffer {
                        let expanded =
                            self.expand_iri(&active_context, key.as_str().into(), false, true);
                        match expanded.as_deref() {
                            Some("@context") => errors.push(JsonLdSyntaxError::msg_and_code(
                                "@context is defined twice",
                                JsonLdErrorCode::CollidingKeywords,
                            )),
                            Some("@type") => {
                                type_data.push((key, value));
                            }
                            Some("@id") => {
                                if id_data.is_some() {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@id is defined twice",
                                        JsonLdErrorCode::CollidingKeywords,
                                    ))
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
                            extra_nodes: Vec::new(),
                            active_property,
                            active_context,
                            container,
                            reverse,
                            in_included,
                        });

                    // We first sort types by key
                    type_data.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
                    // We first process @type and @id then others, then graph
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
                            active_context,
                            container,
                            reverse,
                            in_included,
                        });
                }
            }
            JsonLdExpansionState::ObjectOrContainerStartStreaming {
                active_property,
                active_context,
                container,
                reverse,
                in_included,
                extra_nodes,
            } => {
                let event = match event {
                    JsonEvent::ObjectKey(key) => {
                        match self
                            .expand_iri(&active_context, key.as_ref().into(), false, true)
                            .as_deref()
                        {
                            Some("@context") => {
                                self.state.push(JsonLdExpansionState::Context {
                                    buffer: Vec::new(),
                                    depth: 0,
                                    active_property,
                                    active_context,
                                    container,
                                    reverse,
                                    in_included,
                                    extra_nodes,
                                });
                                return;
                            }
                            Some("@index") => {
                                self.state.push(
                                    JsonLdExpansionState::ObjectOrContainerStartStreaming {
                                        extra_nodes,
                                        active_property,
                                        active_context,
                                        container,
                                        reverse,
                                        in_included,
                                    },
                                );
                                self.state.push(JsonLdExpansionState::Index);
                                return;
                            }
                            Some("@list") => {
                                if in_included {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Lists are not allowed inside of @included",
                                        JsonLdErrorCode::InvalidIncludedValue,
                                    ));
                                }
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
                                        active_context: Arc::clone(&active_context),
                                    });
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property,
                                        active_context,
                                        container: &[],
                                        reverse: false,
                                        in_included: false,
                                        extra_nodes: Vec::new(),
                                    });
                                    results.push(JsonLdEvent::StartList);
                                } else {
                                    // We don't have an active property, we skip the list
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                return;
                            }
                            Some("@set") => {
                                if in_included {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "Sets are not allowed inside of @included",
                                        JsonLdErrorCode::InvalidIncludedValue,
                                    ));
                                }
                                let has_property = active_property.is_some();
                                self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                    needs_end_object: true,
                                    end_event: has_property.then_some(JsonLdEvent::EndSet),
                                    active_context: Arc::clone(&active_context),
                                });
                                self.state.push(JsonLdExpansionState::Element {
                                    is_array: false,
                                    active_property,
                                    active_context,
                                    container: &[],
                                    reverse: false,
                                    in_included: false,
                                    extra_nodes: Vec::new(),
                                });
                                if has_property {
                                    results.push(JsonLdEvent::StartSet);
                                }
                                return;
                            }
                            _ => JsonEvent::ObjectKey(key),
                        }
                    }
                    JsonEvent::EndObject => JsonEvent::EndObject,
                    _ => unreachable!("Inside of an object"),
                };
                if container.contains(&"@list") {
                    results.push(JsonLdEvent::StartList);
                    self.state.push(JsonLdExpansionState::ListOrSetContainer {
                        needs_end_object: false,
                        end_event: Some(JsonLdEvent::EndList),
                        active_context: Arc::clone(&active_context),
                    });
                } else if container.contains(&"@set") {
                    results.push(JsonLdEvent::StartSet);
                    self.state.push(JsonLdExpansionState::ListOrSetContainer {
                        needs_end_object: false,
                        end_event: Some(JsonLdEvent::EndSet),
                        active_context: Arc::clone(&active_context),
                    });
                }
                self.state.push(JsonLdExpansionState::ObjectStart {
                    extra_nodes,
                    types: Vec::new(),
                    id: None,
                    seen_id: false,
                    active_property,
                    active_context,
                    reverse,
                    in_included,
                });
                self.convert_event(event, results, errors)
            }
            JsonLdExpansionState::Context {
                mut buffer,
                mut depth,
                active_property,
                mut active_context,
                container,
                reverse,
                in_included,
                extra_nodes,
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
                    active_context = self.new_context(&active_context, buffer, errors);
                    if self.state.is_empty() {
                        self.root_context = Arc::clone(&active_context);
                    }
                    self.state
                        .push(JsonLdExpansionState::ObjectOrContainerStartStreaming {
                            active_property,
                            active_context,
                            container,
                            reverse,
                            in_included,
                            extra_nodes,
                        });
                } else {
                    self.state.push(JsonLdExpansionState::Context {
                        buffer,
                        depth,
                        active_property,
                        active_context,
                        container,
                        reverse,
                        in_included,
                        extra_nodes,
                    });
                }
            }
            JsonLdExpansionState::ObjectStartIsSingleIdOrValue {
                mut buffer,
                mut depth,
                active_property,
                mut active_context,
                reverse,
                in_included,
                is_array,
                container,
                mut seen_id,
                mut seen_type,
            } => {
                let mut is_single_id_or_value = None;
                match event {
                    JsonEvent::ObjectKey(key) if depth == 1 => {
                        if let Some(iri) =
                            self.expand_iri(&active_context, key.as_ref().into(), false, true)
                        {
                            match iri.as_ref() {
                                "@index" | "@context" => (),
                                "@type" => {
                                    seen_type = true;
                                }
                                "@value" | "@language" | "@direction" => {
                                    is_single_id_or_value = Some(true);
                                }
                                "@id" if !seen_type => {
                                    seen_id = true;
                                }
                                _ => {
                                    is_single_id_or_value = Some(false);
                                }
                            }
                        }
                        buffer.push(to_owned_event(JsonEvent::ObjectKey(key)));
                    }
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
                if depth == 0 && is_single_id_or_value.is_none() {
                    is_single_id_or_value = Some(seen_id);
                }
                if let Some(is_single_id_or_value) = is_single_id_or_value {
                    // 3) 7) 8)
                    let active_context_for_property_scoped_context = Arc::clone(&active_context);
                    if !is_single_id_or_value {
                        if let Some(previous_context) = &active_context.previous_context {
                            active_context = Arc::clone(previous_context);
                        }
                    }
                    if let Some(active_property) = &active_property {
                        if let Some(property_scoped_context) = self.new_scoped_context(
                            &active_context_for_property_scoped_context,
                            active_property,
                            true,
                            true,
                            errors,
                        ) {
                            active_context = Arc::new(property_scoped_context);
                        }
                    }
                    self.state.push(if self.streaming {
                        JsonLdExpansionState::ObjectOrContainerStartStreaming {
                            active_property,
                            active_context,
                            container: if is_array { &[] } else { container },
                            reverse,
                            in_included,
                            extra_nodes: Vec::new(),
                        }
                    } else {
                        JsonLdExpansionState::ObjectOrContainerStart {
                            buffer: Vec::new(),
                            depth: 1,
                            current_key: None,
                            active_property,
                            active_context,
                            container: if is_array { &[] } else { container },
                            reverse,
                            in_included,
                        }
                    });
                    for event in buffer {
                        self.convert_event(event, results, errors);
                    }
                } else {
                    self.state
                        .push(JsonLdExpansionState::ObjectStartIsSingleIdOrValue {
                            buffer,
                            depth,
                            seen_type,
                            seen_id,
                            active_property,
                            active_context,
                            reverse,
                            in_included,
                            is_array,
                            container,
                        });
                }
            }
            JsonLdExpansionState::ObjectStart {
                types,
                id,
                seen_id,
                active_property,
                active_context,
                reverse,
                in_included,
                extra_nodes,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(iri) =
                        self.expand_iri(&active_context, key.as_ref().into(), false, true)
                    {
                        match iri.as_ref() {
                            "@index" => {
                                self.state.push(JsonLdExpansionState::ObjectStart {
                                    extra_nodes,
                                    types,
                                    id,
                                    seen_id,
                                    active_property,
                                    active_context,
                                    reverse,
                                    in_included,
                                });
                                self.state.push(JsonLdExpansionState::Index);
                            }
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
                                    new_types: Vec::new(),
                                    is_array: false,
                                    active_property,
                                    active_context,
                                    from_start: true,
                                    reverse,
                                    in_included,
                                    nesting: 0,
                                    extra_nodes,
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
                                    active_context,
                                    types,
                                    id,
                                    from_start: true,
                                    reverse,
                                    nesting: 0,
                                    extra_nodes,
                                });
                            }
                            _ => {
                                // We process the extra nodes
                                if !extra_nodes.is_empty() {
                                    self.state.push(JsonLdExpansionState::ObjectStart {
                                        extra_nodes: Vec::new(),
                                        types,
                                        id,
                                        seen_id,
                                        active_property,
                                        active_context,
                                        reverse,
                                        in_included,
                                    });
                                    for (k, v) in extra_nodes {
                                        self.convert_event(
                                            JsonEvent::ObjectKey(k.into()),
                                            results,
                                            errors,
                                        );
                                        for e in v {
                                            self.convert_event(e, results, errors);
                                        }
                                    }
                                    self.convert_event(JsonEvent::ObjectKey(key), results, errors);
                                    return;
                                }
                                match iri.as_ref() {
                                    "@value" | "@language" | "@direction" => {
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
                                        if in_included {
                                            errors.push(JsonLdSyntaxError::msg_and_code(
                                                "Literals are not allowed inside of @included",
                                                JsonLdErrorCode::InvalidIncludedValue,
                                            ));
                                        }
                                        self.state.push(JsonLdExpansionState::Value {
                                            active_context,
                                            r#type: types.into_iter().next(),
                                            value: None,
                                            language: None,
                                            direction: None,
                                        });
                                        self.convert_event(
                                            JsonEvent::ObjectKey(key),
                                            results,
                                            errors,
                                        );
                                    }
                                    "@graph"
                                        if id.is_none()
                                            && types.is_empty()
                                            && self.state.is_empty() =>
                                    {
                                        // Graph only for @context
                                        self.state.push(JsonLdExpansionState::RootGraph);
                                        self.state.push(JsonLdExpansionState::Element {
                                            active_property: None,
                                            active_context,
                                            is_array: false,
                                            container: &[],
                                            reverse: false,
                                            in_included: false,
                                            extra_nodes: Vec::new(),
                                        })
                                    }
                                    _ => {
                                        results.push(JsonLdEvent::StartObject);
                                        let has_emitted_id = id.is_some();
                                        if let Some(id) = id {
                                            if let Some(id) = self.expand_iri(
                                                &active_context,
                                                id.into(),
                                                true,
                                                false,
                                            ) {
                                                if has_keyword_form(&id) {
                                                    errors.push(JsonLdSyntaxError::msg(
                                                        "@id value must be an IRI or a blank node",
                                                    ));
                                                } else {
                                                    results.push(JsonLdEvent::Id(id.into()));
                                                }
                                            }
                                        }
                                        results.extend(types.into_iter().map(JsonLdEvent::Type));
                                        self.state.push(JsonLdExpansionState::Object {
                                            active_context,
                                            in_property: false,
                                            has_emitted_id,
                                            nesting: 0,
                                        });
                                        self.convert_event(
                                            JsonEvent::ObjectKey(key),
                                            results,
                                            errors,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            extra_nodes,
                            types,
                            id,
                            seen_id,
                            active_property,
                            active_context,
                            reverse,
                            in_included,
                        });
                        self.state
                            .push(JsonLdExpansionState::Skip { is_array: false });
                    }
                }
                JsonEvent::EndObject => {
                    // We process the extra nodes
                    if !extra_nodes.is_empty() {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            extra_nodes: Vec::new(),
                            types,
                            id,
                            seen_id,
                            active_property,
                            active_context,
                            reverse,
                            in_included,
                        });
                        for (k, v) in extra_nodes {
                            self.convert_event(JsonEvent::ObjectKey(k.into()), results, errors);
                            for e in v {
                                self.convert_event(e, results, errors);
                            }
                        }
                        self.convert_event(JsonEvent::EndObject, results, errors);
                        return;
                    }
                    if let Some(id) = id {
                        if let Some(id) = self.expand_iri(&active_context, id.into(), true, false) {
                            results.push(JsonLdEvent::StartObject);
                            if has_keyword_form(&id) {
                                errors.push(JsonLdSyntaxError::msg(
                                    "@id value must be an IRI or a blank node",
                                ));
                            } else {
                                results.push(JsonLdEvent::Id(id.into()));
                            }
                            results.extend(types.into_iter().map(JsonLdEvent::Type));
                            results.push(JsonLdEvent::EndObject);
                        }
                    } else {
                        results.push(JsonLdEvent::StartObject);
                        results.extend(types.into_iter().map(JsonLdEvent::Type));
                        results.push(JsonLdEvent::EndObject);
                    }
                }
                _ => unreachable!("Inside of an object"),
            },
            JsonLdExpansionState::ObjectType {
                mut types,
                mut new_types,
                id,
                is_array,
                active_property,
                mut active_context,
                from_start,
                reverse,
                in_included,
                nesting,
                extra_nodes,
            } => {
                match event {
                    JsonEvent::Null
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::StartObject => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state.push(JsonLdExpansionState::ObjectType {
                                types,
                                new_types,
                                id,
                                is_array,
                                active_property,
                                active_context,
                                from_start,
                                reverse,
                                in_included,
                                nesting,
                                extra_nodes,
                            });
                        } else {
                            self.state.push(if from_start {
                                JsonLdExpansionState::ObjectStart {
                                    types,
                                    id,
                                    seen_id: false,
                                    active_property,
                                    active_context,
                                    reverse,
                                    in_included,
                                    extra_nodes,
                                }
                            } else {
                                JsonLdExpansionState::Object {
                                    active_context,
                                    in_property: false,
                                    has_emitted_id: id.is_some(),
                                    nesting,
                                }
                            });
                        }
                        if matches!(event, JsonEvent::StartObject) {
                            self.state
                                .push(JsonLdExpansionState::Skip { is_array: false });
                        }
                    }
                    JsonEvent::String(value) => {
                        new_types.push(value.into());
                        if is_array {
                            self.state.push(JsonLdExpansionState::ObjectType {
                                types,
                                new_types,
                                id,
                                is_array,
                                active_property,
                                active_context,
                                from_start,
                                reverse,
                                in_included,
                                nesting,
                                extra_nodes,
                            });
                        } else {
                            (active_context, new_types) =
                                self.map_types(active_context, new_types, errors);
                            if from_start {
                                types.extend(new_types);
                                self.state.push(JsonLdExpansionState::ObjectStart {
                                    types,
                                    id,
                                    seen_id: false,
                                    active_property,
                                    active_context,
                                    reverse,
                                    in_included,
                                    extra_nodes,
                                });
                            } else {
                                results.extend(new_types.into_iter().map(JsonLdEvent::Type));
                                self.state.push(JsonLdExpansionState::Object {
                                    active_context,
                                    in_property: false,
                                    has_emitted_id: id.is_some(),
                                    nesting,
                                });
                            }
                        }
                    }
                    JsonEvent::StartArray => {
                        self.state.push(JsonLdExpansionState::ObjectType {
                            types,
                            new_types,
                            id,
                            is_array: true,
                            active_property,
                            active_context,
                            from_start,
                            reverse,
                            in_included,
                            nesting,
                            extra_nodes,
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
                        (active_context, new_types) =
                            self.map_types(active_context, new_types, errors);
                        if from_start {
                            types.extend(new_types);
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types,
                                id,
                                seen_id: false,
                                active_property,
                                active_context,
                                reverse,
                                in_included,
                                extra_nodes,
                            });
                        } else {
                            results.extend(new_types.into_iter().map(JsonLdEvent::Type));
                            self.state.push(JsonLdExpansionState::Object {
                                active_context,
                                in_property: false,
                                has_emitted_id: id.is_some(),
                                nesting,
                            });
                        }
                    }
                    JsonEvent::ObjectKey(_) | JsonEvent::EndObject | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::ObjectId {
                active_context,
                types,
                id,
                from_start,
                reverse,
                nesting,
                extra_nodes,
            } => {
                if let JsonEvent::String(new_id) = event {
                    if from_start {
                        self.state.push(JsonLdExpansionState::ObjectStart {
                            types,
                            id: Some(new_id.into()),
                            seen_id: true,
                            active_property: None,
                            active_context,
                            reverse,
                            in_included: false,
                            extra_nodes,
                        });
                    } else {
                        if let Some(new_id) = self.expand_iri(&active_context, new_id, true, false)
                        {
                            if has_keyword_form(&new_id) {
                                errors.push(JsonLdSyntaxError::msg(
                                    "@id value must be an IRI or a blank node",
                                ));
                            } else {
                                results.push(JsonLdEvent::Id(new_id.into()));
                            }
                        }
                        self.state.push(JsonLdExpansionState::Object {
                            active_context,
                            in_property: false,
                            has_emitted_id: true,
                            nesting,
                        })
                    }
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@id value must be a string",
                        JsonLdErrorCode::InvalidIdValue,
                    ));
                    self.state.push(if from_start {
                        JsonLdExpansionState::ObjectStart {
                            extra_nodes,
                            types,
                            id,
                            seen_id: true,
                            active_property: None,
                            active_context,
                            reverse,
                            in_included: false,
                        }
                    } else {
                        JsonLdExpansionState::Object {
                            active_context,
                            in_property: false,
                            has_emitted_id: true,
                            nesting,
                        }
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::Object {
                active_context,
                in_property,
                has_emitted_id,
                nesting,
            } => {
                if in_property {
                    results.push(JsonLdEvent::EndProperty);
                }
                match event {
                    JsonEvent::EndObject => {
                        if nesting == 0 {
                            results.push(JsonLdEvent::EndObject);
                        }
                    }
                    JsonEvent::ObjectKey(key) => {
                        if let Some(iri) =
                            self.expand_iri(&active_context, key.as_ref().into(), false, true)
                        {
                            match iri.as_ref() {
                                "@id" => {
                                    if has_emitted_id {
                                        errors.push(JsonLdSyntaxError::msg("Duplicated @id key"));
                                        self.state.push(JsonLdExpansionState::Object {
                                            active_context,
                                            in_property: false,
                                            has_emitted_id: true,
                                            nesting,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ObjectId {
                                            active_context,
                                            types: Vec::new(),
                                            id: None,
                                            from_start: false,
                                            reverse: false,
                                            nesting,
                                            extra_nodes: Vec::new(),
                                        });
                                    }
                                }
                                "@graph" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context: Arc::clone(&active_context),
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state.push(JsonLdExpansionState::Graph);
                                    self.state.push(JsonLdExpansionState::Element {
                                        is_array: false,
                                        active_property: None,
                                        container: &[],
                                        reverse: false,
                                        active_context,
                                        in_included: false,
                                        extra_nodes: Vec::new(),
                                    });
                                    results.push(JsonLdEvent::StartGraph);
                                }
                                "@context" => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@context must be the first key of an object",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ));
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context,
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                "@type" => {
                                    if self.streaming || nesting == 0 {
                                        errors.push(JsonLdSyntaxError::msg_and_code(
                                            "@type must be the first key of an object or right after @context",
                                            JsonLdErrorCode::InvalidStreamingKeyOrder,
                                        ));
                                    }
                                    self.state.push(JsonLdExpansionState::ObjectType {
                                        types: Vec::new(),
                                        new_types: Vec::new(),
                                        id: has_emitted_id.then(String::new),
                                        is_array: false,
                                        active_property: None,
                                        active_context,
                                        from_start: false,
                                        reverse: false,
                                        in_included: false,
                                        nesting,
                                        extra_nodes: Vec::new(),
                                    });
                                }
                                "@index" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context,
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state.push(JsonLdExpansionState::Index);
                                }
                                "@reverse" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context: Arc::clone(&active_context),
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state.push(JsonLdExpansionState::ReverseStart {
                                        active_context,
                                    });
                                }
                                "@included" => {
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context: Arc::clone(&active_context),
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });

                                    if self.context_processor.processing_mode
                                        == JsonLdProcessingMode::JsonLd1_0
                                    {
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        results.push(JsonLdEvent::StartIncluded);
                                        self.state.push(JsonLdExpansionState::Included);
                                        self.state.push(JsonLdExpansionState::Element {
                                            active_property: None,
                                            active_context,
                                            is_array: false,
                                            container: &[],
                                            reverse: false,
                                            in_included: true,
                                            extra_nodes: Vec::new(),
                                        });
                                    }
                                }
                                "@nest" => {
                                    let mut nest_active_context = Arc::clone(&active_context);
                                    if let Some(previous_context) = &active_context.previous_context
                                    {
                                        nest_active_context = Arc::clone(previous_context);
                                    }
                                    if let Some(property_scoped_context) = self.new_scoped_context(
                                        &active_context,
                                        &key,
                                        true,
                                        true,
                                        errors,
                                    ) {
                                        nest_active_context = Arc::new(property_scoped_context);
                                    }
                                    self.state.push(JsonLdExpansionState::NestStart {
                                        active_context: nest_active_context,
                                        parent_active_context: active_context,
                                        has_emitted_id,
                                        nesting,
                                        array_count: 0,
                                    });
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
                                    } else if iri == "@value" && nesting > 0 {
                                        JsonLdSyntaxError::msg_and_code(
                                            "@value is not allowed in @nest",
                                            JsonLdErrorCode::InvalidNestValue,
                                        )
                                    } else {
                                        JsonLdSyntaxError::msg(format!(
                                            "Unsupported JSON-LD keyword: {iri}"
                                        ))
                                    });
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context,
                                        in_property: false,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                _ => {
                                    let (container, reverse) = active_context
                                        .term_definitions
                                        .get(key.as_ref())
                                        .map_or(([].as_slice(), false), |term_definition| {
                                            (
                                                term_definition.container_mapping,
                                                term_definition.reverse_property,
                                            )
                                        });
                                    self.state.push(JsonLdExpansionState::Object {
                                        active_context: Arc::clone(&active_context),
                                        in_property: true,
                                        has_emitted_id,
                                        nesting,
                                    });
                                    self.state.push(JsonLdExpansionState::Element {
                                        active_property: Some(key.clone().into()),
                                        active_context,
                                        is_array: false,
                                        container,
                                        reverse,
                                        in_included: false,
                                        extra_nodes: Vec::new(),
                                    });
                                    results.push(JsonLdEvent::StartProperty {
                                        name: iri.into(),
                                        reverse,
                                    });
                                }
                            }
                        } else {
                            self.state.push(JsonLdExpansionState::Object {
                                active_context,
                                in_property: false,
                                has_emitted_id,
                                nesting,
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
            JsonLdExpansionState::ReverseStart { active_context } => {
                if matches!(event, JsonEvent::StartObject) {
                    self.state.push(JsonLdExpansionState::Reverse {
                        active_context,
                        in_property: false,
                    });
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
            JsonLdExpansionState::Reverse {
                active_context,
                in_property,
            } => {
                if in_property {
                    results.push(JsonLdEvent::EndProperty);
                }
                match event {
                    JsonEvent::EndObject => (),
                    JsonEvent::ObjectKey(key) => {
                        if let Some(iri) =
                            self.expand_iri(&active_context, key.as_ref().into(), false, true)
                        {
                            if has_keyword_form(&iri) {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!(
                                        "@reverse object value cannot contain any keyword, found {iri}",
                                    ),
                                    JsonLdErrorCode::InvalidReversePropertyMap,
                                ));
                                self.state.push(JsonLdExpansionState::Reverse {
                                    active_context,
                                    in_property: false,
                                });
                                self.state
                                    .push(JsonLdExpansionState::Skip { is_array: false });
                            } else {
                                let (container, reverse) = active_context
                                    .term_definitions
                                    .get(key.as_ref())
                                    .map_or(([].as_slice(), false), |term_definition| {
                                        (
                                            term_definition.container_mapping,
                                            term_definition.reverse_property,
                                        )
                                    });
                                let reverse = !reverse; // We are in @reverse
                                self.state.push(JsonLdExpansionState::Reverse {
                                    active_context: Arc::clone(&active_context),
                                    in_property: true,
                                });
                                self.state.push(JsonLdExpansionState::Element {
                                    active_property: Some(key.clone().into()),
                                    active_context,
                                    is_array: false,
                                    container,
                                    reverse,
                                    in_included: false,
                                    extra_nodes: Vec::new(),
                                });
                                results.push(JsonLdEvent::StartProperty {
                                    name: iri.into(),
                                    reverse,
                                });
                            }
                        } else {
                            self.state.push(JsonLdExpansionState::Reverse {
                                active_context,
                                in_property: false,
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
            JsonLdExpansionState::Value {
                active_context,
                r#type,
                value,
                language,
                direction,
            } => {
                match event {
                    JsonEvent::ObjectKey(key) => {
                        if let Some(iri) = self.expand_iri(&active_context, key, false, true) {
                            match iri.as_ref() {
                                "@value" => {
                                    if value.is_some() {
                                        errors.push(JsonLdSyntaxError::msg_and_code(
                                            "@value cannot be set multiple times",
                                            JsonLdErrorCode::InvalidValueObject,
                                        ));
                                        self.state.push(JsonLdExpansionState::Value {
                                            active_context,
                                            r#type,
                                            value,
                                            language,
                                            direction,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ValueValue {
                                            active_context,
                                            r#type,
                                            language,
                                            direction,
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
                                            active_context,
                                            r#type,
                                            value,
                                            language,
                                            direction,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ValueLanguage {
                                            active_context,
                                            r#type,
                                            value,
                                            direction,
                                        });
                                    }
                                }
                                "@direction" => {
                                    if direction.is_some()
                                        || self.context_processor.processing_mode
                                            == JsonLdProcessingMode::JsonLd1_0
                                    {
                                        if direction.is_some() {
                                            errors.push(JsonLdSyntaxError::msg_and_code(
                                                "@direction cannot be set multiple times",
                                                JsonLdErrorCode::CollidingKeywords,
                                            ));
                                        }
                                        self.state.push(JsonLdExpansionState::Value {
                                            active_context,
                                            r#type,
                                            value,
                                            language,
                                            direction,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ValueDirection {
                                            active_context,
                                            r#type,
                                            value,
                                            language,
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
                                            active_context,
                                            r#type,
                                            value,
                                            language,
                                            direction,
                                        });
                                        self.state
                                            .push(JsonLdExpansionState::Skip { is_array: false });
                                    } else {
                                        self.state.push(JsonLdExpansionState::ValueType {
                                            active_context,
                                            value,
                                            language,
                                            direction,
                                        });
                                    }
                                }
                                "@context" => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        "@context must be the first key of an object",
                                        JsonLdErrorCode::InvalidStreamingKeyOrder,
                                    ));
                                    self.state.push(JsonLdExpansionState::Value {
                                        active_context,
                                        r#type,
                                        value,
                                        language,
                                        direction,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                "@index" => {
                                    self.state.push(JsonLdExpansionState::Value {
                                        active_context,
                                        r#type,
                                        value,
                                        language,
                                        direction,
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
                                        active_context,
                                        r#type,
                                        value,
                                        language,
                                        direction,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                                _ => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(format!("Objects with @value cannot contain properties, {iri} found"), JsonLdErrorCode::InvalidValueObject));
                                    self.state.push(JsonLdExpansionState::Value {
                                        active_context,
                                        r#type,
                                        value,
                                        language,
                                        direction,
                                    });
                                    self.state
                                        .push(JsonLdExpansionState::Skip { is_array: false });
                                }
                            }
                        } else {
                            self.state.push(JsonLdExpansionState::Value {
                                active_context,
                                r#type,
                                value,
                                language,
                                direction,
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
                                    direction,
                                })
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
            JsonLdExpansionState::ValueValue {
                active_context,
                r#type,
                language,
                direction,
            } => match event {
                JsonEvent::Null => self.state.push(JsonLdExpansionState::Value {
                    active_context,
                    r#type,
                    value: None,
                    language,
                    direction,
                }),
                JsonEvent::Number(value) => self.state.push(JsonLdExpansionState::Value {
                    active_context,
                    r#type,
                    value: Some(JsonLdValue::Number(value.into())),
                    language,
                    direction,
                }),
                JsonEvent::Boolean(value) => self.state.push(JsonLdExpansionState::Value {
                    active_context,
                    r#type,
                    value: Some(JsonLdValue::Boolean(value)),
                    language,
                    direction,
                }),
                JsonEvent::String(value) => self.state.push(JsonLdExpansionState::Value {
                    active_context,
                    r#type,
                    value: Some(JsonLdValue::String(value.into())),
                    language,
                    direction,
                }),
                _ => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@value value must be a string, number, boolean or null",
                        JsonLdErrorCode::InvalidValueObjectValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type,
                        value: None,
                        language,
                        direction,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            },
            JsonLdExpansionState::ValueLanguage {
                active_context,
                value,
                r#type,
                direction,
            } => {
                if let JsonEvent::String(language) = event {
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type,
                        value,
                        language: Some(language.into()),
                        direction,
                    })
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@language value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type,
                        value,
                        language: None,
                        direction,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::ValueDirection {
                active_context,
                value,
                r#type,
                language,
            } => {
                if let JsonEvent::String(direction) = event {
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type,
                        value,
                        language,
                        direction: match direction.as_ref() {
                            "ltr" => Some("ltr"),
                            "rtl" => Some("rtl"),
                            _ => {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("The allowed @direction values are 'rtl' and 'ltr', found '{direction}'"),
                                    JsonLdErrorCode::InvalidBaseDirection
                                ));
                                None
                            }
                        },
                    })
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@direction value must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type,
                        value,
                        language,
                        direction: None,
                    });
                    self.state
                        .push(JsonLdExpansionState::Skip { is_array: false });
                    self.convert_event(event, results, errors);
                }
            }
            JsonLdExpansionState::ValueType {
                active_context,
                value,
                language,
                direction,
            } => {
                if let JsonEvent::String(t) = event {
                    let mut r#type = self.expand_iri(&active_context, t, true, true);
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
                        active_context,
                        r#type: r#type.map(Into::into),
                        value,
                        language,
                        direction,
                    })
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@type value must be a string when @value is present",
                        JsonLdErrorCode::InvalidTypedValue,
                    ));
                    self.state.push(JsonLdExpansionState::Value {
                        active_context,
                        r#type: None,
                        value,
                        language,
                        direction,
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
                active_context,
                needs_end_object,
                end_event,
            } => {
                if needs_end_object {
                    match event {
                        JsonEvent::EndObject => {
                            results.extend(end_event);
                        }
                        JsonEvent::ObjectKey(key) => {
                            self.state.push(JsonLdExpansionState::ListOrSetContainer {
                                needs_end_object,
                                end_event,
                                active_context: Arc::clone(&active_context),
                            });
                            if let Some(iri) =
                                self.expand_iri(&active_context, key.as_ref().into(), false, true)
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
            JsonLdExpansionState::IndexContainer {
                active_property,
                active_context,
            } => match event {
                JsonEvent::EndObject => (),
                JsonEvent::ObjectKey(key) => {
                    // Events for the @index
                    #[expect(clippy::if_not_else)]
                    let extra_nodes = if key != "@none" {
                        if let Some(index_key) = active_property
                            .as_ref()
                            .and_then(|p| active_context.term_definitions.get(p))
                            .and_then(|d| d.index_mapping.as_ref())
                        {
                            vec![(
                                index_key.clone(),
                                vec![JsonEvent::String(key.into_owned().into())],
                            )]
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };
                    self.state.push(JsonLdExpansionState::IndexContainer {
                        active_context: Arc::clone(&active_context),
                        active_property: active_property.clone(),
                    });
                    self.state.push(JsonLdExpansionState::Element {
                        active_property,
                        active_context,
                        is_array: false,
                        container: &[],
                        reverse: false,
                        in_included: false,
                        extra_nodes,
                    })
                }
                _ => unreachable!(),
            },
            JsonLdExpansionState::LanguageContainer {
                active_context,
                direction,
            } => match event {
                JsonEvent::EndObject => (),
                JsonEvent::ObjectKey(language) => {
                    self.state.push(JsonLdExpansionState::LanguageContainer {
                        active_context: Arc::clone(&active_context),
                        direction,
                    });
                    self.state
                        .push(JsonLdExpansionState::LanguageContainerValue {
                            active_context,
                            language: language.into(),
                            is_array: false,
                            direction,
                        })
                }
                _ => unreachable!(),
            },
            JsonLdExpansionState::LanguageContainerValue {
                active_context,
                language,
                is_array,
                direction,
            } => match event {
                JsonEvent::Null => {
                    if is_array {
                        self.state
                            .push(JsonLdExpansionState::LanguageContainerValue {
                                active_context,
                                language,
                                is_array,
                                direction,
                            });
                    }
                }
                JsonEvent::String(value) => {
                    if is_array {
                        self.state
                            .push(JsonLdExpansionState::LanguageContainerValue {
                                active_context: Arc::clone(&active_context),
                                language: language.clone(),
                                is_array,
                                direction,
                            });
                    }
                    results.push(JsonLdEvent::Value {
                        value: JsonLdValue::String(value.into()),
                        r#type: None,
                        language: (language != "@none"
                            && self.expand_iri(
                                &active_context,
                                language.as_str().into(),
                                false,
                                false,
                            ) != Some("@none".into()))
                        .then_some(language),
                        direction,
                    })
                }
                JsonEvent::StartArray => {
                    self.state
                        .push(JsonLdExpansionState::LanguageContainerValue {
                            active_context,
                            language,
                            is_array: true,
                            direction,
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
                                active_context,
                                language,
                                is_array,
                                direction,
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
            JsonLdExpansionState::Included => {
                results.push(JsonLdEvent::EndIncluded);
                self.convert_event(event, results, errors)
            }
            JsonLdExpansionState::NestStart {
                active_context,
                parent_active_context,
                has_emitted_id,
                nesting,
                array_count,
            } => match event {
                JsonEvent::StartObject => {
                    self.state.push(if array_count > 0 {
                        JsonLdExpansionState::NestStart {
                            active_context: Arc::clone(&active_context),
                            parent_active_context,
                            has_emitted_id,
                            nesting,
                            array_count,
                        }
                    } else {
                        JsonLdExpansionState::Object {
                            active_context: parent_active_context,
                            in_property: false,
                            has_emitted_id,
                            nesting,
                        }
                    });
                    self.state.push(JsonLdExpansionState::Object {
                        active_context,
                        in_property: false,
                        has_emitted_id,
                        nesting: nesting + 1,
                    });
                }
                JsonEvent::StartArray => {
                    self.state.push(JsonLdExpansionState::NestStart {
                        active_context,
                        parent_active_context,
                        has_emitted_id,
                        nesting,
                        array_count: array_count + 1,
                    });
                }
                JsonEvent::EndArray => {
                    self.state.push(if array_count == 1 {
                        JsonLdExpansionState::Object {
                            active_context: parent_active_context,
                            in_property: false,
                            has_emitted_id,
                            nesting,
                        }
                    } else {
                        JsonLdExpansionState::NestStart {
                            active_context,
                            parent_active_context,
                            has_emitted_id,
                            nesting,
                            array_count: array_count - 1,
                        }
                    });
                }
                JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => unreachable!(),
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@nest value must be a JSON object",
                        JsonLdErrorCode::InvalidNestValue,
                    ));
                    self.state.push(if array_count > 0 {
                        JsonLdExpansionState::NestStart {
                            active_context,
                            parent_active_context,
                            has_emitted_id,
                            nesting,
                            array_count,
                        }
                    } else {
                        JsonLdExpansionState::Object {
                            active_context: parent_active_context,
                            in_property: false,
                            has_emitted_id,
                            nesting,
                        }
                    });
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
    ///
    /// `local context` is always `null`
    ///
    /// Warning: take care of synchronizing this implementation with the full one in [`JsonLdContextProcessor`].
    fn expand_iri<'a>(
        &self,
        active_context: &JsonLdContext,
        value: Cow<'a, str>,
        document_relative: bool,
        vocab: bool,
    ) -> Option<Cow<'a, str>> {
        if has_keyword_form(&value) {
            // 1)
            return is_keyword(&value).then_some(value);
        }
        if let Some(term_definition) = active_context.term_definitions.get(value.as_ref()) {
            if let Some(iri_mapping) = &term_definition.iri_mapping {
                let iri_mapping = iri_mapping.as_ref()?;
                // 4)
                if is_keyword(iri_mapping) {
                    return Some(iri_mapping.clone().into());
                }
                // 5)
                if vocab {
                    return Some(iri_mapping.clone().into());
                }
            }
        }
        // 6.1)
        if let Some((prefix, suffix)) = value.split_once(':') {
            // 6.2)
            if prefix == "_" || suffix.starts_with("//") {
                return Some(value);
            }
            // 6.4)
            if let Some(term_definition) = active_context.term_definitions.get(prefix) {
                if let Some(Some(iri_mapping)) = &term_definition.iri_mapping {
                    if term_definition.prefix_flag {
                        return Some(format!("{iri_mapping}{suffix}").into());
                    }
                }
            }
            // 6.5)
            if Iri::parse(value.as_ref()).is_ok() {
                return Some(value);
            }
        }
        // 7)
        if vocab {
            if let Some(vocabulary_mapping) = &active_context.vocabulary_mapping {
                return Some(format!("{vocabulary_mapping}{value}").into());
            }
        }
        // 8)
        if document_relative {
            if let Some(base_iri) = &active_context.base_iri {
                if self.lenient {
                    return Some(base_iri.resolve_unchecked(&value).into_inner().into());
                } else if let Ok(value) = base_iri.resolve(&value) {
                    return Some(base_iri.resolve_unchecked(&value).into_inner().into());
                }
            }
        }

        Some(value)
    }

    fn map_types(
        &self,
        mut active_context: Arc<JsonLdContext>,
        mut types: Vec<String>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> (Arc<JsonLdContext>, Vec<String>) {
        let typed_scoped_context = Arc::clone(&active_context);
        types.sort();
        let types_to_emit = types
            .into_iter()
            .filter_map(|value| {
                // 11.2)
                if let Some(scoped_context) =
                    self.new_scoped_context(&typed_scoped_context, &value, false, false, errors)
                {
                    active_context = Arc::new(scoped_context);
                }
                // 13.4.4.4)
                let iri = self.expand_iri(&typed_scoped_context, value.into(), true, true)?;
                if has_keyword_form(&iri) {
                    errors.push(JsonLdSyntaxError::msg(format!(
                        "{iri} is not a valid value for @type"
                    )));
                    None
                } else {
                    Some(iri.into())
                }
            })
            .collect();
        (active_context, types_to_emit)
    }

    #[expect(clippy::too_many_arguments)]
    fn on_literal_value(
        &mut self,
        value: JsonLdValue,
        active_context: Arc<JsonLdContext>,
        active_property: Option<String>,
        is_array: bool,
        container: &'static [&'static str],
        reverse: bool,
        in_included: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        if in_included {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "@included values must be node objects, literals are not allowed",
                JsonLdErrorCode::InvalidIncludedValue,
            ))
        }
        if is_array {
            self.state.push(JsonLdExpansionState::Element {
                active_property: active_property.clone(),
                active_context: Arc::clone(&active_context),
                is_array,
                container,
                reverse,
                in_included: false,
                extra_nodes: extra_nodes.clone(),
            });
        } else if container.contains(&"@list") {
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
        if let Some(active_property) = active_property {
            let property_scoped_context =
                self.new_scoped_context(&active_context, &active_property, false, true, errors);
            self.expand_value(
                active_property,
                property_scoped_context.map_or_else(|| active_context, Arc::new),
                value,
                reverse,
                extra_nodes,
                results,
                errors,
            );
        }
        if !is_array {
            if container.contains(&"@list") {
                results.push(JsonLdEvent::EndList);
            } else if container.contains(&"@set") {
                results.push(JsonLdEvent::EndSet);
            }
        }
    }

    /// [Value Expansion](https://www.w3.org/TR/json-ld-api/#value-expansion)
    fn expand_value(
        &mut self,
        active_property: String,
        active_context: Arc<JsonLdContext>,
        value: JsonLdValue,
        reverse: bool,
        extra_nodes: Vec<(String, Vec<JsonEvent<'static>>)>,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        let mut r#type = None;
        let mut language = None;
        let mut direction = None;
        if let Some(term_definition) = active_context.term_definitions.get(&active_property) {
            if let Some(type_mapping) = &term_definition.type_mapping {
                match type_mapping.as_ref() {
                    // 1)
                    "@id" => {
                        if let JsonLdValue::String(value) = value {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types: Vec::new(),
                                id: Some(value),
                                seen_id: false,
                                active_property: Some(active_property),
                                active_context,
                                reverse,
                                in_included: false,
                                extra_nodes,
                            });
                            self.convert_event(JsonEvent::EndObject, results, errors);
                            return;
                        }
                    }
                    // 2)
                    "@vocab" => {
                        if let JsonLdValue::String(value) = value {
                            self.state.push(JsonLdExpansionState::ObjectStart {
                                types: Vec::new(),
                                id: self
                                    .expand_iri(&active_context, value.into(), true, true)
                                    .map(Cow::into_owned),
                                seen_id: false,
                                active_property: Some(active_property),
                                active_context: active_context
                                    .previous_context
                                    .clone()
                                    .unwrap_or(active_context),
                                reverse,
                                in_included: false,
                                extra_nodes,
                            });
                            self.convert_event(JsonEvent::EndObject, results, errors);
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
                direction = term_definition
                    .direction_mapping
                    .unwrap_or(active_context.default_direction);
            }
        } else {
            // 5)
            if matches!(value, JsonLdValue::String(_)) {
                if language.is_none() {
                    language.clone_from(&active_context.default_language);
                }
                if direction.is_none() {
                    direction.clone_from(&active_context.default_direction);
                }
            }
        }
        if reverse {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "Literals are not allowed inside of reverse properties",
                JsonLdErrorCode::InvalidReversePropertyValue,
            ))
        }
        self.state.push(JsonLdExpansionState::Value {
            active_context,
            r#type,
            value: Some(value),
            language,
            direction,
        });
        for (k, v) in extra_nodes {
            self.convert_event(JsonEvent::ObjectKey(k.into()), results, errors);
            for e in v {
                self.convert_event(e, results, errors);
            }
        }
        self.convert_event(JsonEvent::EndObject, results, errors);
    }

    fn new_context(
        &mut self,
        active_context: &JsonLdContext,
        context_content: Vec<JsonEvent<'static>>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Arc<JsonLdContext> {
        Arc::new(self.context_processor.process_context(
            active_context,
            json_node_from_events(context_content.into_iter().map(Ok)).unwrap(),
            self.base_url.as_ref(),
            &mut Vec::new(),
            false,
            true,
            true,
            errors,
        ))
    }

    fn new_scoped_context(
        &self,
        active_context: &JsonLdContext,
        active_property: &str,
        override_protected: bool,
        propagate: bool,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<JsonLdContext> {
        active_context
            .term_definitions
            .get(active_property)
            .and_then(|term_definition| {
                Some(self.context_processor.process_context(
                    active_context,
                    term_definition.context.clone()?,
                    term_definition.base_url.as_ref().or(self.base_url.as_ref()),
                    &mut Vec::new(),
                    override_protected,
                    propagate,
                    true,
                    errors,
                ))
            })
    }

    pub fn active_context(&self) -> &JsonLdContext {
        for state in self.state.iter().rev() {
            match state {
                JsonLdExpansionState::Element { active_context, .. }
                | JsonLdExpansionState::ObjectOrContainerStart { active_context, .. }
                | JsonLdExpansionState::ObjectOrContainerStartStreaming {
                    active_context, ..
                }
                | JsonLdExpansionState::Context { active_context, .. }
                | JsonLdExpansionState::ObjectStartIsSingleIdOrValue { active_context, .. }
                | JsonLdExpansionState::ObjectStart { active_context, .. }
                | JsonLdExpansionState::ObjectType { active_context, .. }
                | JsonLdExpansionState::ObjectId { active_context, .. }
                | JsonLdExpansionState::Object { active_context, .. }
                | JsonLdExpansionState::ReverseStart { active_context, .. }
                | JsonLdExpansionState::Reverse { active_context, .. }
                | JsonLdExpansionState::Value { active_context, .. }
                | JsonLdExpansionState::ValueValue { active_context, .. }
                | JsonLdExpansionState::ValueLanguage { active_context, .. }
                | JsonLdExpansionState::ValueDirection { active_context, .. }
                | JsonLdExpansionState::ValueType { active_context, .. }
                | JsonLdExpansionState::ListOrSetContainer { active_context, .. }
                | JsonLdExpansionState::IndexContainer { active_context, .. }
                | JsonLdExpansionState::LanguageContainer { active_context, .. }
                | JsonLdExpansionState::LanguageContainerValue { active_context, .. }
                | JsonLdExpansionState::NestStart { active_context, .. } => {
                    return active_context;
                }
                JsonLdExpansionState::Index
                | JsonLdExpansionState::Graph
                | JsonLdExpansionState::RootGraph
                | JsonLdExpansionState::Included
                | JsonLdExpansionState::Skip { .. } => (),
            }
        }
        &self.root_context
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
