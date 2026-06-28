# RDF Model

Oxigraph provides C# classes to represent basic RDF concepts.

## IRIs — `NamedNode`

[IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) references.

```csharp
namespace Oxigraph;

public sealed record NamedNode(string Value)
    : INamedOrBlankNode, IGraphName
```

| Member | Description |
|--------|-------------|
| `Value` | The absolute IRI string |

```csharp
var ex = new NamedNode("http://example.com/");
Console.WriteLine(ex.Value); // "http://example.com/"
```

Implements `INamedOrBlankNode` (usable as subject in Triple/Quad) and `IGraphName` (usable as graph name in Quad).

---

## Blank Nodes — `BlankNode`

[Blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node) identifiers.

```csharp
namespace Oxigraph;

public sealed record BlankNode(string Value)
    : INamedOrBlankNode, IGraphName
```

| Member | Description |
|--------|-------------|
| `BlankNode()` | Auto-generate a unique blank node ID |
| `BlankNode(string value)` | Create with an explicit ID |
| `Value` | The blank node identifier |

```csharp
// Auto-generated unique ID
var a = new BlankNode();
var b = new BlankNode();
Console.WriteLine(a.Value); // e.g., "b1_abc12345"
Console.WriteLine(b.Value); // e.g., "b2_abc12345"

// Explicit ID
var named = new BlankNode("myNode");
```

Implements `INamedOrBlankNode` and `IGraphName`.

---

## Literals — `Literal`

