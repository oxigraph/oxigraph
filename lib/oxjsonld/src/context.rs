#![allow(clippy::unimplemented)] // TODO: remove me after implementing JSON-LD 1.1
use crate::error::{JsonLdErrorCode, JsonLdSyntaxError};
use crate::{JsonLdProfile, JsonLdProfileSet};
use json_event_parser::{JsonEvent, JsonSyntaxError, SliceJsonParser};
use oxiri::Iri;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::slice;
use std::sync::{Arc, Mutex};

type LoadDocumentCallback = dyn Fn(
        &str,
        &JsonLdLoadDocumentOptions,
    ) -> Result<JsonLdRemoteDocument, Box<dyn Error + Send + Sync>>
    + Send
    + Sync
    + UnwindSafe
    + RefUnwindSafe;

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
    Object(HashMap<String, JsonNode>),
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
    // In the fields, None is unset Some(None) is set to null
    pub iri_mapping: Option<Option<String>>,
    pub prefix_flag: bool,
    pub protected: bool,
    pub reverse_property: bool,
    pub base_url: Option<Iri<String>>,
    pub container_mapping: &'static [&'static str],
    pub language_mapping: Option<Option<String>>,
    pub type_mapping: Option<String>,
}

pub struct JsonLdContextProcessor {
    pub processing_mode: JsonLdProcessingMode,
    pub lenient: bool, // Custom option to ignore invalid base IRIs
    pub max_context_recursion: usize,
    pub remote_context_cache: Arc<Mutex<HashMap<String, (Option<Iri<String>>, JsonNode)>>>,
    pub load_document_callback: Option<Arc<LoadDocumentCallback>>,
}

/// Used to pass various options to the LoadDocumentCallback.
pub struct JsonLdLoadDocumentOptions {
    /// One or more IRIs to use in the request as a profile parameter.
    pub request_profile: JsonLdProfileSet,
}

/// Returned information about a remote JSON-LD document or context.
pub struct JsonLdRemoteDocument {
    /// The retrieved document
    pub document: Vec<u8>,
    /// The final URL of the loaded document. This is important to handle HTTP redirects properly
    pub document_url: String,
}

