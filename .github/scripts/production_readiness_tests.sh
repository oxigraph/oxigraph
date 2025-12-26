#!/bin/bash
set -e

echo "=== Oxigraph Production Readiness Tests ==="
echo "Date: $(date)"
echo "Commit: $(git rev-parse HEAD)"
echo

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

# Helper functions
pass() {
    echo -e "${GREEN}✅ PASS${NC}: $1"
    ((PASS_COUNT++))
}

fail() {
    echo -e "${RED}❌ FAIL${NC}: $1"
    ((FAIL_COUNT++))
}

skip() {
    echo -e "${YELLOW}⚠️  SKIP${NC}: $1"
    ((SKIP_COUNT++))
}

# Test 1: Core RDF Model Tests
echo "=== Test 1: Core RDF Model ==="
if cargo test -p oxrdf --lib 2>&1 | tee /tmp/oxrdf_test.log | grep -q "test result: ok"; then
    pass "Core RDF model tests"
else
    fail "Core RDF model tests"
fi
echo

# Test 2: SPARQL Evaluation Tests
echo "=== Test 2: SPARQL Evaluation ==="
if cargo test -p spareval --lib 2>&1 | tee /tmp/spareval_test.log | grep -q "test result: ok"; then
    pass "SPARQL evaluation tests"
else
    fail "SPARQL evaluation tests"
fi
echo

# Test 3: SHACL Validation Tests
echo "=== Test 3: SHACL Validation ==="
if cargo test -p sparshacl 2>&1 | tee /tmp/shacl_test.log | grep -q "test result: ok"; then
    pass "SHACL validation tests"
else
    fail "SHACL validation tests"
fi
echo

# Test 4: ShEx Validation Tests
echo "=== Test 4: ShEx Validation ==="
if cargo test -p sparshex 2>&1 | tee /tmp/shex_test.log | grep -q "test result: ok"; then
    pass "ShEx validation tests"
else
    fail "ShEx validation tests"
fi
echo

# Test 5: OWL Reasoning Tests
echo "=== Test 5: OWL Reasoning ==="
if cargo test -p oxowl 2>&1 | tee /tmp/owl_test.log | grep -q "test result: ok"; then
    pass "OWL reasoning tests"
else
    fail "OWL reasoning tests"
fi
echo

# Test 6: Adversarial SPARQL Tests (expected to NOT exist)
echo "=== Test 6: Adversarial SPARQL Protection ==="
if cargo test -p spareval adversarial 2>&1 | grep -q "no test"; then
    fail "Adversarial SPARQL tests (NOT IMPLEMENTED)"
else
    skip "Adversarial SPARQL tests (checking if exists)"
fi
echo

# Test 7: Resource Limit Tests (expected to NOT exist)
echo "=== Test 7: Resource Limit Enforcement ==="
if cargo test resource_limits 2>&1 | grep -q "no test"; then
    fail "Resource limit tests (NOT IMPLEMENTED)"
else
    skip "Resource limit tests (checking if exists)"
fi
echo

# Test 8: Memory Leak Detection (expected to NOT exist)
echo "=== Test 8: Memory Leak Detection ==="
if cargo test memory_leak 2>&1 | grep -q "no test"; then
    fail "Memory leak detection (NOT IMPLEMENTED)"
else
    skip "Memory leak detection (checking if exists)"
fi
echo

# Test 9: Observability Tests (expected to NOT exist)
echo "=== Test 9: Observability Infrastructure ==="
if grep -r "use tracing::" lib/ 2>&1 | grep -q "tracing"; then
    pass "Observability infrastructure"
else
    fail "Observability infrastructure (NOT IMPLEMENTED)"
fi
echo

# Test 10: Parser DoS Protection Tests
echo "=== Test 10: Parser DoS Protection ==="
if cargo test -p oxttl parser_dos 2>&1 | grep -q "no test"; then
    fail "Parser DoS protection (NOT IMPLEMENTED)"
else
    skip "Parser DoS protection (checking if exists)"
fi
echo

# Test 11: Determinism Tests
echo "=== Test 11: Determinism Tests ==="
if cargo test deterministic 2>&1 | tee /tmp/determinism_test.log | grep -q "test result: ok"; then
    pass "Determinism tests"
else
    fail "Determinism tests"
fi
echo

# Test 12: W3C SPARQL Test Suite
echo "=== Test 12: W3C SPARQL Compliance ==="
if cargo test -p oxigraph --test testsuite 2>&1 | tee /tmp/w3c_test.log | grep -q "test result: ok"; then
    pass "W3C SPARQL test suite"
else
    fail "W3C SPARQL test suite"
fi
echo

# Test 13: Check for Memory Leak TODO
echo "=== Test 13: Memory Leak TODO Check ==="
if grep -n "TODO: garbage collection" lib/oxigraph/src/storage/memory.rs 2>&1 | grep -q "TODO"; then
    fail "MemoryStore MVCC leak CONFIRMED (TODO exists at memory.rs:743)"
else
    pass "MemoryStore MVCC leak (TODO removed)"
fi
echo

# Test 14: Check for unbounded operations
echo "=== Test 14: Unbounded Operations Check ==="
echo "Checking for unbounded ORDER BY, GROUP BY, transitive closure..."
UNBOUNDED_FOUND=0

if grep -A 5 "fn.*sort_unstable_by" lib/spareval/src/eval.rs | grep -q "sort_unstable_by"; then
    echo "  Found: Unbounded ORDER BY at eval.rs"
    ((UNBOUNDED_FOUND++))
fi

if grep -A 5 "FxHashMap::<Vec<Option" lib/spareval/src/eval.rs | grep -q "FxHashMap"; then
    echo "  Found: Unbounded GROUP BY at eval.rs"
    ((UNBOUNDED_FOUND++))
fi

if grep -A 10 "fn transitive_closure" lib/spareval/src/eval.rs | grep -q "transitive_closure"; then
    echo "  Found: Unbounded transitive closure at eval.rs"
    ((UNBOUNDED_FOUND++))
fi

if [ $UNBOUNDED_FOUND -gt 0 ]; then
    fail "Unbounded operations CONFIRMED ($UNBOUNDED_FOUND patterns found)"
else
    pass "Unbounded operations (none found)"
fi
echo

# Summary
echo "=== PRODUCTION READINESS SUMMARY ==="
echo "Tests Passed:  $PASS_COUNT"
echo "Tests Failed:  $FAIL_COUNT"
echo "Tests Skipped: $SKIP_COUNT"
echo

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}=== ALL PRODUCTION READINESS TESTS PASSED ===${NC}"
    exit 0
else
    echo -e "${RED}=== PRODUCTION READINESS TESTS FAILED ===${NC}"
    echo "Critical issues found: $FAIL_COUNT"
    echo
    echo "VERDICT: NOT PRODUCTION READY"
    echo "See PRODUCTION_READINESS_VERIFICATION_DOSSIER.md for details"
    exit 1
fi
