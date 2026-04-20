"""Tests for the pyoxigraph.Reasoner binding.

The Rust reasoner itself is covered by an extensive integration suite in
`lib/oxreason/tests`. These Python level tests assert only that the binding
glue works: the class constructs, the configuration round trips, and that an
expand call materialises inferred triples back into the dataset's default
graph.
"""

import unittest

from pyoxigraph import (
    DefaultGraph,
    Dataset,
    NamedNode,
    Quad,
    Reasoner,
    ReasoningReport,
)

RDF_TYPE = NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
RDFS_SUB_CLASS_OF = NamedNode("http://www.w3.org/2000/01/rdf-schema#subClassOf")


def ex(local: str) -> NamedNode:
    return NamedNode(f"https://example.org/ontology#{local}")


class TestReasoner(unittest.TestCase):
    def test_default_profile_is_owl2_rl(self) -> None:
        reasoner = Reasoner()
        self.assertEqual(reasoner.profile, "owl2-rl")
        self.assertFalse(reasoner.equality_rules)

    def test_rdfs_profile(self) -> None:
        reasoner = Reasoner(profile="rdfs")
        self.assertEqual(reasoner.profile, "rdfs")

    def test_unknown_profile_raises(self) -> None:
        with self.assertRaises(ValueError):
            Reasoner(profile="not-a-profile")

    def test_equality_rules_flag_round_trips(self) -> None:
        reasoner = Reasoner(profile="owl2-rl", equality_rules=True)
        self.assertTrue(reasoner.equality_rules)

    def test_expand_cax_sco_materialises_superclass_type(self) -> None:
        """`cax-sco`: if ?x a ?c and ?c rdfs:subClassOf ?d then ?x a ?d."""
        dataset = Dataset()
        dataset.add(
            Quad(ex("Acme"), RDF_TYPE, ex("Company"), DefaultGraph())
        )
        dataset.add(
            Quad(
                ex("Company"),
                RDFS_SUB_CLASS_OF,
                ex("Organization"),
                DefaultGraph(),
            )
        )

        report = Reasoner(profile="owl2-rl").expand(dataset)

        self.assertIsInstance(report, ReasoningReport)
        self.assertGreaterEqual(report.added, 1)
        self.assertGreaterEqual(report.rounds, 1)
        inferred = Quad(
            ex("Acme"), RDF_TYPE, ex("Organization"), DefaultGraph()
        )
        self.assertIn(inferred, dataset)

    def test_expand_on_empty_dataset_is_a_noop(self) -> None:
        dataset = Dataset()
        report = Reasoner().expand(dataset)
        self.assertEqual(report.added, 0)
        self.assertEqual(len(dataset), 0)

    def test_report_repr_and_fields(self) -> None:
        dataset = Dataset()
        report = Reasoner().expand(dataset)
        self.assertEqual(report.added, 0)
        self.assertEqual(report.firings, 0)
        self.assertIn("ReasoningReport", repr(report))


if __name__ == "__main__":
    unittest.main()
