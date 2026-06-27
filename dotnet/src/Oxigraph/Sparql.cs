using System.Collections;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>Base class for SPARQL query results. Lazily streams results from the Rust store via FFI iterator.</summary>
public abstract class QueryResults : IDisposable
{
    private protected QueryResultsSafeHandle? _handle;
    private Action? _cleanup;

    private protected QueryResults() { }

    /// <summary>Set a cleanup action to be called when the results are disposed.</summary>
    internal void SetCleanup(Action cleanup) => _cleanup = cleanup;

    /// <summary>Parse a JSON response from Rust (materialized path, kept for backward compat).</summary>
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

    /// <summary>Create a lazy-streaming QueryResults from an FFI iterator handle.</summary>
    internal static QueryResults FromHandle(Store store, QueryResultsSafeHandle handle, Action cleanup)
    {
        // Determine the result type from the iterator
        var typePtr = OxigraphNative.query_iter_get_type(handle.DangerousGetHandle());
        var typeJson = Marshal.PtrToStringUTF8(typePtr) ?? "{}";
        OxigraphNative.free_string(typePtr);
        FFIHelper.ThrowIfError(typeJson);

        using var typeDoc = JsonDocument.Parse(typeJson);
        var type = typeDoc.RootElement.GetProperty("ok").GetString()!;

        var result = type switch
        {
            "boolean" => (QueryResults)LazyQueryBoolean.FromHandle(handle),
            "solutions" => LazyQuerySolutions.FromHandle(handle),
            "triples" => LazyQueryTriples.FromHandle(handle),
            _ => throw new OxigraphException($"Unknown query result type: {type}"),
        };
        result.SetCleanup(cleanup);
        return result;
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

    /// <summary>Release the FFI iterator handle and invoke cleanup.</summary>
    public void Dispose()
    {
        _handle?.Dispose();
        _handle = null;
        _cleanup?.Invoke();
        _cleanup = null;
    }
}

// ─── Lazy (streaming) implementations ─────────────────

/// <summary>Lazily streamed ASK result.</summary>
internal sealed class LazyQueryBoolean : QueryBoolean
{
    private LazyQueryBoolean(bool value) : base(value) { }

    internal static LazyQueryBoolean FromHandle(QueryResultsSafeHandle handle)
    {
        var valPtr = OxigraphNative.query_iter_boolean_value(handle.DangerousGetHandle());
        var valJson = Marshal.PtrToStringUTF8(valPtr) ?? "{}";
        OxigraphNative.free_string(valPtr);
        FFIHelper.ThrowIfError(valJson);

        using var doc = JsonDocument.Parse(valJson);
        var value = doc.RootElement.GetProperty("ok").GetBoolean();
        var result = new LazyQueryBoolean(value) { _handle = handle };
        return result;
    }
}

/// <summary>Lazily streamed SELECT result. Each call to MoveNext pulls one row from Rust.</summary>
internal sealed class LazyQuerySolutions : QuerySolutions
{
    private readonly QueryResultsSafeHandle _iterHandle;
    private bool _exhausted;

    private LazyQuerySolutions(IReadOnlyList<Variable> variables, QueryResultsSafeHandle handle)
        : base(variables, new LazySolutionList(handle))
    {
        _iterHandle = handle;
        _handle = handle;
    }

    internal static LazyQuerySolutions FromHandle(QueryResultsSafeHandle handle)
    {
        // Get variables from the iterator
        var varsPtr = OxigraphNative.query_iter_variables(handle.DangerousGetHandle());
        var varsJson = Marshal.PtrToStringUTF8(varsPtr) ?? "[]";
        OxigraphNative.free_string(varsPtr);
        FFIHelper.ThrowIfError(varsJson);

        using var varsDoc = JsonDocument.Parse(varsJson);
        var varNames = JsonSerializer.Deserialize<List<string>>(
            varsDoc.RootElement.GetProperty("ok").GetRawText()) ?? [];
        var variables = varNames.Select(v => new Variable(v)).ToList();

        return new LazyQuerySolutions(variables, handle);
    }

    internal QuerySolution? FetchNext()
    {
        if (_exhausted) return null;

        var ptr = OxigraphNative.query_iter_next_solution(_iterHandle.DangerousGetHandle());
        var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
        OxigraphNative.free_string(ptr);

        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.TryGetProperty("error", out _))
        {
            FFIHelper.ThrowIfError(json);
            return null;
        }

        var okVal = doc.RootElement.GetProperty("ok");
        if (okVal.ValueKind == JsonValueKind.Null)
        {
            _exhausted = true;
            return null;
        }

        return new QuerySolution(okVal);
    }

