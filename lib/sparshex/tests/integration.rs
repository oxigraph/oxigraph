//! Integration tests for ShEx validation.
//!
//! These tests cover end-to-end validation scenarios, complex schemas,
//! and interaction between multiple components.

use oxrdf::{Graph, Literal, NamedNode, Term, Triple};
use oxrdfio::{RdfFormat, RdfParser};
use sparshex::{
    parse_shex, ShapeExpression, ShapeId, ShapesSchema, ShexValidator, ValidationReport,
};

// =============================================================================
// Helper Functions
// =============================================================================

/// Helper to parse a Turtle string into a Graph.
fn parse_turtle(turtle: &str) -> Graph {
    let mut graph = Graph::new();
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    for quad_result in parser.for_reader(turtle.as_bytes()) {
        let quad = quad_result.expect("Failed to parse turtle");
        graph.insert(quad.as_ref());
    }
    graph
}

/// Helper to create a simple NamedNode.
fn nn(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

/// Helper to create a Term from a NamedNode.
fn term(iri: &str) -> Term {
    Term::NamedNode(nn(iri))
}

// =============================================================================
// End-to-End Validation Tests
// =============================================================================

#[test]
fn test_complete_person_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        ex:PersonShape {
            foaf:name xsd:string {1,1} ;
            foaf:age xsd:integer {0,1} ;
            foaf:email xsd:string * ;
            foaf:knows @ex:PersonShape *
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        @prefix foaf: <http://xmlns.com/foaf/0.1/> .

        ex:alice foaf:name "Alice Smith" ;
                 foaf:age 30 ;
                 foaf:email "alice@example.com" ;
                 foaf:email "alice@work.com" ;
                 foaf:knows ex:bob .

        ex:bob foaf:name "Bob Jones" ;
               foaf:age 25 .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Validate Alice
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.conforms(), "Alice should conform to PersonShape");

    // Validate Bob
    let result = validator.validate_node(&data, &term("http://example.org/bob"), &shape_id);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.conforms(), "Bob should conform to PersonShape");
}

#[test]
fn test_complete_address_book_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:address @ex:AddressShape + ;
            ex:phone @ex:PhoneShape *
        }

        ex:AddressShape {
            ex:street xsd:string ;
            ex:city xsd:string ;
            ex:zipCode xsd:string {0,1}
        }

        ex:PhoneShape {
            ex:number xsd:string ;
            ex:type [ "home" "work" "mobile" ]
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .

        ex:alice ex:name "Alice" ;
                 ex:address ex:addr1 ;
                 ex:address ex:addr2 ;
                 ex:phone ex:phone1 .

        ex:addr1 ex:street "123 Main St" ;
                 ex:city "Springfield" ;
                 ex:zipCode "12345" .

        ex:addr2 ex:street "456 Oak Ave" ;
                 ex:city "Portland" .

        ex:phone1 ex:number "555-1234" ;
                  ex:type "mobile" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.conforms(), "Complete address book should conform");
}

// =============================================================================
// Multiple Shape Schema Tests
// =============================================================================

