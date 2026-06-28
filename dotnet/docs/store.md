# RDF Store

```csharp
namespace Oxigraph;

public sealed class Store : IDisposable, IEnumerable<Quad>
```

The `Store` is the main graph database engine. It supports [SPARQL 1.1](https://www.w3.org/TR/sparql11-overview/) query and update, and can operate in two modes:

- **In-memory** — `new Store()` — fast, zero-config
- **File-backed** — `new Store(path)` — persistent, backed by [RocksDB](http://rocksdb.org/)

> **Thread safety**: Store is **not** thread-safe. Callers must synchronize concurrent access. All operations are transactional using the "repeatable read" isolation level.

## Constructors

| Constructor | Description |
|-------------|-------------|
| `new Store()` | Create an in-memory store |
| `new Store(string path)` | Open/create a file-backed store at `path` |
| `Store.OpenReadOnly(string path)` | Open an existing store read-only |

```csharp
// In-memory
using var memStore = new Store();

// File-backed
using var diskStore = new Store("/path/to/data");

// Read-only
using var readStore = Store.OpenReadOnly("/path/to/data");
```

## Quad CRUD

```csharp
// Insert
store.Add(new Quad(
    new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"),
    new Literal("hello"),
    new DefaultGraph()));

// Check
bool exists = store.Contains(quad);     // true

// Remove
store.Remove(quad);

// State
ulong count = store.Count;              // 0
bool empty = store.IsEmpty;             // true
```

## Pattern Matching

```csharp
public IReadOnlyList<Quad> Match(
    INamedOrBlankNode? subject = null,
    NamedNode? predicate = null,
    ITerm? @object = null,
    IGraphName? graph = null)
```

Null/omitted parameters act as wildcards — they match anything.

```csharp
// All quads
var all = store.Match();

// By subject
var bySubj = store.Match(subject: new NamedNode("http://example.com/s"));

// By predicate
var byPred = store.Match(predicate: new NamedNode("http://example.com/p"));

// By object
var byObj = store.Match(@object: new Literal("hello"));

// By graph
var byGraph = store.Match(graph: new DefaultGraph());

// Combined
var combined = store.Match(
    subject: new NamedNode("http://example.com/s"),
    graph: new NamedNode("http://example.com/g"));
```

## Bulk Operations

```csharp
// Extend — insert multiple quads atomically
store.Extend(quads);

// BulkExtend — large sets in 10k chunks (no full materialization)
store.BulkExtend(largeEnumerable);

// Clear all
store.Clear();
```

## Loading Data

| Method | Description |
|--------|-------------|
| `Load(string data, RdfFormat, LoadOptions?)` | Parse RDF text into store |
| `BulkLoad(string data, RdfFormat, LoadOptions?)` | Optimized bulk parse (delegates to Load for in-memory) |
| `LoadFromFile(string path, RdfFormat, LoadOptions?)` | Parse file into store (streaming) |
| `BulkLoadFromFile(string path, RdfFormat, LoadOptions?)` | Parallel bulk parse from file |
| `LoadFromStream(Stream, RdfFormat, LoadOptions?)` | Parse stream into store |

```csharp
// Simple load — default graph
store.Load("<http://s> <http://p> \"o\" .", RdfFormat.NTriples);

// Load into a specific named graph
store.LoadFromFile("data.ttl", RdfFormat.Turtle,
    new LoadOptions { ToGraph = new NamedNode("http://example.com/g") });

// Bulk load with lenient parsing
store.BulkLoadFromFile("large.nt", RdfFormat.NTriples,
    new LoadOptions { Lenient = true });
```

### `LoadOptions`

```csharp
public sealed record LoadOptions(
    string? BaseIri = null,
    IGraphName? ToGraph = null,
    bool Lenient = false,
    bool RenameBlankNodes = false);
```

| Field | Description |
|-------|-------------|
| `BaseIri` | Base IRI for relative IRI resolution |
| `ToGraph` | Insert all parsed quads into this named graph |
| `Lenient` | Skip some validation |
| `RenameBlankNodes` | Randomize blank node IDs |

## Dumping Data

| Method | Description |
|--------|-------------|
| `Dump(RdfFormat, DumpOptions?)` → `string` | Serialize store to string |
| `DumpToFile(string path, RdfFormat, DumpOptions?)` | Serialize store to file (streaming) |
| `DumpToStream(Stream, RdfFormat, DumpOptions?)` | Serialize store to stream |
| `ToString()` → `string` | N-Quads serialization |

```csharp
// Dump a specific graph as Turtle with custom prefixes
var turtle = store.Dump(RdfFormat.Turtle, new DumpOptions
{
    FromGraph = new DefaultGraph(),
    Prefixes = new Dictionary<string, string> { ["ex"] = "http://example.com/" }
});

// Dump all to file
store.DumpToFile("dump.nq", RdfFormat.NQuads);
```

## Named Graph Management

```csharp
// Create an empty named graph
store.AddGraph(new NamedNode("http://example.com/g"));

// List all named graphs
IReadOnlyList<INamedOrBlankNode> graphs = store.NamedGraphs;

// Check if a graph exists
store.ContainsNamedGraph(new NamedNode("http://example.com/g"));  // true
store.ContainsNamedGraph(new DefaultGraph());                      // always true

// Clear all quads from a specific graph (graph name persists)
store.ClearGraph(new NamedNode("http://example.com/g"));

// Remove the graph entirely
store.RemoveGraph(new NamedNode("http://example.com/g"));
```

## Maintenance

```csharp
// Flush pending writes to disk
store.Flush();

// Optimize storage (RocksDB compaction)
store.Optimize();

// Create a backup at target directory
store.Backup("/path/to/backup");
```

## SPARQL Query

```csharp
public QueryResults Query(
    string sparql,
    QueryOptions? options = null)
```

The result type depends on the query form:
- `SELECT` → `QuerySolutions` (lazy row iterator)
- `ASK` → `QueryBoolean`
- `CONSTRUCT` / `DESCRIBE` → `QueryTriples` (lazy triple iterator)

```csharp
using var store = new Store();
store.Add(new Quad(new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"), new Literal("hello"), new DefaultGraph()));

// SELECT
var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");
var solutions = (QuerySolutions)results;
foreach (var row in solutions)
    Console.WriteLine($"{row["s"]} -> {row["o"]}");

// ASK
var askResult = (QueryBoolean)store.Query("ASK { ?s ?p ?o }");
Console.WriteLine(askResult.Value); // True

// CONSTRUCT
var constructResult = (QueryTriples)store.Query(
    "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
foreach (var triple in constructResult)
    Console.WriteLine(triple);

// DESCRIBE
var describeResult = (QueryTriples)store.Query(
    "DESCRIBE <http://example.com/s>");
```

See [SPARQL](sparql.md) for query options, custom functions, and result serialization.

## SPARQL Update

```csharp
public void Update(
    string sparql,
    UpdateOptions? options = null)
```

```csharp
// INSERT DATA
store.Update("INSERT DATA { <http://example.com/s> <http://example.com/p> \"test\" }");

// DELETE DATA
store.Update("DELETE DATA { <http://example.com/s> <http://example.com/p> \"test\" }");

// DELETE WHERE
store.Update("DELETE WHERE { ?s ?p ?o }");

// LOAD from URL
store.Update("LOAD <https://www.w3.org/1999/02/22-rdf-syntax-ns>");

// RDF-star
store.Update("PREFIX : <http://www.example.org/> " +
    "INSERT DATA { :alice :claims << :bob :age 23 >> }");
```

## Async API

All long-running Store operations have `*Async` overloads with `CancellationToken` support.
The sync API remains unchanged — async methods are additive.

```csharp
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));

// Async query
var results = await store.QueryAsync(
    "SELECT ?s WHERE { ?s ?p ?o }", ct: cts.Token);

// Async update
await store.UpdateAsync(
    "INSERT DATA { <http://s> <http://p> \"test\" }", ct: cts.Token);

// Async file I/O
await store.LoadFromFileAsync("data.ttl", RdfFormat.Turtle,
    new LoadOptions { BaseIri = "http://example.com/" }, cts.Token);
await store.BulkLoadFromFileAsync("large.nt", RdfFormat.NTriples, ct: cts.Token);
await store.DumpToFileAsync("out.nq", RdfFormat.NQuads, ct: cts.Token);

// Async stream I/O
await using var stream = File.OpenRead("data.ttl");
await store.LoadFromStreamAsync(stream, RdfFormat.Turtle, ct: cts.Token);

// Async backup
await store.BackupAsync("/path/to/backup", cts.Token);
```

| Method | Return |
|--------|--------|
| `QueryAsync(sparql, options?, ct)` | `Task<QueryResults>` |
| `UpdateAsync(sparql, options?, ct)` | `Task` |
| `LoadFromFileAsync(path, format, options?, ct)` | `Task` |
| `BulkLoadFromFileAsync(path, format, options?, ct)` | `Task` |
| `DumpToFileAsync(path, format, options?, ct)` | `Task` |
| `LoadFromStreamAsync(stream, format, options?, ct)` | `Task` |
| `DumpToStreamAsync(stream, format, options?, ct)` | `Task` |
| `BackupAsync(target, ct)` | `Task` |

### Cancellation

Cancellation is checked at `Task.Run` dispatch time. Rust operations in progress cannot be
interrupted, but a cancelled token prevents new operations from starting.

```csharp
using var cts = new CancellationTokenSource();
cts.Cancel(); // cancel immediately

// Throws TaskCanceledException
await store.QueryAsync("SELECT ?s WHERE { ?s ?p ?o }", ct: cts.Token);
```

### Design Notes

- Async methods use `Task.Run` to offload blocking FFI calls to the thread pool
- Stream-based methods first `CopyToAsync` the stream content to memory, then process synchronously
- Store is **not thread-safe** — even with async, callers must serialize concurrent access

## Iteration

`Store` implements `IEnumerable<Quad>`. Iteration uses pattern matching internally.

```csharp
foreach (var quad in store)
    Console.WriteLine(quad);

var list = store.ToList();
```