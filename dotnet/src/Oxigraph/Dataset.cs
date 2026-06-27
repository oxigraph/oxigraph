using System.Collections;
using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>
/// An in-memory RDF dataset. Stores quads in memory using the native
/// oxigraph::model::Dataset (no RocksDB, no disk).
///
/// Supports quad CRUD, pattern matching, and canonicalization.
/// For heavy disk persistence and SPARQL, use <see cref="Store"/> instead.
///
/// Warning: This structure interns strings and does not garbage-collect
/// removed terms — memory grows monotonically if you insert and remove
/// many different terms.
/// </summary>
public sealed class Dataset : IEnumerable<Quad>, IDisposable
{
    private readonly DatasetSafeHandle _handle;

    /// <summary>Create an empty dataset.</summary>
    public Dataset()
    {
        var jsonPtr = OxigraphNative.dataset_new();
        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement.GetProperty("ok").GetProperty("handle").GetUInt64();
        _handle = new DatasetSafeHandle((IntPtr)handleVal);
    }

    /// <summary>Create a dataset initialized with the given quads.</summary>
    public Dataset(IEnumerable<Quad> quads)
    {
        var quadsJson = JsonSerializer.Serialize(quads.ToList());
        var jsonPtr = OxigraphNative.dataset_from_quads(quadsJson);
        var response = ReadAndFree(jsonPtr);
        FFIHelper.ThrowIfError(response);

        using var doc = JsonDocument.Parse(response);
        var handleVal = doc.RootElement.GetProperty("ok").GetProperty("handle").GetUInt64();
        _handle = new DatasetSafeHandle((IntPtr)handleVal);
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

    // ─── CRUD ─────────────────────────────

    /// <summary>Insert a quad into the dataset.</summary>
    public void Add(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        FFIHelper.CallVoid(() =>
            OxigraphNative.dataset_insert(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Remove a quad from the dataset. Throws if not found.</summary>
    public void Remove(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        var result = FFIHelper.Call<string>(() =>
            OxigraphNative.dataset_remove(_handle.DangerousGetHandle(), json));
        // The Rust function returns an error JSON if not found
    }

    /// <summary>Remove a quad if present, silently no-op if not.</summary>
    public void Discard(Quad quad)
    {
        // Only try to remove if it exists
        if (Contains(quad))
        {
            try { Remove(quad); } catch { /* ignore */ }
        }
    }

    /// <summary>Check if the dataset contains a quad.</summary>
    public bool Contains(Quad quad)
    {
        var json = JsonSerializer.Serialize(quad);
        return FFIHelper.CallValue<bool>(() =>
            OxigraphNative.dataset_contains(_handle.DangerousGetHandle(), json));
    }

    /// <summary>Number of quads in the dataset.</summary>
    public int Count
    {
        get
        {
            var result = FFIHelper.CallValue<ulong>(() =>
                OxigraphNative.dataset_count(_handle.DangerousGetHandle()));
            return (int)result;
        }
    }

    /// <summary>Whether the dataset is empty.</summary>
    public bool IsEmpty => Count == 0;

    // ─── Pattern matching ─────────────────

    /// <summary>Find quads with the given subject.</summary>
    public IReadOnlyList<Quad> QuadsForSubject(INamedOrBlankNode subject)
        => Match(subject: subject);

    /// <summary>Find quads with the given predicate.</summary>
    public IReadOnlyList<Quad> QuadsForPredicate(NamedNode predicate)
        => Match(predicate: predicate);

    /// <summary>Find quads with the given object.</summary>
    public IReadOnlyList<Quad> QuadsForObject(ITerm @object)
        => Match(@object: @object);

    /// <summary>Find quads with the given graph name.</summary>
    public IReadOnlyList<Quad> QuadsForGraphName(IGraphName graph)
        => Match(graph: graph);

    /// <summary>
    /// Find all quads matching the given pattern.
    /// This fetches all quads from the native Dataset and filters in C#.
    /// </summary>
    public IReadOnlyList<Quad> Match(
        INamedOrBlankNode? subject = null,
        NamedNode? predicate = null,
        ITerm? @object = null,
        IGraphName? graph = null)
    {
        // Fetch all quads and filter client-side
        var allQuads = FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.dataset_iter(_handle.DangerousGetHandle()));
        if (allQuads == null) return [];

        return allQuads.Where(q =>
            (subject == null || q.Subject.Equals(subject)) &&
            (predicate == null || q.Predicate.Equals(predicate)) &&
            (@object == null || q.Object.Equals(@object)) &&
            (graph == null || q.Graph.Equals(graph))
        ).ToList();
    }

    // ─── Manipulation ─────────────────────

    /// <summary>Remove all quads from the dataset.</summary>
    public void Clear()
    {
        FFIHelper.CallVoid(() =>
            OxigraphNative.dataset_clear(_handle.DangerousGetHandle()));
    }

    /// <summary>Add all quads from another collection.</summary>
    public void Extend(IEnumerable<Quad> quads)
    {
        foreach (var q in quads)
            Add(q);
    }

    // ─── Canonicalization ────────────────

    /// <summary>Canonicalize blank nodes in the dataset in-place.</summary>
    public void Canonicalize(CanonicalizationAlgorithm algorithm = CanonicalizationAlgorithm.Unstable)
    {
        var algoStr = algorithm switch
        {
            CanonicalizationAlgorithm.Unstable => "unstable",
            CanonicalizationAlgorithm.Rdfc10Sha256 => "rdfc10_sha256",
            CanonicalizationAlgorithm.Rdfc10Sha384 => "rdfc10_sha384",
            _ => "unstable",
        };
        FFIHelper.CallVoid(() =>
            OxigraphNative.dataset_canonicalize(_handle.DangerousGetHandle(), algoStr));
    }

    // ─── Serialization ────────────────────

    /// <summary>Dump the dataset as N-Quads text.</summary>
    public string Dump(RdfFormat format = RdfFormat.NQuads, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var quads = this.ToList();
        var opts = JsonSerializer.Serialize(new
        {
            quads,
            format = IO.FormatToString(format),
            base_iri = options.BaseIri,
            prefixes = options.Prefixes,
        });
        return FFIHelper.Call<string>(() =>
            OxigraphNative.serialize(opts));
    }

    /// <summary>Load quads from RDF text into the dataset.</summary>
    public void Load(string data, RdfFormat format, LoadOptions? options = null)
    {
        options ??= new LoadOptions();
        var opts = JsonSerializer.Serialize(new
        {
            data,
            format = IO.FormatToString(format),
            base_iri = options.BaseIri,
            lenient = options.Lenient,
            rename_blank_nodes = options.RenameBlankNodes,
        });
        var quads = FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.parse(opts));
        if (quads != null)
            Extend(quads);
    }

    // ─── IEnumerable ──────────────────────

    public IEnumerator<Quad> GetEnumerator()
    {
        var quads = FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.dataset_iter(_handle.DangerousGetHandle())) ?? [];
        foreach (var q in quads)
            yield return q;
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    public void Dispose() => _handle.Dispose();

    /// <summary>N-Quads serialization of the dataset.</summary>
    public override string ToString()
        => Dump(RdfFormat.NQuads);
}