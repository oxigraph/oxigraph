using System.Collections;
using System.Text.Json;
using System.Text.Json.Serialization;
using Oxigraph.Interop;

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

    /// <summary>Serialize the boolean result to a string (XML/JSON/CSV/TSV).</summary>
    public string Serialize(QueryResultsFormat format)
    {
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        return FFIHelper.Call<string>(() =>
            OxigraphNative.query_boolean_serialize(fmtStr, Value));
    }

    /// <summary>Serialize the boolean result to a stream (XML/JSON/CSV/TSV).</summary>
    public void SerializeToStream(Stream stream, QueryResultsFormat format)
    {
        var result = Serialize(format);
        var bytes = System.Text.Encoding.UTF8.GetBytes(result);
        stream.Write(bytes, 0, bytes.Length);
    }

    /// <summary>Serialize the boolean result to a file (XML/JSON/CSV/TSV).</summary>
    public void SerializeToFile(string filePath, QueryResultsFormat format)
    {
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        FFIHelper.CallVoid(() =>
            OxigraphNative.query_boolean_serialize_to_file(filePath, fmtStr, Value));
    }
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

    /// <summary>Serialize SELECT results to a string (XML/JSON/CSV/TSV).</summary>
    public string Serialize(QueryResultsFormat format)
    {
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        var variablesJson = JsonSerializer.Serialize(Variables.Select(v => v.Value));
        var rowsJson = JsonSerializer.Serialize(_rows.Select(r =>
        {
            var dict = new Dictionary<string, ITerm?>();
            foreach (var v in r.Variables)
                dict[v] = r[v];
            return dict;
        }), new JsonSerializerOptions { Converters = { new TermConverter() } });

        return FFIHelper.Call<string>(() =>
            OxigraphNative.query_solutions_serialize(fmtStr, variablesJson, rowsJson));
    }

    /// <summary>Serialize SELECT results to a stream (XML/JSON/CSV/TSV).</summary>
    public void SerializeToStream(Stream stream, QueryResultsFormat format)
    {
        var result = Serialize(format);
        var bytes = System.Text.Encoding.UTF8.GetBytes(result);
        stream.Write(bytes, 0, bytes.Length);
    }

    /// <summary>Serialize SELECT results to a file (XML/JSON/CSV/TSV).</summary>
    public void SerializeToFile(string filePath, QueryResultsFormat format)
    {
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        var variablesJson = JsonSerializer.Serialize(Variables.Select(v => v.Value));
        // Convert each solution to a JSON object { "varName": termJson, ... }
        var rowsJson = JsonSerializer.Serialize(_rows.Select(r =>
        {
            var dict = new Dictionary<string, ITerm?>();
            foreach (var v in r.Variables)
                dict[v] = r[v];
            return dict;
        }), new JsonSerializerOptions { Converters = { new TermConverter() } });

        FFIHelper.CallVoid(() =>
            OxigraphNative.query_solutions_serialize_to_file(filePath, fmtStr, variablesJson, rowsJson));
    }
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

    /// <summary>Serialize CONSTRUCT/DESCRIBE results to a string (any RDF format).</summary>
    public string Serialize(RdfFormat format)
    {
        var triplesJson = JsonSerializer.Serialize(_triples);
        return FFIHelper.Call<string>(() =>
            OxigraphNative.query_triples_serialize(IO.FormatToString(format), triplesJson));
    }

    /// <summary>Serialize CONSTRUCT/DESCRIBE results to a stream (any RDF format).</summary>
    public void SerializeToStream(Stream stream, RdfFormat format)
    {
        var result = Serialize(format);
        var bytes = System.Text.Encoding.UTF8.GetBytes(result);
        stream.Write(bytes, 0, bytes.Length);
    }

    /// <summary>Serialize CONSTRUCT/DESCRIBE results to a file in any RDF format.</summary>
    public void SerializeToFile(string filePath, RdfFormat format)
    {
        var triplesJson = JsonSerializer.Serialize(_triples);
        FFIHelper.CallVoid(() =>
            OxigraphNative.query_triples_serialize_to_file(filePath,
                IO.FormatToString(format), triplesJson));
    }
}

/// <summary>Options for SPARQL queries.</summary>
public sealed record QueryOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null,
    bool UseDefaultGraphAsUnion = false,
    IReadOnlyList<IGraphName>? DefaultGraphs = null,
    IReadOnlyList<IGraphName>? NamedGraphs = null,
    Dictionary<string, Func<ITerm[], ITerm?>>? CustomFunctions = null,
    Dictionary<string, ITerm>? Substitutions = null,
    Dictionary<string, Func<CustomFunctions.IAggregateAccumulator>>? CustomAggregateFunctions = null);

/// <summary>Options for SPARQL updates.</summary>
public sealed record UpdateOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null,
    Dictionary<string, Func<ITerm[], ITerm?>>? CustomFunctions = null,
    Dictionary<string, Func<CustomFunctions.IAggregateAccumulator>>? CustomAggregateFunctions = null);