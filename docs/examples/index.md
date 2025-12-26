# Integration Examples

This directory contains comprehensive, production-ready integration examples for Oxigraph across different programming languages and frameworks.

## Purpose

These examples demonstrate how to integrate Oxigraph into real-world applications, going beyond basic usage to show:

- **Framework Integration** - How to use Oxigraph with popular web frameworks
- **Production Patterns** - Best practices for error handling, logging, and monitoring
- **Performance Optimization** - Connection pooling, caching, and query optimization
- **Deployment** - Docker, cloud platforms, and scaling strategies

Each example is complete and runnable, ready to be adapted for your specific use case.

## Prerequisites

### Rust Examples

- **Rust**: 1.70.0 or later
- **Cargo**: Latest version
- **Knowledge**: Intermediate Rust, async programming

Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Python Examples

- **Python**: 3.8 or later
- **pip**: Latest version
- **Knowledge**: Intermediate Python, web frameworks

Install Python dependencies:
```bash
pip install pyoxigraph flask fastapi uvicorn pandas jupyter click
```

### JavaScript Examples

- **Node.js**: 18.0 or later
- **npm**: 9.0 or later
- **Knowledge**: Modern JavaScript/TypeScript, async/await

Install JavaScript dependencies:
```bash
npm install oxigraph express typescript @types/node
```

## Examples by Language

### Rust Integration Patterns

**[rust-integration.md](rust-integration.md)** covers:

1. **Web Framework Integration**
   - Axum REST API with shared store
   - Actix-web SPARQL endpoint
   - Request/response patterns

2. **Async Runtime Integration**
   - Tokio async operations
   - Blocking I/O handling
   - Concurrent query execution

3. **Production Patterns**
   - Store lifecycle management
   - Error handling strategies
   - Logging with tracing

4. **Advanced Use Cases**
   - Custom SPARQL functions
   - Streaming large result sets
   - Transaction management

**Target Audience**: Backend developers building Rust services

### Python Integration Patterns

**[python-integration.md](python-integration.md)** covers:

1. **Web Framework Integration**
   - Flask REST API
   - FastAPI with async support
   - Django integration

2. **Data Science Integration**
   - Jupyter notebooks
   - Pandas DataFrame conversion
   - NumPy integration

3. **CLI Tools**
   - Click-based RDF tool
   - Typer modern CLI
   - Rich console output

4. **Background Jobs**
   - Celery task queues
   - APScheduler periodic tasks
   - Bulk data processing

**Target Audience**: Python developers, data scientists

### JavaScript Integration Patterns

**[javascript-integration.md](javascript-integration.md)** covers:

1. **Node.js Backend**
   - Express.js middleware
   - REST API patterns
   - GraphQL integration

2. **Frontend Integration**
   - React hooks for RDF
   - Vue.js components
   - State management

3. **Performance Optimization**
   - Worker threads for queries
   - Bundle size optimization
   - Caching strategies

4. **TypeScript**
   - Type-safe RDF operations
   - Custom type definitions
   - Build configuration

**Target Audience**: Full-stack JavaScript/TypeScript developers

## Example Categories

### By Use Case

| Use Case | Rust | Python | JavaScript |
|----------|------|--------|------------|
| REST API | ✓ | ✓ | ✓ |
| SPARQL Endpoint | ✓ | ✓ | ✓ |
| CLI Tool | ✓ | ✓ | ✗ |
| Web Frontend | ✗ | ✗ | ✓ |
| Data Science | ✗ | ✓ | ✗ |
| Microservice | ✓ | ✓ | ✓ |

### By Complexity

- **Beginner**: Basic framework integration, simple queries
- **Intermediate**: Error handling, logging, testing
- **Advanced**: Performance tuning, custom functions, streaming

## Running the Examples

Each example includes:

1. **Dependencies** - Complete list of required packages
2. **Setup** - Step-by-step installation instructions
3. **Code** - Fully commented, production-ready code
4. **Testing** - How to test the integration
5. **Deployment** - Docker/cloud deployment options

### General Pattern

```bash
# 1. Navigate to your project directory
cd my-oxigraph-project

# 2. Copy the example code
# (Copy from the relevant .md file)

# 3. Install dependencies
# Rust: cargo build
# Python: pip install -r requirements.txt
# JavaScript: npm install

# 4. Run the example
# Rust: cargo run
# Python: python app.py
# JavaScript: npm start
```

