# Oxigraph for .NET

[![NuGet](https://img.shields.io/nuget/v/Oxigraph)](https://www.nuget.org/packages/Oxigraph/)
[![GitHub Actions](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

Oxigraph for .NET is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
It is built on top of [Oxigraph](https://crates.io/crates/oxigraph) via an FFI bridge to the Rust library.

It provides two stores with [SPARQL 1.1](https://www.w3.org/TR/sparql11-overview/) capabilities:

- **In-memory Store** (`new Store()`) — fast, zero-config
- **File-backed Store** (`new Store(path)`) — persistent, backed by RocksDB

It also provides standalone parsing/serialization for all major RDF formats:
[JSON-LD](https://www.w3.org/TR/json-ld/),
[Turtle](https://www.w3.org/TR/turtle/),
[TriG](https://www.w3.org/TR/trig/),
[N-Triples](https://www.w3.org/TR/n-triples/),
[N-Quads](https://www.w3.org/TR/n-quads/),
[N3](https://www.w3.org/TR/n3/) and
[RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/).

Two NuGet packages are available:

| Package | Description |
|---------|-------------|
| **Oxigraph** | Core library — Store, SPARQL engine, RDF I/O |
| **Oxigraph.Extensions.DotNetRDF** | Interop adapters for [dotNetRDF](https://github.com/dotnetrdf/dotnetrdf) |

Source code is on [GitHub](https://github.com/oxigraph/oxigraph/tree/main/dotnet).

## Installation

```bash
dotnet add package Oxigraph
dotnet add package Oxigraph.Extensions.DotNetRDF  # optional
```

## Example

Insert the triple `<http://example/> <http://schema.org/name> "example"` and query it with SPARQL:

```csharp
using Oxigraph;

using var store = new Store();
var ex = new NamedNode("http://example/");
var schemaName = new NamedNode("http://schema.org/name");
store.Add(new Quad(ex, schemaName, new Literal("example"), new DefaultGraph()));

var results = store.Query("SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }");
var solutions = (QuerySolutions)results;
foreach (var binding in solutions)
    Console.WriteLine(((Literal)binding["name"]!).Value);
```

## Table of Contents

- [Model](model.md) — RDF core types: NamedNode, BlankNode, Literal, Triple, Quad, DefaultGraph, Dataset
- [IO](io.md) — Parsing and serializing RDF files in all formats
- [Store](store.md) — Store (in-memory & RocksDB-backed) with SPARQL 1.1
- [SPARQL](sparql.md) — Query results, custom functions, and aggregate functions
- [Async API](store.md#async-api) — Task-based async overloads with CancellationToken support

## Help

Feel free to use [GitHub discussions](https://github.com/oxigraph/oxigraph/discussions) or [the Gitter chat](https://gitter.im/oxigraph/community) to ask questions or talk about Oxigraph.
[Bug reports](https://github.com/oxigraph/oxigraph/issues) are also very welcome.

If you need advanced support or are willing to pay to get some extra features, feel free to reach out to [Tpt](https://github.com/Tpt).

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.