    /// <summary>Internal lazy list that fetches from FFI on enumeration.</summary>
    private sealed class LazySolutionList : IReadOnlyList<QuerySolution>
    {
        private readonly QueryResultsSafeHandle _handle;
        private readonly List<QuerySolution> _materialized = [];
        private int? _count;

        public LazySolutionList(QueryResultsSafeHandle handle) => _handle = handle;

        public QuerySolution this[int index]
        {
            get
            {
                MaterializeUpTo(index + 1);
                if (index >= _materialized.Count)
                    throw new ArgumentOutOfRangeException(nameof(index));
                return _materialized[index];
            }
        }

        /// <summary>Returns the total count by fully materializing the result set. Cached after first access.</summary>
        public int Count
        {
            get
            {
                if (_count == null)
                {
                    MaterializeAll();
                    _count = _materialized.Count;
                }
                return _count.Value;
            }
        }

        private void MaterializeUpTo(int required)
        {
            while (_materialized.Count < required && (_count == null || _materialized.Count < _count.Value))
            {
                var ptr = OxigraphNative.query_iter_next_solution(_handle.DangerousGetHandle());
                var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
                OxigraphNative.free_string(ptr);

                using var doc = JsonDocument.Parse(json);
                if (doc.RootElement.TryGetProperty("error", out _))
                    throw new OxigraphException("Error fetching query solution");

                var okVal = doc.RootElement.GetProperty("ok");
                if (okVal.ValueKind == JsonValueKind.Null)
                {
                    _count = _materialized.Count;
                    return;
                }

                _materialized.Add(new QuerySolution(okVal));
            }
        }

        private void MaterializeAll()
        {
            while (_count == null)
            {
                var ptr = OxigraphNative.query_iter_next_solution(_handle.DangerousGetHandle());
                var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
                OxigraphNative.free_string(ptr);

                using var doc = JsonDocument.Parse(json);
                if (doc.RootElement.TryGetProperty("error", out _))
                    throw new OxigraphException("Error fetching query solution");

                var okVal = doc.RootElement.GetProperty("ok");
                if (okVal.ValueKind == JsonValueKind.Null)
                {
                    _count = _materialized.Count;
                    return;
                }

                _materialized.Add(new QuerySolution(okVal));
            }
        }

        public IEnumerator<QuerySolution> GetEnumerator()
        {
            int i = 0;
            while (true)
            {
                QuerySolution? sol;
                try { sol = this[i]; }
                catch (ArgumentOutOfRangeException) { yield break; }
                yield return sol;
                i++;
            }
        }

        IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
    }
}

/// <summary>Lazily streamed CONSTRUCT/DESCRIBE result. Each call to MoveNext pulls one triple from Rust.</summary>
internal sealed class LazyQueryTriples : QueryTriples
{
    private readonly QueryResultsSafeHandle _iterHandle;
    private bool _exhausted;

    private LazyQueryTriples(QueryResultsSafeHandle handle)
        : base(new LazyTripleList(handle))
    {
        _iterHandle = handle;
        _handle = handle;
    }

    internal static LazyQueryTriples FromHandle(QueryResultsSafeHandle handle)
        => new(handle);

    internal Triple? FetchNext()
    {
        if (_exhausted) return null;

        var ptr = OxigraphNative.query_iter_next_triple(_iterHandle.DangerousGetHandle());
        var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
        OxigraphNative.free_string(ptr);

        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.TryGetProperty("error", out _))
        {
            FFIHelper.ThrowIfError(json);
            return null;
        }

        var okVal = doc.RootElement.GetProperty("ok");
        if (okVal.ValueKind == JsonValueKind.Null)
        {
            _exhausted = true;
            return null;
        }

        return JsonSerializer.Deserialize<Triple>(okVal.GetRawText())!;
    }

    private sealed class LazyTripleList : IReadOnlyList<Triple>
    {
        private readonly QueryResultsSafeHandle _handle;
        private readonly List<Triple> _materialized = [];

        public LazyTripleList(QueryResultsSafeHandle handle) => _handle = handle;

        public Triple this[int index]
        {
            get
            {
                while (_materialized.Count <= index)
                {
                    var ptr = OxigraphNative.query_iter_next_triple(_handle.DangerousGetHandle());
                    var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
                    OxigraphNative.free_string(ptr);

                    using var doc = JsonDocument.Parse(json);
                    if (doc.RootElement.TryGetProperty("error", out _))
                        throw new OxigraphException("Error fetching triple");

                    var okVal = doc.RootElement.GetProperty("ok");
                    if (okVal.ValueKind == JsonValueKind.Null)
                        throw new ArgumentOutOfRangeException(nameof(index));

                    _materialized.Add(JsonSerializer.Deserialize<Triple>(okVal.GetRawText())!);
                }
                return _materialized[index];
            }
        }

        public int Count => throw new NotSupportedException("Use foreach to enumerate streaming results");

        public IEnumerator<Triple> GetEnumerator()
        {
            int i = 0;
            while (true)
            {
                Triple? t;
                try { t = this[i]; }
                catch (ArgumentOutOfRangeException) { yield break; }
                yield return t;
                i++;
            }
        }

        IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
    }
}

