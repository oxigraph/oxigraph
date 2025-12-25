# Troubleshooting Guide

This guide helps you diagnose and resolve issues with Oxigraph across all platforms (Rust library, CLI server, Python bindings, JavaScript/WASM).

## Quick Navigation

- **[Common Errors](common-errors.md)** - Error messages and their solutions
- **[Performance Issues](performance.md)** - Slow queries, memory problems, optimization
- **[Data Issues](data-issues.md)** - Invalid RDF, encoding problems, data quality
- **[Deployment Issues](deployment.md)** - Docker, Kubernetes, server configuration

## How to Diagnose Issues

### 1. Gather Information

Before troubleshooting, collect:

- **Oxigraph version**: Check with `cargo --version`, `pip show pyoxigraph`, or `npm list oxigraph`
- **Platform**: OS, architecture (x86_64, ARM, WASM)
- **Environment**: Rust/Python/Node.js version
- **Error message**: Full stack trace or error output
- **Minimal reproduction**: Smallest example that triggers the issue

### 2. Enable Detailed Logging

#### Rust Library / CLI

```bash
# Set log level for detailed diagnostics
export RUST_LOG=oxigraph=debug,spareval=debug

# Or for everything
export RUST_LOG=debug

# With backtrace for panics
export RUST_BACKTRACE=1
```

```rust
// In code, initialize logging
env_logger::init();
```

#### Python

```python
import logging

# Enable debug logging
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

# For pyoxigraph internals (if available)
logging.getLogger('pyoxigraph').setLevel(logging.DEBUG)
```

#### JavaScript/WASM

```javascript
// Browser console shows WASM panics
// Check browser developer console for errors

// Add try-catch for better error messages
try {
  store.load(...);
} catch (e) {
  console.error('Error details:', e);
  console.error('Stack:', e.stack);
}
```

### 3. Check System Resources

```bash
# Memory usage
free -h  # Linux
top      # All platforms

# Disk space
df -h

# Disk I/O
iostat -x 1  # Linux

# File descriptors
lsof -p <pid> | wc -l
ulimit -n
```

### 4. Isolate the Problem

Follow this decision tree:

```
Does the error occur with minimal data?
├─ Yes → Likely a bug, see "Filing Bug Reports" below
└─ No → Probably resource/configuration issue
    │
    ├─ Does it work with smaller datasets?
    │   └─ Yes → See Performance Issues
    │
    ├─ Does it work on different platform?
    │   └─ Yes → Platform-specific issue
    │
    └─ Does it work with different RDF format?
        └─ Yes → Parser issue, see Data Issues
```

## Log Analysis Guide

### Understanding Oxigraph Logs

Oxigraph logs follow this pattern:

```
[2025-12-25T10:30:45Z DEBUG oxigraph::store] Opening store at path: /data/oxigraph
[2025-12-25T10:30:45Z INFO  oxigraph::store] Store opened successfully
[2025-12-25T10:30:46Z WARN  spareval::eval] SPARQL query took longer than 1s
[2025-12-25T10:30:47Z ERROR oxigraph::io] Failed to parse RDF: Invalid IRI
```

**Log levels:**
- `ERROR` - Operation failed, needs attention
- `WARN` - Potential problem, operation continued
- `INFO` - Normal operation milestones
- `DEBUG` - Detailed diagnostic information
- `TRACE` - Very verbose, every operation

### Key Log Patterns to Look For

#### 1. Parse Errors

```
ERROR oxigraph_parser::turtle] Unexpected token at line 42, column 15
```

