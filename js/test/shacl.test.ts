import assert from "node:assert";
import { webcrypto } from "node:crypto";
import { describe, it, vi } from "vitest";
import oxigraph from "../pkg/oxigraph.js";

// thread_rng: Node.js ES modules are not directly supported, see https://docs.rs/getrandom#nodejs-es-module-support
vi.stubGlobal("crypto", webcrypto);

const {
    ShaclShapesGraph,
    ShaclValidator,
    shaclValidate,
    Store,
    namedNode,
} = oxigraph;

describe("SHACL", () => {
    describe("ShaclShapesGraph", () => {
        it("should create an empty shapes graph", () => {
            const shapes = new ShaclShapesGraph();
            assert.strictEqual(shapes.isEmpty(), true);
            assert.strictEqual(shapes.size, 0);
        });

        it("should parse simple shape definition", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person .
            `);
            assert.strictEqual(shapes.isEmpty(), false);
            assert.ok(shapes.size > 0);
        });

        it("should throw error on invalid Turtle", () => {
            const shapes = new ShaclShapesGraph();
            assert.throws(() => {
                shapes.parse("invalid turtle syntax @@@");
            });
        });
    });

    describe("ShaclValidator - minCount constraint", () => {
        it("should validate data conforming to minCount constraint", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:name "Alice" .
            `);

            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
            assert.strictEqual(report.warningCount, 0);
            assert.strictEqual(report.infoCount, 0);
        });

        it("should detect minCount violation", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person .
            `);

            assert.strictEqual(report.conforms, false);
            assert.ok(report.violationCount > 0);

            const results = report.results();
            assert.ok(results.length > 0);

            const violation = results[0];
            assert.strictEqual(violation.severity, "Violation");
            assert.ok(violation.focusNode);
        });
    });

    describe("ShaclValidator - datatype constraint", () => {
        it("should validate correct datatype", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .
                @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:age ;
                        sh:datatype xsd:integer
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .
                @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

                ex:alice a ex:Person ;
                    ex:age 30 .
            `);

            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
        });

        it("should detect datatype violation", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .
                @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:age ;
                        sh:datatype xsd:integer
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person ;
                    ex:age "thirty" .
            `);

            assert.strictEqual(report.conforms, false);
            assert.ok(report.violationCount > 0);

            const results = report.results();
            assert.ok(results.length > 0);
            assert.strictEqual(results[0].severity, "Violation");
        });
    });

    describe("ShaclValidator - class constraint", () => {
        it("should validate correct class", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:knows ;
                        sh:class ex:Person
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:knows ex:bob .
                ex:bob a ex:Person .
            `);

            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
        });

        it("should detect class violation", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:knows ;
                        sh:class ex:Person
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:knows ex:robot .
                ex:robot a ex:Robot .
            `);

            assert.strictEqual(report.conforms, false);
            assert.ok(report.violationCount > 0);
        });
    });

    describe("ShaclValidator.validateStore", () => {
        it("should validate a Store object", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const store = new Store();
            store.add(
                oxigraph.quad(
                    namedNode("http://example.org/alice"),
                    namedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                    namedNode("http://example.org/Person"),
                ),
            );
            store.add(
                oxigraph.quad(
                    namedNode("http://example.org/alice"),
                    namedNode("http://example.org/name"),
                    oxigraph.literal("Alice"),
                ),
            );

            const validator = new ShaclValidator(shapes);
            const report = validator.validateStore(store);

            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
        });

        it("should detect violations in a Store object", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const store = new Store();
            store.add(
                oxigraph.quad(
                    namedNode("http://example.org/bob"),
                    namedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                    namedNode("http://example.org/Person"),
                ),
            );
            // No name property - should violate minCount

            const validator = new ShaclValidator(shapes);
            const report = validator.validateStore(store);

            assert.strictEqual(report.conforms, false);
            assert.ok(report.violationCount > 0);
        });
    });

    describe("ShaclValidationReport", () => {
        it("should provide access to validation results", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1 ;
                        sh:message "Person must have a name"
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person .
            `);

            const results = report.results();
            assert.ok(results.length > 0);

            const result = results[0];
            assert.ok(result.focusNode);
            assert.strictEqual(result.severity, "Violation");
            assert.ok(result.message); // Should have the message from the shape
        });

        it("should serialize to Turtle", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person .
            `);

            const turtle = report.toTurtle();
            assert.ok(typeof turtle === "string");
            assert.ok(turtle.length > 0);
            assert.ok(turtle.includes("sh:ValidationReport") || turtle.includes("shacl#"));
        });
    });

    describe("ShaclValidationResult", () => {
        it("should expose all result properties", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
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
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:age "not a number" .
            `);

            assert.strictEqual(report.conforms, false);

            const results = report.results();
            const result = results[0];

            assert.ok(result.focusNode);
            assert.strictEqual(result.severity, "Violation");
            assert.ok(result.value); // The invalid value
            assert.ok(result.message); // The error message
        });
    });

    describe("shaclValidate convenience function", () => {
        it("should validate with passing data", () => {
            const report = shaclValidate(
                `
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
                `,
                `
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:name "Alice" .
                `,
            );

            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
        });

        it("should validate with failing data", () => {
            const report = shaclValidate(
                `
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
                `,
                `
                @prefix ex: <http://example.org/> .

                ex:bob a ex:Person .
                `,
            );

            assert.strictEqual(report.conforms, false);
            assert.ok(report.violationCount > 0);
        });

        it("should throw on invalid shapes", () => {
            assert.throws(() => {
                shaclValidate("invalid shapes @@@", "@prefix ex: <http://example.org/> .");
            });
        });

        it("should throw on invalid data", () => {
            assert.throws(() => {
                shaclValidate(
                    "@prefix sh: <http://www.w3.org/ns/shacl#> .",
                    "invalid data @@@",
                );
            });
        });
    });

    describe("Complex validation scenarios", () => {
        it("should handle multiple constraints on same property", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
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
            `);

            const validator = new ShaclValidator(shapes);

            // Valid age
            const report1 = validator.validate(`
                @prefix ex: <http://example.org/> .
                ex:alice a ex:Person ; ex:age 30 .
            `);
            assert.strictEqual(report1.conforms, true);

            // Age too high
            const report2 = validator.validate(`
                @prefix ex: <http://example.org/> .
                ex:bob a ex:Person ; ex:age 200 .
            `);
            assert.strictEqual(report2.conforms, false);
        });

        it("should handle multiple shapes", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
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
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate(`
                @prefix ex: <http://example.org/> .

                ex:alice a ex:Person ;
                    ex:name "Alice" .

                ex:acme a ex:Company .
            `);

            // Person conforms, Company doesn't
            assert.strictEqual(report.conforms, false);
            assert.strictEqual(report.violationCount, 1);
        });

        it("should handle empty data graph", () => {
            const shapes = new ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.org/> .

                ex:PersonShape a sh:NodeShape ;
                    sh:targetClass ex:Person ;
                    sh:property [
                        sh:path ex:name ;
                        sh:minCount 1
                    ] .
            `);

            const validator = new ShaclValidator(shapes);
            const report = validator.validate("");

            // No violations because no targets
            assert.strictEqual(report.conforms, true);
            assert.strictEqual(report.violationCount, 0);
        });
    });
});