// ─── Public concrete result classes ─────────────────

/// <summary>Result of an ASK query.</summary>
public class QueryBoolean : QueryResults
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
public class QuerySolutions : QueryResults, IEnumerable<QuerySolution>
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
    public QuerySolution this[int index] => _rows[index];

    /// <summary>The number of solution rows. For streaming results, this enumerates fully on first access.</summary>
    public int Count => _rows.Count;

    /// <summary>Serialize SELECT results to a string (XML/JSON/CSV/TSV).</summary>
    public string Serialize(QueryResultsFormat format)
    {
        var materialized = this.ToList();
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        var variablesJson = JsonSerializer.Serialize(Variables.Select(v => v.Value));
        var rowsJson = JsonSerializer.Serialize(materialized.Select(r =>
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
        var materialized = this.ToList();
        var fmtStr = format switch
        {
            QueryResultsFormat.Json => "json",
            QueryResultsFormat.Xml => "xml",
            QueryResultsFormat.Csv => "csv",
            QueryResultsFormat.Tsv => "tsv",
            _ => "json",
        };
        var variablesJson = JsonSerializer.Serialize(Variables.Select(v => v.Value));
        var rowsJson = JsonSerializer.Serialize(materialized.Select(r =>
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
    private readonly List<string> _orderedKeys;

    internal QuerySolution(JsonElement element)
    {
        _orderedKeys = [];
        foreach (var prop in element.EnumerateObject())
        {
            ITerm? term = null;
            if (prop.Value.ValueKind != JsonValueKind.Null)
                term = JsonSerializer.Deserialize<ITerm>(prop.Value.GetRawText(), new JsonSerializerOptions
                {
                    Converters = { new TermConverter() }
                });
            _bindings[prop.Name] = term;
            _orderedKeys.Add(prop.Name);
        }
    }

    /// <summary>Access a binding by variable name.</summary>
    public ITerm? this[string variable]
    {
        get
        {
            _bindings.TryGetValue(variable, out var term);
            return term;
        }
    }

    /// <summary>Access a binding by variable.</summary>
    public ITerm? this[Variable variable] => this[variable.Value];

    /// <summary>Access a binding by positional index (matches SELECT order).</summary>
    public ITerm? this[int index]
    {
        get
        {
            if (index < 0 || index >= _orderedKeys.Count)
                throw new ArgumentOutOfRangeException(nameof(index), $"Index {index} is out of range (0..{_orderedKeys.Count - 1})");
            return this[_orderedKeys[index]];
        }
    }

    /// <summary>Try to get a binding value by variable name.</summary>
    public bool TryGetValue(string variable, out ITerm? value)
        => _bindings.TryGetValue(variable, out value);

    /// <summary>Returns the ordered list of variable names in this solution.</summary>
    public IEnumerable<string> Variables => _orderedKeys;

    /// <summary>The number of bindings in this solution.</summary>
    public int Count => _bindings.Count;

    /// <summary>Deconstruct into a tuple for pattern matching.</summary>
    public void Deconstruct(out ITerm? first, out ITerm? second)
    {
        first = _orderedKeys.Count > 0 ? this[0] : null;
        second = _orderedKeys.Count > 1 ? this[1] : null;
    }

    /// <summary>Deconstruct into a tuple of three.</summary>
    public void Deconstruct(out ITerm? first, out ITerm? second, out ITerm? third)
    {
        first = _orderedKeys.Count > 0 ? this[0] : null;
        second = _orderedKeys.Count > 1 ? this[1] : null;
        third = _orderedKeys.Count > 2 ? this[2] : null;
    }
}

/// <summary>Result of a CONSTRUCT or DESCRIBE query.</summary>
public class QueryTriples : QueryResults, IEnumerable<Triple>
{
    private readonly IReadOnlyList<Triple> _triples;
    internal QueryTriples(IReadOnlyList<Triple> triples) => _triples = triples;
    public IEnumerator<Triple> GetEnumerator() => _triples.GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    /// <summary>Serialize CONSTRUCT/DESCRIBE results to a string (any RDF format).</summary>
    public string Serialize(RdfFormat format)
    {
        var materialized = this.ToList();
        var triplesJson = JsonSerializer.Serialize(materialized);
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
        var materialized = this.ToList();
        var triplesJson = JsonSerializer.Serialize(materialized);
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