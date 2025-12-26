# Common Errors and Solutions

This guide catalogs common error messages in Oxigraph with their symptoms, causes, solutions, and prevention strategies.

## Table of Contents

- [Parse Errors](#parse-errors)
- [Query Errors](#query-errors)
- [Storage Errors](#storage-errors)
- [Memory Errors](#memory-errors)
- [Network Errors](#network-errors)
- [Type Errors](#type-errors)
- [Configuration Errors](#configuration-errors)

---

## Parse Errors

### Invalid IRI Syntax

**Symptom:**
```
Error: Invalid IRI: 'http://example.org/resource with spaces'
```

**Cause:**
IRIs cannot contain unencoded spaces or other invalid characters according to RFC 3987.

**Solution:**

```rust
// ❌ Wrong
let iri = NamedNode::new("http://example.org/resource with spaces")?;

// ✅ Correct - URL encode spaces
let iri = NamedNode::new("http://example.org/resource%20with%20spaces")?;

// Or use a helper
use urlencoding::encode;
let resource = "resource with spaces";
let iri = NamedNode::new(&format!("http://example.org/{}", encode(resource)))?;
```

**Prevention:**
- Always validate and encode user input before creating IRIs
- Use IRI validation libraries
- Sanitize data during import

---

### Turtle/TTL Parse Error: Unexpected Token

**Symptom:**
```
Error: Parse error at line 42, column 15: Expected '.' or ';' but found ','
```

**Cause:**
Invalid Turtle syntax - using wrong delimiter or malformed structure.

**Solution:**

```turtle
# ❌ Wrong - using comma instead of semicolon
ex:Person1 a ex:Person,
  ex:name "Alice",
  ex:age 30 .

# ✅ Correct - use semicolon for multiple predicates
ex:Person1 a ex:Person ;
  ex:name "Alice" ;
  ex:age 30 .

# ✅ Also correct - use comma for multiple objects of same predicate
ex:Person1 a ex:Person, ex:Employee ;
  ex:name "Alice" .
```

**Prevention:**
- Validate Turtle files with a linter before loading
- Use a Turtle-aware editor with syntax highlighting
- Test with small samples first

---

### Unexpected End of File

**Symptom:**
```
Error: Unexpected end of file while parsing Turtle
```

**Cause:**
- File is truncated or incomplete
- Missing final period (`.`) in Turtle
- Unclosed string literal or comment

**Solution:**

```turtle
# ❌ Wrong - missing final period
@prefix ex: <http://example.org/> .

ex:Person1 a ex:Person ;
  ex:name "Alice"
# File ends here - missing final '.'

# ✅ Correct
@prefix ex: <http://example.org/> .

ex:Person1 a ex:Person ;
  ex:name "Alice" .
# Proper termination
```

**Prevention:**
- Always check file completeness after download/transfer
- Validate checksums for large files
- Use streaming parsers that report progress

---

### Invalid Literal Datatype

**Symptom:**
```
Error: Invalid lexical form for xsd:integer: 'abc'
```

**Cause:**
Literal value doesn't match its declared datatype.

**Solution:**

```rust
// ❌ Wrong - string value with integer datatype
let literal = Literal::new_typed_literal("abc", xsd::INTEGER);

// ✅ Correct - proper integer value
let literal = Literal::new_typed_literal("42", xsd::INTEGER);

// ✅ Better - use helper for validation
use oxrdf::Literal;
let value = "42";
match value.parse::<i64>() {
    Ok(num) => Literal::from(num),
    Err(_) => return Err("Invalid integer"),
}
```

**Prevention:**
- Validate data before creating typed literals
- Use native types (`.from(42)`) instead of string-based constructors
- Implement schema validation with SHACL

---

### UTF-8 Encoding Error

**Symptom:**
```
Error: Invalid UTF-8 sequence at byte position 1024
```

**Cause:**
Input file is not valid UTF-8 (may be Latin-1, Windows-1252, or corrupted).

**Solution:**

```bash
# Detect encoding
file -i data.ttl
# or
chardet data.ttl

# Convert to UTF-8
iconv -f ISO-8859-1 -t UTF-8 data.ttl > data_utf8.ttl

# Or in Python
with open('data.ttl', 'rb') as f:
    content = f.read()
    # Try to detect encoding
    detected = chardet.detect(content)
    text = content.decode(detected['encoding'])

with open('data_utf8.ttl', 'w', encoding='utf-8') as f:
    f.write(text)
```

**Prevention:**
- Always save RDF files as UTF-8
- Validate encoding before processing
- Set proper encoding in HTTP headers: `Content-Type: text/turtle; charset=utf-8`

---

## Query Errors

### Unbound Variable in SELECT

**Symptom:**
```
Error: Variable ?x is not bound in SELECT clause
```

**Cause:**
SELECT clause references a variable that doesn't appear in the WHERE clause or is in an OPTIONAL block that might not match.

**Solution:**

```sparql
# ❌ Wrong - ?email only bound in OPTIONAL
SELECT ?name ?email WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
}
# If OPTIONAL doesn't match, ?email is unbound

# ✅ Correct - use BOUND check or default
SELECT ?name (COALESCE(?email, "no-email") AS ?emailOrDefault) WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
}

# ✅ Or only select when bound
SELECT ?name ?email WHERE {
  ?person foaf:name ?name .
  ?person foaf:mbox ?email .  # Required, not OPTIONAL
}
```

**Prevention:**
- Carefully track variable bindings through OPTIONAL blocks
- Use BOUND() to test before using optional variables
- Consider COALESCE() for default values

---

### SPARQL Syntax Error

**Symptom:**
```
Error: Parse error at line 3: Expected '}' but found 'FILTER'
```

**Cause:**
Invalid SPARQL syntax - typically misplaced clauses or missing punctuation.

**Solution:**

```sparql
# ❌ Wrong - FILTER outside graph pattern
SELECT ?person ?name WHERE {
  ?person foaf:name ?name
}
FILTER(?age > 18)  # FILTER must be inside WHERE {}

# ✅ Correct
SELECT ?person ?name WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age > 18)
}

# ❌ Wrong - missing dot separator
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o
  FILTER(isIRI(?o))  # Missing '.' before FILTER
}

# ✅ Correct
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
  FILTER(isIRI(?o))
}
```

**Prevention:**
- Use SPARQL query validators before execution
- Test queries incrementally, building up complexity
- Use proper indentation to spot structure errors

---

### Query Timeout

**Symptom:**
```
Error: Query execution exceeded timeout of 30 seconds
```

**Cause:**
Query is too complex or creates a large intermediate result (often from cartesian products).

**Solution:**

```rust
// Increase timeout
use std::time::Duration;
use oxigraph::store::Store;

let store = Store::new()?;
let query = "SELECT * WHERE { ?s ?p ?o }";

// With timeout
let results = store.query_with_timeout(query, Duration::from_secs(300))?;
```

```sparql
-- ❌ Slow - Cartesian product
SELECT * WHERE {
  ?person1 a foaf:Person .
  ?person2 a foaf:Person .
}
-- Returns N² results for N persons

-- ✅ Better - Add constraint to reduce combinations
SELECT * WHERE {
  ?person1 a foaf:Person .
  ?person2 a foaf:Person .
  FILTER(?person1 != ?person2)
  ?person1 foaf:knows ?person2 .
}
```

**Prevention:**
- Add LIMIT clauses during development
- Check query plans for cartesian products
- Use more selective triple patterns
- Add indexes on frequently queried properties

---

### Type Error in Expression

**Symptom:**
```
Error: Cannot compare xsd:string with xsd:integer in FILTER
```

**Cause:**
Type mismatch in SPARQL expression (comparison, arithmetic, or function).

**Solution:**

```sparql
# ❌ Wrong - comparing string with number
SELECT ?person WHERE {
  ?person foaf:age ?age .
  FILTER(?age > "18")  # "18" is a string
}

# ✅ Correct - proper type
SELECT ?person WHERE {
  ?person foaf:age ?age .
  FILTER(?age > 18)  # 18 is an integer
}

# ✅ Or explicit cast
SELECT ?person WHERE {
  ?person ex:ageString ?ageStr .
  FILTER(xsd:integer(?ageStr) > 18)
}
```

**Prevention:**
- Use proper datatypes in RDF data
- Be explicit with type casts in queries
- Validate data types with SHACL

---

## Storage Errors

### Permission Denied

**Symptom:**
```
Error: IO error: Permission denied (os error 13) - /data/oxigraph
```

**Cause:**
Process doesn't have read/write permissions for the store directory.

**Solution:**

```bash
# Check current permissions
ls -la /data/

# Fix permissions
sudo chown -R $USER:$USER /data/oxigraph
chmod -R 755 /data/oxigraph

# Or run with appropriate user
sudo -u oxigraph oxigraph serve --location /data/oxigraph
```

**Prevention:**
- Set up proper ownership during installation
- Use dedicated service account for production
- Document required permissions in deployment guide

---

### Store Already in Use

**Symptom:**
```
Error: Cannot acquire lock on store: Resource temporarily unavailable
```

**Cause:**
Another process already has the store open. RocksDB (Oxigraph's storage backend) uses exclusive locks.

**Solution:**

```bash
# Check for running processes
ps aux | grep oxigraph
lsof /data/oxigraph/LOCK

# Kill other processes if safe
kill <pid>

# Or use different store location
oxigraph serve --location /data/oxigraph-instance-2
```

**Prevention:**
- Use process managers (systemd) to prevent duplicate instances
- Implement proper shutdown procedures
- Use advisory locks in wrapper scripts

---

### Disk Full

**Symptom:**
```
Error: IO error: No space left on device (os error 28)
```

**Cause:**
No available disk space for store operations.

**Solution:**

```bash
# Check disk usage
df -h

# Free up space
# 1. Clean old logs
find /var/log -name "*.gz" -mtime +30 -delete

# 2. Compact store (reduces size)
oxigraph compact --location /data/oxigraph

# 3. Move to larger volume
rsync -av /data/oxigraph/ /mnt/large-volume/oxigraph/
```

**Prevention:**
- Monitor disk usage with alerts (e.g., <80% full)
- Estimate storage needs: ~5-10x raw RDF file size
- Set up log rotation
- Plan for growth: data size, indexes, backups

---

### Store Corruption

**Symptom:**
```
Error: RocksDB corruption: Checksum mismatch in file
```

**Cause:**
- Unclean shutdown (power loss, kill -9)
- Disk hardware failure
- Filesystem corruption

**Solution:**

```bash
# 1. Try RocksDB repair
rocksdb_ldb --db=/data/oxigraph repair

# 2. If that fails, restore from backup
rm -rf /data/oxigraph
cp -r /backups/oxigraph-latest /data/oxigraph

# 3. If no backup, export and reimport
# (only works if corruption is partial)
oxigraph dump --location /data/oxigraph > export.nq 2>&1 | tee errors.log
rm -rf /data/oxigraph
oxigraph load --location /data/oxigraph export.nq
```

**Prevention:**
- Use journaling filesystem (ext4, XFS)
- Graceful shutdown procedures
- Regular backups
- Monitor disk health (SMART)
- Use UPS for servers

---

## Memory Errors

### Out of Memory (OOM)

**Symptom:**
```
Error: Cannot allocate memory
```
or process is killed by OS (Linux OOM killer).

**Cause:**
- Loading very large RDF files at once
- Query produces huge intermediate results
- Memory leak (rare)

**Solution:**

```rust
// For large files, use streaming parser
use oxigraph::io::RdfParser;
use std::fs::File;

let file = File::open("large.ttl")?;
let parser = RdfParser::from_format(RdfFormat::Turtle).parse_read(file);

// Process in batches
let mut store = Store::new()?;
let mut batch = Vec::new();
const BATCH_SIZE: usize = 10_000;

for quad in parser {
    batch.push(quad?);
    if batch.len() >= BATCH_SIZE {
        store.bulk_extend(batch.drain(..))?;
    }
}
// Insert remaining
store.bulk_extend(batch)?;
```

```bash
# Increase available memory
# Add swap space (temporary)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Limit query memory
oxigraph serve --memory-limit 8G
```

**Prevention:**
- Use bulk_loader() for imports
- Add LIMIT to development queries
- Monitor memory usage
- Process large files in chunks
- Use streaming where possible

---

### Stack Overflow

**Symptom:**
```
thread 'main' has overflowed its stack
fatal runtime error: stack overflow
```

**Cause:**
- Deeply nested SPARQL queries
- Very deep JSON-LD structures
- Recursive operations without limit

**Solution:**

```rust
// Increase stack size
use std::thread;

let builder = thread::Builder::new()
    .stack_size(8 * 1024 * 1024);  // 8MB stack

let handler = builder.spawn(|| {
    // Your Oxigraph operations here
    let store = Store::new().unwrap();
    // ...
}).unwrap();

handler.join().unwrap();
```

**Prevention:**
- Simplify query structure
- Limit recursion depth in data
- Use iteration instead of recursion where possible

---

## Network Errors

### HTTP Connection Timeout (SPARQL Federation)

**Symptom:**
```
Error: HTTP request timeout for SERVICE <http://remote-endpoint.org/sparql>
```

**Cause:**
Remote SPARQL endpoint is slow or unreachable.

**Solution:**

```rust
// Increase timeout for federated queries
use oxigraph::store::Store;
use std::time::Duration;

let store = Store::new()?;

// Configure HTTP client timeout
store.set_service_timeout(Duration::from_secs(60))?;
```

```sparql
-- Simplify SERVICE query to reduce remote load
SELECT ?label WHERE {
  # Local data
  ?local ex:linkedTo ?remote .

  # Remote query - keep it simple
  SERVICE <http://remote-endpoint.org/sparql> {
    ?remote rdfs:label ?label .
  }
}
```

**Prevention:**
- Cache remote results locally
- Use SPARQL endpoint with better SLA
- Implement retry logic
- Monitor remote endpoint health

---

### SSL/TLS Certificate Error

**Symptom:**
```
Error: Certificate verification failed for https://endpoint.org/sparql
```

**Cause:**
- Self-signed certificate
- Expired certificate
- Certificate chain issue

**Solution:**

```rust
// For development only - disable verification (unsafe!)
// This should only be used for testing with known endpoints

// For production, fix the certificate issue:
// 1. Install proper CA-signed certificate on remote server
// 2. Add custom CA to system trust store
```

```bash
# Add custom CA certificate (Linux)
sudo cp custom-ca.crt /usr/local/share/ca-certificates/
sudo update-ca-certificates

# Verify certificate
openssl s_client -connect endpoint.org:443 -showcerts
```

**Prevention:**
- Use proper SSL certificates (Let's Encrypt)
- Monitor certificate expiration
- Keep system CA certificates updated

---

### HTTP 503 Service Unavailable

**Symptom:**
```
Error: SPARQL endpoint returned 503 Service Unavailable
```

**Cause:**
Remote endpoint is overloaded or down.

**Solution:**

```python
import pyoxigraph
import time

def query_with_retry(store, query, max_retries=3):
    for attempt in range(max_retries):
        try:
            return list(store.query(query))
        except Exception as e:
            if "503" in str(e) and attempt < max_retries - 1:
                time.sleep(2 ** attempt)  # Exponential backoff
                continue
            raise
```

**Prevention:**
- Implement retry with exponential backoff
- Cache frequently accessed remote data
- Have fallback endpoints
- Rate limit federated queries

---

## Type Errors

### Python Type Error

**Symptom:**
```python
TypeError: argument 'quad': expected Quad, found tuple
```

**Cause:**
Passing wrong type to pyoxigraph method.

**Solution:**

```python
import pyoxigraph as ox

# ❌ Wrong - passing tuple
store = ox.Store()
store.add((
    ox.NamedNode("http://ex.org/s"),
    ox.NamedNode("http://ex.org/p"),
    ox.NamedNode("http://ex.org/o")
))

# ✅ Correct - create Quad object
store.add(ox.Quad(
    ox.NamedNode("http://ex.org/s"),
    ox.NamedNode("http://ex.org/p"),
    ox.NamedNode("http://ex.org/o")
))
```

**Prevention:**
- Use type hints in Python code
- Check documentation for parameter types
- Use IDE with type checking (PyCharm, VS Code with Pylance)

---

### JavaScript Type Error

**Symptom:**
```
TypeError: store.add expects DataModel, got Object
```

**Cause:**
Passing plain JavaScript object instead of RDF term.

**Solution:**

```javascript
const oxigraph = require('oxigraph');

// ❌ Wrong - plain object
store.add({
  subject: "http://example.org/s",
  predicate: "http://example.org/p",
  object: "value"
});

// ✅ Correct - use RDF terms
const { NamedNode, Literal, Quad } = oxigraph;
store.add(new Quad(
  new NamedNode("http://example.org/s"),
  new NamedNode("http://example.org/p"),
  new Literal("value")
));
```

**Prevention:**
- Use TypeScript for type checking
- Follow examples in documentation
- Use factory functions to create terms

---

## Configuration Errors

### Invalid Store Path

**Symptom:**
```
Error: Invalid path: /data/store\x00
```

**Cause:**
Path contains null bytes or invalid characters.

**Solution:**

```rust
// ❌ Wrong - path with null byte
let path = "/data/store\0";
let store = Store::open(path)?;

// ✅ Correct
let path = "/data/store";
let store = Store::open(path)?;

// Validate path
use std::path::Path;
let path = Path::new("/data/store");
if !path.is_absolute() {
    return Err("Path must be absolute");
}
```

**Prevention:**
- Validate paths before use
- Use Path/PathBuf for path manipulation
- Sanitize user input

---

### Feature Not Enabled

**Symptom:**
```
Error: GeoSPARQL support not compiled in
```

**Cause:**
Feature flag not enabled during compilation.

**Solution:**

```toml
# Cargo.toml - enable required features
[dependencies]
oxigraph = { version = "0.4", features = ["geosparql"] }
```

```bash
# For CLI, rebuild with features
cargo build --release --features geosparql
```

**Prevention:**
- Document required features in README
- Check feature flags in CI/CD
- Provide pre-built binaries with common features

---

## Quick Reference

| Error Pattern | Most Likely Cause | First Step |
|--------------|-------------------|------------|
| `Invalid IRI` | Unencoded special characters | URL encode the IRI |
| `Parse error at line X` | Syntax error in RDF file | Check syntax at that line |
| `Variable not bound` | Variable only in OPTIONAL | Add COALESCE or BOUND check |
| `Permission denied` | File permissions wrong | Check/fix ownership |
| `Lock acquisition failed` | Store already open | Find and stop other process |
| `Out of memory` | Large dataset or query | Use streaming/batching |
| `Connection timeout` | Network/remote issue | Check network, increase timeout |
| `Type mismatch` | Wrong Python/JS type | Use correct RDF term class |

---

**Not finding your error?** Check the [troubleshooting index](index.md) or [file an issue](https://github.com/oxigraph/oxigraph/issues) with details.
