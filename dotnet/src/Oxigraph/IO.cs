using System.Collections;
using System.Runtime.InteropServices;
using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>Standalone RDF parse and serialize functions.</summary>
public static class IO
{
    /// <summary>Parse RDF text into a list of quads.</summary>
    public static IReadOnlyList<Quad> Parse(string data, RdfFormat format, string? baseIri = null, ParseOptions? parseOptions = null)
    {
        parseOptions ??= new ParseOptions();
        var opts = JsonSerializer.Serialize(new
        {
            data,
            format = FormatToString(format),
            base_iri = baseIri ?? parseOptions.BaseIri,
            without_named_graphs = parseOptions.WithoutNamedGraphs,
            rename_blank_nodes = parseOptions.RenameBlankNodes,
            lenient = parseOptions.Lenient,
        });
        return FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.parse(opts)) ?? [];
    }

    /// <summary>Parse RDF from a file path into quads (streaming, no memory limit on input).</summary>
    public static IReadOnlyList<Quad> ParseFromFile(string filePath, RdfFormat format, string? baseIri = null, ParseOptions? parseOptions = null)
    {
        parseOptions ??= new ParseOptions();
        var optionsJson = JsonSerializer.Serialize(new
        {
            without_named_graphs = parseOptions.WithoutNamedGraphs,
            rename_blank_nodes = parseOptions.RenameBlankNodes,
            lenient = parseOptions.Lenient,
        });
        return FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.parse_from_file(filePath, FormatToString(format), baseIri ?? parseOptions.BaseIri, optionsJson)) ?? [];
    }

    /// <summary>Serialize a list of quads to RDF text.</summary>
    public static string Serialize(IEnumerable<Quad> quads, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var opts = JsonSerializer.Serialize(new
        {
            quads = quads.ToList(),
            format = FormatToString(format),
            base_iri = options.BaseIri,
            prefixes = options.Prefixes,
        });
        return FFIHelper.Call<string>(() =>
            OxigraphNative.serialize(opts));
    }

    /// <summary>Serialize quads to a file path (streaming, no memory limit on output).</summary>
    public static void SerializeToFile(string filePath, IEnumerable<Quad> quads, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var quadsJson = JsonSerializer.Serialize(quads.ToList());
        var prefixesJson = options.Prefixes != null
            ? JsonSerializer.Serialize(options.Prefixes)
            : null;
        FFIHelper.CallVoid(() =>
            OxigraphNative.serialize_to_file(filePath, quadsJson, FormatToString(format), options.BaseIri, prefixesJson));
    }

    /// <summary>Parse RDF from a .NET Stream into quads.</summary>
    public static IReadOnlyList<Quad> ParseFromStream(Stream stream, RdfFormat format, string? baseIri = null)
    {
        using var ctx = new ReadContext(stream);

        var resultPtr = OxigraphNative.parse_from_callback(
            Marshal.GetFunctionPointerForDelegate(ctx.Callback),
            ctx.ContextPtr,
            FormatToString(format),
            baseIri);

        var resultJson = Marshal.PtrToStringUTF8(resultPtr) ?? "{}";
        OxigraphNative.free_string(resultPtr);
        FFIHelper.ThrowIfError(resultJson);

        using var doc = JsonDocument.Parse(resultJson);
        var quads = JsonSerializer.Deserialize<List<Quad>>(
            doc.RootElement.GetProperty("ok").GetRawText());
        return quads ?? [];
    }

    /// <summary>Serialize quads to a .NET Stream.</summary>
    public static void SerializeToStream(Stream stream, IEnumerable<Quad> quads, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var quadsJson = JsonSerializer.Serialize(quads.ToList());
        using var ctx = new WriteContext(stream);

        var resultPtr = OxigraphNative.serialize_to_callback(
            Marshal.GetFunctionPointerForDelegate(ctx.Callback),
            ctx.ContextPtr,
            quadsJson,
            FormatToString(format),
            options.BaseIri);

        var resultJson = Marshal.PtrToStringUTF8(resultPtr) ?? "{}";
        OxigraphNative.free_string(resultPtr);
        FFIHelper.ThrowIfError(resultJson);
    }

    // ─── Lazy iterator parse ─────────────────────────

    /// <summary>Lazily parse RDF from a file, yielding quads one at a time.</summary>
    public static ParseIterator ParseIterator(string filePath, RdfFormat format, string? baseIri = null)
    {
        return new ParseIterator(filePath, format, baseIri);
    }

    /// <summary>Parse SPARQL query results from XML, JSON, CSV, or TSV.</summary>
    public static QueryResults ParseQueryResults(string data, QueryResultsFormat format)
    {
        var opts = JsonSerializer.Serialize(new
        {
            data,
            format = format switch
            {
                QueryResultsFormat.Xml => "xml",
                QueryResultsFormat.Json => "json",
                QueryResultsFormat.Csv => "csv",
                QueryResultsFormat.Tsv => "tsv",
                _ => "xml",
            },
        });
        var element = FFIHelper.CallValue<JsonElement>(() =>
            OxigraphNative.parse_query_results(opts));
        return QueryResults.FromJson(element.GetRawText());
    }

    internal static string FormatToString(RdfFormat format) => format switch
    {
        RdfFormat.N3 => "n3",
        RdfFormat.NQuads => "nquads",
        RdfFormat.NTriples => "ntriples",
        RdfFormat.RdfXml => "rdfxml",
        RdfFormat.TriG => "trig",
        RdfFormat.Turtle => "turtle",
        RdfFormat.JsonLd => "jsonld",
        RdfFormat.StreamingJsonLd => "jsonld",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };
}

/// <summary>
/// Lazy iterator that parses quads from a file one at a time.
/// Implements IEnumerable&lt;Quad&gt; and IDisposable for clean FFI cleanup.
/// </summary>
public sealed class ParseIterator : IEnumerable<Quad>, IDisposable
{
    private readonly QuadIterSafeHandle _handle;
    private bool _done;

    internal ParseIterator(string filePath, RdfFormat format, string? baseIri)
    {
        var jsonPtr = OxigraphNative.parse_iter_from_file(
            filePath, IO.FormatToString(format), baseIri);
        var json = Marshal.PtrToStringUTF8(jsonPtr) ?? "{}";
        OxigraphNative.free_string(jsonPtr);
        FFIHelper.ThrowIfError(json);

        using var doc = JsonDocument.Parse(json);
        var handleVal = doc.RootElement.GetProperty("ok").GetProperty("handle").GetUInt64();
        _handle = new QuadIterSafeHandle((IntPtr)handleVal);
    }

    public IEnumerator<Quad> GetEnumerator()
    {
        while (!_done)
        {
            var ptr = OxigraphNative.parse_iter_next(_handle.DangerousGetHandle());
            var json = Marshal.PtrToStringUTF8(ptr) ?? "null";
            OxigraphNative.free_string(ptr);

            // Check for error
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("error", out _))
            {
                FFIHelper.ThrowIfError(json);
                yield break;
            }

            var okVal = doc.RootElement.GetProperty("ok");
            if (okVal.ValueKind == JsonValueKind.Null)
            {
                _done = true;
                yield break;
            }

            yield return JsonSerializer.Deserialize<Quad>(okVal.GetRawText())!;
        }
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    public void Dispose() => _handle.Dispose();
}