impl JsonLdContextProcessor {
    /// [Context Processing Algorithm](https://www.w3.org/TR/json-ld-api/#algorithm)
    pub fn process_context(
        &self,
        active_context: &JsonLdContext,
        local_context: JsonNode,
        base_url: Option<&Iri<String>>,
        remote_contexts: &mut Vec<String>,
        override_protected: bool,
        mut propagate: bool,
        validate_scoped_context: bool,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> JsonLdContext {
        // 1)
        let mut result = active_context.clone();
        // 2)
        if let JsonNode::Object(local_context) = &local_context {
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
                JsonNode::String(context) => {
                    // 5.2.1)
                    let context = match if let Some(base_url) = base_url {
                        base_url.resolve(&context)
                    } else {
                        Iri::parse(context.clone())
                    } {
                        Ok(url) => url.into_inner(),
                        Err(e) => {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                format!("Invalid remote context URL '{context}': {e}"),
                                JsonLdErrorCode::LoadingDocumentFailed,
                            ));
                            continue;
                        }
                    };
                    // 5.2.2)
                    if !validate_scoped_context && remote_contexts.contains(&context) {
                        continue;
                    }
                    // 5.2.3)
                    if remote_contexts.len() >= self.max_context_recursion {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!(
                                "This processor only allows {} remote context, threshold exceeded",
                                self.max_context_recursion
                            ),
                            JsonLdErrorCode::ContextOverflow,
                        ));
                        continue;
                    }
                    remote_contexts.push(context.clone());
                    let mut remote_context_cache = self.remote_context_cache.lock().unwrap(); // TODO: nest when targeting rust 2024
                    let (loaded_context_base, loaded_context_content) =
                        if let Some(loaded_context) = remote_context_cache.get(&context) {
                            // 5.2.4)
                            loaded_context.clone()
                        } else {
                            // 5.2.5)
                            let Some(load_document_callback) = &self.load_document_callback else {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    "No LoadDocumentCallback has been set to load remote contexts",
                                    JsonLdErrorCode::LoadingRemoteContextFailed,
                                ));
                                continue;
                            };
                            let context_document = match load_document_callback(
                                &context,
                                &JsonLdLoadDocumentOptions {
                                    request_profile: JsonLdProfile::Context.into(),
                                },
                            ) {
                                Ok(document) => document,
                                Err(e) => {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        format!("Failed to load remote context {context}: {e}"),
                                        JsonLdErrorCode::LoadingRemoteContextFailed,
                                    ));
                                    continue;
                                }
                            };
                            let parsed_document =
                                match json_slice_to_node(&context_document.document) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        errors.push(JsonLdSyntaxError::msg_and_code(
                                            format!(
                                                "Failed to parse remote context {context}: {e}"
                                            ),
                                            JsonLdErrorCode::LoadingRemoteContextFailed,
                                        ));
                                        continue;
                                    }
                                };
                            let JsonNode::Object(parsed_document) = parsed_document else {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("Remote context {context} must be a map"),
                                    JsonLdErrorCode::InvalidRemoteContext,
                                ));
                                continue;
                            };
                            let Some(loaded_context) = parsed_document
                                .into_iter()
                                .find_map(|(k, v)| (k == "@context").then_some(v))
                            else {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!(
                                        "Remote context {context} must be contain a @context key"
                                    ),
                                    JsonLdErrorCode::InvalidRemoteContext,
                                ));
                                continue;
                            };
                            let document_url = Iri::parse(context_document.document_url).ok();
                            remote_context_cache.insert(
                                context.clone(),
                                (document_url.clone(), loaded_context.clone()),
                            );
                            (document_url, loaded_context)
                        };
                    // 5.2.6)
                    result = self.process_context(
                        &result,
                        loaded_context_content,
                        loaded_context_base.as_ref(),
                        remote_contexts,
                        false,
                        true,
                        validate_scoped_context,
                        errors,
                    );
                    assert_eq!(
                        remote_contexts.pop(),
                        Some(context),
                        "The remote context stack is invalid"
                    );
                    continue;
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
                JsonNode::Object(context) => context,
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
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@version is only supported in JSON-LD 1.1",
                                JsonLdErrorCode::ProcessingModeConflict,
                            ));
                        }
                    }
                    // 5.6)
                    "@import" => {
                        // 5.6.1)
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                                    if self.lenient {
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
                                if value.starts_with("_:") || self.lenient {
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
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                self.create_term_definition(
                    &mut result,
                    &key_values,
                    term,
                    &mut defined,
                    base_url,
                    protected,
                    override_protected,
                    remote_contexts,
                    errors,
                )
            }
        }
        // 6)
        result
    }

    /// [Create Term Definition](https://www.w3.org/TR/json-ld-api/#create-term-definition)
    #[allow(clippy::only_used_in_recursion)] // TODO: params will be useful for term-specific contexts
    fn create_term_definition(
        &self,
        active_context: &mut JsonLdContext,
        local_context: &HashMap<String, JsonNode>,
        term: &str,
        defined: &mut HashMap<String, bool>,
        base_url: Option<&Iri<String>>,
        protected: bool,
        override_protected: bool,
        remote_contexts: &mut Vec<String>,
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
            if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                errors.push(JsonLdSyntaxError::msg_and_code(
                    "@type keyword can't be redefined in JSON-LD 1.0 @context",
                    JsonLdErrorCode::KeywordRedefinition,
                ));
                return;
            }
            unimplemented!()
        } else if has_keyword_form(term) {
            // 5)
            if is_keyword(term) {
                errors.push(JsonLdSyntaxError::msg_and_code(
                    format!("{term} keyword can't be redefined in context"),
                    JsonLdErrorCode::KeywordRedefinition,
                ));
            }
            return;
        }
        // 6)
        let previous_definition = active_context.term_definitions.remove(term);
        let value = match local_context.get(term) {
            // 7)
            Some(JsonNode::Null) => Cow::Owned([("@id".to_owned(), JsonNode::Null)].into()),
            // 8)
            Some(JsonNode::String(id)) => {
                Cow::Owned([("@id".to_owned(), JsonNode::String(id.clone()))].into())
            }
            // 9)
            Some(JsonNode::Object(map)) => Cow::Borrowed(map),
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
            container_mapping: &[],
            language_mapping: None,
            type_mapping: None,
        };
        for (key, key_value) in value.as_ref() {
            match key.as_str() {
                // 11)
                "@protected" => {
                    if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                    // 12.1)
                    let JsonNode::String(r#type) = key_value else {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "The value of @type in a context must be a string",
                            JsonLdErrorCode::InvalidTypeMapping,
                        ));
                        continue;
                    };
                    // 12.2)
                    let Some(r#type) = self.expand_iri(
                        active_context,
                        r#type.as_str().into(),
                        false,
                        true,
                        Some(local_context),
                        defined,
                        errors,
                    ) else {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!("Invalid @type value in context: {type}"),
                            JsonLdErrorCode::InvalidTypeMapping,
                        ));
                        continue;
                    };
                    // 12.3)
                    if matches!(r#type.as_ref(), "@json" | "@none")
                        && self.processing_mode == JsonLdProcessingMode::JsonLd1_0
                    {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!(
                                "@type value {type} in a context is only supported in JSON-LD 1.1"
                            ),
                            JsonLdErrorCode::InvalidTypeMapping,
                        ));
                    }
                    // 12.4)
                    let is_keyword = has_keyword_form(&r#type);
                    if is_keyword
                        && !matches!(r#type.as_ref(), "@id" | "@json" | "@none" | "@vocab")
                        || r#type.starts_with("_:")
                    {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!("Invalid @type value in context: {type}"),
                            JsonLdErrorCode::InvalidTypeMapping,
                        ));
                    }
                    if !self.lenient && !is_keyword {
                        if let Err(e) = Iri::parse(r#type.as_ref()) {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                format!("Invalid @type iri '{type}': {e}"),
                                JsonLdErrorCode::InvalidTypeMapping,
                            ));
                        }
                    }
                    // 12.5)
                    definition.type_mapping = Some(r#type.into());
                }
                // 13)
                "@reverse" => {
                    // 13.1)
                    if value.contains_key("@id") {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@reverse and @id cannot be used together in a context",
                            JsonLdErrorCode::InvalidReverseProperty,
                        ));
                        continue;
                    }
                    if value.contains_key("@nest") {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@reverse and @nest cannot be used together in a context",
                            JsonLdErrorCode::InvalidReverseProperty,
                        ));
                        continue;
                    }
                    // 13.2)
                    let JsonNode::String(value) = key_value else {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@reverse value must be a string in a context",
                            JsonLdErrorCode::InvalidIriMapping,
                        ));
                        continue;
                    };
                    // 13.4)
                    if let Some(iri) = self.expand_iri(
                        active_context,
                        value.into(),
                        false,
                        true,
                        Some(local_context),
                        defined,
                        errors,
                    ) {
                        if self.lenient && !has_keyword_form(&iri)
                            || !self.lenient
                                && (iri.starts_with("_:") || Iri::parse(iri.as_ref()).is_ok())
                        {
                            definition.iri_mapping = Some(Some(iri.into()));
                        } else {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                format!("{iri} is not a valid IRI or blank node"),
                                JsonLdErrorCode::InvalidIriMapping,
                            ));
                            definition.iri_mapping = Some(None);
                        }
                    } else {
                        definition.iri_mapping = Some(None);
                    }
                    definition.iri_mapping = Some(
                        self.expand_iri(
                            active_context,
                            value.into(),
                            false,
                            true,
                            Some(local_context),
                            defined,
                            errors,
                        )
                        .map(Into::into),
                    );
                    // 13.6)
                    definition.reverse_property = true;
                }
                // 14)
                "@id" => {
                    match key_value {
                        // 14.1)
                        JsonNode::Null => {
                            definition.iri_mapping = Some(None);
                        }
                        JsonNode::String(id) => {
                            if id == term {
                                continue;
                            }
                            let Some(expanded) = self.expand_iri(
                                active_context,
                                id.into(),
                                false,
                                true,
                                Some(local_context),
                                defined,
                                errors,
                            ) else {
                                // 14.2.2)
                                definition.iri_mapping = Some(None);
                                continue;
                            };
                            // 14.2.3)
                            if expanded == "@context" {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    "@context cannot be aliased with @id: @context",
                                    JsonLdErrorCode::InvalidKeywordAlias,
                                ));
                                continue;
                            }
                            definition.iri_mapping = Some(Some(expanded.into()));
                            // 14.2.4)
                            if term
                                .as_bytes()
                                .get(1..term.len() - 1)
                                .is_some_and(|t| t.contains(&b':'))
                                || term.contains('/')
                            {
                                // 14.2.4.1)
                                defined.insert(term.into(), true);
                                let expended_term = self.expand_iri(
                                    active_context,
                                    term.into(),
                                    false,
                                    true,
                                    Some(local_context),
                                    defined,
                                    errors,
                                );
                                // 14.2.4.2)
                                if expended_term.as_deref()
                                    != definition.iri_mapping.as_ref().and_then(|o| o.as_deref())
                                {
                                    errors.push(JsonLdSyntaxError::msg_and_code(
                                        format!("Inconsistent expansion of {term}"),
                                        JsonLdErrorCode::InvalidIriMapping,
                                    ))
                                }
                            }
                            // 14.2.5)
                            if !term.contains(':')
                                && !term.contains('/')
                                && definition.iri_mapping.as_ref().is_some_and(|iri| {
                                    iri.as_ref().is_some_and(|iri| {
                                        iri.ends_with(|c| {
                                            matches!(c, ':' | '/' | '?' | '#' | '[' | ']' | '@')
                                        }) || iri.starts_with("_:")
                                    })
                                })
                            {
                                definition.prefix_flag = true;
                            }
                        }
                        // 14.2.1)
                        _ => {
                            definition.iri_mapping = Some(None);
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@id value must be a string",
                                JsonLdErrorCode::InvalidIriMapping,
                            ))
                        }
                    }
                }
                // 19)
                "@container" => {
                    const ALLOWED_CONTAINER_MAPPINGS: &[&[&str]] = &[
                        &["@index"],
                        &["@language"],
                        &["@list"],
                        &["@set"],
                        &["@index", "@set"],
                        &["@language", "@set"],
                        &["@graph"],
                        &["@graph", "@id"],
                        &["@graph", "@index"],
                        &["@graph", "@id", "@set"],
                        &["@graph", "@index", "@set"],
                        &["@id"],
                        &["@id", "@set"],
                        &["@type"],
                        &["@type", "@set"],
                    ];

                    // 19.1)
                    let mut container_mapping = Vec::new();
                    for value in if let JsonNode::Array(value) = key_value {
                        if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                    "@container definition with multiple values is not supported in JSON-LD 1.0",
                                    JsonLdErrorCode::InvalidContainerMapping,
                                ));
                            continue;
                        }
                        value.as_slice()
                    } else {
                        slice::from_ref(key_value)
                    } {
                        if let JsonNode::String(container) = value {
                            container_mapping.push(container.as_str());
                        } else {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@container value must be a string or an array of strings",
                                JsonLdErrorCode::InvalidContainerMapping,
                            ));
                        }
                    }
                    container_mapping.sort_unstable();
                    let Some(container_mapping) = ALLOWED_CONTAINER_MAPPINGS
                        .iter()
                        .find_map(|c| (*c == container_mapping).then_some(*c))
                    else {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "Not supported @container value combination",
                            JsonLdErrorCode::InvalidContainerMapping,
                        ));
                        continue;
                    };
                    // 19.2)
                    if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        let mut found = false;
                        for bad in ["@graph", "@id", "@type"] {
                            if container_mapping.contains(&bad) {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("{bad} container is not supported in JSON-LD 1.0"),
                                    JsonLdErrorCode::InvalidContainerMapping,
                                ));
                                found = true;
                            }
                        }
                        if found {
                            continue;
                        }
                    }
                    // 19.3)
                    definition.container_mapping = container_mapping;
                    // 19.4)
                    if container_mapping.contains(&"@type") {
                        if let Some(type_mapping) = &definition.type_mapping {
                            if !["@id", "@vocab"].contains(&type_mapping.as_str()) {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("Type mapping must be @id or @vocab, not {type_mapping} when used with @type container"),
                                    JsonLdErrorCode::InvalidContainerMapping,
                                ));
                            }
                        } else {
                            definition.type_mapping = Some("@id".into());
                        }
                    }
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
                    if value.contains_key("@type") {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "Both @language and @type can't be set at the same time",
                            JsonLdErrorCode::InvalidLanguageMapping,
                        ));
                    }
                    definition.language_mapping = Some(match key_value {
                        JsonNode::String(language) => Some(language.clone()),
                        JsonNode::Null => None,
                        _ => {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@language value must be a string or null",
                                JsonLdErrorCode::InvalidLanguageMapping,
                            ));
                            continue;
                        }
                    })
                }
                // 23)
                "@direction" => {
                    // 23.1)
                    if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                    if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
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
                    if self.processing_mode == JsonLdProcessingMode::JsonLd1_0 {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@prefix is only supported in JSON-LD 1.1",
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
        // 13.5)
        if definition.reverse_property
            && !matches!(definition.container_mapping, [] | ["@index" | "@set"])
        {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "@reverse is only compatible with @index or @set containers",
                JsonLdErrorCode::InvalidReverseProperty,
            ))
        }
        if definition.iri_mapping.is_none() {
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
                    self.create_term_definition(
                        active_context,
                        local_context,
                        prefix,
                        defined,
                        base_url,
                        false,
                        false,
                        remote_contexts,
                        errors,
                    )
                }
                if let Some(term_definition) = active_context.term_definitions.get(prefix) {
                    // 15.2)
                    if let Some(Some(iri_mapping)) = &term_definition.iri_mapping {
                        definition.iri_mapping = Some(Some(format!("{iri_mapping}{suffix}")));
                    } else {
                        errors.push(JsonLdSyntaxError::msg(format!(
                            "The prefix '{prefix}' is not associated with an IRI in the context"
                        )));
                    }
                } else {
                    // 15.3)
                    definition.iri_mapping = Some(Some(term.into()));
                }
            } else if term.contains('/') {
                // 16)
                if let Some(iri) = self.expand_iri(
                    active_context,
                    term.into(),
                    false,
                    true,
                    Some(local_context),
                    defined,
                    errors,
                ) {
                    if has_keyword_form(&iri) {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!("Context term @id is not allowed to be a keyword, {iri} found"),
                            JsonLdErrorCode::InvalidIriMapping,
                        ))
                    } else if iri.starts_with("_:") {
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            format!(
                                "Context term @id is not allowed to be a blank node, {iri} found"
                            ),
                            JsonLdErrorCode::InvalidIriMapping,
                        ))
                    } else {
                        definition.iri_mapping = Some(Some(iri.into()));
                    }
                }
            } else if term == "@type" {
                // 17)
                definition.iri_mapping = Some(Some("@type".into()));
            } else {
                // 18)
                if let Some(vocabulary_mapping) = &active_context.vocabulary_mapping {
                    definition.iri_mapping = Some(Some(format!("{vocabulary_mapping}{term}")));
                } else {
                    errors.push(JsonLdSyntaxError::msg_and_code(
                        format!(
                            "No @vocab key to build an IRI from context {term} term definition"
                        ),
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

    /// [IRI Expansion](https://www.w3.org/TR/json-ld-api/#iri-expansion)
    pub fn expand_iri<'a>(
        &self,
        active_context: &mut JsonLdContext,
        value: Cow<'a, str>,
        document_relative: bool,
        vocab: bool,
        local_context: Option<&HashMap<String, JsonNode>>,
        defined: &mut HashMap<String, bool>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<Cow<'a, str>> {
        if has_keyword_form(&value) {
            // 1)
            return is_keyword(&value).then_some(value);
        }
        // 3)
        if let Some(local_context) = local_context {
            if local_context.contains_key(value.as_ref())
                && defined.get(value.as_ref()) != Some(&true)
            {
                self.create_term_definition(
                    active_context,
                    local_context,
                    &value,
                    defined,
                    None,
                    false,
                    false,
                    &mut Vec::new(),
                    errors,
                )
            }
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
            // 6.3)
            if let Some(local_context) = local_context {
                if local_context.contains_key(prefix) && defined.get(prefix) != Some(&true) {
                    self.create_term_definition(
                        active_context,
                        local_context,
                        prefix,
                        defined,
                        None,
                        false,
                        false,
                        &mut Vec::new(),
                        errors,
                    )
                }
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
}

pub fn has_keyword_form(value: &str) -> bool {
    value
        .strip_prefix('@')
        .is_some_and(|suffix| suffix.bytes().all(|b| b.is_ascii_alphabetic()))
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "@base"
            | "@container"
            | "@context"
            | "@direction"
            | "@graph"
            | "@id"
            | "@import"
            | "@included"
            | "@index"
            | "@json"
            | "@language"
            | "@list"
            | "@nest"
            | "@none"
            | "@prefix"
            | "@propagate"
            | "@protected"
            | "@reverse"
            | "@set"
            | "@type"
            | "@value"
            | "@version"
            | "@vocab"
    )
}

fn json_slice_to_node(data: &[u8]) -> Result<JsonNode, JsonSyntaxError> {
    let mut parser = SliceJsonParser::new(data);
    json_node_from_events(std::iter::from_fn(|| match parser.parse_next() {
        Ok(JsonEvent::Eof) => None,
        Ok(event) => Some(Ok(event)),
        Err(e) => Some(Err(e)),
    }))
}

enum BuildingObjectOrArrayNode {
    Object(HashMap<String, JsonNode>),
    ObjectWithPendingKey(HashMap<String, JsonNode>, String),
    Array(Vec<JsonNode>),
}

pub fn json_node_from_events<'a>(
    events: impl IntoIterator<Item = Result<JsonEvent<'a>, JsonSyntaxError>>,
) -> Result<JsonNode, JsonSyntaxError> {
    let mut stack = Vec::new();
    for event in events {
        if let Some(result) = match event? {
            JsonEvent::String(value) => {
                after_to_node_event(&mut stack, JsonNode::String(value.into()))
            }
            JsonEvent::Number(value) => {
                after_to_node_event(&mut stack, JsonNode::Number(value.into()))
            }
            JsonEvent::Boolean(value) => after_to_node_event(&mut stack, JsonNode::Boolean(value)),
            JsonEvent::Null => after_to_node_event(&mut stack, JsonNode::Null),
            JsonEvent::EndArray | JsonEvent::EndObject => {
                let value = match stack.pop() {
                    Some(BuildingObjectOrArrayNode::Object(object)) => JsonNode::Object(object),
                    Some(BuildingObjectOrArrayNode::Array(array)) => JsonNode::Array(array),
                    _ => unreachable!(),
                };
                after_to_node_event(&mut stack, value)
            }
            JsonEvent::StartArray => {
                stack.push(BuildingObjectOrArrayNode::Array(Vec::new()));
                None
            }
            JsonEvent::StartObject => {
                stack.push(BuildingObjectOrArrayNode::Object(HashMap::new()));
                None
            }
            JsonEvent::ObjectKey(key) => {
                if let Some(BuildingObjectOrArrayNode::Object(object)) = stack.pop() {
                    stack.push(BuildingObjectOrArrayNode::ObjectWithPendingKey(
                        object,
                        key.into(),
                    ));
                }
                None
            }
            JsonEvent::Eof => unreachable!(),
        } {
            return Ok(result);
        }
    }
    unreachable!("The JSON emitted by the parser mut be valid")
}

fn after_to_node_event(
    stack: &mut Vec<BuildingObjectOrArrayNode>,
    new_value: JsonNode,
) -> Option<JsonNode> {
    match stack.pop() {
        Some(BuildingObjectOrArrayNode::ObjectWithPendingKey(mut object, key)) => {
            object.insert(key, new_value);
            stack.push(BuildingObjectOrArrayNode::Object(object));
            None
        }
        Some(BuildingObjectOrArrayNode::Object(object)) => {
            stack.push(BuildingObjectOrArrayNode::Object(object));
            None
        }
        Some(BuildingObjectOrArrayNode::Array(mut array)) => {
            array.push(new_value);
            stack.push(BuildingObjectOrArrayNode::Array(array));
            None
        }
        None => Some(new_value),
    }
}
