# SHACL Performance Testing

This directory contains comprehensive performance testing infrastructure for SHACL validation.

## Quick Start

```bash
# Run performance tests
cargo test -p sparshacl --test validation_cost -- --nocapture

# Run benchmarks
cargo bench -p sparshacl validation_scaling

# Run interactive demo
cargo run -p sparshacl --example validation_cost_demo
```

## Components

### 1. Test Suite: `tests/validation_cost.rs`

Comprehensive test suite proving SHACL validation scaling characteristics.

**Tests:**
- `test_validation_scales_with_graph_size` - Proves O(affected_nodes) scaling
- `test_incremental_validation_not_possible` - Demonstrates no incremental validation
- `test_validation_cost_per_node` - Measures per-node validation cost (~5 μs)
- `test_complex_path_validation_bounded` - Verifies bounded cost for complex paths
- `test_target_matching_cost` - Measures target discovery overhead

**Key Findings:**
- ✓ Validation scales primarily with affected node count, not total graph size
- ✗ No incremental validation - every validation is a full scan
- ✓ Per-node cost is constant at ~5 μs
- ✓ Complex property paths are bounded

### 2. Benchmark Suite: `benches/validation_scaling.rs`

Criterion-based benchmarks for detailed performance analysis.

**Benchmarks:**
- `bench_validation_vs_graph_size` - Graph size impact on validation time
- `bench_validation_vs_target_nodes` - Affected node impact on validation time
- `bench_target_discovery` - Target matching cost measurement

Run with:
```bash
cargo bench -p sparshacl validation_scaling
```

### 3. Demo: `examples/validation_cost_demo.rs`

Interactive demonstration showing concrete measurements and production implications.

**Experiments:**
1. **Scaling Analysis** - Does validation scale with graph size or affected nodes?
2. **Incremental Validation** - Can we validate only new triples?
3. **Per-Node Cost** - What's the cost to validate one node?

Run with:
```bash
cargo run -p sparshacl --example validation_cost_demo
```

## Verification Dossier

See `VALIDATION_COST_VERIFICATION.md` for complete analysis including:
- Empirical evidence from tests
- Complexity analysis
- Admission control recommendations
- Production deployment checklist
- PM verdict: 🟡 SHIP WITH LIMITATIONS

## Cost Model

```
Total Validation Cost = O(target_discovery + affected_nodes × validation_per_node)

where:
  target_discovery      = O(type_triple_count)     for sh:targetClass
                        = O(1)                     for sh:targetNode
  affected_nodes        = count matching targets
  validation_per_node   = ~5 μs × constraint_count
```

## Key Insights

### ✅ What Works Well

1. **Efficient Validation**: ~5 μs per node is very fast
2. **Bounded Complexity**: Complex paths don't cause exponential blowup
3. **Predictable Scaling**: Linear with affected node count
4. **Isolated Validation**: Non-targeted data has minimal impact (~25% overhead for 100K triples)

### ❌ Known Limitations

1. **No Incremental Validation**: Adding 1 triple requires full re-scan
2. **Target Discovery Overhead**: `targetClass` scans type assertions
3. **No Caching**: Every validation is independent
4. **No Parallelization**: Single-threaded execution

### 🔧 Recommended Optimizations

1. **Prefer `targetNode`** over `targetClass` for O(1) discovery
2. **Batch Updates** before validation (not after each triple)
3. **Async Validation** for large graphs (don't block writes)
4. **Bound Shape Complexity** (max constraints, path depth)

## Production Guidelines

### Shape Design

✅ **Good:**
```turtle
ex:Shape sh:targetNode ex:knownEntity .     # O(1) discovery
ex:Shape sh:property [ sh:path ex:name ] .  # Simple path
ex:Shape sh:minCount 1 .                    # Simple constraint
```

⚠️ **Acceptable:**
```turtle
ex:Shape sh:targetClass ex:Person .         # O(type_assertions)
ex:Shape sh:property [
    sh:path ( ex:address ex:city )          # Sequence path
] .
```

❌ **Avoid:**
```turtle
ex:Shape sh:targetSubjectsOf ex:commonProp . # O(triple_count)
ex:Shape sh:property [
    sh:path [ sh:zeroOrMorePath ex:parent ]  # Unbounded recursion
] .
ex:Shape sh:and (                            # Deeply nested
    [ sh:and ( ... [ sh:and ( ... ) ] ) ]
) .
```

### Admission Control

Implement these checks before accepting SHACL shapes:

```rust
fn validate_shape_complexity(shape: &Shape) -> Result<(), AdmissionError> {
    // Limit constraint count
    if shape.constraints.len() > MAX_CONSTRAINTS {
        return Err(AdmissionError::TooManyConstraints);
    }

    // Limit path depth
    if shape.max_path_depth() > MAX_PATH_DEPTH {
        return Err(AdmissionError::PathTooDeep);
    }

    // Reject unbounded targets
    if shape.has_unbounded_target() {
        return Err(AdmissionError::UnboundedTarget);
    }

    Ok(())
}
```

### Monitoring

Track these metrics:

```
validation_time_ms              # Total validation duration
focus_node_count                # Number of nodes validated
constraint_evaluation_count     # Total constraint checks
graph_size_at_validation        # Triples in graph
shape_complexity                # Constraints per shape
```

Alert when:
```
validation_time_ms > SLA_THRESHOLD (e.g., 100ms)
```

## Testing Your Shapes

Use the demo to test your specific shapes:

```rust
// In your test file
use sparshacl::{ShaclValidator, ShapesGraph};

#[test]
fn test_my_shape_performance() {
    let shapes = load_my_shapes();
    let validator = ShaclValidator::new(shapes);

    // Create realistic data
    let data = create_test_data(1000); // 1K nodes

    let start = Instant::now();
    let report = validator.validate(&data).unwrap();
    let duration = start.elapsed();

    println!("Validation time: {:?}", duration);
    assert!(duration.as_millis() < 100, "Validation took too long");
}
```

## References

- **W3C SHACL Spec**: https://www.w3.org/TR/shacl/
- **Validation Algorithm**: `src/validator.rs`
- **Verification Dossier**: `VALIDATION_COST_VERIFICATION.md`

## Contributing

When adding new constraint types:

1. Add performance test in `tests/validation_cost.rs`
2. Update benchmark in `benches/validation_scaling.rs`
3. Document cost in `VALIDATION_COST_VERIFICATION.md`
4. Update cost model if needed

## License

Same as main Oxigraph project.
