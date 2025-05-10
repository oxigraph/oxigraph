use crate::io::{RdfFormat, RdfParser};
use crate::model::{GraphName as OxGraphName, GraphNameRef, Quad as OxQuad};
use crate::sparql::algebra::QueryDataset;
use crate::sparql::dataset::DatasetView;
use crate::sparql::http::Client;
use crate::sparql::{EvaluationError, Update, UpdateOptions};
use crate::storage::StorageWriter;
use oxiri::Iri;
use oxrdfio::LoadedDocument;
use rustc_hash::FxHashMap;
use sparesults::QuerySolution;
use spareval::{QueryEvaluator, QueryResults};
use spargebra::algebra::{GraphPattern, GraphTarget};
use spargebra::term::{
    BlankNode, GraphName, GraphNamePattern, GroundQuad, GroundQuadPattern, GroundSubject,
    GroundTerm, GroundTermPattern, GroundTriple, GroundTriplePattern, NamedNode, NamedNodePattern,
    Quad, QuadPattern, Subject, Term, TermPattern, Triple, TriplePattern,
};
use spargebra::{GraphUpdateOperation, Query};
use std::io;
use std::io::Read;

pub fn evaluate_update<'a, 'b: 'a>(
    transaction: &'a mut StorageWriter<'b>,
    update: &Update,
    options: &UpdateOptions,
) -> Result<(), EvaluationError> {
    SimpleUpdateEvaluator {
        transaction,
        base_iri: update.inner.base_iri.clone(),
        query_evaluator: options.query_options.clone().into_evaluator(),
        client: Client::new(
            options.query_options.http_timeout,
            options.query_options.http_redirection_limit,
        ),
    }
    .eval_all(&update.inner.operations, &update.using_datasets)
}

struct SimpleUpdateEvaluator<'a, 'b> {
    transaction: &'a mut StorageWriter<'b>,
    base_iri: Option<Iri<String>>,
    query_evaluator: QueryEvaluator,
    client: Client,
}

