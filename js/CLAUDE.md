# Claude Code Guide for Oxigraph JS

## Project Overview

Oxigraph JS is a WebAssembly-based RDF graph database with a JavaScript/TypeScript API. The JS bindings are built using `wasm-bindgen` and provide RDF/JS-compatible interfaces.

## Architecture

```
js/
├── src/
│   ├── lib.rs       # Main entry point, module declarations
│   ├── model.rs     # RDF terms (NamedNode, BlankNode, Literal, Quad, Dataset)
│   ├── store.rs     # Store class with SPARQL query/update
│   ├── io.rs        # RDF parsing/serialization, formats
│   ├── sparql.rs    # SPARQL result types and serialization
│   ├── shacl.rs     # SHACL validation
│   └── utils.rs     # Utility functions
├── test/            # TypeScript tests
└── build_package.mjs # Post-build script for Symbol.iterator
```

## Key Conventions

### JavaScript Naming (camelCase)

All public APIs use JavaScript camelCase conventions via `js_name` attributes:

```rust
#[wasm_bindgen(js_name = namedGraphs)]
pub fn named_graphs(&self) -> Result<Box<[JsValue]>, JsValue>

#[wasm_bindgen(js_name = fromMediaType)]
pub fn from_media_type(media_type: &str) -> Option<JsRdfFormat>
```

### TypeScript Custom Sections

Custom TypeScript types are defined in `TYPESCRIPT_CUSTOM_SECTION` constants to provide:
- Literal types for `termType` (e.g., `"NamedNode"` not `string`)
- Proper generic signatures
- RDF/JS compatibility

```rust
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
export class Store {
    readonly size: number;
    // ...
}
"###;
```

### Collection Methods Pattern

All collection methods follow JavaScript Array API patterns with `thisArg` support:

```rust
#[wasm_bindgen(js_name = forEach)]
pub fn for_each(&self, callback: &Function, this_arg: &JsValue) -> Result<(), JsValue> {
    let this = if this_arg.is_undefined() { JsValue::NULL } else { this_arg.clone() };
    for item in self.iter() {
        callback.call1(&this, &item)?;
    }
    Ok(())
}
```

### Symbol.iterator Support

wasm-bindgen doesn't support computed property names. Use `__iterator()` method and wire it in `build_package.mjs`:

```rust
#[wasm_bindgen(skip_typescript)]
pub fn __iterator(&self) -> JsValue {
    // Return iterator
}
```

```javascript
// build_package.mjs
content = content.replace(
    /(class Store\s*{[\s\S]*?)(\n}\n)/,
    `$1\n    [Symbol.iterator]() { return this.__iterator(); }$2`
);
```

## Store API

### Collection Methods (Full Array API)
- `forEach(callback, thisArg?)` - iterate all quads
- `filter(predicate, thisArg?)` - filter quads
- `map(callback, thisArg?)` - transform quads
- `reduce(callback, initialValue)` - reduce to single value
- `some(predicate, thisArg?)` - test if any match
- `every(predicate, thisArg?)` - test if all match
- `find(predicate, thisArg?)` - find first match
- `findIndex(predicate, thisArg?)` - find index of first match
- `indexOf(quad)` - find index of quad
- `includes(quad)` - alias for has()
- `flatMap(callback, thisArg?)` - map and flatten
- `concat(...others)` - combine sources
- `slice(start?, end?)` - get portion
- `at(index)` - access by index (negative supported)
- `join(separator?)` - join as string
- `entries()` - iterator of [index, quad]
- `keys()` - iterator of indices
- `values()` - iterator of quads
- `toArray()` - convert to array

### Async Methods
- `queryAsync(sparql, options?)` - non-blocking SPARQL query
- `updateAsync(sparql, options?)` - non-blocking SPARQL update
- `loadAsync(data, options?)` - non-blocking data loading

### Graph Management
- `namedGraphs()` - list all named graphs
- `containsNamedGraph(graph)` - check if graph exists
- `addGraph(graph)` - create empty graph
- `clearGraph(graph)` - remove triples from graph
- `removeGraph(graph)` - delete graph entirely

## Dataset API

In-memory RDF dataset with same collection methods as Store.

## I/O Module

### RDF Formats (Static Constants)
```typescript
RdfFormat.TURTLE
RdfFormat.N_TRIPLES
RdfFormat.N_QUADS
RdfFormat.TRIG
RdfFormat.RDF_XML
RdfFormat.N3
RdfFormat.JSON_LD
```

### Functions
- `parse(data, format, options?)` - parse RDF string
- `parseAsync(data, format, options?)` - non-blocking parse
- `serialize(quads, format, options?)` - serialize to string
- `serializeAsync(quads, format, options?)` - non-blocking serialize
- `canonicalize(quads, algorithm)` - RDFC-1.0 canonicalization

## SPARQL Module

### Result Types
- `QuerySolutions` - SELECT query results with `serialize(format)`
- `QueryBoolean` - ASK query result with `valueOf()`
- `QueryTriples` - CONSTRUCT/DESCRIBE results with `serialize(format)`

### Functions
- `parseQueryResults(data, format)` - parse SPARQL results
- `serializeQuerySolutions(solutions, vars, format)` - serialize solutions
- `serializeQueryBoolean(value, format)` - serialize boolean

## Build Commands

```bash
# Check compilation
cargo check -p oxigraph-js

# Build WASM
wasm-pack build --target web

# Run tests
npm test
```

## Common Patterns

### Error Handling
```rust
// Use Result for fallible operations
pub fn method(&self) -> Result<T, JsValue> {
    something.map_err(JsError::from)?
}

// Use format_err! for custom errors
Err(format_err!("Invalid format: {}", format))
```

### Options Parsing
```rust
let base_iri = if !options.is_undefined() {
    Reflect::get(&options, &"baseIri".into())?
        .as_string()
} else {
    None
};
```

### Quad Conversion
```rust
// JS to Rust
let quad = FROM_JS.with(|c| c.to_quad(&js_value))?;

// Rust to JS
let js_quad: JsValue = JsQuad::from(quad).into();
```

## Testing

Tests are in TypeScript using Node.js test runner:

```typescript
import { describe, it } from "node:test";
import assert from "node:assert";
import { Store, namedNode, quad } from "../pkg/node.js";

describe("Store", () => {
    it("adds quads", () => {
        const store = new Store();
        store.add(quad(s, p, o));
        assert.strictEqual(store.size, 1);
    });
});
```

## Performance Tips

1. Use `bulkLoad()` for large data imports (bypasses transaction overhead)
2. Use async methods (`queryAsync`, `loadAsync`) to avoid blocking
3. Async methods yield to event loop every 1000 items
4. Use `match()` with specific patterns rather than filtering all quads
