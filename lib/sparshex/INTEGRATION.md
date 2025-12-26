# ShEx Integration Guide for Oxigraph

## Overview

The `sparshex` crate provides Shape Expressions (ShEx) validation capabilities for RDF data within the Oxigraph ecosystem. This document outlines how sparshex integrates with other Oxigraph components and provides guidance for extending the integration.

## Module Boundaries

### Core Responsibilities

**sparshex** is responsible for:
- Parsing ShEx schemas (Compact Syntax, JSON-LD, RDF representation)
- Validating RDF data against ShEx shapes
- Reporting validation results with detailed error information
- Managing validation context and resource limits
- Providing a clean API for embedding in applications

**sparshex** does NOT:
- Store or manage RDF data (delegated to `oxrdf`)
- Parse RDF input formats (delegated to `oxrdfio`)
- Execute SPARQL queries (delegated to `spareval`)
- Perform SHACL validation (handled by `sparshacl`)

### Clear Separation from SHACL

While both ShEx and SHACL are validation languages for RDF, they are distinct:

| Aspect | ShEx (sparshex) | SHACL (sparshacl) |
|--------|-----------------|-------------------|
| Schema Language | Shape Expressions | Shapes Constraint Language |
| Syntax | Compact, JSON-LD, RDF | RDF/Turtle only |
| Validation Style | Pattern matching | Constraint checking |
| Expressiveness | Schemas as grammars | Constraint networks |
| Use Cases | Data exchange, documentation | Data quality, inference |

**Key Point**: These crates should remain independent. Applications may use both for different purposes.

## Dependency Diagram

```
┌─────────────────────────────────────────────────────────┐
│                     sparshex                            │
│  ┌──────────┐  ┌──────────┐  ┌───────────┐            │
│  │  parser  │  │validator │  │  result   │            │
│  │  (nom)   │  │          │  │           │            │
│  └─────┬────┘  └────┬─────┘  └─────┬─────┘            │
│        │            │              │                   │
│        └────────────┴──────────────┘                   │
│                     │                                   │
│              ┌──────▼──────┐                           │
│              │    model    │                           │
│              └──────┬──────┘                           │
└──────────────────────┼──────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
   ┌────▼────┐   ┌────▼────┐   ┌────▼──────┐
   │  oxrdf  │   │oxsdatatypes│ │thiserror│
   │         │   │           │  │          │
   │ Graph   │   │ XSD types │  │  Error   │
   │ Triple  │   │           │  │  types   │
   │ Quad    │   │           │  │          │
   └─────────┘   └───────────┘  └──────────┘

Optional Dependencies (Feature Flags):
   
   [sparql feature]
        │
   ┌────▼────────┐  ┌──────────┐
   │ spargebra   │  │ spareval │
   │ (algebra)   │  │ (query)  │
   └─────────────┘  └──────────┘
```

### Dependency Rationale

1. **oxrdf** - Core RDF data model (NamedNode, BlankNode, Literal, Graph, etc.)
2. **oxsdatatypes** - XSD datatype handling for value constraints
3. **thiserror** - Structured error types
4. **rustc-hash** - Fast hashing for internal caches
5. **regex** - Regular expression constraints in ShEx
6. **nom** (dev) - Parser combinator library for ShEx Compact Syntax

### Optional Dependencies

- **spargebra + spareval** (feature: `sparql`) - For SPARQL-based semantic actions in ShEx

## Integration Points

### 1. With oxigraph (Main Database)

```rust
// Future integration in lib/oxigraph/src/store.rs
use sparshex::{ShexSchema, ShexValidator};

impl Store {
    /// Validate RDF data against a ShEx schema
    pub fn validate_shex(&self, schema: &ShexSchema, start_node: &NamedNode) 
        -> Result<ValidationResult, ShexError> 
    {
        let validator = ShexValidator::new(schema.clone());
        let graph = self.get_graph(/* ... */)?;
        validator.validate(&graph, start_node.as_ref())
    }
}
```

### 2. With oxrdfio (I/O Layer)

ShEx schemas can be represented in multiple formats:
- ShExC (Compact Syntax) - Primary format
- ShEx JSON-LD - JSON representation
- RDF (Turtle/TriG) - RDF representation

```rust
// Future parser integration
use sparshex::parser::{parse_shexc, parse_shex_json};

// Parse ShExC
let schema = parse_shexc(shexc_string)?;

// Parse ShEx JSON-LD
let schema = parse_shex_json(json_string)?;
```

### 3. With JavaScript Bindings (js/)

Future `js/src/shex.rs` should provide:

```typescript
export class ShexSchema {
  constructor(schemaString: string, format: "compact" | "json");
  static parse(schema: string): ShexSchema;
  toString(): string;
}

export class ShexValidator {
  constructor(schema: ShexSchema);
  validate(store: Store, node: NamedNode): ValidationResult;
}

export class ValidationResult {
  readonly valid: boolean;
  readonly errors: string[];
  toString(): string;
}
```

**Integration Pattern**:
- Follow same patterns as `js/src/shacl.rs`
- Use `#[wasm_bindgen]` with `js_name` for camelCase
- Provide TypeScript declarations in custom sections
- Include comprehensive examples in `js/test/shex.test.ts`

### 4. With Python Bindings (python/)

Future `python/src/shex.rs` should provide:

```python
from pyoxigraph import Store, NamedNode, ShexSchema, ShexValidator

# Parse schema
schema = ShexSchema.parse(shexc_string, format="compact")

# Validate
validator = ShexValidator(schema)
result = validator.validate(store, NamedNode("http://example.org/node"))

if result.is_valid:
    print("Valid!")
else:
    for error in result.errors:
        print(f"Error: {error}")
```

