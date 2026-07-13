use crate::{QueryEvaluationError, QuerySolutionIter};
#[cfg(feature = "sparql-12")]
use oxrdf::Triple;
use oxrdf::{BlankNode, GraphName, NamedNode, Quad, Term};
use rustc_hash::FxHashMap;
use sparesults::QuerySolution;
use spargebra::term::{
    GraphNamePattern, GroundQuadPattern, GroundTermPattern, NamedNodePattern, QuadPattern,
    TermPattern,
};
#[cfg(feature = "sparql-12")]
use spargebra::term::{GroundTriplePattern, TriplePattern};
use std::collections::VecDeque;
use std::mem::take;

/// A [`Quad`] to delete or insert.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum DeleteInsertQuad {
    Delete(Quad),
    Insert(Quad),
}

/// Output of [`PreparedDeleteInsertUpdate::execute`](super::PreparedDeleteInsertUpdate::execute).
pub struct DeleteInsertIter<'a, 'b> {
    solutions: QuerySolutionIter<'a>,
    ground_delete: Vec<Quad>,
    variable_delete: Vec<&'b GroundQuadPattern>,
    ground_insert: Vec<Quad>,
    variable_insert: Vec<&'b QuadPattern>,
    buffer: VecDeque<DeleteInsertQuad>,
    bnodes: FxHashMap<BlankNode, BlankNode>,
}

impl<'a, 'b> DeleteInsertIter<'a, 'b> {
    pub(crate) fn new(
        solutions: QuerySolutionIter<'a>,
        delete: &'b [GroundQuadPattern],
        insert: &'b [QuadPattern],
        without_optimizations: bool,
    ) -> Self {
        let mut ground_delete = Vec::new();
        let mut variable_delete = Vec::new();
        for quad_pattern in delete {
            let empty_solution = QuerySolution::default();
            if without_optimizations {
                variable_delete.push(quad_pattern);
            } else if let Some(quad) = fill_ground_quad_pattern(quad_pattern, &empty_solution) {
                ground_delete.push(quad);
            } else {
                variable_delete.push(quad_pattern);
            }
        }

        let mut ground_insert = Vec::new();
        let mut variable_insert = Vec::new();
        for quad_pattern in insert {
            let empty_solution = QuerySolution::default();
            let mut blank_nodes = FxHashMap::default();
            if without_optimizations {
                variable_insert.push(quad_pattern);
            } else if let Some(quad) =
                fill_quad_pattern(quad_pattern, &empty_solution, &mut blank_nodes)
            {
                if blank_nodes.is_empty() {
                    // We do this check to ensure we don't have to emit a new blank node for each solution.
                    ground_insert.push(quad);
                } else {
                    variable_insert.push(quad_pattern);
                }
            } else {
                variable_insert.push(quad_pattern);
            }
        }

        Self {
            solutions,
            ground_delete,
            variable_delete,
            ground_insert,
            variable_insert,
            buffer: VecDeque::new(),
            bnodes: FxHashMap::default(),
        }
    }
}

impl Iterator for DeleteInsertIter<'_, '_> {
    type Item = Result<DeleteInsertQuad, QueryEvaluationError>;

    fn next(&mut self) -> Option<Result<DeleteInsertQuad, QueryEvaluationError>> {
        loop {
            if let Some(quad) = self.buffer.pop_front() {
                return Some(Ok(quad));
            }
            let solution = match self.solutions.next() {
                Some(Ok(solution)) => solution,
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            for quad in take(&mut self.ground_delete) {
                self.buffer.push_back(DeleteInsertQuad::Delete(quad));
            }
            for quad in &self.variable_delete {
                if let Some(quad) = fill_ground_quad_pattern(quad, &solution) {
                    self.buffer.push_back(DeleteInsertQuad::Delete(quad));
                }
            }
            for quad in take(&mut self.ground_insert) {
                self.buffer.push_back(DeleteInsertQuad::Insert(quad));
            }
            for quad in &self.variable_insert {
                if let Some(quad) = fill_quad_pattern(quad, &solution, &mut self.bnodes) {
                    self.buffer.push_back(DeleteInsertQuad::Insert(quad));
                }
            }
            self.bnodes.clear();
        }
    }
}

fn fill_quad_pattern(
    quad: &QuadPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Quad> {
    Some(Quad {
        subject: match fill_term_or_var(&quad.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_term_or_var(&quad.object, solution, bnodes)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_term_or_var(
    term: &TermPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Term> {
    Some(match term {
        TermPattern::NamedNode(term) => term.clone().into(),
        TermPattern::BlankNode(bnode) => convert_blank_node(bnode, bnodes).into(),
        TermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "sparql-12")]
        TermPattern::Triple(triple) => fill_triple_pattern(triple, solution, bnodes)?.into(),
        TermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

fn fill_named_node_or_var(term: &NamedNodePattern, solution: &QuerySolution) -> Option<NamedNode> {
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

fn fill_graph_name_or_var(term: &GraphNamePattern, solution: &QuerySolution) -> Option<GraphName> {
    Some(match term {
        GraphNamePattern::NamedNode(term) => term.clone().into(),
        GraphNamePattern::DefaultGraph => GraphName::DefaultGraph,
        GraphNamePattern::Variable(v) => match solution.get(v)? {
            Term::NamedNode(node) => node.clone().into(),
            Term::BlankNode(node) => node.clone().into(),
            Term::Literal(_) => return None,
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
        },
    })
}

#[cfg(feature = "sparql-12")]
fn fill_triple_pattern(
    triple: &TriplePattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_term_or_var(&triple.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_term_or_var(&triple.object, solution, bnodes)?,
    })
}
fn fill_ground_quad_pattern(quad: &GroundQuadPattern, solution: &QuerySolution) -> Option<Quad> {
    Some(Quad {
        subject: match fill_ground_term_or_var(&quad.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_ground_term_or_var(&quad.object, solution)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_ground_term_or_var(term: &GroundTermPattern, solution: &QuerySolution) -> Option<Term> {
    Some(match term {
        GroundTermPattern::NamedNode(term) => term.clone().into(),
        GroundTermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "sparql-12")]
        GroundTermPattern::Triple(triple) => fill_ground_triple_pattern(triple, solution)?.into(),
        GroundTermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

#[cfg(feature = "sparql-12")]
fn fill_ground_triple_pattern(
    triple: &GroundTriplePattern,
    solution: &QuerySolution,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_ground_term_or_var(&triple.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_ground_term_or_var(&triple.object, solution)?,
    })
}

fn convert_blank_node(node: &BlankNode, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> BlankNode {
    bnodes.entry(node.clone()).or_default().clone()
}
