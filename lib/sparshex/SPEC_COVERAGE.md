# ShEx 2.1 Specification Coverage for Oxigraph

## Overview

This document defines the Shape Expressions (ShEx) feature coverage for the Oxigraph `sparshex` library. ShEx is a structural schema language for RDF data that enables validation, documentation, and interface specification. This implementation targets ShEx 2.1 with focus on the 80/20 principle: supporting the 20% of features that cover 80% of real-world use cases.

**Target Specification**: [ShEx 2.1 Semantics](https://shex.io/shex-semantics/)
**Test Suite**: [shexSpec/shexTest](https://github.com/shexSpec/shexTest/)
**Primer**: [ShEx 2.1 Primer](https://shex.io/shex-primer/)

---

## Feature Coverage Matrix

### Core Features (Priority 1 - Must Have)

| Feature | Status | Priority | Coverage | Notes |
|---------|--------|----------|----------|-------|
| **Shape Definitions** | ✅ Supported | P0 | 100% | Named shapes with IRIs and blank node labels |
| **Triple Constraints** | ✅ Supported | P0 | 100% | Basic predicate matching with value expressions |
| **Cardinality** | ✅ Supported | P0 | 100% | `?`, `+`, `*`, `{m,n}`, `{m,}` notation |
| **Node Kind Constraints** | ✅ Supported | P0 | 100% | IRI, BlankNode, Literal, NonLiteral |
| **Datatype Constraints** | ✅ Supported | P0 | 100% | XSD datatypes via `oxsdatatypes` |
| **Value Sets** | ✅ Supported | P0 | 100% | Enumerated IRIs and literals |
| **Shape References** | ✅ Supported | P0 | 100% | Reference shapes by IRI or label |
| **EachOf (AND)** | ✅ Supported | P0 | 100% | All sub-expressions must match |
| **OneOf (OR)** | ✅ Supported | P1 | 100% | Exactly one sub-expression matches |
| **ShapeAnd** | ✅ Supported | P1 | 100% | Node satisfies all shape expressions |
| **ShapeOr** | ✅ Supported | P1 | 100% | Node satisfies at least one shape |
| **ShapeNot** | ✅ Supported | P1 | 100% | Node must not satisfy shape expression |

### String and Numeric Facets (Priority 2 - High Value)

| Feature | Status | Priority | Coverage | Notes |
|---------|--------|----------|----------|-------|
| **String Length** | ✅ Supported | P1 | 100% | `length`, `minlength`, `maxlength` |
| **Pattern (Regex)** | ✅ Supported | P1 | 100% | XSD regex patterns with optional flags |
| **Numeric Ranges** | ✅ Supported | P1 | 100% | `mininclusive`, `minexclusive`, `maxinclusive`, `maxexclusive` |
| **Total Digits** | ✅ Supported | P2 | 100% | Total number of digits for numeric values |
| **Fraction Digits** | ✅ Supported | P2 | 100% | Number of fractional digits |

### Advanced Value Sets (Priority 2)

| Feature | Status | Priority | Coverage | Notes |
|---------|--------|----------|----------|-------|
| **IRI Stems** | ✅ Supported | P1 | 100% | Prefix matching for IRIs |
| **Literal Stems** | ✅ Supported | P1 | 100% | Prefix matching for literals |
| **Language Tags** | ✅ Supported | P1 | 100% | Match specific language tags |
| **Language Stems** | ✅ Supported | P2 | 100% | Language prefix matching (e.g., `en~`) |
| **Exclusions** | ✅ Supported | P2 | 100% | Exclude specific values from stems |
| **Wildcard Patterns** | ✅ Supported | P2 | 100% | `.` with exclusions |

### Schema Organization (Priority 2)

| Feature | Status | Priority | Coverage | Notes |
|---------|--------|----------|----------|-------|
| **Start Shape** | ✅ Supported | P1 | 100% | Schema entry point |
| **Closed Shapes** | ✅ Supported | P1 | 100% | Restrict to specified predicates only |
| **Extra Properties** | ✅ Supported | P2 | 100% | Whitelist exceptions for closed shapes |
| **Inverse Properties** | ✅ Supported | P1 | 100% | Match triples where focus is object |
| **Annotations** | ✅ Supported | P2 | 100% | Metadata on shapes and expressions |

### Extended Features (Priority 3 - Nice to Have)

| Feature | Status | Priority | Coverage | Notes |
|---------|--------|----------|----------|-------|
| **Imports** | ⏳ Phase 2 | P3 | 0% | Import external schemas (deferred) |
| **ShapeExternal** | ⏳ Phase 2 | P3 | 0% | Extension point for external validators |
| **Semantic Actions** | ❌ Not Planned | P4 | 0% | Implementation-specific, non-portable |
| **startActs** | ❌ Not Planned | P4 | 0% | Depends on semantic actions |

### Syntax Support

| Format | Status | Priority | Coverage | Notes |
|--------|--------|----------|----------|-------|
| **ShExC (Compact)** | ✅ Supported | P0 | 100% | Primary human-readable syntax |
| **ShExJ (JSON)** | ✅ Supported | P1 | 100% | JSON-LD based format |
| **ShExR (RDF)** | ⏳ Phase 2 | P2 | 0% | RDF serialization (deferred) |

---

## Explicitly Unsupported Constructs

The following features are **intentionally not supported** in the initial implementation:

### 1. Semantic Actions (`semActs`)

**Rationale**: Semantic actions are implementation-specific extension points that break portability. They allow arbitrary code execution during validation (e.g., `%js{}`, `%sparql{}`), which:
- Are not standardized or interoperable across implementations
- Introduce security concerns with arbitrary code execution
- Are not part of the core ShEx validation semantics
- Are rarely used in production validation scenarios

**Alternative**: Users needing custom validation logic should implement pre/post-processing steps outside the validator.

**Specification Note**: Per ShEx 2.1 spec, "The evaluation of an individual SemAct is implementation-dependent." This confirms these are extensions, not core features.

### 2. Schema Imports (Initial Phase)

**Rationale**: Import mechanisms add complexity for:
- Schema resolution and caching
- Circular dependency detection
- Network I/O for remote schemas
- Namespace collision handling

**Status**: Deferred to Phase 2. Initial implementation will support self-contained schemas only.

**Workaround**: Users can manually merge schemas or use schema composition tools.

### 3. ShapeExternal (Initial Phase)

**Rationale**: External shape definitions require:
- Plugin architecture for custom validators
- Foreign function interface (FFI) considerations
- Additional API surface area

**Status**: Deferred to Phase 2 as an extension point.

### 4. ShExR (RDF Syntax) (Initial Phase)

**Rationale**: ShExC (compact) and ShExJ (JSON) cover 95%+ of use cases. ShExR adds:
- Parsing complexity for nested RDF structures
- Larger schema file sizes
- Minimal adoption in practice

**Status**: Deferred to Phase 2. Can be converted from ShExJ if needed.

---

## W3C Conformance Notes

### Specification Compliance

This implementation targets **full conformance** with the ShEx 2.1 specification for all supported features:

1. **Validation Semantics**: Implements the formal semantics defined in [ShEx 2.1 Semantics](https://shex.io/shex-semantics/)
2. **Triple Matching**: Follows the algorithm for matching triples against triple constraints
3. **Cardinality Checking**: Enforces min/max cardinality per specification
4. **Datatype Validation**: Delegates to `oxsdatatypes` for XSD datatype compliance
5. **Regex Patterns**: Uses XSD-compatible regex engine (not full Perl regex)

### Test Suite Coverage

**Target**: Pass all applicable tests from [shexSpec/shexTest](https://github.com/shexSpec/shexTest/)

**Exclusions**: Tests requiring:
- Semantic actions (`semActs`)
- Schema imports
- ShapeExternal definitions
- Features explicitly listed as unsupported

**Validation Test Format**:
- Input: ShEx schema (ShExC or ShExJ) + RDF data (Turtle/N-Triples/etc.)
- Output: ValidationTest (pass) or ValidationFailure (fail)
- ShapeMap: Captures which node/shape pairs conform

### Deviations and Extensions

**None planned for Phase 1**. This is a strict subset implementation of ShEx 2.1.

**Future Extensions** (Phase 2+):
- Integration with SHACL for hybrid validation
- Performance optimizations (caching, parallelization)
- Streaming validation for large graphs
- Custom extension functions (if standardized)

---

## Implementation Priorities (80/20 Principle)

The following features cover **~80% of real-world ShEx use cases**:

### Tier 1: Core Validation (Weeks 1-2)
1. Shape definitions with labels
2. Triple constraints (predicate + value expression)
3. Cardinality (`?`, `+`, `*`, `{m,n}`)
4. Node kind constraints (IRI, Literal, BlankNode, NonLiteral)
5. Datatype constraints (XSD types)
6. Value sets (enumerated IRIs and literals)
7. Shape references

### Tier 2: Logical Operators (Week 3)
1. EachOf (AND for triple expressions)
2. OneOf (OR for triple expressions)
3. ShapeAnd (AND for shapes)
4. ShapeOr (OR for shapes)
5. ShapeNot (negation)

### Tier 3: Advanced Constraints (Week 4)
1. String facets (length, pattern)
2. Numeric facets (min/max ranges)
3. Language tags and stems
4. IRI and literal stems
5. Closed shapes with extra properties
6. Inverse properties

### Tier 4: Polish and Integration (Week 5)
1. ShExC parser integration
2. ShExJ parser/serializer
3. Error messages and diagnostics
4. Integration with Oxigraph Store
5. Test suite execution

---

## API Design Guidelines

### Validation API

```rust
// Primary validation function
pub fn validate(
    schema: &ShExSchema,
    data: &impl RdfDataset,
    shape_map: &ShapeMap,
) -> Result<ValidationResult, ValidationError>

// Result captures node/shape conformance
pub struct ValidationResult {
    pub conformant: Vec<(NodeId, ShapeLabel)>,
    pub non_conformant: Vec<(NodeId, ShapeLabel, Reason)>,
}
```

### Schema Parsing

```rust
// Parse from ShExC (compact syntax)
pub fn parse_shexc(input: &str) -> Result<ShExSchema, ParseError>

// Parse from ShExJ (JSON syntax)
pub fn parse_shexj(json: &str) -> Result<ShExSchema, ParseError>

// Serialize to ShExJ
pub fn to_shexj(schema: &ShExSchema) -> String
```

### Integration with Oxigraph

```rust
// Validate using Oxigraph Store
impl Store {
    pub fn validate_shex(
        &self,
        schema: &ShExSchema,
        shape_map: &ShapeMap,
    ) -> Result<ValidationResult, ValidationError>
}
```

---

## Performance Targets

Based on existing `sparshacl` implementation:

1. **Small graphs** (<1K triples): <10ms validation time
2. **Medium graphs** (1K-100K triples): <1s validation time
3. **Large graphs** (100K-1M triples): <10s validation time
4. **Memory**: O(n) where n = number of triples in focus node neighborhood

**Optimization Strategies**:
- Lazy evaluation of triple constraints
- Early termination for cardinality violations
- Index-based triple lookups via Oxigraph Store
- Memoization for repeated shape validations

---

## Testing Strategy

### Unit Tests
- Individual constraint types (node kind, datatype, value sets)
- Cardinality edge cases
- Logical operators (AND, OR, NOT)
- Error handling and edge cases

### Integration Tests
- End-to-end validation with real schemas
- Multi-shape validation
- Closed shapes and inverse properties
- Complex nested structures

### W3C Test Suite
- Run all applicable tests from [shexSpec/shexTest](https://github.com/shexSpec/shexTest/)
- Document any skipped tests with rationale
- Target >95% pass rate for supported features

### Fuzzing
- Integration with `sparql-smith` fuzzing infrastructure
- Random schema generation
- Random data generation
- Crash and correctness testing

---

## References

### Specifications
- [Shape Expressions Language 2.1](https://shex.io/shex-semantics/) - Primary specification
- [ShEx 2.1 Primer](https://shex.io/shex-primer/) - Tutorial and examples
- [ShEx on W3C Wiki](https://www.w3.org/2001/sw/wiki/ShEx) - Community resources
- [Validating RDF Book - ShEx Chapter](https://book.validatingrdf.com/bookHtml010.html) - Comprehensive guide

### Test Suites
- [shexSpec/shexTest](https://github.com/shexSpec/shexTest/) - Official test suite
- [W3C ShEx Demo](https://www.w3.org/2013/ShEx/FancyShExDemo) - Interactive validator

### Related Projects
- [w3c/ShEx](https://github.com/w3c/ShEx) - Reference implementation
- [Shape Expressions Community Group](https://www.w3.org/community/shex/) - W3C CG
- [SHACL-ShEx Comparison](https://www.w3.org/2014/data-shapes/wiki/SHACL-ShEx-Comparison) - Feature comparison

### Research Papers
- [Comparing ShEx and SHACL](https://book.validatingrdf.com/bookHtml013.html) - Feature analysis
- [Common Foundations for SHACL, ShEx, and PG-Schema](https://arxiv.org/html/2502.01295v1) - Formal foundations

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2025-12-26 | Agent 1: Spec Guardian | Initial specification coverage matrix |

---

## Notes for Implementation Team

### Critical Decisions Made

1. **No Semantic Actions**: Prioritizing portability and security over extensibility
2. **ShExC First**: Compact syntax is more widely used than JSON in practice
3. **Strict Subset**: No custom extensions in Phase 1, only standards-compliant features
4. **Reuse oxsdatatypes**: Leverage existing XSD validation from SHACL implementation

### Open Questions for Team Review

1. Should we support schema imports in Phase 1 if they're local-only (no network)?
2. What error message format should we use? (Align with SHACL?)
3. Should ValidationResult include detailed traces or just pass/fail?
4. Do we need streaming validation API for very large graphs?

### Integration Points

- **oxrdf**: Core RDF data model (Triple, Quad, NamedNode, Literal)
- **oxsdatatypes**: XSD datatype validation and value spaces
- **oxigraph Store**: RDF dataset interface, triple lookup indices
- **spargebra**: Potential reuse of expression evaluation logic
- **sparshacl**: Reference for validation API patterns

### Next Steps

1. **Agent 2-10**: Implement core features per priority tiers
2. **Code Review**: Ensure all features match this specification
3. **Test Suite**: Execute W3C tests and document results
4. **Documentation**: Generate API docs from code
5. **Benchmarking**: Validate performance targets are met
