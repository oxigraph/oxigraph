using System.Collections;
using System.Runtime.InteropServices;
using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>
/// An RDF store backed by RocksDB (on disk) or in-memory.
/// Thread safety: not guaranteed. Callers must synchronize concurrent access.
/// </summary>
public sealed class Store : IDisposable, IEnumerable<Quad>
{
    private readonly StoreSafeHandle _handle;

    private Store(IntPtr handle)
    {
        _handle = new StoreSafeHandle(handle);
    }

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

        return new Store((IntPtr)handleVal);
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

    /// <summary>Execute a SPARQL query. Supports custom functions via <see cref="CustomFunctions"/>.</summary>
    public QueryResults Query(string sparql, QueryOptions? options = null)
    {
        options ??= new QueryOptions();

        // Register custom functions before query
        if (options.CustomFunctions != null)
        {
            foreach (var (name, func) in options.CustomFunctions)
                CustomFunctions.Register(name, func);
        }
        if (options.CustomAggregateFunctions != null)
        {
            foreach (var (name, factory) in options.CustomAggregateFunctions)
                CustomFunctions.RegisterAggregate(name, factory);
        }

        try
        {
            var queryJson = JsonSerializer.Serialize(new
            {
                query = sparql,
                base_iri = options.BaseIri,
                prefixes = options.Prefixes,
                use_default_graph_as_union = options.UseDefaultGraphAsUnion,
                default_graph = options.DefaultGraphs,
                named_graphs = options.NamedGraphs,
                substitutions = options.Substitutions,
            },
            new JsonSerializerOptions { Converters = { new TermConverter() } });
            var element = FFIHelper.CallValue<JsonElement>(() =>
                OxigraphNative.store_query(_handle.DangerousGetHandle(), queryJson));
            return QueryResults.FromJson(element.GetRawText());
        }
        finally
        {
            // Clean up registered functions
            if (options.CustomFunctions != null)
            {
                foreach (var name in options.CustomFunctions.Keys)
                    CustomFunctions.Unregister(name);
            }
            if (options.CustomAggregateFunctions != null)
            {
                foreach (var name in options.CustomAggregateFunctions.Keys)
                    CustomFunctions.UnregisterAggregate(name);
            }
        }
    }

    /// <summary>Execute a SPARQL update. Supports custom functions via <see cref="CustomFunctions"/>.</summary>
    public void Update(string sparql, UpdateOptions? options = null)
    {
        options ??= new UpdateOptions();

        // Register custom functions before update
        if (options.CustomFunctions != null)
        {
            foreach (var (name, func) in options.CustomFunctions)
                CustomFunctions.Register(name, func);
        }
        if (options.CustomAggregateFunctions != null)
        {
            foreach (var (name, factory) in options.CustomAggregateFunctions)
                CustomFunctions.RegisterAggregate(name, factory);
        }

        try
        {
            var updateJson = JsonSerializer.Serialize(new
            {
                update = sparql,
                base_iri = options.BaseIri,
                prefixes = options.Prefixes,
            });
            FFIHelper.CallVoid(() =>
                OxigraphNative.store_update(_handle.DangerousGetHandle(), updateJson));
        }
        finally
        {
            if (options.CustomFunctions != null)
            {
                foreach (var name in options.CustomFunctions.Keys)
                    CustomFunctions.Unregister(name);
            }
            if (options.CustomAggregateFunctions != null)
            {
                foreach (var name in options.CustomAggregateFunctions.Keys)
                    CustomFunctions.UnregisterAggregate(name);
            }
        }
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

    /// <summary>
    /// Insert a large set of quads without keeping them all in memory.
    /// Uses RocksDB's bulk loading path — writes new SST files instead of
    /// doing an in-memory extend. Much more efficient for huge data loads.
    /// </summary>
    public void BulkExtend(IEnumerable<Quad> quads)
    {
        var json = JsonSerializer.Serialize(quads.ToList());
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_bulk_extend(_handle.DangerousGetHandle(), json));
    }

