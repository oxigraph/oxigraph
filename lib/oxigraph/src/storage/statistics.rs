//! Statistics about the stored data, used by the SPARQL optimizer to estimate cardinalities.

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::model::vocab::rdf;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::StorageError;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use rustc_hash::FxHashMap;

/// Number of quads after which live counts stop
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
pub const QUAD_COUNT_CAP: u64 = 1_000;

/// Counts about the triples using a predicate
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct PredicateCounts {
    pub triples: u64,
    pub distinct_subjects: u64,
    pub distinct_objects: u64,
}

/// Summary of a graph (or of a union of graphs)
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct GraphStatistics {
    pub triples: u64,
    pub predicates: FxHashMap<EncodedTerm, PredicateCounts>,
    /// Number of instances of each class (objects of `rdf:type`)
    pub classes: FxHashMap<EncodedTerm, u64>,
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
impl GraphStatistics {
    /// Counts the triples and distinct subjects from quads sorted by subject then predicate
    pub fn add_quads_sorted_by_subject_predicate(
        &mut self,
        quads: impl IntoIterator<Item = Result<EncodedQuad, StorageError>>,
    ) -> Result<(), StorageError> {
        let mut previous = None::<(EncodedTerm, EncodedTerm)>;
        for quad in quads {
            let quad = quad?;
            self.triples += 1;
            let counts = self.predicates.entry(quad.predicate.clone()).or_default();
            counts.triples += 1;
            if previous
                .as_ref()
                .is_none_or(|(s, p)| *s != quad.subject || *p != quad.predicate)
            {
                counts.distinct_subjects += 1;
                previous = Some((quad.subject, quad.predicate));
            }
        }
        Ok(())
    }

    /// Counts the distinct objects and the class instances from quads sorted by predicate then object
    pub fn add_quads_sorted_by_predicate_object(
        &mut self,
        quads: impl IntoIterator<Item = Result<EncodedQuad, StorageError>>,
    ) -> Result<(), StorageError> {
        let rdf_type = EncodedTerm::from(&rdf::TYPE);
        let mut previous = None::<(EncodedTerm, EncodedTerm)>;
        for quad in quads {
            let quad = quad?;
            if quad.predicate == rdf_type {
                *self.classes.entry(quad.object.clone()).or_default() += 1;
            }
            if previous
                .as_ref()
                .is_none_or(|(p, o)| *p != quad.predicate || *o != quad.object)
            {
                self.predicates
                    .entry(quad.predicate.clone())
                    .or_default()
                    .distinct_objects += 1;
                previous = Some((quad.predicate, quad.object));
            }
        }
        Ok(())
    }

    /// The estimated number of quads matching a pattern from the summary
    ///
    /// It is exact if there is no constant subject or object.
    pub fn quad_count_estimate(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
    ) -> Option<u64> {
        let Some(predicate) = predicate else {
            return (subject.is_none() && object.is_none()).then_some(self.triples);
        };
        let counts = self.predicates.get(predicate).copied().unwrap_or_default();
        Some(match (subject, object) {
            (None, None) => counts.triples,
            (Some(_), None) => counts.triples / counts.distinct_subjects.max(1),
            (None, Some(object)) => {
                if *predicate == EncodedTerm::from(&rdf::TYPE) {
                    self.classes.get(object).copied().unwrap_or(0)
                } else {
                    counts.triples / counts.distinct_objects.max(1)
                }
            }
            (Some(_), Some(_)) => 1,
        })
    }
}

/// Summary of the store content
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct StorageStatistics {
    pub default_graph: GraphStatistics,
    /// The union of all the named graphs
    pub named_graphs: GraphStatistics,
    pub named_graph_count: u64,
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
impl StorageStatistics {
    /// The summary for a graph, `Some(None)` encodes the default graph and `None` the union of the named graphs
    pub fn graph(&self, graph_name: Option<Option<&EncodedTerm>>) -> Option<&GraphStatistics> {
        match graph_name {
            Some(None) => Some(&self.default_graph),
            None => Some(&self.named_graphs),
            Some(Some(_)) => None,
        }
    }
}