**Integration Pattern**:
- Use PyO3 `#[pyclass]` and `#[pymethods]`
- Provide comprehensive docstrings
- Add to `python/tests/test_shex.py`

### 5. With CLI (cli/)

Future CLI integration for `oxigraph validate`:

```bash
# Validate RDF data against ShEx schema
oxigraph validate \
  --data data.ttl \
  --schema schema.shex \
  --start http://example.org/node \
  --format compact

# Output validation report
✓ Valid: http://example.org/node matches :PersonShape
✗ Invalid: http://example.org/other
  - Missing required property :name
  - Value "not-a-number" does not match XSD integer
```

## Integration Checklist

### Phase 1: Core Implementation ✓
- [x] Create sparshex crate structure
- [x] Define core model types (ShexSchema, ShapeExpr, TripleExpr)
- [x] Define error types (ShexError, ValidationError)
- [x] Define validation result types
- [x] Set up workspace integration

### Phase 2: Parser Development
- [ ] Implement ShExC (Compact Syntax) parser using nom
- [ ] Implement ShEx JSON-LD parser
- [ ] Implement RDF representation parser
- [ ] Add comprehensive parser tests
- [ ] Add W3C ShEx test suite integration

### Phase 3: Validator Implementation
- [ ] Implement core validation algorithm
- [ ] Support node constraints (datatypes, values, ranges)
- [ ] Support shape references and recursion
- [ ] Support triple expressions (properties, cardinality)
- [ ] Support logical operators (AND, OR, NOT)
- [ ] Add resource limits and timeout handling
- [ ] Implement validation caching for performance

### Phase 4: Advanced Features
- [ ] Support semantic actions (optional)
- [ ] Support ShEx extensions
- [ ] Support schema imports
- [ ] Performance optimization (parallel validation)
- [ ] Validation reporting (detailed error messages)

### Phase 5: Oxigraph Integration
- [ ] Add ShEx methods to `Store` in lib/oxigraph
- [ ] Integrate with transaction API
- [ ] Add bulk validation support
- [ ] Document integration patterns

### Phase 6: Language Bindings
- [ ] JavaScript/WASM bindings (js/src/shex.rs)
- [ ] TypeScript type definitions
- [ ] JavaScript tests and examples
- [ ] Python bindings (python/src/shex.rs)
- [ ] Python tests and examples
- [ ] Documentation for both bindings

### Phase 7: CLI Integration
- [ ] Add `validate` subcommand to oxigraph CLI
- [ ] Support multiple output formats (human, JSON, XML)
- [ ] Support batch validation
- [ ] Add comprehensive CLI tests

### Phase 8: Documentation & Testing
- [ ] Complete API documentation
- [ ] Add usage examples
- [ ] Performance benchmarks
- [ ] W3C test suite compliance report
- [ ] Integration testing with real-world schemas

## Performance Considerations

### Validation Performance

1. **Schema Compilation**: Parse once, validate many times
   ```rust
   // Good: Compile once
   let validator = ShexValidator::new(schema);
   for node in nodes {
       validator.validate(&graph, node)?;
   }
   
   // Bad: Re-compile each time
   for node in nodes {
       let validator = ShexValidator::new(schema.clone());
       validator.validate(&graph, node)?;
   }
   ```

2. **Resource Limits**: Prevent infinite loops and excessive memory use
   ```rust
   let limits = ValidationLimits {
       max_steps: 1_000_000,
       max_depth: 100,
   };
   validator.set_limits(limits);
   ```

3. **Caching**: Cache validation results for repeated shapes
   - Use `rustc-hash::FxHashMap` for fast lookups
   - Cache intermediate validation results
   - Invalidate cache when graph changes

### Benchmarking

Create benchmarks in `benches/validation.rs`:
- Small graph, simple schema
- Large graph, simple schema
- Small graph, complex schema (recursion, references)
- Large graph, complex schema
- Bulk validation (1000+ nodes)

Target: <1ms for simple validations, <100ms for complex validations

## Testing Strategy

### Unit Tests
- Parser: Test each grammar rule independently
- Validator: Test each constraint type
- Error handling: Test all error paths

### Integration Tests
- End-to-end validation scenarios
- Multi-format schema parsing
- Complex schema validation

### W3C Test Suite
- Import W3C ShEx test suite
- Track compliance percentage
- Document any deviations

### Fuzzing
- Add to `fuzz/` directory
- Fuzz parser with random ShEx input
- Fuzz validator with random graphs

## Security Considerations

1. **Resource Exhaustion**: Enforce validation limits
2. **Parser Safety**: Use safe Rust, no unsafe code
3. **Memory Safety**: Prevent unbounded memory growth
4. **Regex DoS**: Limit regex complexity and matching time

See `SECURITY.md` for full security policy.

## Future Enhancements

### Short-term (Next 3 months)
- Complete parser implementation
- Core validation algorithm
- Basic JavaScript bindings

### Medium-term (3-6 months)
- Python bindings
- CLI integration
- Performance optimization
- W3C test suite compliance

### Long-term (6-12 months)
- Distributed validation (validate across multiple stores)
- Incremental validation (validate only changed portions)
- Visual schema editor (web-based)
- Schema inference (generate ShEx from example data)
- SHACL ↔ ShEx conversion utilities

## Questions & Support

For integration questions:
1. Check this document first
2. Review existing integrations (sparshacl, spargeo)
3. Check Oxigraph documentation
4. Open an issue on GitHub

## References

- ShEx Specification: https://shex.io/shex-semantics/
- ShEx Primer: https://shex.io/shex-primer/
- W3C ShEx Test Suite: https://github.com/shexSpec/shexTest
- Oxigraph Documentation: https://oxigraph.org/
