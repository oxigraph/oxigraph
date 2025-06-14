use crate::model::{PyGraphNameRef, PyNamedNodeRef, PyNamedOrBlankNodeRef, PyQuad, PyTermRef};
use oxigraph::model::dataset::{CanonicalizationAlgorithm, Dataset};
use oxigraph::model::{Quad, QuadRef};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;

/// An in-memory `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_.
///
/// It can accommodate a fairly large number of quads (in the few millions).
///
/// Use :py:class:`Store` if you need on-disk persistence or SPARQL.
///
/// Warning: It interns the strings and does not do any garbage collection yet:
/// if you insert and remove a lot of different terms, memory will grow without any reduction.
///
/// :param quads: some quads to initialize the dataset with.
/// :type quads: collections.abc.Iterable[Quad] or None, optional
///
/// The :py:class:`str` function provides an N-Quads serialization:
///
/// >>> str(Dataset([Quad(NamedNode('http://example.com/s'), NamedNode('http://example.com/p'), NamedNode('http://example.com/o'), NamedNode('http://example.com/g'))]))
/// '<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n'
#[pyclass(name = "Dataset", module = "pyoxigraph", eq)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct PyDataset {
    inner: Dataset,
}

#[pymethods]
impl PyDataset {
    #[new]
    #[pyo3(signature = (quads = None))]
    fn new(quads: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut inner = Dataset::new();
        if let Some(quads) = quads {
            for quad in quads.try_iter()? {
                inner.insert(&*quad?.extract::<PyRef<'_, PyQuad>>()?);
            }
        }
        Ok(Self { inner })
    }

    /// Looks for the quads with the given subject.
    ///
    /// :param subject: the quad subject.
    /// :type subject: NamedNode or BlankNode or Triple
    /// :return: an iterator of the quads.
    /// :rtype: collections.abc.Iterator[Quad]
    ///
    /// >>> store = Dataset([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store.quads_for_subject(NamedNode('http://example.com')))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    fn quads_for_subject(&self, subject: PyNamedOrBlankNodeRef<'_>) -> QuadIter {
        QuadIter {
            inner: self
                .inner
                .quads_for_subject(&subject)
                .map(QuadRef::into_owned)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    /// Looks for the quads with the given predicate.
    ///
    /// :param predicate: the quad predicate.
    /// :type predicate: NamedNode
    /// :return: an iterator of the quads.
    /// :rtype: collections.abc.Iterator[Quad]
    ///
    /// >>> store = Dataset([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store.quads_for_predicate(NamedNode('http://example.com/p')))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    fn quads_for_predicate(&self, predicate: PyNamedNodeRef<'_>) -> QuadIter {
        QuadIter {
            inner: self
                .inner
                .quads_for_predicate(&predicate)
                .map(QuadRef::into_owned)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    /// Looks for the quads with the given object.
    ///
    /// :param object: the quad object.
    /// :type object: NamedNode or BlankNode or Literal or Triple
    /// :return: an iterator of the quads.
    /// :rtype: collections.abc.Iterator[Quad]
    ///
    /// >>> store = Dataset([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store.quads_for_object(Literal('1')))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    fn quads_for_object(&self, object: PyTermRef<'_>) -> QuadIter {
        QuadIter {
            inner: self
                .inner
                .quads_for_object(&object)
                .map(QuadRef::into_owned)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    /// Looks for the quads with the given graph name.
    ///
    /// :param graph_name: the quad graph name.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :return: an iterator of the quads.
    /// :rtype: collections.abc.Iterator[Quad]
    ///
    /// >>> store = Dataset([Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))])
    /// >>> list(store.quads_for_graph_name(NamedNode('http://example.com/g')))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[expect(clippy::needless_pass_by_value)]
    fn quads_for_graph_name(&self, graph_name: PyGraphNameRef<'_>) -> QuadIter {
        QuadIter {
            inner: self
                .inner
                .quads_for_graph_name(&graph_name)
                .map(QuadRef::into_owned)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    /// Adds a quad to the dataset.
    ///
    /// :param quad: the quad to add.
    /// :type quad: Quad
    /// :rtype: None
    ///
    /// >>> quad = Quad(NamedNode('http://example.com/s'), NamedNode('http://example.com/p'), NamedNode('http://example.com/o'), NamedNode('http://example.com/g'))
    /// >>> dataset = Dataset()
    /// >>> dataset.add(quad)
    /// >>> quad in dataset
    /// True
    fn add(&mut self, quad: &PyQuad) {
        self.inner.insert(quad);
    }

    /// Removes a quad from the dataset and raises an exception if it is not in the set.
    ///
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :rtype: None
    /// :raises KeyError: if the element was not in the set.
    ///
    /// >>> quad = Quad(NamedNode('http://example.com/s'), NamedNode('http://example.com/p'), NamedNode('http://example.com/o'), NamedNode('http://example.com/g'))
    /// >>> dataset = Dataset([quad])
    /// >>> dataset.remove(quad)
    /// >>> quad in dataset
    /// False
    fn remove(&mut self, quad: &PyQuad) -> PyResult<()> {
        if self.inner.remove(quad) {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!(
                "{} is not in the Dataset",
                QuadRef::from(quad)
            )))
        }
    }

    /// Removes a quad from the dataset if it is present.
    ///
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :rtype: None
    ///
    /// >>> quad = Quad(NamedNode('http://example.com/s'), NamedNode('http://example.com/p'), NamedNode('http://example.com/o'), NamedNode('http://example.com/g'))
    /// >>> dataset = Dataset([quad])
    /// >>> dataset.discard(quad)
    /// >>> quad in dataset
    /// False
    fn discard(&mut self, quad: &PyQuad) {
        self.inner.remove(quad);
    }

    /// Removes all quads from the dataset.
    ///
    /// :rtype: None
    ///
    /// >>> quad = Quad(NamedNode('http://example.com/s'), NamedNode('http://example.com/p'), NamedNode('http://example.com/o'), NamedNode('http://example.com/g'))
    /// >>> dataset = Dataset([quad])
    /// >>> dataset.clear()
    /// >>> len(dataset)
    /// 0
    fn clear(&mut self) {
        self.inner.clear()
    }

    /// Canonicalizes the dataset by renaming blank nodes.
    ///
    /// Warning: Blank node ids depends on the current shape of the graph. Adding a new quad might change the ids of a lot of blank nodes.
    /// Hence, this canonization might not be suitable for diffs.
    ///
    /// Warning: This implementation worst-case complexity is in *O(b!)* with *b* the number of blank nodes in the input dataset.
    ///
    /// :param algorithm: the canonicalization algorithm to use.
    /// :type algorithm: CanonicalizationAlgorithm
    /// :rtype: None
    ///
    /// >>> d1 = Dataset([Quad(BlankNode(), NamedNode('http://example.com/p'), BlankNode())])
    /// >>> d2 = Dataset([Quad(BlankNode(), NamedNode('http://example.com/p'), BlankNode())])
    /// >>> d1 == d2
    /// False
    /// >>> d1.canonicalize(CanonicalizationAlgorithm.UNSTABLE)
    /// >>> d2.canonicalize(CanonicalizationAlgorithm.UNSTABLE)
    /// >>> d1 == d2
    /// True
    fn canonicalize(&mut self, algorithm: &PyCanonicalizationAlgorithm) {
        self.inner.canonicalize(algorithm.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_empty()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __contains__(&self, quad: &PyQuad) -> bool {
        self.inner.contains(quad)
    }

    fn __iter__(&self) -> QuadIter {
        // TODO: very inefficient
        QuadIter {
            inner: self
                .inner
                .iter()
                .map(QuadRef::into_owned)
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }
}

#[pyclass(unsendable, module = "pyoxigraph")]
pub struct QuadIter {
    inner: std::vec::IntoIter<Quad>,
}

#[pymethods]
impl QuadIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<PyQuad> {
        Some(self.inner.next()?.into())
    }
}

/// RDF canonicalization algorithms.
///
/// The following algorithms are supported:
///
/// * :py:attr:`CanonicalizationAlgorithm.UNSTABLE`: an unstable algorithm preferred by PyOxigraph.
#[pyclass(frozen, name = "CanonicalizationAlgorithm", module = "pyoxigraph")]
#[derive(Clone)]
pub struct PyCanonicalizationAlgorithm {
    inner: CanonicalizationAlgorithm,
}

#[pymethods]
impl PyCanonicalizationAlgorithm {
    /// The algorithm preferred by PyOxigraph.
    ///
    /// Warning: Might change between Oxigraph versions. No stability guaranties.
    #[classattr]
    const UNSTABLE: Self = Self {
        inner: CanonicalizationAlgorithm::Unstable,
    };

    fn __repr__(&self) -> String {
        format!(
            "<CanonicalizationAlgorithm {}>",
            match self.inner {
                CanonicalizationAlgorithm::Unstable => "unstable",
                _ => "unknown",
            }
        )
    }

    /// :rtype: CanonicalizationAlgorithm
    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// :type memo: typing.Any
    /// :rtype: CanonicalizationAlgorithm
    #[expect(unused_variables)]
    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, memo: &'_ Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}
