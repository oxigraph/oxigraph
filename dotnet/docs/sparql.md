# SPARQL Utilities

## Variable

```csharp
namespace Oxigraph;

public sealed record Variable(string Value)
```

```csharp
var v = new Variable("s");
Console.WriteLine(v);          // "?s"
Console.WriteLine(v.Value);    // "s"
```

## QueryResults Base Class

```csharp
public abstract class QueryResults : IDisposable
```

`QueryResults` is the base class returned by `Store.Query()`. It is `IDisposable` — disposing
releases the streaming FFI iterator and cleans up any registered custom functions.

> **Important**: Always dispose `QueryResults` after use, especially when using custom functions.

---

## SELECT Results — `QuerySolutions`

```csharp
public class QuerySolutions : QueryResults, IEnumerable<QuerySolution>
```

Lazy row iterator. Each call to `MoveNext()` fetches one row from the Rust FFI layer.

| Member | Description |
|--------|-------------|
| `Variables` → `IReadOnlyList<Variable>` | Ordered list of result variables |
| `Count` → `int` | Number of solutions (enumerates fully on first access) |
| `this[int index]` → `QuerySolution` | Indexed access (enumerates up to index) |
| `GetEnumerator()` | Lazy row-by-row enumeration |
| `Serialize(QueryResultsFormat)` → `string` | Serialize to XML/JSON/CSV/TSV |
| `SerializeToStream(Stream, QueryResultsFormat)` | Serialize to stream |
| `SerializeToFile(string, QueryResultsFormat)` | Serialize to file |

```csharp
var results = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
var solutions = (QuerySolutions)results;

Console.WriteLine(string.Join(", ", solutions.Variables)); // "s, p, o"

foreach (var row in solutions)
    Console.WriteLine($"{row["s"]} {row["p"]} {row["o"]}");
```

## QuerySolution — Single Row

```csharp
public sealed class QuerySolution
```

| Member | Description |
|--------|-------------|
| `this[string variable]` → `ITerm?` | Access by variable name |
| `this[Variable variable]` → `ITerm?` | Access by Variable object |
| `this[int index]` → `ITerm?` | Access by positional index (SELECT order) |
| `TryGetValue(string, out ITerm?)` → `bool` | Try-get by name |
| `Variables` → `IEnumerable<string>` | Ordered variable names |
| `Count` → `int` | Number of bindings in this row |
| `Deconstruct(out ITerm?, out ITerm?)` | 2-tuple pattern |
| `Deconstruct(out ITerm?, out ITerm?, out ITerm?)` | 3-tuple pattern |

```csharp
var row = solutions.First();

// By variable name
ITerm? s = row["s"];
ITerm? p = row["p"];

// By Variable object
ITerm? s2 = row[new Variable("s")];

// By positional index
ITerm? first = row[0];   // equals row["s"]
ITerm? second = row[1];  // equals row["p"]

// Try-get
if (row.TryGetValue("o", out var o))
    Console.WriteLine(o);

// Deconstruct
var (subj, pred, obj) = row;
```

---

## ASK Results — `QueryBoolean`

```csharp
public class QueryBoolean : QueryResults
```

| Member | Description |
|--------|-------------|
| `Value` → `bool` | The boolean answer |
| `ToString()` → `string` | `"True"` or `"False"` |
| `Serialize(QueryResultsFormat)` → `string` | Serialize to XML/JSON/CSV/TSV |
| `SerializeToStream(Stream, QueryResultsFormat)` | Serialize to stream |
| `SerializeToFile(string, QueryResultsFormat)` | Serialize to file |

```csharp
var result = (QueryBoolean)store.Query("ASK { ?s ?p ?o }");
Console.WriteLine(result.Value); // True
```

---

## CONSTRUCT/DESCRIBE Results — `QueryTriples`

```csharp
public class QueryTriples : QueryResults, IEnumerable<Triple>
```

