using System.Collections;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Oxigraph;

/// <summary>Base class for SPARQL query results.</summary>
public abstract class QueryResults
{
    /// <summary>Parse a JSON response from Rust into the appropriate result type.</summary>
    internal static QueryResults FromJson(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        var type = root.GetProperty("type").GetString()!;

        return type switch
        {
            "boolean" => new QueryBoolean(root.GetProperty("value").GetBoolean()),
            "solutions" => ParseSolutions(root),
            "triples" => ParseTriples(root),
            _ => throw new OxigraphException($"Unknown query result type: {type}"),
        };
    }

    private static QuerySolutions ParseSolutions(JsonElement root)
    {
        var variables = root.GetProperty("variables").EnumerateArray()
            .Select(v => new Variable(v.GetString()!))
            .ToList();
        var rows = root.GetProperty("rows").EnumerateArray()
            .Select(r => new QuerySolution(r))
            .ToList();
        return new QuerySolutions(variables, rows);
    }

    private static QueryTriples ParseTriples(JsonElement root)
    {
        var triples = root.GetProperty("triples").EnumerateArray()
            .Select(t => JsonSerializer.Deserialize<Triple>(t.GetRawText())!)
            .ToList();
        return new QueryTriples(triples);
    }
}

/// <summary>Result of an ASK query.</summary>
public sealed class QueryBoolean : QueryResults
{
    public bool Value { get; }
    internal QueryBoolean(bool value) => Value = value;
    public override string ToString() => Value.ToString();
}

/// <summary>Result of a SELECT query.</summary>
public sealed class QuerySolutions : QueryResults, IEnumerable<QuerySolution>
{
    public IReadOnlyList<Variable> Variables { get; }
    private readonly IReadOnlyList<QuerySolution> _rows;

    internal QuerySolutions(IReadOnlyList<Variable> variables, IReadOnlyList<QuerySolution> rows)
    {
        Variables = variables;
        _rows = rows;
    }

    public IEnumerator<QuerySolution> GetEnumerator() => _rows.GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
    public int Count => _rows.Count;
    public QuerySolution this[int index] => _rows[index];
}

/// <summary>A single solution (row) from a SELECT query.</summary>
public sealed class QuerySolution
{
    private readonly Dictionary<string, ITerm?> _bindings = [];

    internal QuerySolution(JsonElement element)
    {
        foreach (var prop in element.EnumerateObject())
        {
            ITerm? term = null;
            if (prop.Value.ValueKind != JsonValueKind.Null)
                term = JsonSerializer.Deserialize<ITerm>(prop.Value.GetRawText(), new JsonSerializerOptions
                {
                    Converters = { new TermConverter() }
                });
            _bindings[prop.Name] = term;
        }
    }

    public ITerm? this[string variable]
    {
        get
        {
            _bindings.TryGetValue(variable, out var term);
            return term;
        }
    }

    public ITerm? this[Variable variable] => this[variable.Value];

    public bool TryGetValue(string variable, out ITerm? value)
        => _bindings.TryGetValue(variable, out value);

    public IEnumerable<string> Variables => _bindings.Keys;

    public int Count => _bindings.Count;
}

/// <summary>Result of a CONSTRUCT or DESCRIBE query.</summary>
public sealed class QueryTriples : QueryResults, IEnumerable<Triple>
{
    private readonly IReadOnlyList<Triple> _triples;
    internal QueryTriples(IReadOnlyList<Triple> triples) => _triples = triples;
    public IEnumerator<Triple> GetEnumerator() => _triples.GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
    public int Count => _triples.Count;
}

/// <summary>Options for SPARQL queries.</summary>
public sealed record QueryOptions(
    string? BaseIri = null,
    bool UseDefaultGraphAsUnion = false);

/// <summary>Options for SPARQL updates.</summary>
public sealed record UpdateOptions(
    string? BaseIri = null);