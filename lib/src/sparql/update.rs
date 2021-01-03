use crate::error::{invalid_data_error, invalid_input_error};
use crate::io::GraphFormat;
use crate::model::{BlankNode, GraphNameRef, NamedNode, NamedOrBlankNode, Quad, Term};
use crate::sparql::algebra::{
    GraphPattern, GraphTarget, GraphUpdateOperation, NamedNodeOrVariable, QuadPattern,
    QueryDataset, TermOrVariable,
};
use crate::sparql::dataset::{DatasetStrId, DatasetView};
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::http::Client;
use crate::sparql::plan::EncodedTuple;
use crate::sparql::plan_builder::PlanBuilder;
use crate::sparql::{EvaluationError, UpdateOptions, Variable};
use crate::store::numeric_encoder::{
    EncodedQuad, EncodedTerm, ReadEncoder, StrContainer, StrLookup, WriteEncoder,
};
use crate::store::{load_graph, ReadableEncodedStore, StoreOrParseError, WritableEncodedStore};
use http::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use http::{Method, Request, StatusCode};
use oxiri::Iri;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

pub(crate) struct SimpleUpdateEvaluator<'a, R, W> {
    read: R,
    write: &'a mut W,
    base_iri: Option<Rc<Iri<String>>>,
    options: UpdateOptions,
    client: Client,
}

impl<
        'a,
        R: ReadableEncodedStore + Clone + 'static,
        W: StrContainer<StrId = R::StrId> + WritableEncodedStore<StrId = R::StrId> + 'a,
    > SimpleUpdateEvaluator<'a, R, W>