Lazy triple iterator. Each `MoveNext()` fetches one triple from the Rust FFI layer.

| Member | Description |
|--------|-------------|
| `GetEnumerator()` | Lazy triple-by-triple enumeration |
| `Serialize(RdfFormat)` → `string` | Serialize to any RDF format |
| `SerializeToStream(Stream, RdfFormat)` | Serialize to stream |
| `SerializeToFile(string, RdfFormat)` | Serialize to file |

```csharp
var results = store.Query(
    "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
var triples = (QueryTriples)results;

foreach (var triple in triples)
    Console.WriteLine(triple);
```

---

## Result Serialization

All result types support serialization in all formats:

```csharp
// SELECT → JSON
var json = solutions.Serialize(QueryResultsFormat.Json);

// SELECT → XML
var xml = solutions.Serialize(QueryResultsFormat.Xml);

// SELECT → CSV
var csv = solutions.Serialize(QueryResultsFormat.Csv);

// SELECT → TSV
var tsv = solutions.Serialize(QueryResultsFormat.Tsv);

// CONSTRUCT → NTriples
var nt = triples.Serialize(RdfFormat.NTriples);

// CONSTRUCT → Turtle
var tt = triples.Serialize(RdfFormat.Turtle);

// ASK → CSV
var csvBool = boolean.Serialize(QueryResultsFormat.Csv); // "true" or "false"
```

---

## QueryOptions

```csharp
public sealed record QueryOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null,
    bool UseDefaultGraphAsUnion = false,
    IReadOnlyList<IGraphName>? DefaultGraphs = null,
    IReadOnlyList<IGraphName>? NamedGraphs = null,
    Dictionary<string, Func<ITerm[], ITerm?>>? CustomFunctions = null,
    Dictionary<string, ITerm>? Substitutions = null,
    Dictionary<string, Func<CustomFunctions.IAggregateAccumulator>>? CustomAggregateFunctions = null);
```

| Field | Description |
|-------|-------------|
| `BaseIri` | Base IRI for relative IRIs in the query |
| `Prefixes` | Prefix definitions for the query (e.g., `{ "rdf": "http://..." }`) |
| `UseDefaultGraphAsUnion` | Search all named graphs when querying the default graph |
| `DefaultGraphs` | Restrict the default graph to these graphs |
| `NamedGraphs` | Restrict GRAPH clauses to these graphs |
| `CustomFunctions` | Inline custom SPARQL functions (auto-registered & auto-cleaned) |
| `Substitutions` | Pre-bind variables before query evaluation |
| `CustomAggregateFunctions` | Inline custom aggregate functions |

### Base IRI and Prefixes

```csharp
store.Query("ASK { <> bar: baz: }",
    new QueryOptions
    {
        BaseIri = "http://foo",
        Prefixes = new Dictionary<string, string>
        {
            ["bar"] = "http://bar",
            ["baz"] = "http://baz",
        }
    });
```

### Graph Restrictions

```csharp
// Union default graph — search all named graphs
store.Query("SELECT ?s WHERE { ?s ?p ?o }",
    new QueryOptions { UseDefaultGraphAsUnion = true });

// Restrict default graph to specific graphs
store.Query("SELECT ?s WHERE { ?s ?p ?o }",
    new QueryOptions { DefaultGraphs = [new NamedNode("http://example.com/g")] });

// Restrict named graphs
store.Query("SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o } }",
    new QueryOptions { NamedGraphs = [new NamedNode("http://example.com/g1")] });
```

### Substitutions

Pre-bind variables to specific terms before query evaluation:

```csharp
store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }",
    new QueryOptions
    {
        Substitutions = new Dictionary<string, ITerm>
        {
            ["s"] = new NamedNode("http://example.com/s1")
        }
    });
```

---

## UpdateOptions

```csharp
public sealed record UpdateOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null,
    Dictionary<string, Func<ITerm[], ITerm?>>? CustomFunctions = null,
    Dictionary<string, Func<CustomFunctions.IAggregateAccumulator>>? CustomAggregateFunctions = null);
```

