# Data Issues Troubleshooting

This guide helps diagnose and fix data-related problems in Oxigraph including invalid RDF, encoding issues, blank nodes, named graphs, and data quality concerns.

## Table of Contents

- [Invalid RDF Data](#invalid-rdf-data)
- [Encoding Problems](#encoding-problems)
- [Blank Node Issues](#blank-node-issues)
- [Named Graph Confusion](#named-graph-confusion)
- [Duplicate Detection](#duplicate-detection)
- [Data Quality](#data-quality)

---

## Invalid RDF Data

### Malformed IRIs

**Symptom:**
```
Error: Invalid IRI: relative IRI without base
Error: Invalid IRI: 'http://example.org/resource with spaces'
```

**Cause:**
IRIs must be absolute and properly encoded according to RFC 3987.

**Solution:**

#### 1. Relative IRIs without Base

```turtle
# ❌ Wrong - relative IRI without @base
<resource1> <property> <resource2> .

# ✅ Correct - with @base
@base <http://example.org/> .
<resource1> <property> <resource2> .

# ✅ Or use absolute IRIs
<http://example.org/resource1> <property> <http://example.org/resource2> .
```

```rust
use oxigraph::io::RdfParser;

// Set base IRI when parsing
let base_iri = "http://example.org/";
let parser = RdfParser::from_format(RdfFormat::Turtle)
    .with_base_iri(base_iri)?
    .parse_read(file);
```

#### 2. Special Characters in IRIs

```rust
use urlencoding::encode;

// ❌ Wrong
let bad_iri = "http://example.org/resource with spaces";

// ✅ Correct - percent encode
let resource = "resource with spaces";
let good_iri = format!("http://example.org/{}", encode(resource));
// Result: "http://example.org/resource%20with%20spaces"

use oxigraph::model::NamedNode;
let node = NamedNode::new(&good_iri)?;
```

**Characters requiring encoding:**
- Space → `%20`
- `<` `>` → `%3C` `%3E`
- `{` `}` → `%7B` `%7D`
- `|` → `%7C`
- `\` → `%5C`
- `^` → `%5E`
- `` ` `` → `%60`

#### 3. Validate IRIs Before Use

```python
import pyoxigraph as ox
from urllib.parse import urlparse, quote

def create_safe_iri(base, path):
    """Create a valid IRI from potentially unsafe components."""
    # Validate base
    parsed = urlparse(base)
    if not parsed.scheme or not parsed.netloc:
        raise ValueError(f"Invalid base IRI: {base}")

    # Encode path
    safe_path = quote(path, safe='/:@!$&\'()*+,;=')

    # Combine
    iri = f"{base.rstrip('/')}/{safe_path.lstrip('/')}"

    # Validate by creating NamedNode
    try:
        return ox.NamedNode(iri)
    except ValueError as e:
        raise ValueError(f"Could not create valid IRI: {e}")

# Usage
node = create_safe_iri("http://example.org", "resources/my resource")
```

**Prevention:**
- Always validate and encode user input
- Use IRI construction helpers
- Validate IRIs during data import pipeline

---

### Invalid Literals

**Symptom:**
```
Error: Invalid lexical form for xsd:integer: 'abc'
Error: Invalid language tag: 'en-'
Error: Cannot have both language tag and datatype
```

**Cause:**
Literal doesn't match its datatype or has invalid language tag.

**Solution:**

#### 1. Type Validation

```rust
use oxigraph::model::Literal;
use oxigraph::model::vocab::xsd;

// ❌ Wrong - string doesn't match type
let bad = Literal::new_typed_literal("abc", xsd::INTEGER);  // Error!

// ✅ Correct - validate before creating
fn create_integer_literal(s: &str) -> Result<Literal, String> {
    s.parse::<i64>()
        .map(|_| Literal::new_typed_literal(s, xsd::INTEGER))
        .map_err(|_| format!("Invalid integer: {}", s))
}

// ✅ Better - use native type
let good = Literal::from(42i64);  // Automatically xsd:integer
```

#### 2. Language Tag Validation

```rust
// ❌ Wrong - invalid language tags
let bad1 = Literal::new_language_tagged_literal("Hello", "en-")?;  // Error: ends with -
let bad2 = Literal::new_language_tagged_literal("Hello", "EN")?;   // Error: must be lowercase
let bad3 = Literal::new_language_tagged_literal("Hello", "english")?;  // Error: not BCP 47

// ✅ Correct
let good1 = Literal::new_language_tagged_literal("Hello", "en")?;
let good2 = Literal::new_language_tagged_literal("Hello", "en-US")?;
let good3 = Literal::new_language_tagged_literal("Hello", "en-GB")?;

// Validate language tag
fn is_valid_language_tag(tag: &str) -> bool {
    // Simplified check (for full validation, use a BCP 47 library)
    tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !tag.starts_with('-')
        && !tag.ends_with('-')
        && tag == tag.to_lowercase()
}
```

#### 3. Common Datatype Validations

```python
import pyoxigraph as ox
from datetime import datetime
from decimal import Decimal

def validate_and_create_literal(value, datatype_iri):
    """Validate value matches datatype before creating literal."""
    validators = {
        'http://www.w3.org/2001/XMLSchema#integer': lambda v: str(int(v)),
        'http://www.w3.org/2001/XMLSchema#decimal': lambda v: str(Decimal(v)),
        'http://www.w3.org/2001/XMLSchema#double': lambda v: str(float(v)),
        'http://www.w3.org/2001/XMLSchema#boolean': lambda v: str(bool(v)).lower(),
        'http://www.w3.org/2001/XMLSchema#dateTime': lambda v: datetime.fromisoformat(v).isoformat(),
    }

    if datatype_iri in validators:
        try:
            validated_value = validators[datatype_iri](value)
            return ox.Literal(validated_value, datatype=ox.NamedNode(datatype_iri))
        except (ValueError, TypeError) as e:
            raise ValueError(f"Invalid {datatype_iri}: {value} - {e}")
    else:
        # Unknown datatype, create as string
        return ox.Literal(str(value), datatype=ox.NamedNode(datatype_iri))

# Usage
lit = validate_and_create_literal("42", "http://www.w3.org/2001/XMLSchema#integer")
```

**Prevention:**
- Validate data before creating literals
- Use native types when possible
- Implement schema validation with SHACL

---

### Incomplete Triples/Quads

**Symptom:**
```
Error: Expected object, found '.'
Error: Unexpected end of file
```

**Cause:**
Turtle/N-Triples file has incomplete statements.

**Solution:**

```bash
# Validate RDF file structure
rapper -i turtle -c suspicious.ttl

# Check for common issues
grep -n '\.$' suspicious.ttl  # Find lines ending with period
grep -n ';$' suspicious.ttl   # Find lines ending with semicolon

# Fix with streaming validation
python3 << 'EOF'
import pyoxigraph as ox

try:
    count = 0
    for quad in ox.parse(open('suspicious.ttl', 'rb'), 'text/turtle'):
        count += 1
    print(f"Successfully parsed {count} quads")
except Exception as e:
    print(f"Parse error: {e}")
    print("Check the line number in the error message")
EOF
```

**Prevention:**
- Validate RDF files before loading
- Use RDF-aware editors with syntax checking
- Implement data validation in ETL pipeline

---

## Encoding Problems

### UTF-8 Encoding Issues

**Symptom:**
```
Error: Invalid UTF-8 sequence at byte 1024
Error: Stream did not contain valid UTF-8
```

**Cause:**
File is not UTF-8 encoded (may be Latin-1, Windows-1252, UTF-16, etc.)

**Solution:**

#### 1. Detect Encoding

```bash
# Detect file encoding
file -i data.ttl
# output: data.ttl: text/plain; charset=iso-8859-1

# Or use chardet
pip install chardet
chardetect data.ttl
# output: data.ttl: iso-8859-1 with confidence 0.99

# Or enca (for European encodings)
enca -L none data.ttl
```

#### 2. Convert to UTF-8

```bash
# Using iconv
iconv -f ISO-8859-1 -t UTF-8 data.ttl > data_utf8.ttl

# Using recode
recode ISO-8859-1..UTF-8 data.ttl

# Detect and convert automatically
# Python
python3 << 'EOF'
import chardet

with open('data.ttl', 'rb') as f:
    raw_data = f.read()
    detected = chardet.detect(raw_data)
    encoding = detected['encoding']
    confidence = detected['confidence']
    print(f"Detected {encoding} with {confidence:.0%} confidence")

    text = raw_data.decode(encoding)

with open('data_utf8.ttl', 'w', encoding='utf-8') as f:
    f.write(text)
print("Converted to UTF-8")
EOF
```

#### 3. Handle Mixed Encodings

```python
import pyoxigraph as ox

def load_with_encoding_fallback(filename, formats=None):
    """Try multiple encodings until one works."""
    if formats is None:
        formats = ['utf-8', 'iso-8859-1', 'windows-1252', 'utf-16']

    for encoding in formats:
        try:
            with open(filename, 'r', encoding=encoding) as f:
                content = f.read()

            # Re-encode to UTF-8 bytes
            utf8_bytes = content.encode('utf-8')

            # Parse as UTF-8
            store = ox.Store()
            store.load(utf8_bytes, "text/turtle")
            print(f"Successfully loaded with {encoding} encoding")
            return store
        except (UnicodeDecodeError, ValueError) as e:
            print(f"Failed with {encoding}: {e}")
            continue

    raise ValueError(f"Could not load {filename} with any known encoding")

# Usage
store = load_with_encoding_fallback('data.ttl')
```

**Prevention:**
- Always save RDF files as UTF-8
- Set UTF-8 encoding in editors and tools
- Validate encoding in CI/CD pipeline
- Document encoding requirements

---

### Byte Order Mark (BOM) Issues

**Symptom:**
```
Error: Unexpected token '\ufeff' at start of file
```

**Cause:**
File has UTF-8 BOM (Byte Order Mark) which some parsers don't handle.

**Solution:**

```bash
# Detect BOM
file data.ttl
# output: data.ttl: UTF-8 Unicode (with BOM) text

# Remove BOM
sed -i '1s/^\xEF\xBB\xBF//' data.ttl

# Or with Python
python3 << 'EOF'
with open('data.ttl', 'rb') as f:
    content = f.read()

# Remove UTF-8 BOM if present
if content.startswith(b'\xef\xbb\xbf'):
    content = content[3:]

with open('data_no_bom.ttl', 'wb') as f:
    f.write(content)
EOF
```

**Prevention:**
- Configure editors to save as "UTF-8 without BOM"
- Strip BOM in data processing pipeline

---

### Line Ending Issues

**Symptom:**
Parse errors at end of lines, especially after Windows→Linux transfer.

**Cause:**
Mixed line endings (CRLF vs LF).

**Solution:**

```bash
# Detect line endings
file data.ttl
# output: data.ttl: ASCII text, with CRLF line terminators

# Convert CRLF to LF
dos2unix data.ttl

# Or with sed
sed -i 's/\r$//' data.ttl

# Or with Python
python3 << 'EOF'
with open('data.ttl', 'rb') as f:
    content = f.read()

# Normalize to LF
content = content.replace(b'\r\n', b'\n')

with open('data_fixed.ttl', 'wb') as f:
    f.write(content)
EOF
```

**Prevention:**
- Use `.gitattributes` to normalize line endings:
  ```
  *.ttl text eol=lf
  *.nt text eol=lf
  *.nq text eol=lf
  ```

---

## Blank Node Issues

### Blank Node Scope Confusion

**Symptom:**
Blank nodes from different files or parsing sessions are treated as distinct when they should be the same, or vice versa.

**Cause:**
Blank node identifiers are scoped to a single RDF document. Across documents, `_:b1` in file A is different from `_:b1` in file B.

**Solution:**

#### 1. Understanding Blank Node Scope

```turtle
# file1.ttl
_:person foaf:name "Alice" .
_:person foaf:age 30 .

# file2.ttl
_:person foaf:name "Bob" .
_:person foaf:age 25 .
```

When loaded separately, these create **4 distinct blank nodes**, not 2:
- `_:person` from file1 (with name Alice)
- `_:person` from file1 (with age 30) - same as above
- `_:person` from file2 (with name Bob) - DIFFERENT node
- `_:person` from file2 (with age 25) - same as file2's person

```rust
use oxigraph::store::Store;

let store = Store::new()?;

// Load files separately - creates distinct blank nodes
store.load_from_read(RdfFormat::Turtle, File::open("file1.ttl")?)?;
store.load_from_read(RdfFormat::Turtle, File::open("file2.ttl")?)?;

// Query - will find 2 distinct persons
let query = "SELECT ?name ?age WHERE { ?person foaf:name ?name ; foaf:age ?age }";
// Results: [("Alice", 30), ("Bob", 25)]
```

#### 2. Skolemization - Convert Blank Nodes to IRIs

```python
import pyoxigraph as ox
from hashlib import sha256

def skolemize_blank_nodes(input_file, output_file, base_uri="http://example.org/.well-known/genid/"):
    """Convert blank nodes to stable IRIs based on their properties."""
    store = ox.Store()
    store.load(open(input_file, 'rb').read(), "text/turtle")

    skolemized = ox.Store()

    for quad in store:
        def skolemize_term(term):
            if isinstance(term, ox.BlankNode):
                # Create deterministic IRI from blank node properties
                # In real implementation, base on node's properties
                hash_input = str(term).encode('utf-8')
                hash_hex = sha256(hash_input).hexdigest()[:16]
                return ox.NamedNode(f"{base_uri}{hash_hex}")
            return term

        new_quad = ox.Quad(
            skolemize_term(quad.subject),
            quad.predicate,
            skolemize_term(quad.object),
            quad.graph_name
        )
        skolemized.add(new_quad)

    # Save skolemized version
    with open(output_file, 'wb') as f:
        f.write(skolemized.dump("text/turtle"))

# Usage
skolemize_blank_nodes("file_with_blanks.ttl", "file_skolemized.ttl")
```

#### 3. Using Named Graphs to Preserve Provenance

```rust
use oxigraph::model::GraphName;

// Load each file into a separate named graph
let graph1 = GraphName::from(NamedNode::new("http://example.org/graph/file1")?);
let graph2 = GraphName::from(NamedNode::new("http://example.org/graph/file2")?);

store.load_graph(File::open("file1.ttl")?, RdfFormat::Turtle, &graph1, None)?;
store.load_graph(File::open("file2.ttl")?, RdfFormat::Turtle, &graph2, None)?;

// Now can query with graph context
```

**Prevention:**
- Use named nodes (IRIs) instead of blank nodes for cross-document references
- Skolemize blank nodes during import
- Document blank node scoping behavior
- Use UUIDs or stable identifiers

---

### Blank Node Label Collisions

**Symptom:**
Blank node labels conflict when merging RDF from multiple sources.

**Cause:**
Multiple sources use the same blank node identifiers.

**Solution:**

```python
import pyoxigraph as ox
import uuid

def rename_blank_nodes(quads, prefix=""):
    """Rename blank nodes with unique prefix to avoid collisions."""
    blank_node_map = {}

    for quad in quads:
        def get_or_create_blank(term):
            if isinstance(term, ox.BlankNode):
                if term not in blank_node_map:
                    # Create new unique blank node
                    unique_id = f"{prefix}_{uuid.uuid4().hex[:8]}"
                    blank_node_map[term] = ox.BlankNode(unique_id)
                return blank_node_map[term]
            return term

        yield ox.Quad(
            get_or_create_blank(quad.subject),
            quad.predicate,
            get_or_create_blank(quad.object),
            quad.graph_name
        )

# Usage: merge multiple files safely
store = ox.Store()

files = ["source1.ttl", "source2.ttl", "source3.ttl"]
for i, filename in enumerate(files):
    quads = ox.parse(open(filename, 'rb').read(), "text/turtle")
    renamed = rename_blank_nodes(quads, prefix=f"file{i}")
    store.extend(renamed)
```

**Prevention:**
- Use unique prefixes for blank nodes from different sources
- Skolemize at import time
- Use UUIDs for blank node identifiers

---

## Named Graph Confusion

### Default Graph vs Named Graphs

**Symptom:**
Data loaded but queries return no results.

**Cause:**
Data in named graph, query searches default graph (or vice versa).

**Solution:**

#### 1. Understanding Graph Contexts

```rust
use oxigraph::model::*;
use oxigraph::store::Store;

let store = Store::new()?;

// Add to DEFAULT graph (no graph parameter)
let triple = Quad::new(
    NamedNode::new("http://example.org/subject")?,
    NamedNode::new("http://example.org/predicate")?,
    NamedNode::new("http://example.org/object")?,
    GraphName::DefaultGraph,  // Default graph
);
store.insert(&triple)?;

// Add to NAMED graph
let named_triple = Quad::new(
    NamedNode::new("http://example.org/subject2")?,
    NamedNode::new("http://example.org/predicate2")?,
    NamedNode::new("http://example.org/object2")?,
    NamedNode::new("http://example.org/graph1")?,  // Named graph
);
store.insert(&named_triple)?;

// Query DEFAULT graph (implicit)
let query1 = "SELECT * WHERE { ?s ?p ?o }";
// Returns: subject-predicate-object (from default graph only)

// Query NAMED graph
let query2 = "SELECT * WHERE { GRAPH <http://example.org/graph1> { ?s ?p ?o } }";
// Returns: subject2-predicate2-object2

// Query ALL graphs
let query3 = "SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }";
// Returns: subject2-predicate2-object2 (named graphs only, excludes default)

// Query default + all named graphs
let query4 = "SELECT * WHERE { { ?s ?p ?o } UNION { GRAPH ?g { ?s ?p ?o } } }";
// Returns: both triples
```

#### 2. Checking Which Graph Contains Data

```sparql
-- Find all named graphs
SELECT DISTINCT ?g WHERE {
  GRAPH ?g { ?s ?p ?o }
}

-- Count triples per graph
SELECT ?g (COUNT(*) AS ?count) WHERE {
  {
    # Default graph
    BIND(<urn:default> AS ?g)
    ?s ?p ?o
  } UNION {
    # Named graphs
    GRAPH ?g { ?s ?p ?o }
  }
}
GROUP BY ?g
ORDER BY DESC(?count)
```

#### 3. Loading into Specific Graph

```rust
use oxigraph::io::RdfFormat;
use std::fs::File;

let store = Store::new()?;
let graph = NamedNode::new("http://example.org/my-graph")?;

// Load into default graph
store.load_from_read(RdfFormat::Turtle, File::open("data.ttl")?)?;

// Load into named graph
store.load_graph(
    File::open("data.ttl")?,
    RdfFormat::Turtle,
    &GraphName::from(graph.clone()),
    None,
)?;

// Load N-Quads (already has graph info)
store.load_from_read(RdfFormat::NQuads, File::open("data.nq")?)?;
```

```python
import pyoxigraph as ox

store = ox.Store()

# Load Turtle into default graph
store.load(open('data.ttl', 'rb').read(), mime_type="text/turtle")

# Load Turtle into named graph
graph = ox.NamedNode("http://example.org/my-graph")
store.load(
    open('data.ttl', 'rb').read(),
    mime_type="text/turtle",
    base_iri=None,
    to_graph=graph
)

# Load N-Quads (preserves graph info)
store.load(open('data.nq', 'rb').read(), mime_type="application/n-quads")
```

**Prevention:**
- Document which graphs contain which data
- Use N-Quads format to preserve graph information
- Query all graphs during debugging
- Establish naming convention for graphs

---

### Graph Deletion Issues

**Symptom:**
Deleted graph but data still appears in queries.

**Cause:**
Deleting named graph doesn't remove it from default graph.

**Solution:**

```rust
// ❌ Wrong - only deletes from named graph
store.clear_graph(&GraphName::from(graph_iri))?;
// Data in default graph remains!

// ✅ Correct - explicit about what to delete
// Clear specific named graph
store.clear_graph(&GraphName::from(
    NamedNode::new("http://example.org/graph1")?
))?;

// Clear default graph
store.clear_graph(&GraphName::DefaultGraph)?;

// Clear ALL graphs (use with caution!)
for graph_name in store.named_graphs() {
    store.clear_graph(&GraphName::from(graph_name?))?;
}
store.clear_graph(&GraphName::DefaultGraph)?;
```

**Prevention:**
- Be explicit about graph context when adding/removing data
- Use named graphs consistently
- Document graph usage patterns

---

## Duplicate Detection

### Identifying Duplicates

**Symptom:**
Same data appears multiple times in the store.

**Cause:**
Loaded same file multiple times, or data from different sources overlaps.

**Solution:**

#### 1. Detect Exact Duplicates

```sparql
-- Find exact duplicate triples (should not happen in a proper RDF store)
-- RDF stores automatically deduplicate, but check data quality

-- Find resources with duplicate labels
SELECT ?resource (COUNT(?label) AS ?count) WHERE {
  ?resource rdfs:label ?label .
}
GROUP BY ?resource
HAVING (COUNT(?label) > 1)

-- Find duplicate quads across graphs
SELECT ?s ?p ?o (COUNT(?g) AS ?graphCount) WHERE {
  GRAPH ?g { ?s ?p ?o }
}
GROUP BY ?s ?p ?o
HAVING (COUNT(?g) > 1)
```

#### 2. Detect Similar Resources (Potential Duplicates)

```python
import pyoxigraph as ox
from collections import defaultdict

def find_potential_duplicates(store, predicate_iri):
    """Find resources with same value for a key predicate (e.g., label, email)."""
    query = f"""
    SELECT ?s ?value WHERE {{
        ?s <{predicate_iri}> ?value .
    }}
    """

    value_to_subjects = defaultdict(list)

    for result in store.query(query):
        subject = str(result['s'])
        value = str(result['value'])
        value_to_subjects[value].append(subject)

    # Find values with multiple subjects
    duplicates = {
        value: subjects
        for value, subjects in value_to_subjects.items()
        if len(subjects) > 1
    }

    return duplicates

# Usage
store = ox.Store("data")
dups = find_potential_duplicates(store, "http://www.w3.org/2000/01/rdf-schema#label")

for value, subjects in dups.items():
    print(f"Value '{value}' appears for {len(subjects)} subjects:")
    for subj in subjects:
        print(f"  - {subj}")
```

#### 3. Merge Duplicates

```python
def merge_duplicates(store, canonical_iri, duplicate_iris):
    """Merge duplicate resources into canonical IRI."""
    canonical = ox.NamedNode(canonical_iri)

    for dup_iri in duplicate_iris:
        duplicate = ox.NamedNode(dup_iri)

        # Copy all properties from duplicate to canonical
        for quad in store.quads_for_pattern(duplicate, None, None, None):
            new_quad = ox.Quad(canonical, quad.predicate, quad.object, quad.graph_name)
            store.add(new_quad)

        # Update references to duplicate
        for quad in store.quads_for_pattern(None, None, duplicate, None):
            new_quad = ox.Quad(quad.subject, quad.predicate, canonical, quad.graph_name)
            store.add(new_quad)
            store.remove(quad)

        # Remove old duplicate triples
        for quad in list(store.quads_for_pattern(duplicate, None, None, None)):
            store.remove(quad)

# Usage
merge_duplicates(
    store,
    canonical_iri="http://example.org/person/alice",
    duplicate_iris=[
        "http://example.org/person/alice_smith",
        "http://example.org/person/a_smith"
    ]
)
```

**Prevention:**
- Implement data deduplication in ETL pipeline
- Use stable identifiers (URIs) based on business keys
- Validate uniqueness constraints with SHACL

---

## Data Quality

### Implementing Data Validation with SHACL

**Symptom:**
Data quality issues (missing required properties, wrong types, etc.)

**Cause:**
No validation rules enforced during data loading.

**Solution:**

```turtle
# shapes.ttl - SHACL shapes for validation
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

ex:PersonShape a sh:NodeShape ;
    sh:targetClass foaf:Person ;

    # Required properties
    sh:property [
        sh:path foaf:name ;
        sh:minCount 1 ;
        sh:maxCount 1 ;
        sh:datatype xsd:string ;
        sh:minLength 1 ;
    ] ;

    # Optional email with format validation
    sh:property [
        sh:path foaf:mbox ;
        sh:maxCount 1 ;
        sh:pattern "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$" ;
    ] ;

    # Age constraints
    sh:property [
        sh:path foaf:age ;
        sh:maxCount 1 ;
        sh:datatype xsd:integer ;
        sh:minInclusive 0 ;
        sh:maxInclusive 150 ;
    ] .
```

```rust
use oxigraph::store::Store;
use oxigraph::model::*;

let store = Store::new()?;

// Load data
store.load_from_read(RdfFormat::Turtle, File::open("data.ttl")?)?;

// Load SHACL shapes
let shapes_graph = NamedNode::new("http://example.org/shapes")?;
store.load_graph(
    File::open("shapes.ttl")?,
    RdfFormat::Turtle,
    &GraphName::from(shapes_graph.clone()),
    None,
)?;

// Validate
use oxigraph::shacl;
let validation_report = shacl::validate(&store, &GraphName::from(shapes_graph))?;

if !validation_report.is_valid() {
    eprintln!("Validation failed!");
    for result in validation_report.results() {
        eprintln!("  - {}", result);
    }
}
```

**Prevention:**
- Define SHACL shapes for all data models
- Validate before importing data
- Include validation in CI/CD pipeline

---

### Data Profiling

**Symptom:**
Don't know what's in the data.

**Solution:**

```sparql
-- Basic statistics
SELECT
  (COUNT(*) AS ?totalTriples)
  (COUNT(DISTINCT ?s) AS ?uniqueSubjects)
  (COUNT(DISTINCT ?p) AS ?uniquePredicates)
  (COUNT(DISTINCT ?o) AS ?uniqueObjects)
WHERE { ?s ?p ?o }

-- Most common predicates
SELECT ?predicate (COUNT(*) AS ?count) WHERE {
  ?s ?predicate ?o .
}
GROUP BY ?predicate
ORDER BY DESC(?count)
LIMIT 20

-- Most common types
SELECT ?type (COUNT(?instance) AS ?count) WHERE {
  ?instance a ?type .
}
GROUP BY ?type
ORDER BY DESC(?count)

-- Resources with most properties
SELECT ?resource (COUNT(?property) AS ?propertyCount) WHERE {
  ?resource ?property ?value .
}
GROUP BY ?resource
ORDER BY DESC(?propertyCount)
LIMIT 10

-- Datatype distribution
SELECT ?datatype (COUNT(?literal) AS ?count) WHERE {
  ?s ?p ?literal .
  FILTER(isLiteral(?literal))
  BIND(DATATYPE(?literal) AS ?datatype)
}
GROUP BY ?datatype
ORDER BY DESC(?count)

-- Language tag distribution
SELECT ?lang (COUNT(?literal) AS ?count) WHERE {
  ?s ?p ?literal .
  FILTER(isLiteral(?literal))
  BIND(LANG(?literal) AS ?lang)
  FILTER(?lang != "")
}
GROUP BY ?lang
ORDER BY DESC(?count)
```

**Prevention:**
- Run data profiling regularly
- Monitor data quality metrics
- Document expected data patterns

---

## Quick Diagnostic Checklist

When encountering data issues:

- [ ] Validate RDF syntax with `rapper -c` or similar tool
- [ ] Check file encoding (should be UTF-8)
- [ ] Verify IRIs are properly encoded
- [ ] Check for BOM or line ending issues
- [ ] Understand blank node scope
- [ ] Verify which named graph contains the data
- [ ] Run SPARQL queries to profile the data
- [ ] Use SHACL validation for data quality
- [ ] Check for duplicates
- [ ] Review data loading logs for errors

---

**Still having data issues?** See the [troubleshooting index](index.md) or report with:
1. Sample of problematic data (minimal example)
2. Error messages
3. Expected vs actual behavior
4. Data format and encoding information
