using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>
/// An RDF store backed by RocksDB (on disk) or in-memory.
/// Thread safety: not guaranteed. Callers must synchronize concurrent access.
/// </summary>
public sealed class Store : IDisposable
{
    private readonly StoreSafeHandle _handle;

    /// <summary>Create an in-memory store, or open/create a file-backed store at the given path.</summary>
    public Store(string? path = null)
    {
        IntPtr jsonPtr;
        if (path != null)
            jsonPtr = OxigraphNative.store_open(path);
        else
            jsonPtr = OxigraphNative.store_new();

        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement
            .GetProperty("ok")
            .GetProperty("handle")
            .GetUInt64();

        _handle = new StoreSafeHandle((IntPtr)handleVal);
    }

    /// <summary>Open an existing store in read-only mode.</summary>
    public static Store OpenReadOnly(string path)
    {
        var jsonPtr = OxigraphNative.store_open_read_only(path);
        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement.GetProperty("ok").GetProperty("handle").GetUInt64();

        var store = new Store(); // dummy, we'll replace _handle
        store._handle.Dispose();
        typeof(Store).GetField("_handle", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)!
            .SetValue(store, new StoreSafeHandle((IntPtr)handleVal));
        return store;
    }

    /// <summary>Insert a quad into the store.</summary>
    public void Add(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_add(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Remove a quad from the store.</summary>
    public void Remove(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_remove(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Check if a quad exists in the store.</summary>
    public bool Contains(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        var result = FFIHelper.CallValue<bool>(() =>
            OxigraphNative.store_contains(_handle.DangerousGetHandle(), json));
        return result;
    }

    /// <summary>Number of quads in the store.</summary>
    public ulong Count
    {
        get
        {
            var result = FFIHelper.CallValue<ulong>(() =>
                OxigraphNative.store_count(_handle.DangerousGetHandle()));
            return result;
        }
    }

    /// <summary>Whether the store is empty.</summary>
    public bool IsEmpty => Count == 0;

    /// <summary>
    /// Find all quads in the store matching the given pattern.
    /// Null/optional parameters match anything (wildcard).
    /// </summary>
    public IReadOnlyList<Quad> Match(
        INamedOrBlankNode? subject = null,
        NamedNode? predicate = null,
        ITerm? @object = null,
        IGraphName? graph = null)
    {
        var pattern = new Dictionary<string, object?>();
        if (subject != null) pattern["subject"] = subject;
        if (predicate != null) pattern["predicate"] = predicate;
        if (@object != null) pattern["object"] = @object;
        if (graph != null) pattern["graph"] = graph;
        var patternJson = JsonSerializer.Serialize(pattern, new JsonSerializerOptions
        {
            Converters = { new NamedOrBlankNodeConverter(), new NamedNodeConverter(), new TermConverter(), new GraphNameConverter() }
        });
        var quads = FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.store_match(_handle.DangerousGetHandle(), patternJson));
        return quads ?? [];
    }

    /// <summary>Execute a SPARQL query.</summary>
    public QueryResults Query(string sparql, QueryOptions? options = null)
    {
        options ??= new QueryOptions();
        var queryJson = JsonSerializer.Serialize(new
        {
            query = sparql,
            base_iri = options.BaseIri,
            prefixes = options.Prefixes,
            use_default_graph_as_union = options.UseDefaultGraphAsUnion,
            default_graph = options.DefaultGraphs,
            named_graphs = options.NamedGraphs,
        });
        var element = FFIHelper.CallValue<JsonElement>(() =>
            OxigraphNative.store_query(_handle.DangerousGetHandle(), queryJson));
        return QueryResults.FromJson(element.GetRawText());
    }

    /// <summary>Execute a SPARQL update.</summary>
    public void Update(string sparql, UpdateOptions? options = null)
    {
        options ??= new UpdateOptions();
        var updateJson = JsonSerializer.Serialize(new
        {
            update = sparql,
            base_iri = options.BaseIri,
            prefixes = options.Prefixes,
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_update(_handle.DangerousGetHandle(), updateJson));
    }

    /// <summary>Clear all quads from the store.</summary>
    public void Clear()
    {
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_clear(_handle.DangerousGetHandle()));
    }

    /// <summary>Insert multiple quads atomically.</summary>
    public void Extend(IEnumerable<Quad> quads)
    {
        var json = JsonSerializer.Serialize(quads.ToList());
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_extend(_handle.DangerousGetHandle(), json));
    }

    // ─── Named graph operations ───

    /// <summary>List all named graphs in the store.</summary>
    public IReadOnlyList<INamedOrBlankNode> NamedGraphs
    {
        get
        {
            var graphs = FFIHelper.Call<List<JsonElement>>(() =>
                OxigraphNative.store_named_graphs(_handle.DangerousGetHandle()));
            return graphs?.Select(e =>
            {
                var json = e.GetRawText();
                return JsonSerializer.Deserialize<INamedOrBlankNode>(json, new JsonSerializerOptions
                    { Converters = { new NamedOrBlankNodeConverter() } })!;
            }).ToList() ?? [];
        }
    }

    /// <summary>Check if a named graph exists.</summary>
    public bool ContainsNamedGraph(IGraphName graph)
    {
        var json = JsonSerializer.Serialize(graph, new JsonSerializerOptions
        {
            Converters = { new GraphNameConverter() }
        });
        return FFIHelper.CallValue<bool>(() =>
            OxigraphNative.store_contains_named_graph(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Create an empty named graph.</summary>
    public void AddGraph(INamedOrBlankNode graphName)
    {
        var json = JsonSerializer.Serialize(graphName, new JsonSerializerOptions
        {
            Converters = { new NamedOrBlankNodeConverter() }
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_insert_named_graph(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Clear all quads from a specific graph.</summary>
    public void ClearGraph(IGraphName graph)
    {
        var json = JsonSerializer.Serialize(graph, new JsonSerializerOptions
        {
            Converters = { new GraphNameConverter() }
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_clear_graph(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Remove a named graph entirely.</summary>
    public void RemoveGraph(INamedOrBlankNode graphName)
    {
        var json = JsonSerializer.Serialize(graphName, new JsonSerializerOptions
        {
            Converters = { new NamedOrBlankNodeConverter() }
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_remove_named_graph(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Load RDF text into the store.</summary>
    public void Load(string data, RdfFormat format, LoadOptions? options = null)
    {
        options ??= new LoadOptions();
        var json = JsonSerializer.Serialize(new
        {
            data,
            format = IO.FormatToString(format),
            base_iri = options.BaseIri,
            to_graph = options.ToGraph,
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_load(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Bulk-load RDF data optimized for large files.</summary>
    public void BulkLoad(string data, RdfFormat format, LoadOptions? options = null)
    {
        // In the current memory-only implementation, BulkLoad delegates to Load.
        // With RocksDB, this would use the bulk loader for better performance.
        Load(data, format, options);
    }

    /// <summary>Dump store contents as RDF text.</summary>
    public string Dump(RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var converterOptions = new JsonSerializerOptions
        {
            Converters = { new GraphNameConverter() }
        };
        var json = JsonSerializer.Serialize(new
        {
            format = IO.FormatToString(format),
            base_iri = options.BaseIri,
            from_graph = options.FromGraph,
        }, converterOptions);
        return FFIHelper.Call<string>(() =>
            OxigraphNative.store_dump(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Flush pending writes to disk.</summary>
    public void Flush()
    {
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_flush(_handle.DangerousGetHandle()));
    }

    /// <summary>Optimize database storage.</summary>
    public void Optimize()
    {
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_optimize(_handle.DangerousGetHandle()));
    }

    /// <summary>Create a backup at the target directory.</summary>
    public void Backup(string targetDirectory)
    {
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_backup(_handle.DangerousGetHandle(), targetDirectory));
    }

    public void Dispose()
    {
        _handle.Dispose();
    }

    private static string ReadAndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
            throw new OxigraphException("FFI returned null pointer");
        var json = System.Runtime.InteropServices.Marshal.PtrToStringUTF8(ptr)
            ?? throw new OxigraphException("FFI returned invalid UTF-8");
        OxigraphNative.free_string(ptr);
        return json;
    }
}