RDF [literals](https://www.w3.org/TR/rdf11-concepts/#dfn-literal) — plain, language-tagged, or typed.

```csharp
namespace Oxigraph;

public sealed record Literal(string Value,
    string? Language = null,
    NamedNode? Datatype = null,
    BaseDirection? Direction = null) : ITerm
```

| Member | Description |
|--------|-------------|
| `Value` | The lexical form |
| `Language` | Optional language tag (RFC 5646 / BCP 47) |
| `Datatype` | Optional datatype IRI (`xsd:string` by default) |
| `Direction` | Optional base direction (RDF 1.2: `Ltr` or `Rtl`) |

### Factory Methods

| Method | Description |
|--------|-------------|
| `Literal.FromInt(int)` → `Literal` | `xsd:integer` literal |
| `Literal.FromDouble(double)` → `Literal` | `xsd:double` literal |
| `Literal.FromBool(bool)` → `Literal` | `xsd:boolean` literal |

### Implicit Conversions

```csharp
Literal intLit = 42;     // xsd:integer
Literal dblLit = 3.14;   // xsd:double
Literal boolLit = true;  // xsd:boolean
```

### XSD Constants

| Constant | IRI |
|----------|-----|
| `Literal.XsdString` | `http://www.w3.org/2001/XMLSchema#string` |
| `Literal.XsdInteger` | `http://www.w3.org/2001/XMLSchema#integer` |
| `Literal.XsdDouble` | `http://www.w3.org/2001/XMLSchema#double` |
| `Literal.XsdBoolean` | `http://www.w3.org/2001/XMLSchema#boolean` |

### Examples

```csharp
// Simple literal
var plain = new Literal("hello");

// Language-tagged
var enLit = new Literal("hello", Language: "en");

// With direction (RDF 1.2)
var rtlLit = new Literal("שלום", Language: "he", Direction: BaseDirection.Rtl);

// Typed
var intLit = new Literal("42", Datatype: Literal.XsdInteger);
```

---

## `BaseDirection`

RDF 1.2 text direction for language-tagged strings.

```csharp
namespace Oxigraph;

public enum BaseDirection
{
    Ltr,  // Left-to-right
    Rtl,  // Right-to-left
}
```

---

## Triples — `Triple`

[RDF triple](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple) (subject, predicate, object).
Supports [RDF-star](https://w3c.github.io/rdf-star/) — a triple can appear in subject or object position.

```csharp
namespace Oxigraph;

public sealed record Triple(
    INamedOrBlankNode Subject,
    NamedNode Predicate,
    ITerm Object) : ITerm, INamedOrBlankNode
```

| Member | Type | Description |
|--------|------|-------------|
| `Subject` | `INamedOrBlankNode` | NamedNode, BlankNode, or Triple (RDF-star) |
| `Predicate` | `NamedNode` | Always an IRI |
| `Object` | `ITerm` | NamedNode, BlankNode, Literal, or Triple (RDF-star) |

```csharp
var t = new Triple(
    new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"),
    new NamedNode("http://example.com/o"));

Console.WriteLine(t.Subject);   // http://example.com/s
Console.WriteLine(t.Predicate); // http://example.com/p
Console.WriteLine(t.Object);    // http://example.com/o

// RDF-star: triple as object
var inner = new Triple(
    new NamedNode("http://example.com/os"),
    new NamedNode("http://example.com/op"),
    new NamedNode("http://example.com/oo"));
var outer = new Triple(
    new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"),
    inner); // Triple as object

// Deconstruct
var (s, p, o) = t;
```

Triple implements `ITerm` and `INamedOrBlankNode` for RDF-star support (triple in subject position).

---

## Quads — `Quad`

[Quad](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) — a triple in a named graph.

```csharp
namespace Oxigraph;

public sealed record Quad(
    INamedOrBlankNode Subject,
    NamedNode Predicate,
    ITerm Object,
    IGraphName Graph)
```

| Member | Type | Description |
|--------|------|-------------|
| `Subject` | `INamedOrBlankNode` | NamedNode, BlankNode, or Triple |
| `Predicate` | `NamedNode` | Always an IRI |
| `Object` | `ITerm` | NamedNode, BlankNode, Literal, or Triple |
| `Graph` | `IGraphName` | NamedNode, BlankNode, or DefaultGraph |
| `Triple` (computed) | `Triple` | The underlying triple (same S/P/O, no graph) |

```csharp
// Quad in the default graph
var qDefault = new Quad(
    new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"),
    new Literal("1"),
    new DefaultGraph());

// Quad in a named graph
var qNamed = new Quad(
    new NamedNode("http://example.com/s"),
    new NamedNode("http://example.com/p"),
    new Literal("1"),
    new NamedNode("http://example.com/g"));

// Deconstruct
var (s, p, o, g) = qDefault;
```

---

## Default Graph — `DefaultGraph`

The [RDF default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph) name.

```csharp
namespace Oxigraph;

public readonly record struct DefaultGraph : IGraphName
```

```csharp
var dg = new DefaultGraph();
Console.WriteLine(dg); // "DEFAULT"
```

---

## Dataset

An in-memory [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset). Uses the native
`oxigraph::model::Dataset` — no RocksDB, no disk.

> **Warning**: This structure interns strings and does not garbage-collect removed terms — memory grows monotonically if you insert and remove many different terms.

```csharp
namespace Oxigraph;

public sealed class Dataset : IEnumerable<Quad>, IDisposable
```

### Constructors

| Constructor | Description |
|-------------|-------------|
| `new Dataset()` | Create an empty dataset |
| `new Dataset(IEnumerable<Quad> quads)` | Pre-populated from quads |

### Methods

| Method | Description |
|--------|-------------|
| `Add(Quad)` | Insert a quad |
| `Remove(Quad)` | Remove a quad (throws `KeyNotFoundException` if not found) |
| `Discard(Quad)` | Remove if present, silent no-op if not |
| `Contains(Quad)` → `bool` | Check existence |
| `Count` → `int` | Number of quads |
| `IsEmpty` → `bool` | Whether empty |
| `Clear()` | Remove all quads |
| `Extend(IEnumerable<Quad>)` | Add all quads from a collection |
| `Match(s?, p?, o?, g?)` → `IReadOnlyList<Quad>` | Pattern-based lookup |
| `QuadsForSubject(INamedOrBlankNode)` | Shorthand for `Match(subject: ...)` |
| `QuadsForPredicate(NamedNode)` | Shorthand for `Match(predicate: ...)` |
| `QuadsForObject(ITerm)` | Shorthand for `Match(@object: ...)` |
| `QuadsForGraphName(IGraphName)` | Shorthand for `Match(graph: ...)` |
| `Canonicalize(CanonicalizationAlgorithm)` | Canonicalize blank nodes in-place |
| `Load(string, RdfFormat, LoadOptions?)` | Parse RDF text into the dataset |
| `Dump(RdfFormat, DumpOptions?)` → `string` | Serialize dataset to RDF text |
| `LoadAsync(string, RdfFormat, LoadOptions?, CancellationToken)` → `Task` | Async parse RDF text into the dataset |
| `CanonicalizeAsync(CanonicalizationAlgorithm, CancellationToken)` → `Task` | Async canonicalization |

### CanonicalizationAlgorithm

```csharp
public enum CanonicalizationAlgorithm
{
    Unstable,        // PyOxigraph preferred algorithm
    Rdfc10Sha256,    // RDFC-1.0 with SHA-256
    Rdfc10Sha384,    // RDFC-1.0 with SHA-384
}
```

```csharp
using var ds1 = new Dataset();
ds1.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));
ds1.Canonicalize(CanonicalizationAlgorithm.Rdfc10Sha256);
Console.WriteLine(ds1);
```

---

## JSON Serialization

All RDF model types support `System.Text.Json` serialization matching the Rust serde format:

```csharp
using System.Text.Json;

// Serialize with proper converters
var opts = new JsonSerializerOptions
{
    Converters = { new TermConverter() }
};
var json = JsonSerializer.Serialize<ITerm>(new NamedNode("http://foo"), opts);
// → {"type":"uri","value":"http://foo"}

// Deserialize
var term = JsonSerializer.Deserialize<ITerm>(json, opts);
Console.WriteLine(term); // NamedNode { Value = http://foo }
```

### Converters

| Converter | For type |
|-----------|----------|
| `TermConverter` | `ITerm` (NamedNode, BlankNode, Literal, Triple) |
| `NamedOrBlankNodeConverter` | `INamedOrBlankNode` |
| `NamedNodeConverter` | `NamedNode` |
| `GraphNameConverter` | `IGraphName` (NamedNode, BlankNode, DefaultGraph) |