# Oxigraph Examples

This directory contains example code demonstrating how to use Oxigraph in various scenarios.

## Available Examples

- **query_explanation.rs**: Demonstrates how to execute a SPARQL query with Oxigraph and get detailed execution information to understand query performance.

## How to Run Examples

You can run any example using Cargo from the root of the Oxigraph repository:

```bash
# General format
cargo run --example <example_name>

# For example, to run the query_explanation example:
cargo run --example query_explanation
```

## Creating New Examples

If you want to add a new example:

1. Create a new Rust file in this directory (e.g., `my_example.rs`)
2. Add an entry to `Cargo.toml` in this directory:
```toml
[[example]]
name = "my_example"
path = "my_example.rs"
```
3. Make sure to import the necessary dependencies from Oxigraph

All examples automatically use the dependencies defined in the `[dependencies]` section of this directory's `Cargo.toml`.

## Continuous Integration

All examples in this directory are automatically built and run as part of the CI pipeline. This ensures that:

1. Examples always compile correctly
2. Examples execute without errors
3. Changes to the Oxigraph codebase don't break the examples

When adding a new example, it will automatically be included in the CI checks without any additional configuration. 