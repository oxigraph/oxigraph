use crate::error::JsonLdErrorCode;
use crate::JsonLdSyntaxError;
use oxiri::Iri;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum JsonLdProcessingMode {
    JsonLd1_0,
    // TODO JsonLd1_1,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum JsonNode {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
    Array(Vec<JsonNode>),
    Map(HashMap<String, JsonNode>),
}

#[derive(Default, Clone)]
pub struct JsonLdContext {
    pub base_iri: Option<Iri<String>>,
    pub original_base_url: Option<Iri<String>>,
    pub vocabulary_mapping: Option<String>,
    pub default_language: Option<String>,
    pub term_definitions: HashMap<String, JsonLdTermDefinition>,
    pub previous_context: Option<Box<JsonLdContext>>,
}

impl JsonLdContext {
    pub fn new_empty(original_base_url: Option<Iri<String>>) -> Self {
        JsonLdContext {
            base_iri: original_base_url.clone(),
            original_base_url,
            vocabulary_mapping: None,
            default_language: None,
            term_definitions: HashMap::new(),
            previous_context: None,
        }
    }
}

#[derive(Clone)]
pub struct JsonLdTermDefinition {
    pub iri_mapping: Option<String>,
    pub prefix_flag: bool,
    pub protected: bool,
    pub reverse_property: bool,
    pub base_url: Option<Iri<String>>,
    pub language_mapping: Option<String>,
    pub type_mapping: Option<String>,
}

