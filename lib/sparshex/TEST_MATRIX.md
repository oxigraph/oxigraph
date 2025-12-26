# ShEx Test Coverage Matrix

This document provides a comprehensive overview of the test coverage for the ShEx (Shape Expressions) implementation in Oxigraph.

## Test Organization

```
lib/sparshex/
├── src/
│   └── tests.rs           # Unit tests for individual components
└── tests/
    └── integration.rs     # End-to-end integration tests
```

## Test Coverage by Component

### 1. Parser Tests (`src/tests.rs`)

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Empty schema parsing | Edge case | ✓ | `test_parse_empty_schema` |
| Simple shape parsing | Basic functionality | ✓ | `test_parse_simple_shape` |
| Cardinality operators (`*`, `+`) | Syntax | ✓ | `test_parse_shape_with_cardinality` |
| Min/max cardinality `{m,n}` | Syntax | ✓ | `test_parse_shape_with_minmax` |
| Invalid syntax handling | Error handling | ✓ | `test_parse_invalid_syntax` |
| Missing PREFIX declarations | Error handling | ✓ | `test_parse_missing_prefix` |
| Shape OR operator | Boolean logic | ✓ | `test_parse_shape_or` |
| Shape AND operator | Boolean logic | ✓ | `test_parse_shape_and` |
| Shape NOT operator | Boolean logic | ✓ | `test_parse_shape_not` |
| String node constraint | Datatype constraints | ✓ | `test_parse_node_constraint_string` |
| IRI node constraint | Datatype constraints | ✓ | `test_parse_node_constraint_iri` |

**Parser Coverage: 11 tests**

### 2. Model Tests (`src/tests.rs`)

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Empty schema creation | Basic API | ✓ | `test_shapes_schema_new` |
| ShapeExpression construction | Object model | ✓ | `test_shape_expression_construction` |
| TripleConstraint construction | Object model | ✓ | `test_triple_constraint_construction` |
| NodeConstraint datatype | Object model | ✓ | `test_node_constraint_datatype` |

**Model Coverage: 4 tests**

### 3. Validator Tests (`src/tests.rs`)

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Validator initialization | Basic API | ✓ | `test_validator_new` |
| Empty schema + empty data | Edge case | ✓ | `test_validate_empty_schema_empty_data` |
| Simple conforming validation | Basic validation | ✓ | `test_validate_simple_conforming` |
| Missing required property | Validation failure | ✓ | `test_validate_missing_required_property` |
| Wrong datatype | Type checking | ✓ | `test_validate_wrong_datatype` |
| Cardinality `*` (0 or more) | Cardinality | ✓ | `test_validate_cardinality_zero_or_more` |
| Cardinality `+` (1 or more) | Cardinality | ✓ | `test_validate_cardinality_one_or_more` |
| Exact cardinality `{1,1}` | Cardinality | ✓ | `test_validate_cardinality_exact` |
| Range cardinality `{1,3}` | Cardinality | ✓ | `test_validate_cardinality_range` |
| Empty shape | Edge case | ✓ | `test_empty_shape` |
| Nonexistent node | Error handling | ✓ | `test_nonexistent_node` |
| Nonexistent shape | Error handling | ✓ | `test_nonexistent_shape` |
| Max recursion depth | Cycle detection | ✓ | `test_max_recursion_depth` |
| Nested shape validation | Shape references | ✓ | `test_nested_shape_validation` |
| Nested validation failure | Shape references | ✓ | `test_nested_shape_validation_failure` |

**Validator Coverage: 15 tests**

### 4. Validation Report Tests (`src/tests.rs`)

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Conforming report | Report API | ✓ | `test_validation_report_conforms` |
| Violation report | Report API | ✓ | `test_validation_report_violation` |

**Report Coverage: 2 tests**

### 5. Boolean Operators Tests (`src/tests.rs`)

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| OR validation | Boolean logic | ✓ | `test_shape_or_validation` |
| AND validation | Boolean logic | ✓ | `test_shape_and_validation` |
| NOT validation | Boolean logic | ✓ | `test_shape_not_validation` |

**Boolean Operators Coverage: 3 tests**

## Integration Tests (`tests/integration.rs`)

### 6. End-to-End Validation

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Complete person validation | Real-world schema | ✓ | `test_complete_person_validation` |
| Address book validation | Complex structure | ✓ | `test_complete_address_book_validation` |

**E2E Coverage: 2 tests**