impl<'a, 'b: 'a> SimpleUpdateEvaluator<'a, 'b> {
    fn eval_all(
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
            } => self.eval_delete_insert(
                delete,
                insert,
                using_dataset.as_ref().unwrap_or(&QueryDataset::new()),
                pattern,
            ),
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                if let Err(error) = self.eval_load(source, destination) {
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
        let mut bnodes = FxHashMap::default();
        for quad in data {
            let quad = Self::convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref())?;
        }
        Ok(())
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) -> Result<(), EvaluationError> {
        for quad in data {
            let quad = Self::convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref())?;
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
        let QueryResults::Solutions(solutions) = self.query_evaluator.clone().execute(
            DatasetView::new(self.transaction.reader(), using),
            &Query::Select {
                dataset: None,
                pattern: algebra.clone(),
                base_iri: self.base_iri.clone(),
            },
        )?
        else {
            unreachable!("We provided a SELECT query, we must get back solutions")
        };

        let mut bnodes = FxHashMap::default();
        for solution in solutions {
            let solution = solution?;
            for quad in delete {
                if let Some(quad) = Self::fill_ground_quad_pattern(quad, &solution) {
                    self.transaction.remove(quad.as_ref())?;
                }
            }
            for quad in insert {
                if let Some(quad) = Self::fill_quad_pattern(quad, &solution, &mut bnodes) {
                    self.transaction.insert(quad.as_ref())?;
                }
            }
            bnodes.clear();
        }
        Ok(())
    }

    fn eval_load(&mut self, from: &NamedNode, to: &GraphName) -> Result<(), EvaluationError> {
        let (content_type, body) = self
            .client
            .get(
                from.as_str(),
                "application/n-triples, text/turtle, application/rdf+xml",
            )
            .map_err(|e| EvaluationError::Service(Box::new(e)))?;
        let format = RdfFormat::from_media_type(&content_type)
            .ok_or_else(|| EvaluationError::UnsupportedContentType(content_type))?;
        let to_graph_name = match to {
            GraphName::NamedNode(graph_name) => graph_name.into(),
            GraphName::DefaultGraph => GraphNameRef::DefaultGraph,
        };
        let client = self.client.clone();
        let parser = RdfParser::from_format(format)
            .rename_blank_nodes()
            .without_named_graphs()
            .with_default_graph(to_graph_name)
            .with_base_iri(from.as_str())
            .map_err(|e| {
                EvaluationError::Service(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid URL: {from}: {e}"),
                )))
            })?
            .for_reader(body)
            .with_document_loader(move |url| {
                let (content_type, mut body) = client.get(
                    url,
                    "application/n-triples, text/turtle, application/rdf+xml, application/ld+json",
                )?;
                let mut content = Vec::new();
                body.read_to_end(&mut content)?;
                Ok(LoadedDocument {
                    url: url.into(),
                    content,
                    format: RdfFormat::from_media_type(&content_type)
                        .ok_or_else(|| EvaluationError::UnsupportedContentType(content_type))?,
                })
            });
        for q in parser {
            self.transaction.insert(q?.as_ref())?;
        }
        Ok(())
    }

    fn eval_create(&mut self, graph_name: &NamedNode, silent: bool) -> Result<(), EvaluationError> {
        if self.transaction.insert_named_graph(graph_name.into())? || silent {
            Ok(())
        } else {
            Err(EvaluationError::GraphAlreadyExists(graph_name.clone()))
        }
    }

    fn eval_clear(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.as_ref().into())?
                {
                    Ok(self.transaction.clear_graph(graph_name.into())?)
                } else if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::GraphDoesNotExist(graph_name.clone()))
                }
            }
            GraphTarget::DefaultGraph => {
                self.transaction.clear_graph(GraphNameRef::DefaultGraph)?;
                Ok(())
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.clear_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear_all_graphs()?),
        }
    }

    fn eval_drop(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if self.transaction.remove_named_graph(graph_name.into())? || silent {
                    Ok(())
                } else {
                    Err(EvaluationError::GraphDoesNotExist(graph_name.clone()))
                }
            }
            GraphTarget::DefaultGraph => {
                Ok(self.transaction.clear_graph(GraphNameRef::DefaultGraph)?)
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.remove_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear()?),
        }
    }

    fn convert_quad(quad: &Quad, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                Subject::NamedNode(subject) => subject.clone().into(),
                Subject::BlankNode(subject) => Self::convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            predicate: quad.predicate.clone(),
            object: match &quad.object {
                Term::NamedNode(object) => object.clone().into(),
                Term::BlankNode(object) => Self::convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => object.clone().into(),
                Term::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => graph_name.clone().into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_triple(triple: &Triple, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> Triple {
        Triple {
            subject: match &triple.subject {
                Subject::NamedNode(subject) => subject.clone().into(),
                Subject::BlankNode(subject) => Self::convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            predicate: triple.predicate.clone(),
            object: match &triple.object {
                Term::NamedNode(object) => object.clone().into(),
                Term::BlankNode(object) => Self::convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => object.clone().into(),
                Term::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
        }
    }

    fn convert_blank_node(
        node: &BlankNode,
        bnodes: &mut FxHashMap<BlankNode, BlankNode>,
    ) -> BlankNode {
        bnodes.entry(node.clone()).or_default().clone()
    }

    fn convert_ground_quad(quad: &GroundQuad) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                GroundSubject::NamedNode(subject) => subject.clone().into(),
                GroundSubject::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            predicate: quad.predicate.clone(),
            object: match &quad.object {
                GroundTerm::NamedNode(object) => object.clone().into(),
                GroundTerm::Literal(object) => object.clone().into(),
                GroundTerm::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => graph_name.clone().into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_ground_triple(triple: &GroundTriple) -> Triple {
        Triple {
            subject: match &triple.subject {
                GroundSubject::NamedNode(subject) => subject.clone().into(),
                GroundSubject::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            predicate: triple.predicate.clone(),
            object: match &triple.object {
                GroundTerm::NamedNode(object) => object.clone().into(),
                GroundTerm::Literal(object) => object.clone().into(),
                GroundTerm::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
        }
    }

    fn fill_quad_pattern(
        quad: &QuadPattern,
        solution: &QuerySolution,
        bnodes: &mut FxHashMap<BlankNode, BlankNode>,
    ) -> Option<OxQuad> {
        Some(OxQuad {
            subject: match Self::fill_term_or_var(&quad.subject, solution, bnodes)? {
                Term::NamedNode(node) => node.into(),
                Term::BlankNode(node) => node.into(),
                Term::Triple(triple) => triple.into(),
                Term::Literal(_) => return None,
            },
            predicate: Self::fill_named_node_or_var(&quad.predicate, solution)?,
            object: Self::fill_term_or_var(&quad.object, solution, bnodes)?,
            graph_name: Self::fill_graph_name_or_var(&quad.graph_name, solution)?,
        })
    }

    fn fill_term_or_var(
        term: &TermPattern,
        solution: &QuerySolution,
        bnodes: &mut FxHashMap<BlankNode, BlankNode>,
    ) -> Option<Term> {
        Some(match term {
            TermPattern::NamedNode(term) => term.clone().into(),
            TermPattern::BlankNode(bnode) => Self::convert_blank_node(bnode, bnodes).into(),
            TermPattern::Literal(term) => term.clone().into(),
            TermPattern::Triple(triple) => {
                Self::fill_triple_pattern(triple, solution, bnodes)?.into()
            }
            TermPattern::Variable(v) => solution.get(v)?.clone(),
        })
    }

    fn fill_named_node_or_var(
        term: &NamedNodePattern,
        solution: &QuerySolution,
    ) -> Option<NamedNode> {
        Some(match term {
            NamedNodePattern::NamedNode(term) => term.clone(),
            NamedNodePattern::Variable(v) => {
                if let Term::NamedNode(s) = solution.get(v)? {
                    s.clone()
                } else {
                    return None;
                }
            }
        })
    }

    fn fill_graph_name_or_var(
        term: &GraphNamePattern,
        solution: &QuerySolution,
    ) -> Option<OxGraphName> {
        Some(match term {
            GraphNamePattern::NamedNode(term) => term.clone().into(),
            GraphNamePattern::DefaultGraph => OxGraphName::DefaultGraph,
            GraphNamePattern::Variable(v) => match solution.get(v)? {
                Term::NamedNode(node) => node.clone().into(),
                Term::BlankNode(node) => node.clone().into(),
                Term::Triple(_) | Term::Literal(_) => return None,
            },
        })
    }

    fn fill_triple_pattern(
        triple: &TriplePattern,
        solution: &QuerySolution,
        bnodes: &mut FxHashMap<BlankNode, BlankNode>,
    ) -> Option<Triple> {
        Some(Triple {
            subject: match Self::fill_term_or_var(&triple.subject, solution, bnodes)? {
                Term::NamedNode(node) => node.into(),
                Term::BlankNode(node) => node.into(),
                Term::Triple(triple) => triple.into(),
                Term::Literal(_) => return None,
            },
            predicate: Self::fill_named_node_or_var(&triple.predicate, solution)?,
            object: Self::fill_term_or_var(&triple.object, solution, bnodes)?,
        })
    }
    fn fill_ground_quad_pattern(
        quad: &GroundQuadPattern,
        solution: &QuerySolution,
    ) -> Option<OxQuad> {
        Some(OxQuad {
            subject: match Self::fill_ground_term_or_var(&quad.subject, solution)? {
                Term::NamedNode(node) => node.into(),
                Term::BlankNode(node) => node.into(),
                Term::Triple(triple) => triple.into(),
                Term::Literal(_) => return None,
            },
            predicate: Self::fill_named_node_or_var(&quad.predicate, solution)?,
            object: Self::fill_ground_term_or_var(&quad.object, solution)?,
            graph_name: Self::fill_graph_name_or_var(&quad.graph_name, solution)?,
        })
    }

    fn fill_ground_term_or_var(term: &GroundTermPattern, solution: &QuerySolution) -> Option<Term> {
        Some(match term {
            GroundTermPattern::NamedNode(term) => term.clone().into(),
            GroundTermPattern::Literal(term) => term.clone().into(),
            GroundTermPattern::Triple(triple) => {
                Self::fill_ground_triple_pattern(triple, solution)?.into()
            }
            GroundTermPattern::Variable(v) => solution.get(v)?.clone(),
        })
    }

    fn fill_ground_triple_pattern(
        triple: &GroundTriplePattern,
        solution: &QuerySolution,
    ) -> Option<Triple> {
        Some(Triple {
            subject: match Self::fill_ground_term_or_var(&triple.subject, solution)? {
                Term::NamedNode(node) => node.into(),
                Term::BlankNode(node) => node.into(),
                Term::Triple(triple) => triple.into(),
                Term::Literal(_) => return None,
            },
            predicate: Self::fill_named_node_or_var(&triple.predicate, solution)?,
            object: Self::fill_ground_term_or_var(&triple.object, solution)?,
        })
    }
}