/// [Context Processing Algorithm](https://www.w3.org/TR/json-ld-api/#algorithm)
pub fn process_context(
    active_context: &JsonLdContext,
    local_context: JsonNode,
    base_url: Option<&Iri<String>>,
    remote_contexts: &mut Vec<String>,
    override_protected: bool,
    mut propagate: bool,
    processing_mode: JsonLdProcessingMode,
    lenient: bool, // Custom option to ignore invalid base IRIs
    errors: &mut Vec<JsonLdSyntaxError>,
) -> JsonLdContext {
    // 1)
    let mut result = active_context.clone();
    // 2)
    if let JsonNode::Map(local_context) = &local_context {
        if let Some(propagate_node) = local_context.get("@propagate") {
            if let JsonNode::Boolean(new) = propagate_node {
                propagate = *new;
            } else {
                errors.push(JsonLdSyntaxError::msg("@propagate value must be a boolean"))
            }
        }
    }
    // 3)
    if !propagate && result.previous_context.is_none() {
        result.previous_context = Some(Box::new(active_context.clone()));
    }
    // 4)
    let local_context = if let JsonNode::Array(c) = local_context {
        c
    } else {
        vec![local_context]
    };
    // 5)
    for context in local_context {
        let context = match context {
            // 5.1)
            JsonNode::Null => {
                // 5.1.1)
                if !override_protected {
                    for (name, def) in &active_context.term_definitions {
                        if def.protected {
                            errors.push(JsonLdSyntaxError::msg_and_code(format!("Definition of {name} will be overridden even if it's protected"), JsonLdErrorCode::InvalidContextNullification));
                        }
                    }
                }
                // 5.1.2)
                result = JsonLdContext::new_empty(active_context.original_base_url.clone());
                // 5.1.3)
                continue;
            }
            // 5.2)
            JsonNode::String(_) => {
                errors.push(JsonLdSyntaxError::msg(
                    "Loading remote contexts is not implemented yet",
                ));
                continue; // TODO
            }
            // 5.3)
            JsonNode::Array(_) | JsonNode::Number(_) | JsonNode::Boolean(_) => {
                errors.push(JsonLdSyntaxError::msg_and_code(
                    "@context value must be null, a string or an object",
                    JsonLdErrorCode::InvalidLocalContext,
                ));
                continue;
            }
            // 5.4)
            JsonNode::Map(context) => context,
        };
        let mut key_values = HashMap::new();
        let mut protected = false;
        for (key, value) in context {
            match key.as_str() {
                // 5.5)
                "@version" => {
                    // 5.5.1)
                    if let JsonNode::Number(version) = value {
                        if version != "1.1" {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                format!(
                                    "The only supported @version value is 1.1, found {version}"
                                ),
                                JsonLdErrorCode::InvalidVersionValue,
                            ));
                        }
                    } else {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@version value must be a number",
                            JsonLdErrorCode::InvalidVersionValue,
                        ));
                    }
                    // 5.5.2)
                    if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@version is only supported in JSON-LD 1.1",
                            JsonLdErrorCode::ProcessingModeConflict,
                        ));
                    }
                }
                // 5.6)
                "@import" => {
                    // 5.6.1)
                    if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@import is only supported in JSON-LD 1.1",
                            JsonLdErrorCode::InvalidContextEntry,
                        ));
                        continue;
                    }
                    unimplemented!()
                }
                // 5.7)
                "@base" => {
                    if remote_contexts.is_empty() {
                        match value {
                            // 5.7.2)
                            JsonNode::Null => {
                                result.base_iri = None;
                            }
                            // 5.7.3) and 5.7.4)
                            JsonNode::String(value) => {
                                if lenient {
                                    result.base_iri =
                                        Some(if let Some(base_iri) = &result.base_iri {
                                            base_iri.resolve_unchecked(&value)
                                        } else {
                                            Iri::parse_unchecked(value.clone())
                                        })
                                } else {
                                    match if let Some(base_iri) = &result.base_iri {
                                        base_iri.resolve(&value)
                                    } else {
                                        Iri::parse(value.clone())
                                    } {
                                        Ok(iri) => result.base_iri = Some(iri),
                                        Err(e) => errors.push(JsonLdSyntaxError::msg_and_code(
                                            format!("Invalid @base '{value}': {e}"),
                                            JsonLdErrorCode::InvalidBaseIri,
                                        )),
                                    }
                                }
                            }
                            _ => errors.push(JsonLdSyntaxError::msg_and_code(
                                "@base value must be a string",
                                JsonLdErrorCode::InvalidBaseIri,
                            )),
                        }
                    }
                }
                // 5.8)
                "@vocab" => {
                    match value {
                        // 5.8.2)
                        JsonNode::Null => {
                            result.vocabulary_mapping = None;
                        }
                        // 5.8.3)
                        JsonNode::String(value) => {
                            // TODO: validate blank node?
                            if value.starts_with("_:") || lenient {
                                result.vocabulary_mapping = Some(value);
                            } else {
                                match Iri::parse(value.as_str()) {
                                    Ok(_) => result.vocabulary_mapping = Some(value),
                                    Err(e) => errors.push(JsonLdSyntaxError::msg_and_code(
                                        format!("Invalid @vocab '{value}': {e}"),
                                        JsonLdErrorCode::InvalidVocabMapping,
                                    )),
                                }
                            }
                        }
                        _ => errors.push(JsonLdSyntaxError::msg_and_code(
                            "@vocab value must be a string",
                            JsonLdErrorCode::InvalidVocabMapping,
                        )),
                    }
                }
                // 5.9)
                "@language" => {
                    match value {
                        // 5.9.2)
                        JsonNode::Null => {
                            result.default_language = None;
                        }
                        // 5.9.3)
                        JsonNode::String(value) => result.default_language = Some(value),
                        _ => errors.push(JsonLdSyntaxError::msg_and_code(
                            "@language value must be a string",
                            JsonLdErrorCode::InvalidDefaultLanguage,
                        )),
                    }
                }
                // 5.10)
                "@direction" => {
                    // 5.10.1)
                    if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@direction is only supported in JSON-LD 1.1",
                            JsonLdErrorCode::InvalidContextEntry,
                        ));
                        continue;
                    }
                    unimplemented!()
                }
                // 5.11)
                "@propagate" => {
                    // 5.10.1)
                    if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@propagate is only supported in JSON-LD 1.1",
                            JsonLdErrorCode::InvalidContextEntry,
                        ));
                        continue;
                    }
                    unimplemented!()
                }
                // 5.13)
                "@protected" => {
                    if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@protected is only supported in JSON-LD 1.1",
                            JsonLdErrorCode::InvalidContextEntry,
                        ));
                    }
                    match value {
                        JsonNode::Boolean(value) => protected = value,
                        _ => errors.push(JsonLdSyntaxError::msg_and_code(
                            "@protected value must be a boolean",
                            JsonLdErrorCode::InvalidProtectedValue,
                        )),
                    }
                }
                _ => {
                    key_values.insert(key, value);
                }
            }
        }
        let mut defined = HashMap::new();
        for term in key_values.keys() {
            create_term_definition(
                &mut result,
                &key_values,
                term,
                &mut defined,
                base_url,
                protected,
                override_protected,
                remote_contexts,
                true,
                processing_mode,
                lenient,
                errors,
            )
        }
    }
    // 6)
    result
}

