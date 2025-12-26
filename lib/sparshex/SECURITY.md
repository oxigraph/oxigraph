# ShEx Security Guide

This document describes security considerations, known attack vectors, and best practices for safely using ShEx validation in production environments.

## Table of Contents

1. [Overview](#overview)
2. [Known Attack Vectors](#known-attack-vectors)
3. [Resource Limits](#resource-limits)
4. [Shape Rejection Criteria](#shape-rejection-criteria)
5. [Safe Recursion Limits](#safe-recursion-limits)
6. [Production Configuration](#production-configuration)
7. [Threat Model](#threat-model)
8. [Security Checklist](#security-checklist)

## Overview

ShEx validation involves processing potentially untrusted schemas and data. This creates several attack surfaces where malicious actors could:

- Cause denial-of-service (DoS) through resource exhaustion
- Trigger stack overflow through deep recursion
- Execute ReDoS (Regular Expression Denial of Service) attacks
- Consume excessive memory through large data structures
- Cause infinite loops through cyclic references

This implementation includes comprehensive protections against these threats through configurable resource limits.

## Known Attack Vectors

### 1. Deeply Nested Shape References

**Attack**: A malicious schema with deeply nested shape references can cause stack overflow.

```shex
:Shape1 { :p @:Shape2 }
:Shape2 { :p @:Shape3 }
:Shape3 { :p @:Shape4 }
# ... hundreds or thousands of levels deep
```

**Impact**: Stack overflow, process crash, denial of service

**Mitigation**:
- Default `max_recursion_depth` of 100
- Track recursion depth and terminate early
- Use `ValidationLimits::strict()` for untrusted input (depth = 50)

### 2. Cyclic Shape References

**Attack**: Circular references between shapes can cause infinite recursion.

```shex
:PersonShape {
  :knows @:PersonShape *
}
```

**Impact**: Infinite recursion, stack overflow, denial of service

**Mitigation**:
- Recursion depth tracking prevents infinite loops
- Visited node tracking (implementation-specific)
- Shape reference counting (`max_shape_references`)

### 3. Combinatorial Explosion

**Attack**: Shapes with many logical operators (AND, OR, NOT) can cause exponential evaluation.

```shex
:Shape {
  :p1 @:S1 OR @:S2 OR @:S3 OR ... OR @:S100 ;
  :p2 @:S1 OR @:S2 OR @:S3 OR ... OR @:S100 ;
  # ... many more properties
}
```

**Impact**: Exponential time complexity, CPU exhaustion, timeout

**Mitigation**:
- `max_shape_references` limit (default: 1000)
- Validation timeout (default: 30 seconds)
- Shape reference counting across entire validation

### 4. Regex Denial of Service (ReDoS)

**Attack**: Crafted regular expressions with catastrophic backtracking.

```shex
:Shape {
  :value ["^(a+)+$"]  # Classic ReDoS pattern
  :email ["^([a-zA-Z0-9])(([\\-.]|[_]+)?([a-zA-Z0-9]+))*(@){1}[a-z0-9]+[.]{1}(([a-z]{2,3})|([a-z]{2,3}[.]{1}[a-z]{2,3}))$"]  # Complex pattern
}
```

**Impact**: CPU exhaustion, denial of service, indefinite processing

**Mitigation**:
- `max_regex_length` limit (default: 1000 characters)
- Validation timeout
- Regex compilation caching
- Consider disabling user-provided regex in high-security environments

**Dangerous Regex Patterns**:
- `(a+)+` - Nested quantifiers
- `(a*)*` - Nested zero-or-more
- `(a|ab)*` - Alternation with overlap
- `(a|a)*` - Redundant alternation

### 5. Excessive Memory Consumption

**Attack**: Shapes with very large value lists or examining many triples.

```shex
:Shape {
  :value [ "val1" "val2" "val3" ... "val100000" ]  # 100k values
}
```

**Impact**: Memory exhaustion, OOM kill, denial of service

**Mitigation**:
- `max_list_length` limit (default: 10,000)
- `max_triples_examined` limit (default: 100,000)
- Streaming validation where possible

### 6. Billion Laughs Attack (XML-style)

**Attack**: Nested shape expansions causing exponential data growth.

```shex
:L1 { :p @:L2 ; :p @:L2 }
:L2 { :p @:L3 ; :p @:L3 }
:L3 { :p @:L4 ; :p @:L4 }
# Each level doubles the evaluations
```

**Impact**: Exponential memory/CPU consumption

**Mitigation**:
- Shape reference counting
- Recursion depth limits
- Timeout enforcement

### 7. Large Graph Traversal

**Attack**: Shapes that force examination of entire large graphs.

```shex
:Shape {
  :connectedTo @:Shape *  # Must check all nodes in connected component
}
```

**Impact**: CPU/memory exhaustion on large graphs

**Mitigation**:
- `max_triples_examined` limit
- Timeout enforcement
- Consider graph size restrictions

## Resource Limits

### Default Limits (Development/Testing)

```rust
use sparshex::ValidationLimits;

let limits = ValidationLimits::default();
// max_recursion_depth: 100
// max_shape_references: 1000
// max_triples_examined: 100,000
// timeout: 30 seconds
// max_regex_length: 1000
// max_list_length: 10,000
```

### Strict Limits (Production/Public Services)

```rust
let limits = ValidationLimits::strict();
// max_recursion_depth: 50
// max_shape_references: 500
// max_triples_examined: 10,000
// timeout: 5 seconds
// max_regex_length: 500
// max_list_length: 1,000
```

### Permissive Limits (Trusted Environments)

```rust
let limits = ValidationLimits::permissive();
// max_recursion_depth: 500
// max_shape_references: 100,000
// max_triples_examined: 10,000,000
// timeout: None (disabled)
// max_regex_length: 10,000
// max_list_length: 1,000,000
```

### Custom Limits

```rust
use std::time::Duration;

let limits = ValidationLimits::new()
    .with_max_recursion_depth(75)
    .with_max_shape_references(2000)
    .with_max_triples_examined(50000)
    .with_timeout(Duration::from_secs(15))
    .with_max_regex_length(750)
    .with_max_list_length(5000);
```

## Shape Rejection Criteria

Reject shapes **before validation** if they exhibit these characteristics:

### 1. Excessive Recursion Depth
- Reject if maximum nesting depth > configured limit
- Analyze schema structure before accepting

### 2. Suspicious Regex Patterns
- Patterns longer than `max_regex_length`
- Nested quantifiers: `(a+)+`, `(a*)*`, `(a+)*`
- Excessive alternation: `(a|b|c|...|z)+` with many branches
- Very long character classes: `[a-zA-Z0-9...]{100,1000}`

### 3. Excessive List Sizes
- Value lists exceeding `max_list_length`
- Language tag lists exceeding reasonable sizes

### 4. Known Problematic Patterns
- Mutual recursion without base cases
- Shapes referencing themselves directly without guards
- Unbounded cardinality on recursive shapes: `@:Self *`

### 5. Schema Complexity Metrics
Consider rejecting if:
- Total number of shapes > 10,000
- Maximum shape fan-out > 100 (references to other shapes)
- Total constraints > 100,000

## Safe Recursion Limits

### Recursion Depth Guidelines

| Environment | Max Depth | Rationale |
|------------|-----------|-----------|
| Public API | 50 | Conservative limit for untrusted input |
| Internal Service | 100 | Balanced for typical schemas |
| Batch Processing | 200 | Handle complex but trusted schemas |
| Development | 500 | Permissive for testing |

### Stack Considerations

- Each recursion level consumes ~1-10KB of stack
- Default stack size: 2MB (Linux), 1MB (Windows)
- Safe maximum: ~100-200 levels with default stack
- Monitor actual stack usage in production

### Detecting Cycles

Beyond depth limits, implement cycle detection:

```rust
// Pseudo-code for cycle detection
let mut visited = HashSet::new();

fn validate_with_cycle_detection(shape_id) {
    if visited.contains(shape_id) {
        return Err("Cyclic reference detected");
    }
    visited.insert(shape_id);
    // ... validation logic ...
    visited.remove(shape_id);
}
```

## Production Configuration

### Recommended Settings by Use Case

#### Public API (Untrusted Input)
```rust
let limits = ValidationLimits::strict()
    .with_timeout(Duration::from_secs(3));

// Additional hardening:
// - Disable user-provided regex
// - Require schema pre-validation
// - Implement rate limiting
// - Monitor resource usage
```

#### Internal Service (Semi-Trusted)
```rust
let limits = ValidationLimits::default()
    .with_max_recursion_depth(75)
    .with_timeout(Duration::from_secs(10));

// Additional measures:
// - Schema allowlist/review process
// - Per-tenant resource quotas
// - Alerting on limit violations
```

#### Batch Processing (Trusted)
```rust
let limits = ValidationLimits::permissive()
    .with_timeout(Duration::from_secs(300))
    .with_max_recursion_depth(200);

// Additional measures:
// - Process isolation (containers/VMs)
// - Dedicated resource pools
// - Checkpoint/resume for long validations
```

#### Development/Testing
```rust
let limits = ValidationLimits::permissive()
    .without_timeout();

// Use only in controlled environments
```

### Defense in Depth

Layer multiple protections:

1. **Schema Validation**: Pre-validate schemas before acceptance
2. **Resource Limits**: Configure appropriate limits
3. **Timeout Enforcement**: Always set timeouts for untrusted input
4. **Rate Limiting**: Limit validation requests per user/IP
5. **Resource Monitoring**: Track CPU, memory, time metrics
6. **Alerting**: Alert on limit violations
7. **Isolation**: Use containers/sandboxes for validation
8. **Quotas**: Per-user/tenant resource quotas

### Monitoring and Alerting

Monitor these metrics:

- Validation request rate
- Average validation duration
- 95th/99th percentile duration
- Timeout rate
- Limit violation rate (by type)
- CPU usage during validation
- Memory usage during validation
- Schema complexity metrics

Alert on:
- Timeout rate > 5%
- Limit violation rate > 10%
- Average duration > 50% of timeout
- Unusual spikes in request rate
- Repeated violations from same source

## Threat Model

### Attacker Capabilities

**Low Sophistication**:
- Submit very large schemas
- Submit schemas with obvious recursion
- Basic ReDoS patterns

**Medium Sophistication**:
- Craft schemas with subtle combinatorial explosions
- Advanced ReDoS patterns
- Exploit interaction between multiple limits

**High Sophistication**:
- Craft schemas that maximize resource usage within limits
- Exploit parser vulnerabilities
- Combine multiple attack vectors
- Timing attacks based on validation behavior

### Assets Protected

- **CPU Resources**: Prevent exhaustion
- **Memory Resources**: Prevent OOM conditions
- **Service Availability**: Prevent DoS
- **Response Time**: Maintain SLA for legitimate users

### Out of Scope

This implementation does **not** protect against:

- **Information Disclosure**: Validation results may leak data structure
- **Side Channel Attacks**: Timing differences may leak information
- **Parser Vulnerabilities**: Bugs in ShEx parser itself
- **RDF Injection**: Malicious RDF content in data graphs
- **Logic Vulnerabilities**: Flaws in validation algorithm

Address these separately through:
- Careful result filtering
- Constant-time operations where needed
- Fuzzing and security testing
- Input sanitization
- Formal verification

## Security Checklist

Use this checklist when deploying ShEx validation:

### Schema Handling
- [ ] Validate schemas before acceptance
- [ ] Implement schema allowlist for production
- [ ] Reject schemas exceeding complexity thresholds
- [ ] Scan for dangerous regex patterns
- [ ] Check for excessive nesting/recursion

### Resource Configuration
- [ ] Configure appropriate `ValidationLimits`
- [ ] Always set timeout for untrusted input
- [ ] Set recursion depth ≤ 100 for public APIs
- [ ] Monitor and tune limits based on metrics
- [ ] Document limit configuration decisions

### Infrastructure
- [ ] Implement rate limiting
- [ ] Use resource quotas (CPU, memory)
- [ ] Deploy in isolated environments (containers)
- [ ] Configure appropriate stack sizes
- [ ] Set up monitoring and alerting

### Operational Security
- [ ] Log validation attempts and failures
- [ ] Alert on suspicious patterns
- [ ] Review limit violations regularly
- [ ] Maintain incident response procedures
- [ ] Conduct regular security reviews

### Testing
- [ ] Test with attack schemas
- [ ] Verify limit enforcement
- [ ] Load test with concurrent validations
- [ ] Test timeout behavior
- [ ] Fuzz test the validator

### Documentation
- [ ] Document validation limits for users
- [ ] Publish security guidelines
- [ ] Maintain runbook for incidents
- [ ] Document escalation procedures

## References

- ShEx Specification: https://shex.io/shex-semantics/
- OWASP ReDoS: https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS
- Billion Laughs Attack: https://en.wikipedia.org/wiki/Billion_laughs_attack
- SHACL Security Considerations: Similar validation security concerns

## Contact

For security issues specific to this implementation:
1. Do not open public issues for security vulnerabilities
2. Review SECURITY.md in the repository root for disclosure procedures
3. Follow responsible disclosure practices

## Version History

- 2025-12-26: Initial security guide for ShEx implementation