#[test]
fn test_multiple_shapes_library_schema() {
    let shex = r#"
        PREFIX lib: <http://library.example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        lib:BookShape {
            lib:title xsd:string ;
            lib:author @lib:AuthorShape + ;
            lib:isbn xsd:string {0,1} ;
            lib:publishedYear xsd:integer
        }

        lib:AuthorShape {
            lib:name xsd:string ;
            lib:birthYear xsd:integer {0,1}
        }

        lib:LibraryShape {
            lib:name xsd:string ;
            lib:location xsd:string ;
            lib:hasBook @lib:BookShape *
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix lib: <http://library.example.org/> .

        lib:library1 lib:name "Central Library" ;
                     lib:location "Downtown" ;
                     lib:hasBook lib:book1 ;
                     lib:hasBook lib:book2 .

        lib:book1 lib:title "The Great Gatsby" ;
                  lib:author lib:author1 ;
                  lib:isbn "978-0-7432-7356-5" ;
                  lib:publishedYear 1925 .

        lib:book2 lib:title "1984" ;
                  lib:author lib:author2 ;
                  lib:publishedYear 1949 .

        lib:author1 lib:name "F. Scott Fitzgerald" ;
                    lib:birthYear 1896 .

        lib:author2 lib:name "George Orwell" ;
                    lib:birthYear 1903 .
    "#);

    // Validate library
    let library_shape = ShapeId::new(nn("http://library.example.org/LibraryShape"));
    let result = validator.validate_node(&data, &term("http://library.example.org/library1"), &library_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Library should conform");

    // Validate books
    let book_shape = ShapeId::new(nn("http://library.example.org/BookShape"));
    let result = validator.validate_node(&data, &term("http://library.example.org/book1"), &book_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Book 1 should conform");

    // Validate authors
    let author_shape = ShapeId::new(nn("http://library.example.org/AuthorShape"));
    let result = validator.validate_node(&data, &term("http://library.example.org/author1"), &author_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Author should conform");
}

#[test]
fn test_organization_hierarchy_schema() {
    let shex = r#"
        PREFIX org: <http://org.example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        org:CompanyShape {
            org:name xsd:string ;
            org:hasDepartment @org:DepartmentShape +
        }

        org:DepartmentShape {
            org:name xsd:string ;
            org:hasEmployee @org:EmployeeShape *
        }

        org:EmployeeShape {
            org:name xsd:string ;
            org:employeeId xsd:integer ;
            org:role xsd:string ;
            org:manager @org:EmployeeShape {0,1}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix org: <http://org.example.org/> .

        org:acme org:name "ACME Corp" ;
                 org:hasDepartment org:engineering ;
                 org:hasDepartment org:sales .

        org:engineering org:name "Engineering" ;
                       org:hasEmployee org:emp1 ;
                       org:hasEmployee org:emp2 .

        org:sales org:name "Sales" ;
                 org:hasEmployee org:emp3 .

        org:emp1 org:name "Alice" ;
                org:employeeId 101 ;
                org:role "Senior Engineer" .

        org:emp2 org:name "Bob" ;
                org:employeeId 102 ;
                org:role "Junior Engineer" ;
                org:manager org:emp1 .

        org:emp3 org:name "Charlie" ;
                org:employeeId 201 ;
                org:role "Sales Rep" .
    "#);

    let company_shape = ShapeId::new(nn("http://org.example.org/CompanyShape"));
    let result = validator.validate_node(&data, &term("http://org.example.org/acme"), &company_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Company hierarchy should conform");
}

// =============================================================================
// Error Handling and Validation Failure Tests
// =============================================================================

#[test]
fn test_validation_failure_detailed_report() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:age xsd:integer ;
            ex:email xsd:string +
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Data with multiple violations
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Should not conform due to missing properties");

    // Should have violations for missing age and email
    assert!(report.results().count() >= 2, "Should have at least 2 violations");
}

#[test]
fn test_validation_failure_wrong_cardinality() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string {1,1}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Data with too many names
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:name "Alicia" ;
                 ex:name "Ally" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Should not conform due to cardinality violation");
}

#[test]
fn test_validation_failure_nested_shape() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:address @ex:AddressShape
        }

        ex:AddressShape {
            ex:street xsd:string ;
            ex:city xsd:string ;
            ex:country xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Data with incomplete address
    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:address ex:addr1 .
        ex:addr1 ex:street "123 Main St" ;
                 ex:city "Springfield" .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));
    let result = validator.validate_node(&data, &term("http://example.org/alice"), &shape_id);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.conforms(), "Should not conform due to incomplete nested shape");
}

// =============================================================================
// Complex Schema Tests
// =============================================================================

