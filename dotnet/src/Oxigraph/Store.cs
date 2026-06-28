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
        // Build pattern JSON manually with per-value converters, because
        // Dictionary<string,object?> doesn't invoke custom converters (the
        // compile-time value type is 'object', not the specific interface).
        var opts = new JsonSerializerOptions
        {
            Converters = { new NamedOrBlankNodeConverter(), new NamedNodeConverter(), new TermConverter(), new GraphNameConverter() }
        };
        var sb = new System.Text.StringBuilder();
        sb.Append('{');
        var first = true;
        void AddJson(string key, string jsonValue)
        {
            if (!first) sb.Append(',');
            first = false;
            sb.Append('"'); sb.Append(key); sb.Append("\":");
            sb.Append(jsonValue);
        }
        if (subject != null)
            AddJson("subject", JsonSerializer.Serialize(subject, opts));
        if (predicate != null)
            AddJson("predicate", JsonSerializer.Serialize(predicate, opts));
        if (@object != null)
            AddJson("object", JsonSerializer.Serialize(@object, opts));
        if (graph != null)
            AddJson("graph", JsonSerializer.Serialize(graph, opts));
        sb.Append('}');
        var patternJson = sb.ToString();

        var quads = FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.store_match(_handle.DangerousGetHandle(), patternJson));
        return quads ?? [];
    }

    /// <summary>
    /// Execute a SPARQL query. Results are lazily streamed from the store — safe for large result sets.
    /// Supports custom functions via <see cref="CustomFunctions"/>.
    /// Custom functions are automatically cleaned up when the returned <see cref="QueryResults"/> is disposed.
    /// </summary>
    public QueryResults Query(string sparql, QueryOptions? options = null)
    {
        options ??= new QueryOptions();

        // Register custom functions before query (they stay alive until results are disposed)
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

        // Build cleanup action for deferred custom function cleanup
        var hasCustomFns = options.CustomFunctions != null;
        var hasCustomAgg = options.CustomAggregateFunctions != null;
        var customFnNames = options.CustomFunctions?.Keys.ToList();
        var customAggNames = options.CustomAggregateFunctions?.Keys.ToList();

        Action cleanup = () =>
        {
            if (hasCustomFns && customFnNames != null)
            {
                foreach (var name in customFnNames)
                    CustomFunctions.Unregister(name);
            }
            if (hasCustomAgg && customAggNames != null)
            {
                foreach (var name in customAggNames)
                    CustomFunctions.UnregisterAggregate(name);
            }
        };

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
            new JsonSerializerOptions
            {
                Converters = {
                    new TermConverter(),
                    new NamedOrBlankNodeConverter(),
                    new NamedNodeConverter(),
                    new GraphNameConverter()
                }
            });

            // Use the lazy streaming query path (gap 3 fix)
            var jsonPtr = OxigraphNative.store_query_iter(
                _handle.DangerousGetHandle(), queryJson);
            var response = ReadAndFree(jsonPtr);
            FFIHelper.ThrowIfError(response);

            using var doc = JsonDocument.Parse(response);
            var handleVal = doc.RootElement
                .GetProperty("ok")
                .GetProperty("handle")
                .GetUInt64();

            var handle = new QueryResultsSafeHandle((IntPtr)handleVal);
            return QueryResults.FromHandle(this, handle, cleanup);
        }
        catch
        {
            // Clean up immediately on setup error
            cleanup();
            throw;
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
    /// Uses RocksDB's bulk loading in chunks — streams quads through FFI
    /// in batches of 10,000 to avoid materializing the entire set.
    /// Much more efficient than <see cref="Extend"/> for huge data loads.
    /// </summary>
    public void BulkExtend(IEnumerable<Quad> quads)
    {
        const int chunkSize = 10_000;
        var chunk = new List<Quad>(chunkSize);

        // Begin bulk loader
        var jsonPtr = OxigraphNative.store_bulk_extend_begin(_handle.DangerousGetHandle());
        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement
            .GetProperty("ok")
            .GetProperty("handle")
            .GetUInt64();

        using var bulkHandle = new BulkLoaderSafeHandle((IntPtr)handleVal);

        try
        {
            foreach (var quad in quads)
            {
                chunk.Add(quad);
                if (chunk.Count >= chunkSize)
                {
                    var batchJson = JsonSerializer.Serialize(chunk);
                    FFIHelper.CallVoid(() =>
                        OxigraphNative.store_bulk_extend_add_chunk(
                            bulkHandle.DangerousGetHandle(), batchJson));
                    chunk.Clear();
                }
            }

            // Flush remaining
            if (chunk.Count > 0)
            {
                var batchJson = JsonSerializer.Serialize(chunk);
                FFIHelper.CallVoid(() =>
                    OxigraphNative.store_bulk_extend_add_chunk(
                        bulkHandle.DangerousGetHandle(), batchJson));
            }

            // Commit
            var commitPtr = OxigraphNative.store_bulk_extend_commit(
                bulkHandle.DangerousGetHandle());
            var commitJson = ReadAndFree(commitPtr);
            FFIHelper.ThrowIfError(commitJson);

            // Prevent the SafeHandle from calling cancel — already committed
            bulkHandle.SetHandleAsInvalid();
        }
        catch
        {
            // bulkHandle's ReleaseHandle will call cancel automatically
            throw;
        }
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
        using var stream = new System.IO.MemoryStream();
        using (var writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            writer.WriteString("data", data);
            writer.WriteString("format", IO.FormatToString(format));
            if (options.BaseIri != null)
                writer.WriteString("base_iri", options.BaseIri);
            if (options.ToGraph is not null)
            {
                writer.WritePropertyName("to_graph");
                var opts = new JsonSerializerOptions { Converters = { new GraphNameConverter() } };
                JsonSerializer.Serialize(writer, options.ToGraph, opts);
            }
            else
            {
                writer.WriteNull("to_graph");
            }
            writer.WriteBoolean("lenient", options.Lenient);
            writer.WriteBoolean("rename_blank_nodes", options.RenameBlankNodes);
            writer.WriteEndObject();
        }
        var json = System.Text.Encoding.UTF8.GetString(stream.ToArray());
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

    // ═══════════════════════════════════════════════════
    // Async API
    // ═══════════════════════════════════════════════════

    /// <inheritdoc cref="Query" />
    public Task<QueryResults> QueryAsync(string sparql, QueryOptions? options = null,
        CancellationToken ct = default)
        => Task.Run(() => Query(sparql, options), ct);

    /// <inheritdoc cref="Update" />
    public Task UpdateAsync(string sparql, UpdateOptions? options = null,
        CancellationToken ct = default)
        => Task.Run(() => Update(sparql, options), ct);

    /// <inheritdoc cref="LoadFromFile" />
    public Task LoadFromFileAsync(string filePath, RdfFormat format,
        LoadOptions? options = null, CancellationToken ct = default)
        => Task.Run(() => LoadFromFile(filePath, format, options), ct);

    /// <inheritdoc cref="BulkLoadFromFile" />
    public Task BulkLoadFromFileAsync(string filePath, RdfFormat format,
        LoadOptions? options = null, CancellationToken ct = default)
        => Task.Run(() => BulkLoadFromFile(filePath, format, options), ct);

    /// <inheritdoc cref="DumpToFile" />
    public Task DumpToFileAsync(string filePath, RdfFormat format,
        DumpOptions? options = null, CancellationToken ct = default)
        => Task.Run(() => DumpToFile(filePath, format, options), ct);

    /// <inheritdoc cref="LoadFromStream" />
    public async Task LoadFromStreamAsync(Stream stream, RdfFormat format,
        LoadOptions? options = null, CancellationToken ct = default)
    {
        using var ms = new MemoryStream();
        await stream.CopyToAsync(ms, ct).ConfigureAwait(false);
        ms.Position = 0;
        await Task.Run(() => LoadFromStream(ms, format, options), ct)
            .ConfigureAwait(false);
    }

    /// <inheritdoc cref="DumpToStream" />
    public async Task DumpToStreamAsync(Stream stream, RdfFormat format,
        DumpOptions? options = null, CancellationToken ct = default)
    {
        using var ms = new MemoryStream();
        await Task.Run(() => DumpToStream(ms, format, options), ct)
            .ConfigureAwait(false);
        ms.Position = 0;
        await ms.CopyToAsync(stream, ct).ConfigureAwait(false);
    }

    /// <inheritdoc cref="Backup" />
    public Task BackupAsync(string targetDirectory, CancellationToken ct = default)
        => Task.Run(() => Backup(targetDirectory), ct);

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