## Project Templates

Each example can serve as a project template:

### Rust Web Service Template

```bash
cargo new oxigraph-service
cd oxigraph-service
# Add dependencies to Cargo.toml from rust-integration.md
# Copy code from Axum example
cargo run
```

### Python API Template

```bash
mkdir oxigraph-api
cd oxigraph-api
python -m venv venv
source venv/bin/activate  # or venv\Scripts\activate on Windows
# Install dependencies from python-integration.md
# Copy code from FastAPI example
python main.py
```

### JavaScript App Template

```bash
npm create vite@latest oxigraph-app -- --template react-ts
cd oxigraph-app
npm install oxigraph
# Copy code from javascript-integration.md
npm run dev
```

## Best Practices

Across all examples, we follow these principles:

### Error Handling

- **Rust**: Use `Result` and `?` operator, custom error types
- **Python**: Use exceptions with proper context
- **JavaScript**: Use try/catch with async/await, Promise rejection handling

### Resource Management

- **Rust**: RAII, drop handlers, Arc for shared ownership
- **Python**: Context managers (`with` statements)
- **JavaScript**: Explicit cleanup, WeakRef for memory management

### Logging

- **Rust**: `tracing` crate with structured logging
- **Python**: `logging` module with JSON formatters
- **JavaScript**: Winston or Pino for structured logs

### Testing

- **Rust**: Unit tests with `#[test]`, integration tests
- **Python**: pytest with fixtures
- **JavaScript**: Jest or Vitest

### Documentation

- **Rust**: rustdoc comments with examples
- **Python**: Docstrings with type hints
- **JavaScript**: JSDoc or TypeScript types

## Common Integration Patterns

### Singleton Store Pattern

Most applications need a single shared store instance:

**Rust**:
```rust
use once_cell::sync::Lazy;
static STORE: Lazy<Store> = Lazy::new(|| Store::new().unwrap());
```

**Python**:
```python
# store.py
_store = None

def get_store():
    global _store
    if _store is None:
        _store = Store()
    return _store
```

**JavaScript**:
```javascript
// store.js
let store = null;

export function getStore() {
    if (!store) {
        store = new oxigraph.Store();
    }
    return store;
}
```

### Configuration Management

**Environment Variables**:
```bash
# .env
OXIGRAPH_PATH=/data/oxigraph
OXIGRAPH_LOG_LEVEL=info
SPARQL_TIMEOUT_MS=30000
```

**Rust (with dotenvy)**:
```rust
let path = env::var("OXIGRAPH_PATH").unwrap_or_else(|_| "./data".to_string());
```

**Python (with python-dotenv)**:
```python
from dotenv import load_dotenv
load_dotenv()
path = os.getenv("OXIGRAPH_PATH", "./data")
```

**JavaScript (with dotenv)**:
```javascript
require('dotenv').config();
const path = process.env.OXIGRAPH_PATH || './data';
```

## Troubleshooting

### Common Issues

1. **Store Already Open**
   - Issue: Multiple processes accessing same persistent store
   - Solution: Use in-memory store for tests, or different paths

2. **WASM Memory Limits** (JavaScript)
   - Issue: Out of memory in browser
   - Solution: Use worker threads, paginate results

3. **Blocking Operations** (Async runtimes)
   - Issue: Blocking the event loop
   - Solution: Use `spawn_blocking` (Tokio) or `run_in_executor` (asyncio)

4. **SPARQL Timeout**
   - Issue: Complex queries taking too long
   - Solution: Add LIMIT clauses, optimize queries, add indexes

### Getting Help

- **GitHub Issues**: https://github.com/oxigraph/oxigraph/issues
- **Gitter Chat**: https://gitter.im/oxigraph/community
- **Stack Overflow**: Tag with `oxigraph`

## Contributing

Found a better pattern? Have a new integration example? Contributions welcome!

1. Fork the repository
2. Add your example following the existing format
3. Test thoroughly
4. Submit a pull request

## Additional Resources

- **[Tutorials](../tutorials/)** - Step-by-step learning guides
- **[How-To Guides](../how-to/)** - Task-specific solutions
- **[Reference](../reference/)** - API documentation
- **[FAQ](../faq.md)** - Frequently asked questions

---

**Ready to integrate?** Choose your language and framework, then dive into the examples!