```csharp
store.Update("INSERT DATA { <> bar: \"test\" }",
    new UpdateOptions
    {
        BaseIri = "http://example.com/s",
        Prefixes = new Dictionary<string, string> { ["bar"] = "http://example.com/p" }
    });
```

---

## Custom Functions

### Simple Custom Functions

Register a C# function for use in SPARQL expressions:

```csharp
// Global registration (persists until unregistered)
CustomFunctions.Register("http://example.com/upper",
    args => new Literal(((Literal)args[0]).Value.ToUpperInvariant()));

// Use in query
var results = store.Query(@"
    PREFIX my: <http://example.com/>
    SELECT ?upper WHERE {
        ?s ?p ?o .
        BIND(my:upper(?o) AS ?upper)
    }");

CustomFunctions.Unregister("http://example.com/upper");
```

Or provide inline with auto-cleanup via `QueryOptions`:

```csharp
store.Query("PREFIX my: <http://example.com/> SELECT (my:concat(?s, ?p) AS ?c) WHERE { ?s ?p ?o }",
    new QueryOptions
    {
        CustomFunctions = new()
        {
            ["http://example.com/concat"] = args =>
                new Literal(((Literal)args[0]).Value + ((Literal)args[1]).Value)
        }
    });
// Auto-unregistered when QueryResults is disposed
```

Use in `Update` with the same pattern:

```csharp
store.Update(@"PREFIX my: <http://example.com/>
    INSERT { ?s <http://new> ?upper }
    WHERE { ?s ?p ?o . BIND(my:upper(?o) AS ?upper) }",
    new UpdateOptions
    {
        CustomFunctions = new()
        {
            ["http://example.com/upper"] = args =>
                new Literal(((Literal)args[0]).Value.ToUpperInvariant())
        }
    });
```

### Custom Aggregate Functions

Implement `IAggregateAccumulator`:

```csharp
public interface IAggregateAccumulator
{
    void Accumulate(ITerm term);  // Called for each value in the group
    ITerm? Finish();               // Return the final aggregated value
}
```

```csharp
sealed class ConcatAggregate : CustomFunctions.IAggregateAccumulator
{
    private readonly List<string> _values = [];
    public void Accumulate(ITerm term) => _values.Add(((Literal)term).Value);
    public ITerm? Finish() => new Literal(string.Join(", ", _values));
}

// Global registration
CustomFunctions.RegisterAggregate("http://example.com/concat",
    () => new ConcatAggregate());

// Use in query with GROUP BY
var results = store.Query(@"
    PREFIX my: <http://example.com/>
    SELECT ?s (my:concat(?p) AS ?ps) WHERE { ?s ?p ?o }
    GROUP BY ?s");

CustomFunctions.UnregisterAggregate("http://example.com/concat");
```

Or inline via `QueryOptions`:

```csharp
store.Query(@"PREFIX my: <http://example.com/>
    SELECT (my:total(?o) AS ?cnt) WHERE { ?s ?p ?o }",
    new QueryOptions
    {
        CustomAggregateFunctions = new()
        {
            ["http://example.com/total"] = () => new CountAggregate()
        }
    });
```

### How It Works

Custom functions use a C# → Rust callback bridge:
1. Functions are registered with the Rust FFI layer via `register_custom_function`
2. During SPARQL evaluation, Rust calls back into C# through a pinned delegate
3. Function arguments arrive as JSON, are deserialized to `ITerm[]`, and the result is serialized back
4. Cleanup happens when `QueryResults.Dispose()` is called (for inline functions) or explicitly via `Unregister()`

---

## Exceptions

| Exception | Description |
|-----------|-------------|
| `SparqlSyntaxException` | SPARQL syntax error |
| `SparqlEvaluationException` | SPARQL runtime evaluation error |
| `OxigraphException` | Base exception for all Oxigraph errors |