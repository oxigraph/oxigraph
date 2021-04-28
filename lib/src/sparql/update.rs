use crate::error::{invalid_data_error, invalid_input_error};
use crate::io::GraphFormat;
use crate::model::{BlankNode as OxBlankNode, GraphNameRef, LiteralRef, NamedNodeRef};
use crate::sparql::algebra::QueryDataset;
use crate::sparql::dataset::DatasetView;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::http::Client;
use crate::sparql::plan::EncodedTuple;
use crate::sparql::plan_builder::PlanBuilder;
use crate::sparql::{EvaluationError, UpdateOptions};
use crate::storage::io::load_graph;
use crate::storage::numeric_encoder::{
    get_encoded_literal, get_encoded_named_node, EncodedQuad, EncodedTerm, EncodedTriple,
    StrLookup, WriteEncoder,
};
use crate::storage::Storage;
use http::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use http::{Method, Request, StatusCode};
use oxiri::Iri;
use spargebra::algebra::{GraphPattern, GraphTarget};
use spargebra::term::{
    BlankNode, GraphName, GraphNamePattern, GroundQuad, GroundQuadPattern, GroundSubject,
    GroundTerm, GroundTermPattern, GroundTriple, GroundTriplePattern, Literal, NamedNode,
    NamedNodePattern, Quad, QuadPattern, Subject, Term, TermPattern, Triple, TriplePattern,
    Variable,
};
use spargebra::GraphUpdateOperation;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

pub(crate) struct SimpleUpdateEvaluator<'a> {
    storage: &'a Storage,
    base_iri: Option<Rc<Iri<String>>>,
    options: UpdateOptions,
    client: Client,
}

impl<'a> SimpleUpdateEvaluator<'a> {
    pub fn new(
        storage: &'a Storage,
        base_iri: Option<Rc<Iri<String>>>,
        options: UpdateOptions,
    ) -> Self {
        Self {
            storage,
            base_iri,
            options,
            client: Client::new(),
        }
    }

    pub fn eval_all(
        &mut self,
        updates: &[GraphUpdateOperation],
        using_datasets: &[Option<QueryDataset>],
    ) -> Result<(), EvaluationError> {
        for (update, using_dataset) in updates.iter().zip(using_datasets) {
            self.eval(update, using_dataset)?;
        }
        Ok(())
    }