### 7. Multiple Shape Schemas

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Library schema (books, authors) | Multi-entity schema | ✓ | `test_multiple_shapes_library_schema` |
| Organization hierarchy | Recursive structure | ✓ | `test_organization_hierarchy_schema` |

**Multiple Shapes Coverage: 2 tests**

### 8. Error Handling and Validation Failures

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Detailed failure report | Error reporting | ✓ | `test_validation_failure_detailed_report` |
| Wrong cardinality failure | Validation failure | ✓ | `test_validation_failure_wrong_cardinality` |
| Nested shape failure | Validation failure | ✓ | `test_validation_failure_nested_shape` |

**Error Handling Coverage: 3 tests**

### 9. Complex Schemas

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Boolean combinations | AND/OR together | ✓ | `test_complex_boolean_combinations` |
| Deeply nested shapes | 4-level nesting | ✓ | `test_deeply_nested_shapes` |

**Complex Schema Coverage: 2 tests**

### 10. Special Cases and Edge Conditions

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Circular references | Graph cycles | ✓ | `test_circular_references_with_validation` |
| Optional properties | All optionality patterns | ✓ | `test_optional_properties_comprehensive` |
| Mixed datatypes | Multiple XSD types | ✓ | `test_mixed_datatype_validation` |

**Special Cases Coverage: 3 tests**

### 11. Full Graph Validation

| Test Case | Coverage | Status | Location |
|-----------|----------|--------|----------|
| Full graph validation | Whole-graph validation | ✓ | `test_full_graph_validation` |
| Batch validation | Multiple nodes | ✓ | `test_batch_validation_multiple_nodes` |

**Full Graph Coverage: 2 tests**

## Summary Statistics

| Category | Unit Tests | Integration Tests | Total |
|----------|-----------|-------------------|-------|
| Parser | 11 | 0 | 11 |
| Model | 4 | 0 | 4 |
| Validator | 15 | 0 | 15 |
| Report | 2 | 0 | 2 |
| Boolean Operators | 3 | 0 | 3 |
| End-to-End | 0 | 2 | 2 |
| Multiple Shapes | 0 | 2 | 2 |
| Error Handling | 0 | 3 | 3 |
| Complex Schemas | 0 | 2 | 2 |
| Special Cases | 0 | 3 | 3 |
| Full Graph | 0 | 2 | 2 |
| **TOTAL** | **35** | **14** | **49** |

## Feature Coverage Matrix

| Feature | Covered | Tests | Notes |
|---------|---------|-------|-------|
| **Parsing** |
| ShExC syntax | ✓ | 11 | Basic ShExC compact syntax |
| PREFIX declarations | ✓ | 2 | Required for all schemas |
| Shape definitions | ✓ | 8 | All shape types |
| Triple constraints | ✓ | 15 | Properties with datatypes |
| Cardinality | ✓ | 7 | `*`, `+`, `{m,n}` |
| Node constraints | ✓ | 3 | Datatypes, IRI, etc. |
| Boolean operators | ✓ | 6 | AND, OR, NOT |
| **Validation** |
| Basic shape matching | ✓ | 15 | Core validation logic |
| Cardinality checking | ✓ | 7 | All cardinality types |
| Datatype validation | ✓ | 5 | XSD datatypes |
| Shape references | ✓ | 6 | Nested shapes |
| Recursive shapes | ✓ | 2 | Circular references |
| Boolean logic | ✓ | 6 | AND/OR/NOT combinations |
| **Error Handling** |
| Parse errors | ✓ | 2 | Invalid syntax |
| Validation errors | ✓ | 8 | Constraint violations |
| Missing shapes | ✓ | 1 | Shape lookup failures |
| Recursion limits | ✓ | 1 | Cycle detection |
| **Reporting** |
| Conformance reports | ✓ | 2 | Success cases |
| Violation reports | ✓ | 4 | Failure details |
| **Advanced Features** |
| Value constraints | ⚠ | 0 | Future: value sets, ranges |
| String facets | ⚠ | 0 | Future: length, pattern |
| Numeric facets | ⚠ | 0 | Future: min, max values |
| Semantic actions | ⚠ | 0 | Future: custom actions |
| External schemas | ⚠ | 0 | Future: IMPORT |

**Legend:**
- ✓ Fully covered
- ⚠ Partially covered or planned
- ✗ Not covered

## W3C ShEx Test Suite Mapping

