import unittest

from pyoxigraph import (
    Dataset,
    NamedNode,
    Quad,
    ShaclShapesGraph,
    ShaclValidator,
    shacl_validate,
)


class TestShaclShapesGraph(unittest.TestCase):
    def test_empty_shapes_graph(self) -> None:
        shapes = ShaclShapesGraph()
        self.assertTrue(shapes.is_empty())
        self.assertEqual(len(shapes), 0)

    def test_parse_simple_shape(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person .
        """)
        self.assertFalse(shapes.is_empty())
        self.assertGreater(len(shapes), 0)

    def test_parse_invalid_turtle(self) -> None:
        shapes = ShaclShapesGraph()
        with self.assertRaises(ValueError):
            shapes.parse("invalid turtle syntax @@@")

    def test_repr(self) -> None:
        shapes = ShaclShapesGraph()
        repr_str = repr(shapes)
        self.assertIn("ShaclShapesGraph", repr_str)
        self.assertIn("size=0", repr_str)


class TestShaclValidatorMinCount(unittest.TestCase):
    def test_valid_mincount(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person ;
                ex:name "Alice" .
        """)

        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)
        self.assertEqual(report.warning_count, 0)
        self.assertEqual(report.info_count, 0)

    def test_mincount_violation(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:bob a ex:Person .
        """)

        self.assertFalse(report.conforms)
        self.assertGreater(report.violation_count, 0)

        results = report.results()
        self.assertGreater(len(results), 0)

        violation = results[0]
        self.assertEqual(violation.severity, "Violation")
        self.assertIsNotNone(violation.focus_node)


class TestShaclValidatorDatatype(unittest.TestCase):
    def test_valid_datatype(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:age ;
                    sh:datatype xsd:integer
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            ex:alice a ex:Person ;
                ex:age 30 .
        """)

        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)

    def test_datatype_violation(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:age ;
                    sh:datatype xsd:integer
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:bob a ex:Person ;
                ex:age "thirty" .
        """)

        self.assertFalse(report.conforms)
        self.assertGreater(report.violation_count, 0)

        results = report.results()
        self.assertGreater(len(results), 0)
        self.assertEqual(results[0].severity, "Violation")


class TestShaclValidatorClass(unittest.TestCase):
    def test_valid_class(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:knows ;
                    sh:class ex:Person
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person ;
                ex:knows ex:bob .
            ex:bob a ex:Person .
        """)

        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)

    def test_class_violation(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:knows ;
                    sh:class ex:Person
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person ;
                ex:knows ex:robot .
            ex:robot a ex:Robot .
        """)

        self.assertFalse(report.conforms)
        self.assertGreater(report.violation_count, 0)


class TestShaclValidatorGraph(unittest.TestCase):
    def test_validate_graph(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        dataset = Dataset()
        dataset.add(
            Quad(
                NamedNode("http://example.org/alice"),
                NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                NamedNode("http://example.org/Person"),
            )
        )
        dataset.add(
            Quad(
                NamedNode("http://example.org/alice"),
                NamedNode("http://example.org/name"),
                "Alice",
            )
        )

        validator = ShaclValidator(shapes)
        report = validator.validate_graph(dataset)

        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)

    def test_validate_graph_with_violation(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        dataset = Dataset()
        dataset.add(
            Quad(
                NamedNode("http://example.org/bob"),
                NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                NamedNode("http://example.org/Person"),
            )
        )
        # No name property - should violate minCount

        validator = ShaclValidator(shapes)
        report = validator.validate_graph(dataset)

        self.assertFalse(report.conforms)
        self.assertGreater(report.violation_count, 0)


class TestShaclValidationReport(unittest.TestCase):
    def test_report_properties(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1 ;
                    sh:message "Person must have a name"
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:bob a ex:Person .
        """)

        results = report.results()
        self.assertGreater(len(results), 0)

        result = results[0]
        self.assertIsNotNone(result.focus_node)
        self.assertEqual(result.severity, "Violation")
        self.assertIsNotNone(result.message)

    def test_report_to_turtle(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:bob a ex:Person .
        """)

        turtle = report.to_turtle()
        self.assertIsInstance(turtle, str)
        self.assertGreater(len(turtle), 0)
        self.assertTrue("sh:ValidationReport" in turtle or "shacl#" in turtle)

    def test_report_repr(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person .
        """)

        repr_str = repr(report)
        self.assertIn("ShaclValidationReport", repr_str)
        self.assertIn("conforms=True", repr_str)


