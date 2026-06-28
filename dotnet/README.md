# Oxigraph for .NET

[![NuGet](https://img.shields.io/nuget/v/Oxigraph)](https://www.nuget.org/packages/Oxigraph/)
[![GitHub Actions](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Oxigraph for .NET is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
It is a .NET library written on top of the [Oxigraph](https://crates.io/crates/oxigraph) Rust library via FFI, targeting `net10.0`.

Oxigraph offers two stores with [SPARQL 1.1](https://www.w3.org/TR/sparql11-overview/) capabilities:

- **In-memory Store** (`new Store()`) – fast, zero-config
- **File-backed Store** (`new Store(path)`) – persistent, backed by RocksDB

It also provides standalone parsing/serialization for all major RDF formats:

| Format                                                          | Parse | Serialize |
|-----------------------------------------------------------------|-------|-----------|
| [Turtle](https://www.w3.org/TR/turtle/)                         | ✅     | ✅         |
| [TriG](https://www.w3.org/TR/trig/)                             | ✅     | ✅         |
| [N-Triples](https://www.w3.org/TR/n-triples/)                   | ✅     | ✅         |
| [N-Quads](https://www.w3.org/TR/n-quads/)                       | ✅     | ✅         |
| [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/)            | ✅     | ✅         |
| [N3](https://www.w3.org/TR/n3/)                                 | ✅     | ✅         |
| [JSON-LD](https://www.w3.org/TR/json-ld/)                       | ✅     | ✅         |

SPARQL query results can be serialized to JSON, XML, CSV, and TSV.

## Packages

| Package | NuGet | Description |
|---------|-------|-------------|
| **Oxigraph** | [![NuGet](https://img.shields.io/nuget/v/Oxigraph)](https://www.nuget.org/packages/Oxigraph/) | Core library — Store, SPARQL engine, RDF I/O |
| **Oxigraph.Extensions.DotNetRDF** | [![NuGet](https://img.shields.io/nuget/v/Oxigraph.Extensions.DotNetRDF)](https://www.nuget.org/packages/Oxigraph.Extensions.DotNetRDF/) | Interop adapters between Oxigraph and [dotNetRDF](https://github.com/dotnetrdf/dotnetrdf) types |

```bash
dotnet add package Oxigraph
dotnet add package Oxigraph.Extensions.DotNetRDF  # optional
```

## Quick Start

```csharp
using Oxigraph;

// ─── In-memory store ──────────────────────────────
using var store = new Store();

// Insert a triple
store.Add(new Quad(
    new NamedNode("http://example.com/Alice"),
    new NamedNode("http://example.com/name"),
    new Literal("Alice"),
    new DefaultGraph()));

// SPARQL SELECT
var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");
var solutions = (QuerySolutions)results;
foreach (var row in solutions)
    Console.WriteLine($"{row["s"]} -> {row["o"]}");

// SPARQL UPDATE
store.Update("INSERT DATA { <http://example.com/Bob> <http://example.com/name> \"Bob\" }");

// Load from a file (streaming, no memory limit)
store.LoadFromFile("data.ttl", RdfFormat.Turtle, new LoadOptions { BaseIri = "http://example.com/" });

// Dump to string
Console.WriteLine(store.Dump(RdfFormat.NTriples));
```

```csharp
// ─── File-backed store ─────────────────────────────
using var store = new Store("/path/to/data");

// Bulk-load large files efficiently
store.BulkLoadFromFile("large.nt", RdfFormat.NTriples);
store.Flush();
store.Backup("/path/to/backup");
```

```csharp
// ─── Standalone RDF I/O ────────────────────────────
// Parse
var quads = IO.Parse("@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .",
    RdfFormat.Turtle, "http://example.com/");
Console.WriteLine(quads[0].Subject);

// Serialize
IO.SerializeToFile("out.ttl", quads, RdfFormat.Turtle);

// Lazy file parser (streaming, no memory limit)
using var iter = IO.ParseIterator("huge.ttl", RdfFormat.Turtle);
foreach (var quad in iter)
    Process(quad);
```

```csharp
// ─── SPARQL with custom functions ──────────────────
CustomFunctions.Register("http://example.com/upper", args =>
    new Literal(((Literal)args[0]).Value.ToUpperInvariant()));

var results = store.Query(@"
    PREFIX my: <http://example.com/>
    SELECT ?upper WHERE {
        ?s ?p ?o .
        BIND(my:upper(?o) AS ?upper)
    }");

// Clean up when done
CustomFunctions.Unregister("http://example.com/upper");
```

```csharp
// ─── Custom aggregate functions ────────────────────
// Implements IAggregateAccumulator
sealed class ConcatAggregate : CustomFunctions.IAggregateAccumulator
{
    private readonly List<string> _values = [];
    public void Accumulate(ITerm term) => _values.Add(((Literal)term).Value);
    public ITerm? Finish() => new Literal(string.Join(", ", _values));
}

CustomFunctions.RegisterAggregate("http://example.com/concat",
    () => new ConcatAggregate());
```

```csharp
// ─── DotNetRDF interop ─────────────────────────────
using Oxigraph.Extensions.DotNetRDF;
using VDS.RDF;

var graph = new Graph();
IUriNode alice = graph.CreateUriNode(new Uri("http://example.com/Alice"));
IUriNode name  = graph.CreateUriNode(new Uri("http://example.com/name"));
ILiteralNode label = graph.CreateLiteralNode("Alice", "en");
graph.Assert(new VDS.RDF.Triple(alice, name, label));

// Convert dotNetRDF graph → Oxigraph Store
using var store = new Store();
store.LoadFromGraph(graph);
```

```csharp
// ─── Async API ─────────────────────────────────────
// All long-running operations have *Async overloads
// that accept CancellationToken for timeouts/cancellation.

using var store = new Store();
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));

// Async SPARQL query
var results = await store.QueryAsync(
    "SELECT ?s ?o WHERE { ?s ?p ?o }", ct: cts.Token);

// Async file I/O
await store.LoadFromFileAsync("data.ttl", RdfFormat.Turtle,
    new LoadOptions { BaseIri = "http://example.com/" }, cts.Token);

// Async bulk load
await store.BulkLoadFromFileAsync("large.nt", RdfFormat.NTriples,
    ct: cts.Token);

// Async stream I/O
await using var fileStream = File.OpenRead("data.ttl");
await store.LoadFromStreamAsync(fileStream, RdfFormat.Turtle,
    ct: cts.Token);

// Standalone async I/O
var quads = await IO.ParseFromFileAsync("data.ttl",
    RdfFormat.Turtle, "http://example.com/", ct: cts.Token);
await IO.SerializeToFileAsync("out.ttl", quads,
    RdfFormat.Turtle, ct: cts.Token);
```

## Build from Source

### Prerequisites

- [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0) or later
- [Rust toolchain](https://rustup.rs/) (stable)

### Build

```bash
# Clone with submodules
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph

# Build the .NET bindings
cd dotnet
dotnet build
```

### Run Tests

```bash
# Run all tests
cd dotnet
dotnet test

# Run a specific test file
dotnet test --filter "FullyQualifiedName~ModelTests"
```

## Architecture

The .NET bindings communicate with the Rust Oxigraph library via an FFI bridge:

```
┌──────────────────────────────────────────────────┐
│  .NET Application                                │
│  ┌────────────┐  ┌─────────────────────────────┐ │
│  │  Oxigraph  │  │  Oxigraph.Extensions.       │ │
│  │  (Core)    │  │  DotNetRDF                  │ │
│  └─────┬──────┘  └─────────────────────────────┘ │
└────────┼─────────────────────────────────────────┘
         │ P/Invoke (JSON over FFI)
┌────────┴─────────────────────────────────────────┐
│  oxigraph_ffi.dll / liboxigraph_ffi.so           │
│  ┌──────────────────────────────────────────────┐│
│  │  oxigraph (Rust) + RocksDB                   ││
│  └──────────────────────────────────────────────┘│
└──────────────────────────────────────────────────┘
```

- **Interop layer** (`Oxigraph.Interop`): C# FFI wrapper — delegates, safe handles, native method bindings
- **Model** (`Oxigraph.Model` etc.): RDF primitives — NamedNode, BlankNode, Literal, Triple, Quad, Variable
- **Store**: Thread-safe quad store with SPARQL 1.1 query & update
- **Dataset**: Lightweight in-memory dataset with canonicalization
- **IO**: Standalone parser/serializer for all RDF formats
- **CustomFunctions**: C# → Rust callback bridge for user-defined SPARQL functions & aggregates
- **Oxigraph.Extensions.DotNetRDF**: Bidirectional conversion between Oxigraph and dotNetRDF types

## API Reference

### Core Types

| Type | Description |
|------|-------------|
| `NamedNode` | RDF IRI reference |
| `BlankNode` | RDF blank node (auto-generated or named) |
| `Literal` | RDF literal — plain, language-tagged, or typed |
| `Triple` | RDF triple (subject, predicate, object) — supports RDF-star |
| `Quad` | RDF quad (triple + graph name) |
| `Variable` | SPARQL query variable |
| `DefaultGraph` | The RDF default graph name |
| `BaseDirection` | RDF 1.2 text direction (`Ltr`/`Rtl`) |

### Store

| Member | Description |
|--------|-------------|
| `Store(path?)` | Create in-memory or file-backed store |
| `Store.OpenReadOnly(path)` | Open existing store read-only |
| `Add(Quad)` / `Remove(Quad)` / `Contains(Quad)` | CRUD |
| `Count` / `IsEmpty` | State inspection |
| `Match(s?, p?, o?, g?)` | Pattern-based quad lookup |
| `Query(sparql, options?)` | SPARQL query (returns `QueryResults`) |
| `Update(sparql, options?)` | SPARQL update |
| `Extend(quads)` / `BulkExtend(quads)` | Batch insert |
| `Load(data, format, options?)` | Parse RDF text into store |
| `LoadFromFile(path, format, options?)` | Parse file into store (streaming) |
| `LoadFromStream(stream, format, options?)` | Parse stream into store |
| `BulkLoad(data, format, options?)` | Optimized bulk parse into store |
| `BulkLoadFromFile(path, format, options?)` | Parallel bulk parse from file |
| `Dump(format, options?)` → `string` | Serialize store to string |
| `DumpToFile(path, format, options?)` | Serialize store to file (streaming) |
| `DumpToStream(stream, format, options?)` | Serialize store to stream |
| `Clear()` | Remove all quads |
| `AddGraph(name)` / `RemoveGraph(name)` / `ClearGraph(graph)` | Named graph management |
| `ContainsNamedGraph(graph)` / `NamedGraphs` | Graph introspection |
| `Flush()` / `Optimize()` / `Backup(path)` | Maintenance |
| `QueryAsync(sparql, options?, ct)` → `Task<QueryResults>` | Async SPARQL query |
| `UpdateAsync(sparql, options?, ct)` → `Task` | Async SPARQL update |
| `LoadFromFileAsync(path, format, options?, ct)` → `Task` | Async file parse into store |
| `BulkLoadFromFileAsync(path, format, options?, ct)` → `Task` | Async parallel bulk parse |
| `DumpToFileAsync(path, format, options?, ct)` → `Task` | Async store serialization to file |
| `LoadFromStreamAsync(stream, format, options?, ct)` → `Task` | Async stream parse into store |
| `DumpToStreamAsync(stream, format, options?, ct)` → `Task` | Async store serialization to stream |
| `BackupAsync(targetDirectory, ct)` → `Task` | Async backup |

### Dataset (In-Memory)

| Member | Description |
|--------|-------------|
| `Dataset()` / `Dataset(quads)` | Create empty or pre-populated |
| `Add(Quad)` / `Remove(Quad)` / `Discard(Quad)` | CRUD |
| `Count` / `IsEmpty` / `Contains(Quad)` | State |
| `Match(s?, p?, o?, g?)` | Pattern matching |
| `QuadsForSubject/QuadsForPredicate/QuadsForObject/QuadsForGraphName` | Shortcut lookups |
| `Extend(quads)` / `Clear()` | Bulk insert / clear |
| `Canonicalize(algorithm)` | RDF dataset canonicalization (Unstable, Rdfc10Sha256, Rdfc10Sha384) |
| `Load(data, format, options?)` / `Dump(format, options?)` | I/O |
| `LoadAsync(data, format, options?, ct)` → `Task` | Async RDF text parse into dataset |
| `CanonicalizeAsync(algorithm?, ct)` → `Task` | Async canonicalization |

### IO (Standalone)

Static parse/serialize functions on `Oxigraph.IO`:

| Member | Description |
|--------|-------------|
| `Parse(data, format, baseIri?, options?)` → `IReadOnlyList<Quad>` | Parse RDF text |
| `ParseFromFile(path, format, baseIri?, options?)` → `IReadOnlyList<Quad>` | Parse RDF file |
| `ParseFromStream(stream, format, baseIri?)` → `IReadOnlyList<Quad>` | Parse RDF stream |
| `ParseIterator(path, format, baseIri?)` → `ParseIterator` | Lazy file parser |
| `Serialize(quads, format, options?)` → `string` | Serialize to string |
| `SerializeToFile(path, quads, format, options?)` | Serialize to file |
| `SerializeToStream(stream, quads, format, options?)` | Serialize to stream |
| `ParseQueryResults(data, format)` → `QueryResults` | Parse SPARQL results |
| **Async methods** | |
| `ParseFromFileAsync(path, format, baseIri?, options?, ct)` → `Task<IReadOnlyList<Quad>>` | Async RDF file parse |
| `SerializeToFileAsync(path, quads, format, options?, ct)` → `Task` | Async RDF serialization to file |
| `ParseFromStreamAsync(stream, format, baseIri?, ct)` → `Task<IReadOnlyList<Quad>>` | Async RDF stream parse |
| `SerializeToStreamAsync(stream, quads, format, options?, ct)` → `Task` | Async RDF serialization to stream |

### SPARQL Query Results

| Type | Returned By | Description |
|------|-------------|-------------|
| `QueryBoolean` | ASK | Boolean result |
| `QuerySolutions` | SELECT | Row iterator (lazy) |
| `QueryTriples` | CONSTRUCT / DESCRIBE | Triple iterator (lazy) |

Each has `Serialize(format)` → `string`, `SerializeToStream(stream, format)`, and `SerializeToFile(path, format)`.

### Options Records

| Record | Used By | Key Fields |
|--------|---------|------------|
| `QueryOptions` | `Store.Query()` | `BaseIri`, `Prefixes`, `UseDefaultGraphAsUnion`, `DefaultGraphs`, `NamedGraphs`, `Substitutions`, `CustomFunctions`, `CustomAggregateFunctions` |
| `UpdateOptions` | `Store.Update()` | `BaseIri`, `Prefixes`, `CustomFunctions`, `CustomAggregateFunctions` |
| `LoadOptions` | `Store.Load()` / `Dataset.Load()` | `BaseIri`, `ToGraph`, `Lenient`, `RenameBlankNodes` |
| `DumpOptions` | `Store.Dump()` / `IO.Serialize()` | `FromGraph`, `BaseIri`, `Prefixes` |
| `ParseOptions` | `IO.Parse()` | `BaseIri`, `WithoutNamedGraphs`, `RenameBlankNodes`, `Lenient` |

### Format Metadata

```csharp
// RDF formats
RdfFormat.Turtle.MediaType()        // → "text/turtle"
RdfFormat.Turtle.FileExtension()    // → "ttl"
RdfFormat.Turtle.Iri()              // → W3C format IRI
RdfFormat.Turtle.Name()             // → "Turtle"
RdfFormat.Turtle.SupportsDatasets() // → false

// Lookup
FormatMetadata.FromExtension("ttl")     // → RdfFormat.Turtle
FormatMetadata.FromMediaType("text/turtle") // → RdfFormat.Turtle

// Query results formats
QueryResultsFormat.Json.MediaType()     // → "application/sparql-results+json"
FormatMetadata.QueryFromExtension("srj") // → QueryResultsFormat.Json
```

### Exceptions

| Exception | Description |
|-----------|-------------|
| `OxigraphException` | Base exception |
| `StoreException` | Store operation errors |
| `ParseException` | RDF parse errors (with `FilePath` and `Line`) |
| `SparqlSyntaxException` | SPARQL syntax errors |
| `SparqlEvaluationException` | SPARQL runtime errors |

## Help

Feel free to use [GitHub discussions](https://github.com/oxigraph/oxigraph/discussions) or [the Gitter chat](https://gitter.im/oxigraph/community) to ask questions or talk about Oxigraph.
[Bug reports](https://github.com/oxigraph/oxigraph/issues) are also very welcome.

If you need advanced support or are willing to pay to get some extra features, feel free to reach out to [Tpt](https://github.com/Tpt).

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.