The [W3C ShEx Test Suite](https://github.com/shexSpec/shexTest) provides comprehensive conformance tests. This section maps our tests to the official test suite.

### Current Coverage by W3C Category

| W3C Category | Our Tests | W3C Tests | Coverage % | Priority |
|--------------|-----------|-----------|------------|----------|
| **Validation** |
| - Basic validation | 15 | ~50 | 30% | High |
| - Cardinality | 7 | ~30 | 23% | High |
| - Node constraints | 3 | ~40 | 7.5% | High |
| - Boolean operators | 6 | ~20 | 30% | Medium |
| - Recursive shapes | 2 | ~15 | 13% | Medium |
| **Parsing** |
| - ShExC parsing | 11 | ~100 | 11% | High |
| - ShExJ parsing | 0 | ~50 | 0% | Medium |
| **Negative Tests** |
| - Invalid schemas | 2 | ~30 | 6.7% | Medium |
| - Validation failures | 8 | ~40 | 20% | High |

### Integration Roadmap

#### Phase 1: Core Conformance (Priority: High)
- [ ] Download W3C ShEx test suite
- [ ] Set up test harness for manifest-driven testing
- [ ] Implement ShExJ (JSON) parser for test schemas
- [ ] Add ~50 basic validation tests from W3C suite
- [ ] Add ~30 cardinality tests
- [ ] Add ~20 negative validation tests

#### Phase 2: Extended Features (Priority: Medium)
- [ ] Add value constraint tests (value sets)
- [ ] Add string facet tests (length, pattern, minLength, maxLength)
- [ ] Add numeric facet tests (minInclusive, maxExclusive, etc.)
- [ ] Add IRI constraints and patterns
- [ ] Add BNode constraint tests

#### Phase 3: Advanced Features (Priority: Low)
- [ ] Semantic actions (if implementing)
- [ ] External schema imports
- [ ] SPARQL integration tests
- [ ] Performance benchmarks

## Property-Based Testing Ideas

Property-based testing can help discover edge cases. Here are test strategies:

### 1. Schema Generation Properties

```rust
// Property: Any valid ShEx schema should parse successfully
// Generate random valid schemas and ensure parsing succeeds
#[quickcheck]
fn prop_valid_schemas_parse(schema: ValidShexSchema) -> bool {
    parse_shex(&schema.to_string()).is_ok()
}

// Property: Parsing a schema and serializing it should be idempotent
#[quickcheck]
fn prop_parse_serialize_roundtrip(schema: ShexSchema) -> bool {
    let parsed = parse_shex(&schema).unwrap();
    let serialized = parsed.to_shexc();
    let reparsed = parse_shex(&serialized).unwrap();
    parsed == reparsed
}
```

### 2. Validation Properties

```rust
// Property: Empty shapes accept any node
#[quickcheck]
fn prop_empty_shape_accepts_all(node: RdfNode) -> bool {
    let schema = ShapesSchema::new();
    schema.add_shape(ShapeId::new("EmptyShape"), ShapeExpression::Empty);
    let validator = ShexValidator::new(schema);
    validator.validate_node(&node, &ShapeId::new("EmptyShape"))
        .unwrap()
        .conforms()
}

// Property: If a node conforms to ShapeA AND ShapeB,
// it should conform to (ShapeA AND ShapeB)
#[quickcheck]
fn prop_and_is_conjunction(node: RdfNode, shape_a: Shape, shape_b: Shape) -> bool {
    let conforms_a = validate_against(node, shape_a);
    let conforms_b = validate_against(node, shape_b);
    let conforms_and = validate_against(node, ShapeAnd::new(shape_a, shape_b));

    (conforms_a && conforms_b) == conforms_and
}

// Property: If a node conforms to ShapeA OR ShapeB,
// it should conform to at least one of them
#[quickcheck]
fn prop_or_is_disjunction(node: RdfNode, shape_a: Shape, shape_b: Shape) -> bool {
    let conforms_or = validate_against(node, ShapeOr::new(shape_a, shape_b));
    let conforms_a = validate_against(node, shape_a);
    let conforms_b = validate_against(node, shape_b);

    conforms_or == (conforms_a || conforms_b)
}
```

### 3. Cardinality Properties

```rust
// Property: Cardinality {n,n} should match exactly n occurrences
#[quickcheck]
fn prop_exact_cardinality(n: u32, values: Vec<RdfValue>) -> bool {
    let shape = create_shape_with_cardinality(n, n);
    let node = create_node_with_values(values.clone());
    let conforms = validate_against(node, shape);

    conforms == (values.len() == n as usize)
}

// Property: If data conforms with cardinality {m,n},
// removing values should eventually violate min
#[quickcheck]
fn prop_cardinality_minimum(m: u32, n: u32, values: Vec<RdfValue>) -> bool {
    let shape = create_shape_with_cardinality(m, n);
    // Property holds for values with length in range
}
```

### 4. Recursion Properties

```rust
// Property: Circular shape references should terminate
#[quickcheck]
fn prop_circular_shapes_terminate(depth: u32) -> bool {
    let schema = create_circular_schema(depth);
    let data = create_circular_data(depth);
    let result = validate(schema, data);

    // Should complete (not infinite loop) and return a result
    result.is_ok() || result.is_err()
}
```

### 5. Datatype Properties

```rust
// Property: Valid XSD datatypes should validate correctly
#[quickcheck]
fn prop_xsd_datatype_validation(value: XsdValue) -> bool {
    let shape = create_datatype_shape(value.datatype());
    let node = create_node_with_literal(value);

    validate_against(node, shape).conforms()
}

// Property: Wrong datatype should always fail
#[quickcheck]
fn prop_wrong_datatype_fails(expected: XsdDatatype, actual: XsdDatatype) -> bool {
    if expected == actual {
        return true; // Skip equal cases
    }

    let shape = create_datatype_shape(expected);
    let node = create_node_with_datatype(actual);

    !validate_against(node, shape).conforms()
}
```

## Test Dependencies

The tests require the following dependencies (already in `Cargo.toml`):

```toml
[dev-dependencies]
oxrdfio.workspace = true

# For future property-based testing
[dev-dependencies]
quickcheck = "1.0"  # To be added
proptest = "1.0"    # Alternative to quickcheck
```

## Running Tests

```bash
# Run all unit tests
cargo test -p sparshex

# Run only unit tests in lib
cargo test -p sparshex --lib

# Run only integration tests
cargo test -p sparshex --test integration

# Run specific test
cargo test -p sparshex test_validate_cardinality_range

# Run with output
cargo test -p sparshex -- --nocapture

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin -p sparshex --out Html
```

## Coverage Goals

| Component | Current | Target | Notes |
|-----------|---------|--------|-------|
| Parser | ~60% | 90% | Add error cases, all syntax forms |
| Model | ~40% | 80% | Add all constraint types |
| Validator | ~70% | 95% | Core logic, needs edge cases |
| Report | ~50% | 80% | Add detailed reporting tests |
| Overall | ~60% | 85% | Comprehensive coverage target |

## Future Test Additions

### High Priority
1. **ShExJ Parser Tests**: Add JSON format parsing tests
2. **Value Constraints**: Test value sets `[val1 val2 val3]`
3. **String Facets**: Test length, pattern, minLength, maxLength
4. **Numeric Facets**: Test minInclusive, maxExclusive, totalDigits, fractionDigits
5. **SPARQL Integration**: If `sparql` feature is enabled

### Medium Priority
1. **Language Tags**: Test language constraints `@en @fr`
2. **IRI Patterns**: Test IRI stems and patterns
3. **CLOSED Shapes**: Test closed vs. open shapes
4. **EXTRA Properties**: Test ignored properties
5. **Negated Property Sets**: Test `!prop`

### Low Priority
1. **Semantic Actions**: If implementing extensions
2. **ShapeMap Tests**: For focused validation
3. **Import/Include**: External schema references
4. **Start Shape**: Test default entry points
5. **Abstract Shapes**: If supporting inheritance

## Notes

- All tests follow the pattern from `sparshacl` integration tests
- Helper functions (`parse_turtle`, `nn`, `term`) are consistent across test files
- Tests use descriptive names following `test_<component>_<scenario>` pattern
- Integration tests focus on realistic, multi-shape scenarios
- Edge cases and error conditions are tested in both unit and integration tests

## Contributing

When adding new tests:

1. **Update this matrix** with new test coverage
2. **Follow naming conventions**: `test_<component>_<scenario>`
3. **Add documentation**: Describe what the test validates
4. **Group related tests**: Use comment sections
5. **Test both success and failure**: Don't just test happy paths
6. **Consider edge cases**: Empty data, missing properties, circular references

## References

- [ShEx Specification](https://shex.io/shex-spec/)
- [ShEx Primer](https://shex.io/shex-primer/)
- [W3C ShEx Test Suite](https://github.com/shexSpec/shexTest)
- [Oxigraph SHACL Tests](../sparshacl/tests/integration.rs) (reference implementation)
