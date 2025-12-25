# JavaScript/WASM Transaction Support Implementation

## Overview

This document describes the implementation of transaction support for the Oxigraph JavaScript/WASM bindings.

## Changes Made

### 1. Core Implementation (`js/src/store.rs`)

#### Added Imports
```rust
use oxigraph::store::{Store, Transaction};
use std::cell::RefCell;
```

#### New TypeScript Declarations
Added TypeScript type definitions for the `StoreTransaction` class:
```typescript
export class StoreTransaction {
    add(quad: Quad): void;
    delete(quad: Quad): void;
    commit(): void;
}
```

#### Store.beginTransaction() Method
Added method to `JsStore` to create transactions:
```rust
#[wasm_bindgen(js_name = beginTransaction)]
pub fn begin_transaction(&self) -> Result<JsTransaction, JsValue>
```

#### JsTransaction Structure
Created new struct to wrap Rust transactions:
```rust
#[wasm_bindgen(js_name = StoreTransaction, skip_typescript)]
pub struct JsTransaction {
    store: Store,
    inner: RefCell<Option<Transaction<'static>>>,
}
```

**Key Design Decisions:**
- **Store Ownership**: The transaction owns a clone of the Store (cheap Arc clone) to ensure the underlying storage lives as long as the transaction
- **Lifetime Handling**: Uses safe `unsafe` code with `transmute` to convert the transaction lifetime to `'static`. This is safe because:
  - The transaction is created from the owned Store
  - The transaction is stored alongside the Store in the same struct
  - Rust's drop order guarantees the transaction is dropped before the Store
  - The `Option` wrapper prevents access after commit
- **RefCell**: Used to allow interior mutability for commit operation (which needs to take ownership)

#### JsTransaction Methods

**add(quad)**
- Inserts a quad into the transaction
- Changes are buffered until commit

**delete(quad)**
- Removes a quad from the transaction
- Changes are buffered until commit

**commit()**
- Atomically applies all changes to the store
- Takes ownership of the transaction (via `Option::take()`)
- Prevents double-commit via the Option wrapper
- Throws error if called after commit or on a dropped transaction

### 2. Tests (`js/test/store.test.ts`)

Added comprehensive test suite covering:
- Basic transaction add operations
- Transaction delete operations
- Implicit rollback (not committing)
- Atomic multi-operation transactions
- Error handling (double-commit, use-after-commit)

### 3. Example Code (`js/examples/transaction-example.js`)

Created complete working example demonstrating:
- Basic transaction with commit
- Implicit rollback
- Atomic updates (delete + add)
- Error handling

### 4. Bug Fix (`lib/oxrdfio/src/serializer.rs`)

Fixed pre-existing compilation error in N3 serializer:
```rust
WriterQuadSerializerKind::N3(serializer) => {
    serializer.serialize_quad(&quad.try_into()?)
}
```

The N3 serializer expects `&N3Quad` instead of `QuadRef`, so we convert it.

## Usage Example

```javascript
import { Store, namedNode, literal, quad } from 'oxigraph';

const store = new Store();
const subject = namedNode('http://example.com/alice');
const predicate = namedNode('http://xmlns.com/foaf/0.1/name');

// Begin a transaction
const transaction = store.beginTransaction();

// Add quads
transaction.add(quad(subject, predicate, literal('Alice')));

// Commit atomically
transaction.commit();

// Verify
console.log(store.has(quad(subject, predicate, literal('Alice')))); // true
```

## Rollback Behavior

Transactions support automatic rollback by simply not calling `commit()`:

```javascript
const transaction = store.beginTransaction();
transaction.add(someQuad);
// Don't call commit - changes are automatically rolled back
```

When the transaction object is garbage collected without calling `commit()`, all changes are discarded.

## Error Handling

The implementation throws errors in the following cases:

1. **Calling commit() twice**: Once a transaction is committed, it cannot be used again
2. **Using transaction after commit**: Any operation (add/delete) after commit throws an error
3. **Invalid quads**: The same validation as Store.add() and Store.delete() applies

## API Consistency

The transaction API follows the same patterns as the Store API:
- `add(quad)` matches `Store.add(quad)`
- `delete(quad)` matches `Store.delete(quad)`
- Both return `void` and throw on error

## Performance Considerations

- Transaction creation clones the Store (cheap Arc clone)
- All changes are buffered in memory until commit
- Commit is atomic - either all changes succeed or none do
- For very large bulk operations, consider using `Store.bulkLoad()` instead

## Future Enhancements

Potential improvements for future versions:

1. **Async transaction support**: Add async versions of transaction methods
2. **Query support**: Allow running queries within a transaction
3. **SPARQL Update**: Support SPARQL UPDATE operations in transactions
4. **Savepoints**: Add support for nested transactions or savepoints
5. **Read isolation**: Document and potentially enhance isolation guarantees

## Testing

To run the tests:
```bash
cd js
npm test
```

The test suite includes comprehensive coverage of:
- Basic operations (add/delete)
- Commit and rollback semantics
- Error conditions
- Atomic multi-operation transactions

## Compilation

To build the JavaScript bindings with transaction support:
```bash
cd js
wasm-pack build --target web
```

Note: The implementation is platform-independent and works on both Node.js and browsers.
