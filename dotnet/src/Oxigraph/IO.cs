using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>Standalone RDF parse and serialize functions.</summary>
public static class IO
{
    /// <summary>Parse RDF text into a list of quads.</summary>
    public static IReadOnlyList<Quad> Parse(string data, RdfFormat format, string? baseIri = null)
    {
        var opts = JsonSerializer.Serialize(new
        {
            data,
            format = FormatToString(format),
            base_iri = baseIri,
        });
        return FFIHelper.Call<List<Quad>>(() =>
            OxigraphNative.parse(opts)) ?? [];
    }

    /// <summary>Serialize a list of quads to RDF text.</summary>
    public static string Serialize(IEnumerable<Quad> quads, RdfFormat format, DumpOptions? options = null)
    {
        options ??= new DumpOptions();
        var opts = new Dictionary<string, object?>
        {
            ["format"] = FormatToString(format),
            ["base_iri"] = options.BaseIri,
        };
        if (options.FromGraph != null)
            opts["from_graph"] = options.FromGraph;

        // The parse/serialize standalone functions use different FFI.
        // For simplicity, create a temporary store.
        using var store = new Store();
        store.Extend(quads);
        return store.Dump(format, options);
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
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };
}