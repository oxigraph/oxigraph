use crate::error::{invalid_data_error, invalid_input_error};
use crate::io::GraphFormat;
use crate::model::{
    BlankNode as OxBlankNode, GraphName as OxGraphName, GraphNameRef, Literal as OxLiteral,
    NamedNode as OxNamedNode, NamedNodeRef, Quad as OxQuad, Term as OxTerm, Triple as OxTriple,
};
use crate::sparql::algebra::QueryDataset;
use crate::sparql::dataset::DatasetView;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::http::Client;
use crate::sparql::plan::EncodedTuple;
use crate::sparql::plan_builder::PlanBuilder;
use crate::sparql::{EvaluationError, UpdateOptions};
use crate::storage::io::load_graph;
use crate::storage::numeric_encoder::{Decoder, EncodedTerm, WriteEncoder};
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
            let quad = self.convert_quad(quad, &mut bnodes);
            let quad = self.storage.encode_quad(quad.as_ref())?;
            self.storage.insert(&quad)?;
        }
        Ok(())
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) -> Result<(), EvaluationError> {
        for quad in data {
            let quad = self.convert_ground_quad(quad);
            self.storage.remove(&quad.as_ref().into())?;
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
            let tuple = tuple?;
            for quad in delete {
                if let Some(quad) =
                    self.convert_ground_quad_pattern(quad, &variables, &tuple, &dataset)?
                {
                    self.storage.remove(&quad.as_ref().into())?;
                }
            }
            for quad in insert {
                if let Some(quad) =
                    self.convert_quad_pattern(quad, &variables, &tuple, &dataset, &mut bnodes)?
                {
                    let quad = self.storage.encode_quad(quad.as_ref())?;
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

    fn eval_create(&mut self, graph_name: &NamedNode, silent: bool) -> Result<(), EvaluationError> {
        let encoded_graph_name = self
            .storage
            .encode_named_node(NamedNodeRef::new_unchecked(&graph_name.iri))?;
        if self.storage.contains_named_graph(&encoded_graph_name)? {
            if silent {
                Ok(())
            } else {
                Err(EvaluationError::msg(format!(
                    "The graph {} already exists",
                    graph_name
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
                let encoded_graph_name = self
                    .storage
                    .encode_named_node(NamedNodeRef::new_unchecked(&graph_name.iri))?;
                if self.storage.contains_named_graph(&encoded_graph_name)? {
                    Ok(self.storage.clear_graph(&encoded_graph_name)?)
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
                let encoded_graph_name = self
                    .storage
                    .encode_named_node(NamedNodeRef::new_unchecked(&graph_name.iri))?;
                if self.storage.contains_named_graph(&encoded_graph_name)? {
                    self.storage.remove_named_graph(&encoded_graph_name)?;
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

    fn convert_quad(&self, quad: &Quad, bnodes: &mut HashMap<BlankNode, OxBlankNode>) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                Subject::NamedNode(subject) => self.convert_named_node(subject).into(),
                Subject::BlankNode(subject) => self.convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => self.convert_triple(subject, bnodes).into(),
            },
            predicate: self.convert_named_node(&quad.predicate),
            object: match &quad.object {
                Term::NamedNode(object) => self.convert_named_node(object).into(),
                Term::BlankNode(object) => self.convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => self.convert_literal(object).into(),
                Term::Triple(subject) => self.convert_triple(subject, bnodes).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => self.convert_named_node(graph_name).into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_triple(
        &self,
        triple: &Triple,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> OxTriple {
        OxTriple {
            subject: match &triple.subject {
                Subject::NamedNode(subject) => self.convert_named_node(subject).into(),
                Subject::BlankNode(subject) => self.convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => self.convert_triple(subject, bnodes).into(),
            },
            predicate: self.convert_named_node(&triple.predicate),
            object: match &triple.object {
                Term::NamedNode(object) => self.convert_named_node(object).into(),
                Term::BlankNode(object) => self.convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => self.convert_literal(object).into(),
                Term::Triple(subject) => self.convert_triple(subject, bnodes).into(),
            },
        }
    }

    fn convert_named_node(&self, node: &NamedNode) -> OxNamedNode {
        OxNamedNode::new_unchecked(&node.iri)
    }

    fn convert_blank_node(
        &self,
        node: &BlankNode,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> OxBlankNode {
        bnodes.entry(node.clone()).or_default().clone()
    }

    fn convert_literal(&self, literal: &Literal) -> OxLiteral {
        match literal {
            Literal::Simple { value } => OxLiteral::new_simple_literal(value),
            Literal::LanguageTaggedString { value, language } => {
                OxLiteral::new_language_tagged_literal_unchecked(value, language)
            }
            Literal::Typed { value, datatype } => {
                OxLiteral::new_typed_literal(value, NamedNodeRef::new_unchecked(&datatype.iri))
            }
        }
    }

    fn convert_ground_quad(&self, quad: &GroundQuad) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                GroundSubject::NamedNode(subject) => self.convert_named_node(subject).into(),
                GroundSubject::Triple(subject) => self.convert_ground_triple(subject).into(),
            },
            predicate: self.convert_named_node(&quad.predicate),
            object: match &quad.object {
                GroundTerm::NamedNode(object) => self.convert_named_node(object).into(),
                GroundTerm::Literal(object) => self.convert_literal(object).into(),
                GroundTerm::Triple(subject) => self.convert_ground_triple(subject).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => self.convert_named_node(graph_name).into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_ground_triple(&self, triple: &GroundTriple) -> OxTriple {
        OxTriple {
            subject: match &triple.subject {
                GroundSubject::NamedNode(subject) => self.convert_named_node(subject).into(),
                GroundSubject::Triple(subject) => self.convert_ground_triple(subject).into(),
            },
            predicate: self.convert_named_node(&triple.predicate),
            object: match &triple.object {
                GroundTerm::NamedNode(object) => self.convert_named_node(object).into(),
                GroundTerm::Literal(object) => self.convert_literal(object).into(),
                GroundTerm::Triple(subject) => self.convert_ground_triple(subject).into(),
            },
        }
    }

    fn convert_quad_pattern(
        &self,
        quad: &QuadPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxQuad>, EvaluationError> {
        Ok(Some(OxQuad {
            subject: match self.convert_term_or_var(
                &quad.subject,
                variables,
                values,
                dataset,
                bnodes,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                self.convert_named_node_or_var(&quad.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.convert_term_or_var(&quad.object, variables, values, dataset, bnodes)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) =
                self.convert_graph_name_or_var(&quad.graph_name, variables, values, dataset)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_term_or_var(
        &self,
        term: &TermPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxTerm>, EvaluationError> {
        Ok(match term {
            TermPattern::NamedNode(term) => Some(self.convert_named_node(term).into()),
            TermPattern::BlankNode(bnode) => Some(self.convert_blank_node(bnode, bnodes).into()),
            TermPattern::Literal(term) => Some(self.convert_literal(term).into()),
            TermPattern::Triple(triple) => self
                .convert_triple_pattern(triple, variables, values, dataset, bnodes)?
                .map(|t| t.into()),
            TermPattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .map(|node| dataset.decode_term(&node))
                .transpose()?,
        })
    }

    fn convert_named_node_or_var(
        &self,
        term: &NamedNodePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxNamedNode>, EvaluationError> {
        Ok(match term {
            NamedNodePattern::NamedNode(term) => Some(self.convert_named_node(term)),
            NamedNodePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .map(|node| dataset.decode_named_node(&node))
                .transpose()?,
        })
    }

    fn convert_graph_name_or_var(
        &self,
        term: &GraphNamePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxGraphName>, EvaluationError> {
        match term {
            GraphNamePattern::NamedNode(term) => Ok(Some(self.convert_named_node(term).into())),
            GraphNamePattern::DefaultGraph => Ok(Some(OxGraphName::DefaultGraph)),
            GraphNamePattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .map(|node| {
                    Ok(if node == EncodedTerm::DefaultGraph {
                        OxGraphName::DefaultGraph
                    } else {
                        dataset.decode_named_node(&node)?.into()
                    })
                })
                .transpose(),
        }
    }

    fn convert_triple_pattern(
        &self,
        triple: &TriplePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxTriple>, EvaluationError> {
        Ok(Some(OxTriple {
            subject: match self.convert_term_or_var(
                &triple.subject,
                variables,
                values,
                dataset,
                bnodes,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                self.convert_named_node_or_var(&triple.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.convert_term_or_var(&triple.object, variables, values, dataset, bnodes)?
            {
                object
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_ground_quad_pattern(
        &self,
        quad: &GroundQuadPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxQuad>, EvaluationError> {
        Ok(Some(OxQuad {
            subject: match self.convert_ground_term_or_var(
                &quad.subject,
                variables,
                values,
                dataset,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                self.convert_named_node_or_var(&quad.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.convert_ground_term_or_var(&quad.object, variables, values, dataset)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) =
                self.convert_graph_name_or_var(&quad.graph_name, variables, values, dataset)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_ground_term_or_var(
        &self,
        term: &GroundTermPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxTerm>, EvaluationError> {
        Ok(match term {
            GroundTermPattern::NamedNode(term) => Some(self.convert_named_node(term).into()),
            GroundTermPattern::Literal(term) => Some(self.convert_literal(term).into()),
            GroundTermPattern::Triple(triple) => self
                .convert_ground_triple_pattern(triple, variables, values, dataset)?
                .map(|t| t.into()),
            GroundTermPattern::Variable(v) => self
                .lookup_variable(v, variables, values)
                .map(|node| dataset.decode_term(&node))
                .transpose()?,
        })
    }

    fn convert_ground_triple_pattern(
        &self,
        triple: &GroundTriplePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxTriple>, EvaluationError> {
        Ok(Some(OxTriple {
            subject: match self.convert_ground_term_or_var(
                &triple.subject,
                variables,
                values,
                dataset,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                self.convert_named_node_or_var(&triple.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                self.convert_ground_term_or_var(&triple.object, variables, values, dataset)?
            {
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
        values: &EncodedTuple,
    ) -> Option<EncodedTerm> {
        variables
            .iter()
            .position(|v2| v == v2)
            .and_then(|i| values.get(i))
            .cloned()
    }
}