/// [Create Term Definition](https://www.w3.org/TR/json-ld-api/#create-term-definition)
fn create_term_definition(
    active_context: &mut JsonLdContext,
    local_context: &HashMap<String, JsonNode>,
    term: &str,
    defined: &mut HashMap<String, bool>,
    base_url: Option<&Iri<String>>,
    protected: bool,
    override_protected: bool,
    remote_contexts: &mut Vec<String>,
    validate_scoped_context: bool,
    processing_mode: JsonLdProcessingMode,
    lenient: bool, // Custom option to ignore invalid base IRIs
    errors: &mut Vec<JsonLdSyntaxError>,
) {
    // 1)
    if let Some(defined_value) = defined.get(term) {
        if !defined_value {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "Cyclic IRI mapping, ignoring",
                JsonLdErrorCode::CyclicIriMapping,
            ))
        }
        return;
    }
    // 2)
    if term.is_empty() {
        errors.push(JsonLdSyntaxError::msg_and_code(
            "@context terms must not be the empty strings",
            JsonLdErrorCode::InvalidTermDefinition,
        ));
        return;
    }
    defined.insert(term.into(), false);
    // 4)
    if term == "@type" {
        if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "@type keyword can't be redefined in JSON-LD 1.0 @context",
                JsonLdErrorCode::KeywordRedefinition,
            ));
            return;
        }
        unimplemented!()
    } else if has_keyword_form(&term) {
        // 5)
        errors.push(JsonLdSyntaxError::msg_and_code(
            format!("{term} keyword can't be redefined in @context"),
            JsonLdErrorCode::KeywordRedefinition,
        ));
        return;
    }
    // 6)
    let previous_definition = active_context.term_definitions.remove(term);
    let (value, mut simple_term) = match local_context.get(term) {
        // 7)
        Some(JsonNode::Null) => (
            Cow::Owned([("@id".to_owned(), JsonNode::Null)].into()),
            true,
        ), // TODO: undefined
        // 8)
        Some(JsonNode::String(id)) => (
            Cow::Owned([("@id".to_owned(), JsonNode::String(id.clone()))].into()),
            true,
        ),
        // 9)
        Some(JsonNode::Map(map)) => (Cow::Borrowed(map), false),
        _ => {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "Term definition value must be null, a string or a map",
                JsonLdErrorCode::InvalidTermDefinition,
            ));
            return;
        }
    };
    // 10)
    let mut definition = JsonLdTermDefinition {
        iri_mapping: None,
        prefix_flag: false,
        protected,
        reverse_property: false,
        base_url: None,
        language_mapping: None,
        type_mapping: None,
    };
    let mut found_id = false;
    for (key, value) in value.as_ref() {
        match key.as_str() {
            // 11)
            "@protected" => {
                if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@protected keyword can't be used in JSON-LD 1.0 @context",
                        JsonLdErrorCode::InvalidTermDefinition,
                    ));
                    continue;
                }
                unimplemented!()
            }
            // 12)
            "@type" => {
                // 22) moved
                if definition.language_mapping.is_some() {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "Both @language and @type can't be set at the same time",
                        JsonLdErrorCode::InvalidLanguageMapping,
                    ));
                }
                // 12.1)
                let JsonNode::String(r#type) = value else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "The value of @type in a context must be a string",
                        JsonLdErrorCode::InvalidTypeMapping,
                    ));
                    continue;
                };
                // 12.2)
                let Some(r#type) = expand_iri(
                    active_context,
                    r#type.as_str().into(),
                    false,
                    true,
                    Some(local_context),
                    defined,
                    processing_mode,
                    lenient,
                    errors,
                ) else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("Invalid @type value in @context: {type}"),
                        JsonLdErrorCode::InvalidTypeMapping,
                    ));
                    continue;
                };
                // 12.3)
                if matches!(r#type.as_ref(), "@json" | "@none")
                    && processing_mode == JsonLdProcessingMode::JsonLd1_0
                {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("@type value {type} in a context is only supported in JSON-LD 1.1"),
                        JsonLdErrorCode::InvalidTypeMapping,
                    ));
                }
                // 12.4)
                if has_keyword_form(&r#type)
                    && !matches!(r#type.as_ref(), "@id" | "@json" | "@none" | "@vocab")
                {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("Invalid @type value in @context: {type}"),
                        JsonLdErrorCode::InvalidTypeMapping,
                    ));
                }
                // 12.5)
                definition.type_mapping = Some(r#type.into());
            }
            // 13)
            "@reverse" => {
                errors.push(JsonLdSyntaxError::msg("@reverse is not implemented yet"));
                // TODO
            }
            // 14)
            "@id" => {
                match value {
                    // 14.1)
                    JsonNode::Null => {
                        found_id = true;
                    }
                    JsonNode::String(id) => {
                        if id == term {
                            continue;
                        }
                        found_id = true;
                        // 14.2.2)
                        if has_keyword_form(&id) {
                            continue;
                        }
                        // 14.2.3)
                        definition.iri_mapping = expand_iri(
                            active_context,
                            id.into(),
                            false,
                            true,
                            Some(local_context),
                            defined,
                            processing_mode,
                            lenient,
                            errors,
                        )
                        .map(Into::into);
                        // 14.2.4)
                        if term
                            .as_bytes()
                            .get(1..term.len() - 1)
                            .is_some_and(|t| t.contains(&b':'))
                            || term.contains('/')
                        {
                            // 14.2.4.1)
                            defined.insert(term.into(), true);
                            let expended_term = expand_iri(
                                active_context,
                                term.into(),
                                false,
                                true,
                                Some(local_context),
                                defined,
                                processing_mode,
                                lenient,
                                errors,
                            );
                            // 14.2.4.2)
                            if expended_term.as_deref() != definition.iri_mapping.as_deref() {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("Inconsistent expansion of {term}"),
                                    JsonLdErrorCode::InvalidIriMapping,
                                ))
                            }
                        }
                        // 14.2.5)
                        if !term.contains(':') && !term.contains('/') {
                            simple_term = true;
                            if definition.iri_mapping.as_deref().is_some_and(|iri| {
                                iri.ends_with(|c| {
                                    matches!(c, ':' | '/' | '?' | '#' | '[' | ']' | '@')
                                }) || iri.starts_with("_:")
                            }) {
                                definition.prefix_flag = true;
                            }
                        }
                    }
                    // 14.2.1)
                    _ => {
                        found_id = true;
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@id value must be a string",
                            JsonLdErrorCode::InvalidIriMapping,
                        ))
                    }
                }
            }
            // 19)
            "@container" => {
                errors.push(JsonLdSyntaxError::msg("@container is not implemented yet"));
                // TODO
            }
            // 20)
            "@index" => {
                errors.push(JsonLdSyntaxError::msg("@index is not implemented yet"));
                // TODO
            }
            // 21)
            "@context" => {
                errors.push(JsonLdSyntaxError::msg(
                    "@context in context is not implemented yet",
                )); // TODO
            }
            // 22)
            "@language" => {
                if definition.type_mapping.is_some() {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "Both @language and @type can't be set at the same time",
                        JsonLdErrorCode::InvalidLanguageMapping,
                    ));
                }
                definition.language_mapping = match value {
                    JsonNode::String(language) => Some(language.clone()),
                    JsonNode::Null => None, // TODO: Some(None)?
                    _ => {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@language value must be a string or null",
                            JsonLdErrorCode::InvalidLanguageMapping,
                        ));
                        continue;
                    }
                }
            }
            // 23)
            "@direction" => {
                // 23.1)
                if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@direction is only supported in JSON-LD 1.1",
                        JsonLdErrorCode::InvalidTermDefinition,
                    ));
                    continue;
                }
                unimplemented!()
            }
            // 24)
            "@nest" => {
                // 24.1)
                if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@nest is only supported in JSON-LD 1.1",
                        JsonLdErrorCode::InvalidTermDefinition,
                    ));
                    continue;
                }
                unimplemented!()
            }
            // 25)
            "@prefix" => {
                // 25.1)
                if processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        "@direction is only supported in JSON-LD 1.1",
                        JsonLdErrorCode::InvalidTermDefinition,
                    ));
                    continue;
                }
                unimplemented!()
            }
            // 26)
            _ => errors.push(JsonLdSyntaxError::msg_and_code(
                format!("Unexpected key in term definition '{key}'"),
                JsonLdErrorCode::InvalidTermDefinition,
            )),
        }
    }
    if !found_id {
        if let Some((prefix, suffix)) = term.split_once(':').and_then(|(prefix, suffix)| {
            if prefix.is_empty() {
                // We ignore the empty prefixes
                suffix.split_once(':')
            } else {
                Some((prefix, suffix))
            }
        }) {
            // 15)
            if local_context.contains_key(prefix) {
                // 15.1)
                create_term_definition(
                    active_context,
                    local_context,
                    prefix,
                    defined,
                    base_url,
                    false,
                    false,
                    remote_contexts,
                    false,
                    processing_mode,
                    lenient, // Custom option to ignore invalid base IRIs
                    errors,
                )
            }
            if let Some(term_definition) = active_context.term_definitions.get(prefix) {
                // 15.2)
                if let Some(iri_mapping) = &term_definition.iri_mapping {
                    definition.iri_mapping = Some(format!("{iri_mapping}{suffix}"));
                } else {
                    errors.push(JsonLdSyntaxError::msg(format!(
                        "The prefix '{prefix}' is not associated with an IRI in the context"
                    )));
                }
            } else {
                // 15.3)
                definition.iri_mapping = Some(term.into());
            }
        } else if term.contains('/') {
            // 16)
            if let Some(iri) = expand_iri(
                active_context,
                term.into(),
                false,
                true,
                Some(local_context),
                defined,
                processing_mode,
                lenient,
                errors,
            ) {
                if has_keyword_form(&iri) {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("Context term @id is not allowed to be a keyword, {iri} found"),
                        JsonLdErrorCode::InvalidIriMapping,
                    ))
                } else if iri.starts_with("_:") {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("Context term @id is not allowed to be a blank node, {iri} found"),
                        JsonLdErrorCode::InvalidIriMapping,
                    ))
                } else {
                    definition.iri_mapping = Some(iri.into());
                }
            }
        } else if term == "@type" {
            // 17)
            definition.iri_mapping = Some("@type".into());
        } else {
            // 18)
            if let Some(vocabulary_mapping) = &active_context.vocabulary_mapping {
                definition.iri_mapping = Some(format!("{vocabulary_mapping}{term}"));
            } else {
                errors.push(JsonLdSyntaxError::msg_and_code(
                    format!("No @vocab key to build an IRI from context {term} term definition"),
                    JsonLdErrorCode::InvalidIriMapping,
                ))
            }
        }
    }
    // 27)
    if !override_protected {
        if let Some(previous_definition) = previous_definition {
            if previous_definition.protected {
                // 27.1)
                if definition.prefix_flag != previous_definition.prefix_flag
                    || definition.reverse_property != previous_definition.reverse_property
                    || definition.iri_mapping != previous_definition.iri_mapping
                    || definition.base_url != previous_definition.base_url
                {
                    // TODO: make sure it's full
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!("Overriding the protected term {term}"),
                        JsonLdErrorCode::ProtectedTermRedefinition,
                    ));
                }
                // 27.2)
                definition = previous_definition;
            }
        }
    }
    // 28)
    active_context
        .term_definitions
        .insert(term.into(), definition);
    defined.insert(term.into(), true);
}