    /// <summary>N-Quads serialization of the store contents.</summary>
    public override string ToString()
        => Dump(RdfFormat.NQuads);

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
            lenient = options.Lenient,
            rename_blank_nodes = options.RenameBlankNodes,
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_load(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Load RDF from a file path into the store (streaming, no memory limit).</summary>
    public void LoadFromFile(string filePath, RdfFormat format, LoadOptions? options = null)
    {
        options ??= new LoadOptions();
        var toGraphJson = options.ToGraph is not null
            ? JsonSerializer.Serialize(options.ToGraph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } })
            : null;
        var optsJson = JsonSerializer.Serialize(new
        {
            lenient = options.Lenient,
            rename_blank_nodes = options.RenameBlankNodes,
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_load_from_file(
                _handle.DangerousGetHandle(), filePath,
                IO.FormatToString(format), options.BaseIri, toGraphJson, optsJson));
    }

    /// <summary>
    /// Bulk-load RDF from a file path using parallel parsing (optimized for very large files).
    /// This uses RocksDB's bulk loading path for maximum performance.
    /// </summary>
    public void BulkLoadFromFile(string filePath, RdfFormat format, LoadOptions? options = null)
    {
        options ??= new LoadOptions();
        var toGraphJson = options.ToGraph is not null
            ? JsonSerializer.Serialize(options.ToGraph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } })
            : null;
        var optsJson = JsonSerializer.Serialize(new
        {
            lenient = options.Lenient,
            rename_blank_nodes = options.RenameBlankNodes,
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_bulk_load_from_file(
                _handle.DangerousGetHandle(), filePath,
                IO.FormatToString(format), options.BaseIri, toGraphJson, optsJson));
    }

    /// <summary>Bulk-load RDF data optimized for large files.</summary>
    public void BulkLoad(string data, RdfFormat format, LoadOptions? options = null)
    {
        // For in-memory data, delegate to Load (same transaction behavior).
        Load(data, format, options);
    }

    /// <summary>Load RDF from a .NET Stream into the store.</summary>
    public void LoadFromStream(Stream stream, RdfFormat format, LoadOptions? options = null)
    {
        options ??= new LoadOptions();
        using var ctx = new ReadContext(stream);
        var toGraphJson = options.ToGraph is not null
            ? JsonSerializer.Serialize(options.ToGraph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } })
            : null;

        var resultPtr = OxigraphNative.store_load_from_callback(
            _handle.DangerousGetHandle(),
            Marshal.GetFunctionPointerForDelegate(ctx.Callback),
            ctx.ContextPtr,
            IO.FormatToString(format),
            options.BaseIri,
            toGraphJson);

        var resultJson = Marshal.PtrToStringUTF8(resultPtr) ?? "{}";
        OxigraphNative.free_string(resultPtr);
        FFIHelper.ThrowIfError(resultJson);
    }

    /// <summary>Dump store contents to a .NET Stream.</summary>
    public void DumpToStream(Stream stream, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        using var ctx = new WriteContext(stream);
        var fromGraphJson = options.FromGraph is not null
            ? JsonSerializer.Serialize(options.FromGraph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } })
            : null;
        var prefixesJson = options.Prefixes != null
            ? JsonSerializer.Serialize(options.Prefixes)
            : null;

        var resultPtr = OxigraphNative.store_dump_to_callback(
            _handle.DangerousGetHandle(),
            Marshal.GetFunctionPointerForDelegate(ctx.Callback),
            ctx.ContextPtr,
            IO.FormatToString(format),
            options.BaseIri,
            fromGraphJson,
            prefixesJson);

        var resultJson = Marshal.PtrToStringUTF8(resultPtr) ?? "{}";
        OxigraphNative.free_string(resultPtr);
        FFIHelper.ThrowIfError(resultJson);
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
            prefixes = options.Prefixes,
        }, converterOptions);
        return FFIHelper.Call<string>(() =>
            OxigraphNative.store_dump(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Dump store contents to a file (streaming, no memory limit on output).</summary>
    public void DumpToFile(string filePath, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var fromGraphJson = options.FromGraph is not null
            ? JsonSerializer.Serialize(options.FromGraph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } })
            : null;
        var prefixesJson = options.Prefixes != null
            ? JsonSerializer.Serialize(options.Prefixes)
            : null;
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_dump_to_file(
                _handle.DangerousGetHandle(), filePath,
                IO.FormatToString(format), options.BaseIri, fromGraphJson, prefixesJson));
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

    /// <summary>Iterate over all quads in the store via pattern matching.</summary>
    public IEnumerator<Quad> GetEnumerator()
    {
        foreach (var q in Match())
            yield return q;
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

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