class TestShaclValidationResult(unittest.TestCase):
    def test_result_properties(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:age ;
                    sh:datatype xsd:integer ;
                    sh:message "Age must be an integer"
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person ;
                ex:age "not a number" .
        """)

        self.assertFalse(report.conforms)

        results = report.results()
        result = results[0]

        self.assertIsNotNone(result.focus_node)
        self.assertEqual(result.severity, "Violation")
        self.assertIsNotNone(result.value)  # The invalid value
        self.assertIsNotNone(result.message)  # The error message

    def test_result_repr(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:bob a ex:Person .
        """)

        results = report.results()
        result = results[0]

        repr_str = repr(result)
        self.assertIn("ShaclValidationResult", repr_str)
        self.assertIn("focusNode=", repr_str)


class TestShaclValidateConvenience(unittest.TestCase):
    def test_validate_passing(self) -> None:
        report = shacl_validate(
            shapes_data="""
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            """,
            data="""
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:name "Alice" .
            """,
        )

        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)

    def test_validate_failing(self) -> None:
        report = shacl_validate(
            shapes_data="""
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            """,
            data="""
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person .
            """,
        )

        self.assertFalse(report.conforms)
        self.assertGreater(report.violation_count, 0)

    def test_validate_invalid_shapes(self) -> None:
        with self.assertRaises(ValueError):
            shacl_validate(
                shapes_data="invalid shapes @@@",
                data="@prefix ex: <http://example.org/> .",
            )

    def test_validate_invalid_data(self) -> None:
        with self.assertRaises(ValueError):
            shacl_validate(
                shapes_data="@prefix sh: <http://www.w3.org/ns/shacl#> .",
                data="invalid data @@@",
            )


class TestComplexValidation(unittest.TestCase):
    def test_multiple_constraints(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:age ;
                    sh:datatype xsd:integer ;
                    sh:minInclusive 0 ;
                    sh:maxInclusive 150
                ] .
        """)

        validator = ShaclValidator(shapes)

        # Valid age
        report1 = validator.validate("""
            @prefix ex: <http://example.org/> .
            ex:alice a ex:Person ; ex:age 30 .
        """)
        self.assertTrue(report1.conforms)

        # Age too high
        report2 = validator.validate("""
            @prefix ex: <http://example.org/> .
            ex:bob a ex:Person ; ex:age 200 .
        """)
        self.assertFalse(report2.conforms)

    def test_multiple_shapes(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .

            ex:CompanyShape a sh:NodeShape ;
                sh:targetClass ex:Company ;
                sh:property [
                    sh:path ex:companyName ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("""
            @prefix ex: <http://example.org/> .

            ex:alice a ex:Person ;
                ex:name "Alice" .

            ex:acme a ex:Company .
        """)

        # Person conforms, Company doesn't
        self.assertFalse(report.conforms)
        self.assertEqual(report.violation_count, 1)

    def test_empty_data_graph(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape ;
                sh:targetClass ex:Person ;
                sh:property [
                    sh:path ex:name ;
                    sh:minCount 1
                ] .
        """)

        validator = ShaclValidator(shapes)
        report = validator.validate("")

        # No violations because no targets
        self.assertTrue(report.conforms)
        self.assertEqual(report.violation_count, 0)

    def test_validator_repr(self) -> None:
        shapes = ShaclShapesGraph()
        shapes.parse("""
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix ex: <http://example.org/> .

            ex:PersonShape a sh:NodeShape .
        """)

        validator = ShaclValidator(shapes)
        repr_str = repr(validator)
        self.assertEqual(repr_str, "<ShaclValidator>")


if __name__ == "__main__":
    unittest.main()