pub fn has_keyword_form(value: &str) -> bool {
    value
        .strip_prefix('@')
        .is_some_and(|suffix| suffix.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// [IRI Expansion](https://www.w3.org/TR/json-ld-api/#iri-expansion)
pub fn expand_iri<'a>(
    active_context: &mut JsonLdContext,
    value: Cow<'a, str>,
    document_relative: bool,
    vocab: bool,
    local_context: Option<&HashMap<String, JsonNode>>,
    defined: &mut HashMap<String, bool>,
    processing_mode: JsonLdProcessingMode,
    lenient: bool,
    errors: &mut Vec<JsonLdSyntaxError>,
) -> Option<Cow<'a, str>> {
    if let Some(suffix) = value.strip_prefix('@') {
        // 1)
        match suffix {
            "base" => return Some("@base".into()),
            "container" => return Some("@container".into()),
            "context" => return Some("@context".into()),
            "direction" => return Some("@direction".into()),
            "graph" => return Some("@graph".into()),
            "id" => return Some("@id".into()),
            "import" => return Some("@import".into()),
            "included" => return Some("@included".into()),
            "index" => return Some("@index".into()),
            "json" => return Some("@json".into()),
            "language" => return Some("@language".into()),
            "list" => return Some("@list".into()),
            "nest" => return Some("@nest".into()),
            "none" => return Some("@none".into()),
            "prefix" => return Some("@prefix".into()),
            "propagate" => return Some("@propagate".into()),
            "protected" => return Some("@protected".into()),
            "reverse" => return Some("@reverse".into()),
            "set" => return Some("@set".into()),
            "type" => return Some("@type".into()),
            "value" => return Some("@value".into()),
            "version" => return Some("@version".into()),
            "vocab" => return Some("@vocab".into()),
            _ if has_keyword_form(&value) => {
                // 2)
                return None;
            }
            _ => (),
        }
    }
    // 3)
    if let Some(local_context) = local_context {
        if local_context.contains_key(value.as_ref()) && defined.get(value.as_ref()) != Some(&true)
        {
            create_term_definition(
                active_context,
                local_context,
                &value,
                defined,
                None,
                false,
                false,
                &mut Vec::new(),
                false,
                processing_mode,
                lenient, // Custom option to ignore invalid base IRIs
                errors,
            )
        }
    }
    if let Some(term_definition) = active_context.term_definitions.get(value.as_ref()) {
        if let Some(iri_mapping) = &term_definition.iri_mapping {
            // 4)
            if let Some(keyword) = iri_mapping.strip_prefix('@') {
                return Some(keyword.to_owned().into());
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
        // 6.3)
        if let Some(local_context) = local_context {
            if local_context.contains_key(prefix) && defined.get(prefix) != Some(&true) {
                create_term_definition(
                    active_context,
                    local_context,
                    prefix,
                    defined,
                    None,
                    false,
                    false,
                    &mut Vec::new(),
                    false,
                    processing_mode,
                    lenient, // Custom option to ignore invalid base IRIs
                    errors,
                )
            }
        }
        // 6.4)
        if let Some(term_definition) = active_context.term_definitions.get(prefix) {
            if let Some(iri_mapping) = &term_definition.iri_mapping {
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
            if lenient {
                return Some(base_iri.resolve_unchecked(&value).into_inner().into());
            } else if let Ok(value) = base_iri.resolve(&value) {
                return Some(base_iri.resolve_unchecked(&value).into_inner().into());
            }
        }
    }

    Some(value)
}