→ See [Data Issues - Invalid RDF](data-issues.md#invalid-rdf-data)

#### 2. Query Errors

```
ERROR spareval::eval] SPARQL query error: Variable ?x is not bound
```

→ See [Common Errors - Query Errors](common-errors.md#query-errors)

#### 3. Storage Errors

```
ERROR oxigraph::store] RocksDB error: IO error: Permission denied
```

→ See [Common Errors - Storage Errors](common-errors.md#storage-errors)

#### 4. Memory Issues

```
WARN oxigraph::store] Large result set (>1M triples), consider pagination
```

→ See [Performance Issues - Memory](performance.md#memory-profiling)

#### 5. Performance Warnings

```
WARN spareval::eval] Query plan has cartesian product, may be slow
```

→ See [Performance Issues - Query Analysis](performance.md#identifying-slow-queries)

## When to File a Bug Report

File a bug report on [GitHub Issues](https://github.com/oxigraph/oxigraph/issues) when:

### Definite Bugs

- **Crashes/Panics**: Oxigraph terminates unexpectedly
- **Data corruption**: Data becomes unreadable or incorrect
- **SPARQL non-compliance**: Query results differ from W3C spec
- **Memory leaks**: Memory usage grows unbounded
- **Deadlocks**: Process hangs indefinitely

### Possible Bugs

- **Performance regression**: Same query much slower in new version
- **Unexpected errors**: Error message doesn't match operation
- **Inconsistent behavior**: Same operation gives different results

### Not Bugs (Ask for Help Instead)

- **Configuration questions**: How to set up something
- **Usage questions**: How to use a feature
- **Performance tuning**: General optimization advice
- **Documentation unclear**: Request for clarification

## How to File an Effective Bug Report

### 1. Check Existing Issues

Search [existing issues](https://github.com/oxigraph/oxigraph/issues) first to avoid duplicates.

### 2. Use the Bug Report Template

Include:

```markdown
**Oxigraph Version**: 0.4.0
**Platform**: Ubuntu 22.04, x86_64
**Language Binding**: Rust library / Python / JavaScript

**Description**
Clear, concise description of the problem.

**Minimal Reproduction**
Smallest possible code that reproduces the issue:

```rust
use oxigraph::store::Store;

fn main() {
    let store = Store::new().unwrap();
    // Minimal steps to reproduce
}
```

**Expected Behavior**
What should happen.

**Actual Behavior**
What actually happens.

**Error Output**
Full error message with RUST_BACKTRACE=1.

**Additional Context**
- Data size: 1M triples
- RDF format: Turtle
- Any relevant configuration
```

### 3. Provide Minimal Data

If the issue requires specific data:

- **Small dataset**: Include inline in issue
- **Large dataset**: Link to file or create minimal subset
- **Sensitive data**: Create synthetic example with same structure

### 4. Include Environment Details

```bash
# Rust
rustc --version
cargo --version

# Python
python --version
pip show pyoxigraph

# JavaScript
node --version
npm list oxigraph
```

## Getting Help Effectively

### Community Resources

1. **GitHub Discussions**: For questions and general help
   - https://github.com/oxigraph/oxigraph/discussions

2. **GitHub Issues**: For confirmed bugs only
   - https://github.com/oxigraph/oxigraph/issues

3. **Documentation**: Check docs first
   - https://oxigraph.org/

### How to Ask Questions

**Good Question:**
```
I'm trying to load a 100GB Turtle file into Oxigraph CLI server.
The process uses all 32GB RAM and gets killed by OOM.

What I tried:
- bulk_loader() with default settings - OOM after 20GB loaded
- Splitting file into 10GB chunks - same issue

Environment:
- Oxigraph CLI 0.4.0
- Ubuntu 22.04, 32GB RAM, 500GB SSD
- File: DBpedia 2024 dataset

Is there a way to limit memory usage during bulk loading?
```

**Bad Question:**
```
Oxigraph doesn't work with big files. How do I fix it?
```

### What to Include

1. **What you're trying to achieve** (goal)
2. **What you tried** (steps taken)
3. **What happened** (actual result)
4. **Environment details** (versions, platform)
5. **Relevant code/data** (minimal example)

## Quick Troubleshooting Checklist

Before seeking help, verify:

- [ ] Running latest version of Oxigraph
- [ ] Checked documentation and FAQ
- [ ] Searched existing GitHub issues
- [ ] Enabled debug logging
- [ ] Tested with minimal data
- [ ] Verified system resources (memory, disk, permissions)
- [ ] Tried on different platform (if possible)
- [ ] Created minimal reproduction case
- [ ] Collected version and environment information

## Common Diagnostic Commands

### Rust/CLI

```bash
# Version info
cargo --version
oxigraph --version

# Check data store integrity
oxigraph check --location /path/to/store

# Query performance analysis
RUST_LOG=spareval=debug oxigraph query --location /path/to/store

# Memory profiling (Linux)
/usr/bin/time -v oxigraph serve --location /path/to/store
```

### Python

```python
import pyoxigraph
import sys

# Version
print(f"pyoxigraph version: {pyoxigraph.__version__}")
print(f"Python version: {sys.version}")

# Store diagnostics
store = pyoxigraph.Store("/path/to/store")
print(f"Store size: {len(store)} triples")

# Memory usage
import tracemalloc
tracemalloc.start()
# ... operations ...
current, peak = tracemalloc.get_traced_memory()
print(f"Peak memory: {peak / 1024 / 1024:.2f} MB")
```

### JavaScript

```javascript
// Version
console.log('Node version:', process.version);
const oxigraph = require('oxigraph');
console.log('Oxigraph loaded');

// Store diagnostics
const store = new oxigraph.Store();
console.log('Store size:', store.size);

// Memory usage (Node.js)
const usage = process.memoryUsage();
console.log('Memory:', {
  rss: `${usage.rss / 1024 / 1024} MB`,
  heapUsed: `${usage.heapUsed / 1024 / 1024} MB`
});
```

## Emergency Procedures

### Store Corruption Recovery

If your store becomes corrupted:

```bash
# 1. Backup current store
cp -r /path/to/store /path/to/store.backup

# 2. Export data (if possible)
oxigraph dump --location /path/to/store > backup.nq

# 3. Create new store
rm -rf /path/to/store
oxigraph load --location /path/to/store backup.nq
```

### Out of Memory Recovery

If Oxigraph runs out of memory:

```bash
# 1. Check current memory
free -h

# 2. Increase swap (temporary fix)
sudo fallocate -l 16G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# 3. Process data in smaller batches
# See Performance Issues guide
```

### Deadlock Recovery

If Oxigraph appears hung:

```bash
# 1. Get process info
ps aux | grep oxigraph

# 2. Check for locks
lsof -p <pid> | grep LOCK

# 3. Get stack trace (Linux)
gdb -p <pid> -batch -ex "thread apply all bt"

# 4. If necessary, kill and restart
kill -9 <pid>
```

## Next Steps

- **Errors?** → [Common Errors Guide](common-errors.md)
- **Slow performance?** → [Performance Guide](performance.md)
- **Data problems?** → [Data Issues Guide](data-issues.md)
- **Deployment issues?** → [Deployment Guide](deployment.md)

---

**Still stuck?** Create a [GitHub Discussion](https://github.com/oxigraph/oxigraph/discussions) with:
1. What you tried from this guide
2. Diagnostic output
3. Minimal reproduction
4. Environment details
