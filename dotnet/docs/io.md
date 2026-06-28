# RDF Parsing and Serialization

## Static IO Class

All parse/serialize functions are on the static `Oxigraph.IO` class.

```csharp
namespace Oxigraph;

public static class IO
```

## Parsing

### `IO.Parse`

Parse an RDF string into quads.

```csharp
public static IReadOnlyList<Quad> Parse(
    string data,
    RdfFormat format,
    string? baseIri = null,
    ParseOptions? parseOptions = null)
```

```csharp
var quads = IO.Parse(
    "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .",
    RdfFormat.Turtle,
    "http://example.com/");
// → [Quad(S=NamedNode("http://example.com/s"), P=..., O=Literal("hello"))]
```

### `IO.ParseFromFile`

Parse an RDF file into quads (streaming, no memory limit).

```csharp
public static IReadOnlyList<Quad> ParseFromFile(
    string filePath,
    RdfFormat format,
    string? baseIri = null,
    ParseOptions? parseOptions = null)
```

### `IO.ParseFromStream`

Parse RDF from a .NET `Stream` into quads.

```csharp
public static IReadOnlyList<Quad> ParseFromStream(
    Stream stream,
    RdfFormat format,
    string? baseIri = null)
```

### `IO.ParseIterator`

Lazily parse an RDF file, yielding quads one at a time. Provides prefix and base IRI introspection during iteration.

```csharp
public static ParseIterator ParseIterator(
    string filePath,
    RdfFormat format,
    string? baseIri = null)
```

```csharp
using var iter = IO.ParseIterator("huge.ttl", RdfFormat.Turtle);

// Prefixes and BaseIri are available after reading the first quad
foreach (var quad in iter)
    Process(quad);

Console.WriteLine(iter.BaseIri);    // e.g., "http://example.com/"
foreach (var (prefix, iri) in iter.Prefixes)
    Console.WriteLine($"@{prefix}: {iri}");
```

### ParseOptions

```csharp
public sealed record ParseOptions(
    string? BaseIri = null,
    bool WithoutNamedGraphs = false,
    bool RenameBlankNodes = false,
    bool Lenient = false);
```

| Field | Description |
|-------|-------------|
| `BaseIri` | Base IRI for relative IRI resolution |
| `WithoutNamedGraphs` | Reject input with named graphs (throws on TriG) |
| `RenameBlankNodes` | Randomize blank node IDs to avoid conflicts |
| `Lenient` | Skip some validation (accepts invalid IRIs/language tags) |

## Serialization

### `IO.Serialize`

Serialize quads to an RDF string.

```csharp
public static string Serialize(
    IEnumerable<Quad> quads,
    RdfFormat format,
    DumpOptions? options = null)
```

```csharp
var result = IO.Serialize(quads, RdfFormat.NTriples);
// → "<http://example.com/s> <http://example.com/p> \"hello\" .\n"
```

### `IO.SerializeToFile`

Serialize quads to a file (streaming, no memory limit).

```csharp
public static void SerializeToFile(
    string filePath,
    IEnumerable<Quad> quads,
    RdfFormat format,
    DumpOptions? options = null)
```

### `IO.SerializeToStream`

Serialize quads to a .NET `Stream`.

```csharp
public static void SerializeToStream(
    Stream stream,
    IEnumerable<Quad> quads,
    RdfFormat format,
    DumpOptions? options = null)
```

### DumpOptions

```csharp
public sealed record DumpOptions(
    IGraphName? FromGraph = null,
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null);
```

| Field | Description |
|-------|-------------|
| `FromGraph` | Only serialize quads from this graph |
| `BaseIri` | Base IRI for relative IRI output (Turtle/TriG) |
| `Prefixes` | Prefix definitions for compact output (Turtle/TriG) |

```csharp
var result = IO.Serialize(quads, RdfFormat.Turtle, new DumpOptions
{
    Prefixes = new Dictionary<string, string> { ["ex"] = "http://example.com/" }
});
// → "@prefix ex: <http://example.com/> .\nex:s ex:p \"hello\" .\n"
```

## Formats

### `RdfFormat` Enum

