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
        var opts = JsonSerializer.Serialize(new
        {
            quads = quads.ToList(),
            format = FormatToString(format),
            base_iri = options.BaseIri,
        });
        return FFIHelper.Call<string>(() =>
            OxigraphNative.serialize(opts));
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
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };
}