where
    io::Error: From<StoreOrParseError<W::Error>>,
{
    pub fn new(
        read: R,
        write: &'a mut W,
        base_iri: Option<Rc<Iri<String>>>,
        options: UpdateOptions,
    ) -> Self {
        Self {
            read,
            write,
            base_iri,
            options,
            client: Client::new(),
        }
    }

    pub fn eval_all(&mut self, updates: &[GraphUpdateOperation]) -> Result<(), EvaluationError> {
        for update in updates {
            self.eval(update)?;
        }
        Ok(())
    }

    fn eval(&mut self, update: &GraphUpdateOperation) -> Result<(), EvaluationError> {
        match update {
            GraphUpdateOperation::InsertData { data } => self.eval_insert_data(data),
            GraphUpdateOperation::DeleteData { data } => self.eval_delete_data(data),
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => self.eval_delete_insert(delete, insert, using, pattern),
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
            if let Some(quad) = self.encode_quad_for_insertion(quad, &mut bnodes)? {
                self.write.insert_encoded(&quad).map_err(to_eval_error)?;
            }
        }
        Ok(())
    }

    fn eval_delete_data(&mut self, data: &[Quad]) -> Result<(), EvaluationError> {
        for quad in data {
            if let Some(quad) = self.encode_quad_for_deletion(quad)? {
                self.write.remove_encoded(&quad).map_err(to_eval_error)?;
            }
        }
        Ok(())
    }

    fn eval_delete_insert(
        &mut self,
        delete: &[QuadPattern],
        insert: &[QuadPattern],
        using: &QueryDataset,
        algebra: &GraphPattern,
    ) -> Result<(), EvaluationError> {
        let dataset = Rc::new(DatasetView::new(self.read.clone(), using)?);
        let (plan, variables) = PlanBuilder::build(dataset.as_ref(), algebra)?;
        let evaluator = SimpleEvaluator::<DatasetView<R>>::new(
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
                        Some(
                            t.try_map_id(|id| {
                                if let DatasetStrId::Store(s) = id {
                                    Ok(s)
                                } else {
                                    self.write
                                        .insert_str(
                                            &dataset
                                                .get_str(id)
                                                .map_err(to_eval_error)?
                                                .ok_or_else(|| {
                                                    EvaluationError::msg(
                                                        "String not stored in the string store",
                                                    )
                                                })
                                                .map_err(to_eval_error)?,
                                        )
                                        .map_err(to_eval_error)
                                }
                            })
                            .map_err(to_eval_error)?,
                        )
                    } else {
                        None
                    })
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?;

            for quad in delete {
                if let Some(quad) =
                    self.encode_quad_pattern_for_deletion(quad, &variables, &tuple)?
                {
                    self.write.remove_encoded(&quad).map_err(to_eval_error)?;
                }
            }
            for quad in insert {
                if let Some(quad) =
                    self.encode_quad_pattern_for_insertion(quad, &variables, &tuple, &mut bnodes)?
                {
                    self.write.insert_encoded(&quad).map_err(to_eval_error)?;
                }
            }
            bnodes.clear();
        }
        Ok(())
    }

    fn eval_load(
        &mut self,
        from: &NamedNode,
        to: &Option<NamedNode>,
    ) -> Result<(), EvaluationError> {
        let request = Request::builder()
            .method(Method::GET)
            .uri(from.as_str())
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
        let to_graph_name = if let Some(graph_name) = to {
            graph_name.as_ref().into()
        } else {
            GraphNameRef::DefaultGraph
        };
        load_graph(
            self.write,
            response.into_body(),
            format,
            to_graph_name,
            Some(from.as_str()),
        )
        .map_err(io::Error::from)?;
        Ok(())
    }

    fn eval_create(&mut self, graph: &NamedNode, silent: bool) -> Result<(), EvaluationError> {
        let encoded_graph_name = self
            .write
            .encode_named_node(graph.as_ref())
            .map_err(to_eval_error)?;
        if self
            .read
            .contains_encoded_named_graph(encoded_graph_name)
            .map_err(to_eval_error)?
        {
            if silent {
                Ok(())
            } else {
                Err(EvaluationError::msg(format!(
                    "The graph {} already exists",
                    graph
                )))
            }
        } else {
            self.write
                .insert_encoded_named_graph(encoded_graph_name)
                .map_err(to_eval_error)
        }
    }

    fn eval_clear(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if let Some(graph_name) = self
                    .read
                    .get_encoded_named_node(graph_name.as_ref())
                    .map_err(to_eval_error)?
                {
                    if self
                        .read
                        .contains_encoded_named_graph(graph_name)
                        .map_err(to_eval_error)?
                    {
                        return self
                            .write
                            .clear_encoded_graph(graph_name)
                            .map_err(to_eval_error);
                    }
                }
                if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph
                    )))
                }
            }
            GraphTarget::DefaultGraph => self
                .write
                .clear_encoded_graph(EncodedTerm::DefaultGraph)
                .map_err(to_eval_error),
            GraphTarget::NamedGraphs => {
                // TODO: optimize?
                for graph in self.read.encoded_named_graphs() {
                    self.write
                        .clear_encoded_graph(graph.map_err(to_eval_error)?)
                        .map_err(to_eval_error)?;
                }
                Ok(())
            }
            GraphTarget::AllGraphs => {
                // TODO: optimize?
                for graph in self.read.encoded_named_graphs() {
                    self.write
                        .clear_encoded_graph(graph.map_err(to_eval_error)?)
                        .map_err(to_eval_error)?;
                }
                self.write
                    .clear_encoded_graph(EncodedTerm::DefaultGraph)
                    .map_err(to_eval_error)
            }
        }
    }

    fn eval_drop(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if let Some(graph_name) = self
                    .read
                    .get_encoded_named_node(graph_name.as_ref())
                    .map_err(to_eval_error)?
                {
                    if self
                        .read
                        .contains_encoded_named_graph(graph_name)
                        .map_err(to_eval_error)?
                    {
                        return self
                            .write
                            .remove_encoded_named_graph(graph_name)
                            .map_err(to_eval_error);
                    }
                }
                if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph
                    )))
                }
            }
            GraphTarget::DefaultGraph => self
                .write
                .clear_encoded_graph(EncodedTerm::DefaultGraph)
                .map_err(to_eval_error),
            GraphTarget::NamedGraphs => {
                // TODO: optimize?
                for graph in self.read.encoded_named_graphs() {
                    self.write
                        .remove_encoded_named_graph(graph.map_err(to_eval_error)?)
                        .map_err(to_eval_error)?;
                }
                Ok(())
            }
            GraphTarget::AllGraphs => self.write.clear().map_err(to_eval_error),
        }
    }

    fn encode_quad_for_insertion(
        &mut self,
        quad: &Quad,
        bnodes: &mut HashMap<BlankNode, BlankNode>,
    ) -> Result<Option<EncodedQuad<R::StrId>>, EvaluationError> {
        Ok(Some(EncodedQuad {
            subject: match &quad.subject {
                NamedOrBlankNode::NamedNode(subject) => {
                    self.write.encode_named_node(subject.as_ref())
                }
                NamedOrBlankNode::BlankNode(subject) => self
                    .write
                    .encode_blank_node(bnodes.entry(subject.clone()).or_default().as_ref()),
            }
            .map_err(to_eval_error)?,
            predicate: self
                .write
                .encode_named_node(quad.predicate.as_ref())
                .map_err(to_eval_error)?,
            object: match &quad.object {
                Term::NamedNode(object) => self.write.encode_named_node(object.as_ref()),
                Term::BlankNode(object) => self
                    .write
                    .encode_blank_node(bnodes.entry(object.clone()).or_default().as_ref()),
                Term::Literal(object) => self.write.encode_literal(object.as_ref()),
            }
            .map_err(to_eval_error)?,
            graph_name: self
                .write
                .encode_graph_name(quad.graph_name.as_ref())
                .map_err(to_eval_error)?,
        }))
    }

    fn encode_quad_pattern_for_insertion(
        &mut self,
        quad: &QuadPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
        bnodes: &mut HashMap<BlankNode, BlankNode>,
    ) -> Result<Option<EncodedQuad<R::StrId>>, EvaluationError> {
        Ok(Some(EncodedQuad {
            subject: if let Some(subject) =
                self.encode_term_for_insertion(&quad.subject, variables, values, bnodes, |t| {
                    t.is_named_node() || t.is_blank_node()
                })? {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) =
                self.encode_named_node_for_insertion(&quad.predicate, variables, values)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.encode_term_for_insertion(&quad.object, variables, values, bnodes, |t| {
                    !t.is_default_graph()
                })? {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) = &quad.graph_name {
                if let Some(graph_name) =
                    self.encode_named_node_for_insertion(graph_name, variables, values)?
                {
                    graph_name
                } else {
                    return Ok(None);
                }
            } else {
                EncodedTerm::DefaultGraph
            },
        }))
    }

    fn encode_term_for_insertion(
        &mut self,
        term: &TermOrVariable,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
        bnodes: &mut HashMap<BlankNode, BlankNode>,
        validate: impl FnOnce(&EncodedTerm<R::StrId>) -> bool,
    ) -> Result<Option<EncodedTerm<R::StrId>>, EvaluationError> {
        Ok(match term {
            TermOrVariable::Term(term) => Some(
                self.write
                    .encode_term(if let Term::BlankNode(bnode) = term {
                        bnodes.entry(bnode.clone()).or_default().as_ref().into()
                    } else {
                        term.as_ref()
                    })
                    .map_err(to_eval_error)?,
            ),
            TermOrVariable::Variable(v) => {
                if let Some(Some(term)) = variables
                    .iter()
                    .position(|v2| v == v2)
                    .and_then(|i| values.get(i))
                {
                    if validate(term) {
                        Some(*term)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        })
    }

    fn encode_named_node_for_insertion(
        &mut self,
        term: &NamedNodeOrVariable,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
    ) -> Result<Option<EncodedTerm<R::StrId>>, EvaluationError> {
        Ok(match term {
            NamedNodeOrVariable::NamedNode(term) => Some(
                self.write
                    .encode_named_node(term.into())
                    .map_err(to_eval_error)?,
            ),
            NamedNodeOrVariable::Variable(v) => {
                if let Some(Some(term)) = variables
                    .iter()
                    .position(|v2| v == v2)
                    .and_then(|i| values.get(i))
                {
                    if term.is_named_node() {
                        Some(*term)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        })
    }

    fn encode_quad_for_deletion(
        &mut self,
        quad: &Quad,
    ) -> Result<Option<EncodedQuad<R::StrId>>, EvaluationError> {
        Ok(Some(EncodedQuad {
            subject: if let Some(subject) = self
                .read
                .get_encoded_named_or_blank_node(quad.subject.as_ref())
                .map_err(to_eval_error)?
            {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) = self
                .read
                .get_encoded_named_node(quad.predicate.as_ref())
                .map_err(to_eval_error)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) = self
                .read
                .get_encoded_term(quad.object.as_ref())
                .map_err(to_eval_error)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) = self
                .read
                .get_encoded_graph_name(quad.graph_name.as_ref())
                .map_err(to_eval_error)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn encode_quad_pattern_for_deletion(
        &self,
        quad: &QuadPattern,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
    ) -> Result<Option<EncodedQuad<R::StrId>>, EvaluationError> {
        Ok(Some(EncodedQuad {
            subject: if let Some(subject) =
                self.encode_term_for_deletion(&quad.subject, variables, values)?
            {
                subject
            } else {
                return Ok(None);
            },
            predicate: if let Some(predicate) =
                self.encode_named_node_for_deletion(&quad.predicate, variables, values)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.encode_term_for_deletion(&quad.object, variables, values)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) = &quad.graph_name {
                if let Some(graph_name) =
                    self.encode_named_node_for_deletion(graph_name, variables, values)?
                {
                    graph_name
                } else {
                    return Ok(None);
                }
            } else {
                EncodedTerm::DefaultGraph
            },
        }))
    }

    fn encode_term_for_deletion(
        &self,
        term: &TermOrVariable,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
    ) -> Result<Option<EncodedTerm<R::StrId>>, EvaluationError> {
        match term {
            TermOrVariable::Term(term) => {
                if term.is_blank_node() {
                    Err(EvaluationError::msg(
                        "Blank node are not allowed in deletion patterns",
                    ))
                } else {
                    self.read
                        .get_encoded_term(term.into())
                        .map_err(to_eval_error)
                }
            }
            TermOrVariable::Variable(v) => Ok(
                if let Some(Some(term)) = variables
                    .iter()
                    .position(|v2| v == v2)
                    .and_then(|i| values.get(i))
                {
                    Some(*term)
                } else {
                    None
                },
            ),
        }
    }

    fn encode_named_node_for_deletion(
        &self,
        term: &NamedNodeOrVariable,
        variables: &[Variable],
        values: &[Option<EncodedTerm<R::StrId>>],
    ) -> Result<Option<EncodedTerm<R::StrId>>, EvaluationError> {
        Ok(match term {
            NamedNodeOrVariable::NamedNode(term) => self
                .read
                .get_encoded_named_node(term.into())
                .map_err(to_eval_error)?,
            NamedNodeOrVariable::Variable(v) => {
                if let Some(Some(term)) = variables
                    .iter()
                    .position(|v2| v == v2)
                    .and_then(|i| values.get(i))
                {
                    if term.is_named_node() {
                        Some(*term)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        })
    }
}

fn to_eval_error(e: impl Into<EvaluationError>) -> EvaluationError {
    e.into()
}