    fn eval(
        &mut self,
        update: &GraphUpdateOperation,
        using_dataset: &Option<QueryDataset>,
    ) -> Result<(), EvaluationError> {
        match update {
            GraphUpdateOperation::InsertData { data } => self.eval_insert_data(data),
            GraphUpdateOperation::DeleteData { data } => self.eval_delete_data(data),
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                pattern,
                ..
            } => self.eval_delete_insert(delete, insert, using_dataset.as_ref().unwrap(), pattern),
            GraphUpdateOperation::Load { silent, from, to } => {
                if let Err(error) = self.eval_load(from, to) {
                    if *silent {
                        Ok(())
                    } else {
                        Err(error)
                    }
                } else {
                    Ok(())
                }
            }
            GraphUpdateOperation::Clear { graph, silent } => self.eval_clear(graph, *silent),
            GraphUpdateOperation::Create { graph, silent } => self.eval_create(graph, *silent),
            GraphUpdateOperation::Drop { graph, silent } => self.eval_drop(graph, *silent),
        }
    }

    fn eval_insert_data(&mut self, data: &[Quad]) -> Result<(), EvaluationError> {
        let mut bnodes = HashMap::new();
        for quad in data {
            let quad = self.encode_quad_for_insertion(quad, &mut bnodes)?;
            self.storage.insert(&quad)?;
        }
        Ok(())
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) -> Result<(), EvaluationError> {
        for quad in data {
            let quad = self.encode_quad_for_deletion(quad);
            self.storage.remove(&quad)?;
        }
        Ok(())
    }

    fn eval_delete_insert(
        &mut self,
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        using: &QueryDataset,
        algebra: &GraphPattern,
    ) -> Result<(), EvaluationError> {
        let dataset = Rc::new(DatasetView::new(self.storage.clone(), using)?);
        let (plan, variables) = PlanBuilder::build(dataset.as_ref(), algebra)?;
        let evaluator = SimpleEvaluator::new(
            dataset.clone(),
            self.base_iri.clone(),
            self.options.query_options.service_handler.clone(),
        );
        let mut bnodes = HashMap::new();
        for tuple in evaluator.eval_plan(&plan, EncodedTuple::with_capacity(variables.len())) {
            // We map the tuple to only get store strings
            let tuple = tuple?
                .into_iter()
                .map(|t| {
                    Ok(if let Some(t) = t {
                        let r: Result<_, EvaluationError> = t.on_each_id(&mut |id| {
                            self.storage.insert_str(
                                id,
                                &dataset.get_str(id)?.ok_or_else(|| {
                                    EvaluationError::msg("String not stored in the string store")
                                })?,
                            )?;
                            Ok(())
                        });
                        r?;
                        Some(t)
                    } else {
                        None
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?;

            for quad in delete {
                if let Some(quad) = self.encode_quad_pattern_for_deletion(quad, &variables, &tuple)
                {
                    self.storage.remove(&quad)?;
                }
            }
            for quad in insert {
                if let Some(quad) =
                    self.encode_quad_pattern_for_insertion(quad, &variables, &tuple, &mut bnodes)?
                {
                    self.storage.insert(&quad)?;
                }
            }
            bnodes.clear();
        }
        Ok(())
    }

    fn eval_load(&mut self, from: &NamedNode, to: &GraphName) -> Result<(), EvaluationError> {
        let request = Request::builder()
            .method(Method::GET)
            .uri(&from.iri)
            .header(
                ACCEPT,
                "application/n-triples, text/turtle, application/rdf+xml",
            )
            .header(USER_AGENT, concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .body(None)
            .map_err(invalid_input_error)?;
        let response = self.client.request(&request)?;
        if response.status() != StatusCode::OK {
            return Err(EvaluationError::msg(format!(
                "HTTP error code {} returned when fetching {}",
                response.status(),
                from
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .ok_or_else(|| {
                EvaluationError::msg(format!("No Content-Type header returned by {}", from))
            })?
            .to_str()
            .map_err(invalid_data_error)?;
        let format = GraphFormat::from_media_type(content_type).ok_or_else(|| {
            EvaluationError::msg(format!(
                "Unsupported Content-Type returned by {}: {}",
                from, content_type
            ))
        })?;
        let to_graph_name = match to {
            GraphName::NamedNode(graph_name) => NamedNodeRef::new_unchecked(&graph_name.iri).into(),
            GraphName::DefaultGraph => GraphNameRef::DefaultGraph,
        };
        load_graph(
            self.storage,
            response.into_body(),
            format,
            to_graph_name,
            Some(&from.iri),
        )
        .map_err(io::Error::from)?;
        Ok(())
    }

    fn eval_create(&mut self, graph: &NamedNode, silent: bool) -> Result<(), EvaluationError> {
        let encoded_graph_name = self.encode_named_node_for_insertion(graph)?;
        if self.storage.contains_named_graph(&encoded_graph_name)? {
            if silent {
                Ok(())
            } else {
                Err(EvaluationError::msg(format!(
                    "The graph {} already exists",
                    graph
                )))
            }
        } else {
            self.storage.insert_named_graph(&encoded_graph_name)?;
            Ok(())
        }
    }

    fn eval_clear(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                let graph_name = self.encode_named_node_for_deletion(graph_name);
                if self.storage.contains_named_graph(&graph_name)? {
                    Ok(self.storage.clear_graph(&graph_name)?)
                } else if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph
                    )))
                }
            }
            GraphTarget::DefaultGraph => {
                Ok(self.storage.clear_graph(&EncodedTerm::DefaultGraph)?)
            }
            GraphTarget::NamedGraphs => {
                // TODO: optimize?
                for graph in self.storage.named_graphs() {
                    self.storage.clear_graph(&graph?)?;
                }
                Ok(())
            }
            GraphTarget::AllGraphs => {
                // TODO: optimize?
                for graph in self.storage.named_graphs() {
                    self.storage.clear_graph(&graph?)?;
                }
                Ok(self.storage.clear_graph(&EncodedTerm::DefaultGraph)?)
            }
        }
    }

    fn eval_drop(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                let graph_name = self.encode_named_node_for_deletion(graph_name);
                if self.storage.contains_named_graph(&graph_name)? {
                    self.storage.remove_named_graph(&graph_name)?;
                    Ok(())
                } else if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph
                    )))
                }
            }
            GraphTarget::DefaultGraph => {
                Ok(self.storage.clear_graph(&EncodedTerm::DefaultGraph)?)
            }
            GraphTarget::NamedGraphs => {
                // TODO: optimize?
                for graph in self.storage.named_graphs() {
                    self.storage.remove_named_graph(&graph?)?;
                }
                Ok(())
            }
            GraphTarget::AllGraphs => Ok(self.storage.clear()?),
        }
    }

    fn encode_quad_for_insertion(
        &mut self,
        quad: &Quad,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<EncodedQuad, EvaluationError> {
        Ok(EncodedQuad {
            subject: match &quad.subject {
                Subject::NamedNode(subject) => self.encode_named_node_for_insertion(subject)?,
                Subject::BlankNode(subject) => self
                    .storage
                    .encode_blank_node(bnodes.entry(subject.clone()).or_default().as_ref())?,
                Subject::Triple(subject) => {
                    self.encode_triple_for_insertion(subject, bnodes)?.into()
                }
            },
            predicate: self
                .storage
                .encode_named_node(NamedNodeRef::new_unchecked(&quad.predicate.iri))?,
            object: match &quad.object {
                Term::NamedNode(object) => self.encode_named_node_for_insertion(object)?,
                Term::BlankNode(object) => self
                    .storage
                    .encode_blank_node(bnodes.entry(object.clone()).or_default().as_ref())?,
                Term::Literal(object) => self.encode_literal_for_insertion(object)?,
                Term::Triple(subject) => self.encode_triple_for_insertion(subject, bnodes)?.into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => {
                    self.encode_named_node_for_insertion(graph_name)?
                }
                GraphName::DefaultGraph => EncodedTerm::DefaultGraph,
            },
        })
    }

    fn encode_quad_pattern_for_insertion(
        &mut self,
        quad: &QuadPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<EncodedQuad>, EvaluationError> {
        Ok(Some(EncodedQuad {
            subject: if let Some(subject) = self.encode_term_or_var_for_insertion(
                &quad.subject,
                variables,
                values,
                bnodes,
                |t| t.is_named_node() || t.is_blank_node(),
            )? {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) =
                self.encode_named_node_or_var_for_insertion(&quad.predicate, variables, values)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) = self.encode_term_or_var_for_insertion(
                &quad.object,
                variables,
                values,
                bnodes,
                |t| !t.is_default_graph(),
            )? {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) =
                self.encode_graph_name_or_var_for_insertion(&quad.graph_name, variables, values)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn encode_term_or_var_for_insertion(
        &mut self,
        term: &TermPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
        validate: impl FnOnce(&EncodedTerm) -> bool,
    ) -> Result<Option<EncodedTerm>, EvaluationError> {
        Ok(match term {
            TermPattern::NamedNode(term) => Some(self.encode_named_node_for_insertion(term)?),
            TermPattern::BlankNode(bnode) => Some(
                self.storage
                    .encode_blank_node(bnodes.entry(bnode.clone()).or_default().as_ref())?,
            ),
            TermPattern::Literal(term) => Some(self.encode_literal_for_insertion(term)?),
            TermPattern::Triple(triple) => self
                .encode_triple_pattern_for_insertion(triple, variables, values, bnodes)?
                .map(EncodedTerm::from),
            TermPattern::Variable(v) => self.lookup_variable(v, variables, values).filter(validate),
        })
    }

    fn encode_named_node_or_var_for_insertion(
        &mut self,
        term: &NamedNodePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Result<Option<EncodedTerm>, EvaluationError> {
        Ok(match term {
            NamedNodePattern::NamedNode(term) => Some(self.encode_named_node_for_insertion(term)?),
            NamedNodePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .filter(|value| value.is_named_node()),
        })
    }

    fn encode_graph_name_or_var_for_insertion(
        &mut self,
        term: &GraphNamePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Result<Option<EncodedTerm>, EvaluationError> {
        Ok(match term {
            GraphNamePattern::NamedNode(term) => Some(self.encode_named_node_for_insertion(term)?),
            GraphNamePattern::DefaultGraph => Some(EncodedTerm::DefaultGraph),
            GraphNamePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .filter(|value| value.is_named_node()),
        })
    }

    fn encode_triple_pattern_for_insertion(
        &mut self,
        triple: &TriplePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<EncodedTriple>, EvaluationError> {
        Ok(Some(EncodedTriple {
            subject: if let Some(subject) = self.encode_term_or_var_for_insertion(
                &triple.subject,
                variables,
                values,
                bnodes,
                |t| t.is_named_node() || t.is_blank_node(),
            )? {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) =
                self.encode_named_node_or_var_for_insertion(&triple.predicate, variables, values)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) = self.encode_term_or_var_for_insertion(
                &triple.object,
                variables,
                values,
                bnodes,
                |t| !t.is_default_graph(),
            )? {
                object
            } else {
                return Ok(None);
            },
        }))
    }

    fn lookup_variable(
        &self,
        v: &Variable,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        {
            if let Some(Some(term)) = variables
                .iter()
                .position(|v2| v == v2)
                .and_then(|i| values.get(i))
            {
                Some(term.clone())
            } else {
                None
            }
        }
    }

    fn encode_named_node_for_insertion(
        &mut self,
        term: &NamedNode,
    ) -> Result<EncodedTerm, EvaluationError> {
        Ok(self
            .storage
            .encode_named_node(NamedNodeRef::new_unchecked(&term.iri))?)
    }

    fn encode_triple_for_insertion(
        &mut self,
        triple: &Triple,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<EncodedTriple, EvaluationError> {
        Ok(EncodedTriple {
            subject: match &triple.subject {
                Subject::NamedNode(subject) => self.encode_named_node_for_insertion(subject)?,
                Subject::BlankNode(subject) => self
                    .storage
                    .encode_blank_node(bnodes.entry(subject.clone()).or_default().as_ref())?,
                Subject::Triple(subject) => {
                    self.encode_triple_for_insertion(subject, bnodes)?.into()
                }
            },
            predicate: self
                .storage
                .encode_named_node(NamedNodeRef::new_unchecked(&triple.predicate.iri))?,
            object: match &triple.object {
                Term::NamedNode(object) => self.encode_named_node_for_insertion(object)?,
                Term::BlankNode(object) => self
                    .storage
                    .encode_blank_node(bnodes.entry(object.clone()).or_default().as_ref())?,
                Term::Literal(object) => self.encode_literal_for_insertion(object)?,
                Term::Triple(object) => self.encode_triple_for_insertion(object, bnodes)?.into(),
            },
        })
    }

    fn encode_literal_for_insertion(
        &mut self,
        term: &Literal,
    ) -> Result<EncodedTerm, EvaluationError> {
        Ok(self.storage.encode_literal(match term {
            Literal::Simple { value } => LiteralRef::new_simple_literal(value),
            Literal::LanguageTaggedString { value, language } => {
                LiteralRef::new_language_tagged_literal_unchecked(value, language)
            }
            Literal::Typed { value, datatype } => {
                LiteralRef::new_typed_literal(value, NamedNodeRef::new_unchecked(&datatype.iri))
            }
        })?)
    }

    fn encode_quad_for_deletion(&mut self, quad: &GroundQuad) -> EncodedQuad {
        EncodedQuad {
            subject: match &quad.subject {
                GroundSubject::NamedNode(subject) => self.encode_named_node_for_deletion(subject),
                GroundSubject::Triple(subject) => self.encode_triple_for_deletion(subject),
            },
            predicate: self.encode_named_node_for_deletion(&quad.predicate),
            object: match &quad.object {
                GroundTerm::NamedNode(object) => self.encode_named_node_for_deletion(object),
                GroundTerm::Literal(object) => self.encode_literal_for_deletion(object),
                GroundTerm::Triple(object) => self.encode_triple_for_deletion(object),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => self.encode_named_node_for_deletion(graph_name),
                GraphName::DefaultGraph => EncodedTerm::DefaultGraph,
            },
        }
    }

    fn encode_quad_pattern_for_deletion(
        &self,
        quad: &GroundQuadPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedQuad> {
        Some(EncodedQuad {
            subject: self.encode_term_or_var_for_deletion(&quad.subject, variables, values)?,
            predicate: self.encode_named_node_or_var_for_deletion(
                &quad.predicate,
                variables,
                values,
            )?,
            object: self.encode_term_or_var_for_deletion(&quad.object, variables, values)?,
            graph_name: self.encode_graph_name_or_var_for_deletion(
                &quad.graph_name,
                variables,
                values,
            )?,
        })
    }

    fn encode_term_or_var_for_deletion(
        &self,
        term: &GroundTermPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        match term {
            GroundTermPattern::NamedNode(term) => Some(self.encode_named_node_for_deletion(term)),
            GroundTermPattern::Literal(term) => Some(self.encode_literal_for_deletion(term)),
            GroundTermPattern::Variable(v) => self.lookup_variable(v, variables, values),
            GroundTermPattern::Triple(triple) => Some(
                self.encode_triple_pattern_for_deletion(triple, variables, values)?
                    .into(),
            ),
        }
    }

    fn encode_named_node_or_var_for_deletion(
        &self,
        term: &NamedNodePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        match term {
            NamedNodePattern::NamedNode(term) => Some(self.encode_named_node_for_deletion(term)),
            NamedNodePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .filter(|v| v.is_named_node()),
        }
    }

    fn encode_graph_name_or_var_for_deletion(
        &self,
        graph_name: &GraphNamePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        match graph_name {
            GraphNamePattern::NamedNode(term) => Some(self.encode_named_node_for_deletion(term)),
            GraphNamePattern::DefaultGraph => Some(EncodedTerm::DefaultGraph),
            GraphNamePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .filter(|v| v.is_named_node()),
        }
    }

    fn encode_triple_pattern_for_deletion(
        &self,
        triple: &GroundTriplePattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm>],
    ) -> Option<EncodedTriple> {
        Some(EncodedTriple {
            subject: self.encode_term_or_var_for_deletion(&triple.subject, variables, values)?,
            predicate: self.encode_named_node_or_var_for_deletion(
                &triple.predicate,
                variables,
                values,
            )?,
            object: self.encode_term_or_var_for_deletion(&triple.object, variables, values)?,
        })
    }

    fn encode_named_node_for_deletion(&self, term: &NamedNode) -> EncodedTerm {
        get_encoded_named_node(NamedNodeRef::new_unchecked(&term.iri))
    }

    fn encode_literal_for_deletion(&self, term: &Literal) -> EncodedTerm {
        get_encoded_literal(match term {
            Literal::Simple { value } => LiteralRef::new_simple_literal(value),
            Literal::LanguageTaggedString { value, language } => {
                LiteralRef::new_language_tagged_literal_unchecked(value, language)
            }
            Literal::Typed { value, datatype } => {
                LiteralRef::new_typed_literal(value, NamedNodeRef::new_unchecked(&datatype.iri))
            }
        })
    }

    fn encode_triple_for_deletion(&self, triple: &GroundTriple) -> EncodedTerm {
        EncodedTriple::new(
            match &triple.subject {
                GroundSubject::NamedNode(subject) => self.encode_named_node_for_deletion(subject),
                GroundSubject::Triple(subject) => self.encode_triple_for_deletion(subject),
            },
            self.encode_named_node_for_deletion(&triple.predicate),
            match &triple.object {
                GroundTerm::NamedNode(object) => self.encode_named_node_for_deletion(object),
                GroundTerm::Literal(object) => self.encode_literal_for_deletion(object),
                GroundTerm::Triple(object) => self.encode_triple_for_deletion(object),
            },
        )
        .into()
    }
}