#[test]
fn test_complex_boolean_combinations() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:VerifiedPersonShape = ex:PersonShape AND ex:VerificationShape

        ex:PersonShape {
            ex:name xsd:string ;
            ex:email xsd:string
        }

        ex:VerificationShape {
            ex:verified xsd:boolean ;
            ex:verifiedDate xsd:date
        }

        ex:ContactShape = ex:PersonShape OR ex:OrgShape

        ex:OrgShape {
            ex:orgName xsd:string ;
            ex:orgEmail xsd:string
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    // Test AND shape
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        ex:alice ex:name "Alice" ;
                 ex:email "alice@example.com" ;
                 ex:verified "true"^^xsd:boolean ;
                 ex:verifiedDate "2024-01-15"^^xsd:date .
    "#);

    let verified_shape = ShapeId::new(nn("http://example.org/VerifiedPersonShape"));
    let result = validator.validate_node(&data1, &term("http://example.org/alice"), &verified_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Should conform to AND shape");

    // Test OR shape with person data
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:bob ex:name "Bob" ;
               ex:email "bob@example.com" .
    "#);

    let contact_shape = ShapeId::new(nn("http://example.org/ContactShape"));
    let result = validator.validate_node(&data2, &term("http://example.org/bob"), &contact_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Should conform to OR shape (PersonShape branch)");

    // Test OR shape with org data
    let data3 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:acme ex:orgName "ACME Corp" ;
                ex:orgEmail "info@acme.com" .
    "#);

    let result = validator.validate_node(&data3, &term("http://example.org/acme"), &contact_shape);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Should conform to OR shape (OrgShape branch)");
}

#[test]
fn test_deeply_nested_shapes() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:UniversityShape {
            ex:name xsd:string ;
            ex:hasDepartment @ex:DepartmentShape +
        }

        ex:DepartmentShape {
            ex:deptName xsd:string ;
            ex:hasProfessor @ex:ProfessorShape +
        }

        ex:ProfessorShape {
            ex:profName xsd:string ;
            ex:teaches @ex:CourseShape *
        }

        ex:CourseShape {
            ex:courseName xsd:string ;
            ex:credits xsd:integer
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .

        ex:university ex:name "Example University" ;
                     ex:hasDepartment ex:cs_dept .

        ex:cs_dept ex:deptName "Computer Science" ;
                  ex:hasProfessor ex:prof1 .

        ex:prof1 ex:profName "Dr. Smith" ;
                ex:teaches ex:course1 ;
                ex:teaches ex:course2 .

        ex:course1 ex:courseName "Data Structures" ;
                  ex:credits 4 .

        ex:course2 ex:courseName "Algorithms" ;
                  ex:credits 4 .
    "#);

    let uni_shape = ShapeId::new(nn("http://example.org/UniversityShape"));
    let result = validator.validate_node(&data, &term("http://example.org/university"), &uni_shape);

    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Deeply nested structure should conform");
}

// =============================================================================
// Special Cases and Edge Conditions
// =============================================================================

#[test]
fn test_circular_references_with_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:friend @ex:PersonShape * ;
            ex:spouse @ex:PersonShape {0,1}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .

        ex:alice ex:name "Alice" ;
                 ex:friend ex:bob ;
                 ex:spouse ex:charlie .

        ex:bob ex:name "Bob" ;
               ex:friend ex:alice ;
               ex:friend ex:charlie .

        ex:charlie ex:name "Charlie" ;
                  ex:spouse ex:alice ;
                  ex:friend ex:bob .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // All three should validate even with circular references
    for person in ["alice", "bob", "charlie"] {
        let result = validator.validate_node(
            &data,
            &term(&format!("http://example.org/{}", person)),
            &shape_id
        );
        assert!(result.is_ok());
        assert!(result.unwrap().conforms(), "{} should conform", person);
    }
}

#[test]
fn test_optional_properties_comprehensive() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:nickname xsd:string {0,1} ;
            ex:age xsd:integer {0,1} ;
            ex:email xsd:string * ;
            ex:website IRI {0,1}
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);
    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Minimal data (only required property)
    let data1 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" .
    "#);

    let result = validator.validate_node(&data1, &term("http://example.org/alice"), &shape_id);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Minimal data should conform");

    // Maximal data (all optional properties)
    let data2 = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:bob ex:name "Bob" ;
               ex:nickname "Bobby" ;
               ex:age 25 ;
               ex:email "bob@example.com" ;
               ex:email "bob@work.com" ;
               ex:website <http://bob.example.com> .
    "#);

    let result = validator.validate_node(&data2, &term("http://example.org/bob"), &shape_id);
    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Maximal data should conform");
}

#[test]
fn test_mixed_datatype_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:MixedShape {
            ex:stringProp xsd:string ;
            ex:intProp xsd:integer ;
            ex:boolProp xsd:boolean ;
            ex:dateProp xsd:date ;
            ex:decimalProp xsd:decimal
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

        ex:entity ex:stringProp "Hello" ;
                  ex:intProp 42 ;
                  ex:boolProp "true"^^xsd:boolean ;
                  ex:dateProp "2024-01-15"^^xsd:date ;
                  ex:decimalProp "3.14"^^xsd:decimal .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/MixedShape"));
    let result = validator.validate_node(&data, &term("http://example.org/entity"), &shape_id);

    assert!(result.is_ok());
    assert!(result.unwrap().conforms(), "Mixed datatypes should conform");
}

// =============================================================================
// Full Validation (No Specific Shape) Tests
// =============================================================================

#[test]
fn test_full_graph_validation() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:age xsd:integer
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        ex:alice ex:name "Alice" ;
                 ex:age 30 .
        ex:bob ex:name "Bob" ;
               ex:age 25 .
    "#);

    // Validate entire graph without specifying nodes
    let result = validator.validate(&data);
    assert!(result.is_ok());

    // When no specific shape targets are defined, validation should pass
    let report = result.unwrap();
    assert!(report.conforms(), "Graph validation without targets should conform");
}

#[test]
fn test_batch_validation_multiple_nodes() {
    let shex = r#"
        PREFIX ex: <http://example.org/>
        PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

        ex:PersonShape {
            ex:name xsd:string ;
            ex:validId xsd:boolean
        }
    "#;

    let schema = parse_shex(shex).expect("Failed to parse schema");
    let validator = ShexValidator::new(schema);

    let data = parse_turtle(r#"
        @prefix ex: <http://example.org/> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

        ex:alice ex:name "Alice" ;
                 ex:validId "true"^^xsd:boolean .

        ex:bob ex:name "Bob" ;
               ex:validId "true"^^xsd:boolean .

        ex:charlie ex:name "Charlie" ;
                  ex:validId "false"^^xsd:boolean .
    "#);

    let shape_id = ShapeId::new(nn("http://example.org/PersonShape"));

    // Validate all three nodes
    let nodes = vec!["alice", "bob", "charlie"];
    for node in nodes {
        let result = validator.validate_node(
            &data,
            &term(&format!("http://example.org/{}", node)),
            &shape_id
        );
        assert!(result.is_ok(), "Validation should succeed for {}", node);
        assert!(result.unwrap().conforms(), "{} should conform", node);
    }
}
