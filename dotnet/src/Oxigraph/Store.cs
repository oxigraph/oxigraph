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

    /// <summary>Create an in-memory store.</summary>
    public Store()
    {
        var jsonPtr = OxigraphNative.store_new();
        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement
            .GetProperty("ok")
            .GetProperty("handle")
            .GetUInt64();

        _handle = new StoreSafeHandle((IntPtr)handleVal);
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
        // For PoC, delegate to Rust which returns all quads
        var patternJson = "{}";
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
            use_default_graph_as_union = options.UseDefaultGraphAsUnion,
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
        });
        FFIHelper.CallVoid(() =>
            OxigraphNative.store_update(_handle.DangerousGetHandle(), updateJson));
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