```csharp
public enum RdfFormat
{
    N3,
    NQuads,
    NTriples,
    RdfXml,
    TriG,
    Turtle,
    JsonLd,
    StreamingJsonLd,  // Parse only — flatter JSON-LD structure
}
```

### Format Metadata

Extension methods on `RdfFormat` via `FormatMetadata`:

| Method | Example Output |
|--------|---------------|
| `format.MediaType()` | `"text/turtle"` |
| `format.FileExtension()` | `"ttl"` |
| `format.Iri()` | `"http://www.w3.org/ns/formats/Turtle"` |
| `format.Name()` | `"Turtle"` |
| `format.SupportsDatasets()` | `false` (Turtle doesn't) |

Format lookup:

| Method | Example |
|--------|---------|
| `FormatMetadata.FromExtension("ttl")` → `RdfFormat?` | → `RdfFormat.Turtle` |
| `FormatMetadata.FromMediaType("text/turtle")` → `RdfFormat?` | → `RdfFormat.Turtle` |

```csharp
if (FormatMetadata.FromExtension("xyz") is null)
    throw new Exception("Unknown format");
```

## Query Results Parsing

### `IO.ParseQueryResults`

Parse SPARQL query results from XML, JSON, CSV, or TSV.

```csharp
public static QueryResults ParseQueryResults(
    string data,
    QueryResultsFormat format)
```

```csharp
// Parse TSV boolean
var result = IO.ParseQueryResults("true", QueryResultsFormat.Tsv);
var boolean = (QueryBoolean)result;
Console.WriteLine(boolean.Value); // True

// Parse JSON solutions
var json = @"{
  ""head"": {""vars"": [""s""]},
  ""results"": {""bindings"": [
    {""s"": {""type"": ""uri"", ""value"": ""http://example.com/s""}}
  ]}
}";
var sols = (QuerySolutions)IO.ParseQueryResults(json, QueryResultsFormat.Json);
Console.WriteLine(sols.Count); // 1
```

### `QueryResultsFormat` Enum

```csharp
public enum QueryResultsFormat
{
    Xml,
    Json,
    Csv,
    Tsv,
}
```

### Query Results Format Metadata

| Method | Example Output |
|--------|---------------|
| `format.MediaType()` | `"application/sparql-results+json"` |
| `format.FileExtension()` | `"srj"` |
| `format.Iri()` | `"http://www.w3.org/ns/formats/SPARQL_Results_JSON"` |
| `format.Name()` | `"SPARQL Results in JSON"` |

Format lookup:

| Method | Example |
|--------|---------|
| `FormatMetadata.QueryFromExtension("srj")` → `QueryResultsFormat?` | → `QueryResultsFormat.Json` |
| `FormatMetadata.QueryFromMediaType("application/sparql-results+json")` | → `QueryResultsFormat.Json` |

## Async API

All long-running I/O operations have async overloads with `CancellationToken` support:

```csharp
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));

// Async file parse
var quads = await IO.ParseFromFileAsync(
    "data.ttl", RdfFormat.Turtle, "http://example.com/", ct: cts.Token);

// Async file serialize
await IO.SerializeToFileAsync(
    "out.ttl", quads, RdfFormat.Turtle, ct: cts.Token);

// Async stream parse
await using var stream = File.OpenRead("data.ttl");
var quads = await IO.ParseFromStreamAsync(
    stream, RdfFormat.Turtle, "http://example.com/", cts.Token);

// Async stream serialize
await using var output = File.Create("out.ttl");
await IO.SerializeToStreamAsync(
    output, quads, RdfFormat.Turtle, ct: cts.Token);
```

| Method | Return |
|--------|--------|
| `ParseFromFileAsync(path, format, baseIri?, options?, ct)` | `Task<IReadOnlyList<Quad>>` |
| `SerializeToFileAsync(path, quads, format, options?, ct)` | `Task` |
| `ParseFromStreamAsync(stream, format, baseIri?, ct)` | `Task<IReadOnlyList<Quad>>` |
| `SerializeToStreamAsync(stream, quads, format, options?, ct)` | `Task` |