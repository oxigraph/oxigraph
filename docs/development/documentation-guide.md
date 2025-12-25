# Documentation Guide

This guide explains how to write, maintain, and publish documentation for Oxigraph.

## Table of Contents

- [Documentation Standards](#documentation-standards)
- [Documentation Types](#documentation-types)
- [Adding New Documentation](#adding-new-documentation)
- [Code Example Requirements](#code-example-requirements)
- [Review Process](#review-process)
- [Building Documentation Locally](#building-documentation-locally)
- [Documentation Tools](#documentation-tools)

---

## Documentation Standards

### General Principles

**Clarity:**
- Use simple, clear language
- Avoid jargon when possible
- Define technical terms on first use
- Use active voice

**Completeness:**
- Cover all public APIs
- Include examples for common use cases
- Document error conditions
- Explain parameters and return values

**Accuracy:**
- Test all code examples
- Keep docs in sync with code
- Update docs when APIs change
- Mark deprecated features

**Accessibility:**
- Use descriptive headings
- Include table of contents for long docs
- Provide navigation links
- Use proper markdown formatting

### Writing Style

**Voice and Tone:**
- Professional but friendly
- Direct and concise
- Encouraging and helpful
- Avoid colloquialisms

**Examples:**

**Good:**
```markdown
The `Store` class provides a persistent RDF store backed by RocksDB.
Use it to store and query large RDF datasets efficiently.
```

**Avoid:**
```markdown
The Store thingy is like a database or whatever for RDF stuff.
You can totally use it to do queries and stuff!
```

### Formatting Guidelines

**Headers:**
```markdown
# Top-level Title (H1) - One per document
## Major Section (H2)
### Subsection (H3)
#### Minor Section (H4)
```

**Code Blocks:**
````markdown
```rust
// Rust code with syntax highlighting
let store = Store::new()?;
```

```python
# Python code with syntax highlighting
store = Store()
```

```javascript
// JavaScript code with syntax highlighting
const store = new Store();
```
````

**Links:**
```markdown
[Descriptive link text](https://example.com)
[Relative link to other docs](../reference/api.md)
[Section link](#specific-section)
```

**Lists:**
```markdown
**Ordered lists:**
1. First step
2. Second step
3. Third step

**Unordered lists:**
- Important point
- Another point
  - Nested point
  - Another nested point
```

**Emphasis:**
```markdown
*Italic for emphasis*
**Bold for strong emphasis**
`Code for inline code`
```

---

## Documentation Types

### 1. API Documentation (Code Comments)

**Rust Documentation:**

```rust
/// Parses a SPARQL query string into a query object.
///
/// This function parses the query according to the SPARQL 1.1 specification.
/// It supports SELECT, ASK, CONSTRUCT, and DESCRIBE query forms.
///
/// # Arguments
///
/// * `query` - The SPARQL query string to parse
/// * `base_iri` - Optional base IRI for resolving relative IRIs
///
/// # Returns
///
/// Returns a [`Query`] object on success.
///
/// # Errors
///
/// Returns [`ParseError`] if:
/// - The query syntax is invalid
/// - The query contains unsupported features
/// - Relative IRIs are used without a base IRI
///
/// # Examples
///
/// ```
/// use oxigraph::sparql::Query;
///
/// // Parse a simple SELECT query
/// let query = Query::parse("SELECT * WHERE { ?s ?p ?o }", None)?;
/// assert!(query.is_select());
///
/// // Parse with base IRI
/// let query = Query::parse(
///     "SELECT * WHERE { ?s ?p <resource> }",
///     Some("http://example.com/")
/// )?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`Query::from_str`] - Alternative parsing method
/// - [SPARQL 1.1 Specification](https://www.w3.org/TR/sparql11-query/)
pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Query, ParseError> {
    // Implementation
}
```

**Documentation sections:**
- Summary (first line, one sentence)
- Detailed description (optional)
- `# Arguments` - Parameter descriptions
- `# Returns` - Return value description
- `# Errors` - Error conditions
- `# Panics` - Panic conditions (if any)
- `# Safety` - Safety requirements (for unsafe code)
- `# Examples` - Usage examples (must compile)
- `# See Also` - Related items

**Python Documentation (docstrings):**

```python
def parse(query: str, base_iri: Optional[str] = None) -> Query:
    """Parse a SPARQL query string.

    Parses the query according to the SPARQL 1.1 specification,
    supporting SELECT, ASK, CONSTRUCT, and DESCRIBE forms.

    Args:
        query: The SPARQL query string to parse.
        base_iri: Optional base IRI for resolving relative IRIs.

    Returns:
        A Query object representing the parsed query.

    Raises:
        SyntaxError: If the query syntax is invalid.
        ValueError: If relative IRIs are used without a base IRI.

    Examples:
        >>> from pyoxigraph import Query
        >>> query = Query.parse("SELECT * WHERE { ?s ?p ?o }")
        >>> query.is_select()
        True

        >>> # Parse with base IRI
        >>> query = Query.parse(
        ...     "SELECT * WHERE { ?s ?p <resource> }",
        ...     base_iri="http://example.com/"
        ... )

    See Also:
        - SPARQL 1.1 Specification: https://www.w3.org/TR/sparql11-query/
    """
    pass
```

**JavaScript Documentation (TypeScript):**

```typescript
/**
 * Parses a SPARQL query string.
 *
 * Parses the query according to the SPARQL 1.1 specification,
 * supporting SELECT, ASK, CONSTRUCT, and DESCRIBE forms.
 *
 * @param query - The SPARQL query string to parse
 * @param baseIri - Optional base IRI for resolving relative IRIs
 * @returns A Query object representing the parsed query
 * @throws {SyntaxError} If the query syntax is invalid
 * @throws {Error} If relative IRIs are used without a base IRI
 *
 * @example
 * ```javascript
 * import { Query } from 'oxigraph';
 *
 * // Parse a simple SELECT query
 * const query = Query.parse("SELECT * WHERE { ?s ?p ?o }");
 * console.log(query.isSelect()); // true
 *
 * // Parse with base IRI
 * const query2 = Query.parse(
 *   "SELECT * WHERE { ?s ?p <resource> }",
 *   "http://example.com/"
 * );
 * ```
 *
 * @see {@link https://www.w3.org/TR/sparql11-query/|SPARQL 1.1 Specification}
 */
export function parse(query: string, baseIri?: string): Query;
```

### 2. Tutorials (Step-by-Step Guides)

**Purpose:** Teach users how to accomplish specific tasks

**Location:** `docs/tutorials/`

**Structure:**
```markdown
# Tutorial: Building a SPARQL Query API

**Level:** Beginner
**Time:** 30 minutes
**Prerequisites:** Basic Rust knowledge

## What You'll Learn

- How to set up an Oxigraph store
- How to load RDF data
- How to execute SPARQL queries
- How to handle results

## Setup

1. Create a new Rust project:
   ```bash
   cargo new sparql-api
   cd sparql-api
   ```

2. Add Oxigraph dependency:
   ```toml
   [dependencies]
   oxigraph = "0.5.0"
   ```

## Step 1: Initialize the Store

Create a new store instance:

```rust
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    Ok(())
}
```

This creates an in-memory store...

[Continue with detailed steps]

## Next Steps

- Try the [Advanced SPARQL Tutorial](advanced-sparql.md)
- Read the [Store API Reference](../reference/store-api.md)
```

**Best practices:**
- Start with clear learning objectives
- Include estimated time and skill level
- Provide complete, working code examples
- Explain each step thoroughly
- Link to related resources

### 3. How-To Guides (Task-Oriented)

**Purpose:** Show how to solve specific problems

**Location:** `docs/how-to/`

**Structure:**
```markdown
# How to Import Large RDF Datasets

**Problem:** Loading large RDF files (>1GB) is slow and memory-intensive.

**Solution:** Use the bulk loader for optimal performance.

## Prerequisites

- Oxigraph 0.4.0 or later
- RDF data file in Turtle, N-Triples, or RDF/XML format

## Steps

1. **Create a store with bulk loading:**

   ```rust
   use oxigraph::store::Store;

   let store = Store::new()?;
   let mut loader = store.bulk_loader();
   ```

2. **Load the data:**

   ```rust
   use std::fs::File;
   use oxigraph::io::RdfFormat;

   let file = File::open("large_dataset.nt")?;
   loader.load_from_reader(RdfFormat::NTriples, file)?;
   ```

3. **Finalize the load:**

   ```rust
   loader.finish()?;
   println!("Loaded {} triples", store.len());
   ```

## Performance Tips

- Use N-Triples format for fastest parsing
- Load data before creating indexes
- Disable synchronous writes for speed

## Related

- [Store API Reference](../reference/store.md)
- [Performance Tuning Guide](performance-tuning.md)
```

### 4. Reference Documentation

**Purpose:** Comprehensive API details

**Location:** `docs/reference/`

**Structure:**
```markdown
# Store API Reference

## Overview

The `Store` class provides persistent RDF storage with SPARQL query support.

## Class: Store

### Constructor

#### `Store::new() -> Result<Store>`

Creates a new in-memory store.

**Example:**
```rust
let store = Store::new()?;
```

#### `Store::open(path) -> Result<Store>`

Opens or creates a persistent store.

**Parameters:**
- `path: impl AsRef<Path>` - Directory path for storage

**Example:**
```rust
let store = Store::open("./data")?;
```

### Methods

#### `insert(&self, quad: &Quad) -> Result<bool>`

Inserts a quad into the store.

**Parameters:**
- `quad: &Quad` - The quad to insert

**Returns:**
- `true` if the quad was newly inserted
- `false` if the quad already existed

**Errors:**
- `StorageError` if a storage error occurs

**Example:**
```rust
let quad = Quad::new(subject, predicate, object, graph);
let was_new = store.insert(&quad)?;
```

[... continue with all methods ...]

## Related Types

- [`Quad`](quad.md)
- [`NamedNode`](named-node.md)
- [`Literal`](literal.md)
```

### 5. Explanatory Documentation

**Purpose:** Explain concepts and design decisions

**Location:** `docs/explanation/`

**Topics:**
- Architecture overview
- Design principles
- Performance characteristics
- Comparison with alternatives

---

## Adding New Documentation

### 1. Identify Documentation Need

**When to add docs:**
- New public API added
- Common user question
- Breaking change requiring migration
- New feature or capability
- Existing docs are unclear

### 2. Choose Documentation Type

| Need | Type | Location |
|------|------|----------|
| How does this API work? | API docs | Code comments |
| How do I get started? | Tutorial | `docs/tutorials/` |
| How do I solve X? | How-to | `docs/how-to/` |
| What are all the options? | Reference | `docs/reference/` |
| Why is it designed this way? | Explanation | `docs/explanation/` |

### 3. Write the Documentation

**Follow the templates above for each type**

### 4. Add Examples

**Every piece of documentation should include:**
- At least one complete, working example
- Examples covering common use cases
- Examples showing error handling

### 5. Test Documentation

**Before submitting:**

```bash
# Test Rust doc examples
cargo test --doc

# Test specific crate docs
cargo test --doc -p oxigraph

# Build docs to check formatting
cargo doc --open

# Check Python docs
cd python
python -m pytest --doctest-modules

# Check JS docs
cd js
npm run docs
```

### 6. Link Related Documentation

**Always include links to:**
- Related API documentation
- Relevant tutorials
- Background information
- External specifications

---

## Code Example Requirements

### All Examples Must:

1. **Compile and run successfully**
   ```rust
   // Good - compiles
   use oxigraph::store::Store;
   let store = Store::new()?;
   # Ok::<_, Box<dyn std::error::Error>>(())

   // Bad - won't compile
   let store = Store.new()  // Missing semicolon and ?
   ```

2. **Include necessary imports**
   ```rust
   // Good
   use oxigraph::store::Store;
   use oxigraph::model::*;

   // Bad - missing imports
   let store = Store::new()?;  // Where does Store come from?
   ```

3. **Handle errors appropriately**
   ```rust
   // Good - shows error handling
   match store.insert(&quad) {
       Ok(was_new) => println!("Inserted: {}", was_new),
       Err(e) => eprintln!("Error: {}", e),
   }

   // Or use ? for examples
   store.insert(&quad)?;
   # Ok::<_, Box<dyn std::error::Error>>(())
   ```

4. **Be self-contained**
   ```rust
   // Good - complete example
   use oxigraph::store::Store;
   use oxigraph::model::*;

   let store = Store::new()?;
   let quad = Quad::new(
       NamedNode::new("http://example.com/s")?,
       NamedNode::new("http://example.com/p")?,
       Literal::new_simple_literal("object"),
       GraphName::DefaultGraph,
   );
   store.insert(&quad)?;
   # Ok::<_, Box<dyn std::error::Error>>(())

   // Bad - incomplete
   store.insert(&quad)?;  // Where did quad come from?
   ```

5. **Use realistic examples**
   ```rust
   // Good - realistic use case
   let person = NamedNode::new("http://example.com/alice")?;
   let name_predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
   let name = Literal::new_simple_literal("Alice");

   // Avoid - meaningless
   let foo = NamedNode::new("http://foo.com/bar")?;
   let baz = Literal::new_simple_literal("qux");
   ```

### Testing Examples

**Rust doc tests:**
```rust
/// # Examples
///
/// ```
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
```

**Hidden lines in doc tests:**
```rust
/// ```
/// # use oxigraph::store::Store;
/// # let store = Store::new()?;
/// store.insert(&quad)?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
// Lines starting with # are hidden from rendered docs
```

**Python doctests:**
```python
def example():
    """
    Example function.

    >>> from pyoxigraph import Store
    >>> store = Store()
    >>> len(store)
    0
    """
    pass
```

---

## Review Process

### Documentation Pull Requests

**Checklist for reviewers:**

- [ ] **Accuracy:** Is the information correct?
- [ ] **Completeness:** Does it cover all important aspects?
- [ ] **Clarity:** Is it easy to understand?
- [ ] **Examples:** Are there working examples?
- [ ] **Formatting:** Is the markdown properly formatted?
- [ ] **Links:** Do all links work?
- [ ] **Grammar:** Is the grammar and spelling correct?
- [ ] **Tests:** Do doc tests pass?

### Documentation Review Template

Use the `.github/PULL_REQUEST_TEMPLATE/documentation.md` template:

```markdown
## Documentation Changes

### What documentation is being added/changed?
- [ ] API documentation (code comments)
- [ ] Tutorial
- [ ] How-to guide
- [ ] Reference documentation
- [ ] Explanatory documentation

### Checklist
- [ ] All code examples compile and run
- [ ] Doc tests pass: `cargo test --doc`
- [ ] Spelling and grammar checked
- [ ] Links verified
- [ ] Screenshots included (if applicable)
- [ ] Related docs updated

### Preview
<!-- Include screenshots or rendered markdown -->
```

### Approval Process

1. **Self-review:** Check your own documentation
2. **Peer review:** Another contributor reviews
3. **Maintainer approval:** Final approval from maintainer
4. **Merge:** Documentation is merged and published

---

## Building Documentation Locally

### Rust Documentation

```bash
# Build all Rust docs
cargo doc --all --no-deps

# Open in browser
cargo doc --all --no-deps --open

# Build specific crate
cargo doc -p oxigraph --open

# Build with private items (for maintainers)
cargo doc --all --document-private-items
```

**Output location:** `target/doc/`

**View locally:**
```bash
open target/doc/oxigraph/index.html
```

### Python Documentation

```bash
cd python

# Install doc tools
pip install sphinx sphinx-rtd-theme

# Build docs
cd docs
make html

# Open in browser
open _build/html/index.html
```

### JavaScript Documentation

```bash
cd js

# Install TypeDoc
npm install --save-dev typedoc

# Generate docs
npm run docs

# Open in browser
open docs/index.html
```

### Website Documentation

The main documentation website is built with a static site generator:

```bash
# Install dependencies (example with MkDocs)
pip install mkdocs mkdocs-material

# Serve locally
mkdocs serve

# Build for production
mkdocs build
```

---

## Documentation Tools

### Linting and Validation

**Markdown linting:**
```bash
# Install markdownlint
npm install -g markdownlint-cli

# Lint all markdown files
markdownlint 'docs/**/*.md'
```

**Link checking:**
```bash
# Install markdown-link-check
npm install -g markdown-link-check

# Check links
markdown-link-check docs/**/*.md
```

**Spell checking:**
```bash
# Install cspell
npm install -g cspell

# Check spelling
cspell 'docs/**/*.md'
```

### Automated Tools

**cargo-readme:**
```bash
# Generate README from doc comments
cargo install cargo-readme
cargo readme > README.md
```

**cargo-toc:**
```bash
# Generate table of contents
cargo install cargo-toc
cargo toc --title "Table of Contents" README.md
```

### CI Integration

Documentation checks in CI (`.github/workflows/docs.yml`):

```yaml
name: Documentation

on: [push, pull_request]

jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6

      - name: Build Rust docs
        run: cargo doc --all --no-deps

      - name: Check doc tests
        run: cargo test --doc

      - name: Lint markdown
        run: |
          npm install -g markdownlint-cli
          markdownlint 'docs/**/*.md'

      - name: Check links
        run: |
          npm install -g markdown-link-check
          markdown-link-check docs/**/*.md
```

---

## Documentation Maintenance

### Regular Tasks

**Monthly:**
- [ ] Check for broken links
- [ ] Update version numbers in examples
- [ ] Review and respond to documentation issues

**Per Release:**
- [ ] Update changelog
- [ ] Update version numbers
- [ ] Add migration guides for breaking changes
- [ ] Update API documentation
- [ ] Review and update examples

**Annually:**
- [ ] Review all tutorials for relevance
- [ ] Update screenshots
- [ ] Refresh external links
- [ ] Review overall documentation structure

### Deprecation Documentation

When deprecating an API:

```rust
/// Loads data into the store.
///
/// # Deprecated
///
/// This method is deprecated since version 0.5.0.
/// Use [`Store::bulk_load`] instead for better performance.
///
/// This method will be removed in version 1.0.0.
///
/// # Migration
///
/// ```
/// // Old way (deprecated)
/// store.load(data)?;
///
/// // New way
/// store.bulk_load().load(data)?.finish()?;
/// ```
#[deprecated(since = "0.5.0", note = "Use `bulk_load` instead")]
pub fn load(&self, data: &[u8]) -> Result<()> {
    // Implementation
}
```

---

## Additional Resources

- [Rust Documentation Guide](https://doc.rust-lang.org/rustdoc/)
- [Google Developer Documentation Style Guide](https://developers.google.com/style)
- [Write the Docs](https://www.writethedocs.org/)
- [Diátaxis Documentation Framework](https://diataxis.fr/)
