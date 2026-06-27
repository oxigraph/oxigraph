namespace Oxigraph;

/// <summary>Supported RDF serialization formats.</summary>
public enum RdfFormat
{
    N3,
    NQuads,
    NTriples,
    RdfXml,
    TriG,
    Turtle,
    JsonLd,
    /// <summary>Streaming JSON-LD (for parsing only, flatter structure).</summary>
    StreamingJsonLd,
}

/// <summary>Options for loading RDF data.</summary>
public sealed record LoadOptions(
    string? BaseIri = null,
    IGraphName? ToGraph = null,
    /// <summary>Skip some data validation (faster loading, may ingest invalid data).</summary>
    bool Lenient = false,
    /// <summary>Rename blank node identifiers to random IDs to avoid conflicts when merging.</summary>
    bool RenameBlankNodes = false);

/// <summary>Options for parsing RDF data (standalone IO.Parse).</summary>
public sealed record ParseOptions(
    string? BaseIri = null,
    bool WithoutNamedGraphs = false,
    bool RenameBlankNodes = false,
    bool Lenient = false);

/// <summary>SPARQL query results serialization formats.</summary>
public enum QueryResultsFormat
{
    Xml,
    Json,
    Csv,
    Tsv,
}

/// <summary>RDF canonicalization algorithms.</summary>
public enum CanonicalizationAlgorithm
{
    /// <summary>PyOxigraph preferred algorithm (unstable).</summary>
    Unstable,
    /// <summary>RDFC-1.0 with SHA-256.</summary>
    Rdfc10Sha256,
    /// <summary>RDFC-1.0 with SHA-384.</summary>
    Rdfc10Sha384,
}

/// <summary>Options for dumping/serializing RDF data.</summary>
public sealed record DumpOptions(
    IGraphName? FromGraph = null,
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null);

/// <summary>Extension methods for format metadata.</summary>
public static class FormatMetadata
{
    /// <summary>Get the format canonical IRI (from W3C NS registry).</summary>
    public static string Iri(this RdfFormat format) => format switch
    {
        RdfFormat.N3 => "http://www.w3.org/ns/formats/N3",
        RdfFormat.NQuads => "http://www.w3.org/ns/formats/N-Quads",
        RdfFormat.NTriples => "http://www.w3.org/ns/formats/N-Triples",
        RdfFormat.RdfXml => "http://www.w3.org/ns/formats/RDF_XML",
        RdfFormat.TriG => "http://www.w3.org/ns/formats/TriG",
        RdfFormat.Turtle => "http://www.w3.org/ns/formats/Turtle",
        RdfFormat.JsonLd => "http://www.w3.org/ns/formats/JSON-LD",
        RdfFormat.StreamingJsonLd => "http://www.w3.org/ns/formats/JSON-LD",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the format display name.</summary>
    public static string Name(this RdfFormat format) => format switch
    {
        RdfFormat.N3 => "N3",
        RdfFormat.NQuads => "N-Quads",
        RdfFormat.NTriples => "N-Triples",
        RdfFormat.RdfXml => "RDF/XML",
        RdfFormat.TriG => "TriG",
        RdfFormat.Turtle => "Turtle",
        RdfFormat.JsonLd => "JSON-LD",
        RdfFormat.StreamingJsonLd => "Streaming JSON-LD",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Whether this format supports RDF datasets (named graphs), not just triples.</summary>
    public static bool SupportsDatasets(this RdfFormat format) => format switch
    {
        RdfFormat.NQuads or RdfFormat.TriG or RdfFormat.JsonLd or RdfFormat.StreamingJsonLd => true,
        RdfFormat.N3 or RdfFormat.NTriples or RdfFormat.RdfXml or RdfFormat.Turtle => false,
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the IANA media type for an RDF format.</summary>
    public static string MediaType(this RdfFormat format) => format switch
    {
        RdfFormat.N3 => "text/n3",
        RdfFormat.NQuads => "application/n-quads",
        RdfFormat.NTriples => "application/n-triples",
        RdfFormat.RdfXml => "application/rdf+xml",
        RdfFormat.TriG => "application/trig",
        RdfFormat.Turtle => "text/turtle",
        RdfFormat.JsonLd => "application/ld+json",
        RdfFormat.StreamingJsonLd => "application/ld+json",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the file extension for an RDF format.</summary>
    public static string FileExtension(this RdfFormat format) => format switch
    {
        RdfFormat.N3 => "n3",
        RdfFormat.NQuads => "nq",
        RdfFormat.NTriples => "nt",
        RdfFormat.RdfXml => "rdf",
        RdfFormat.TriG => "trig",
        RdfFormat.Turtle => "ttl",
        RdfFormat.JsonLd => "jsonld",
        RdfFormat.StreamingJsonLd => "jsonld",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Look up an RDF format from a file extension.</summary>
    public static RdfFormat? FromExtension(string extension) => extension.ToLowerInvariant() switch
    {
        "n3" => RdfFormat.N3,
        "nq" or "nquads" => RdfFormat.NQuads,
        "nt" or "ntriples" => RdfFormat.NTriples,
        "rdf" or "rdfxml" or "owl" => RdfFormat.RdfXml,
        "trig" => RdfFormat.TriG,
        "ttl" or "turtle" => RdfFormat.Turtle,
        "jsonld" or "json-ld" => RdfFormat.JsonLd,
        _ => null,
    };

    /// <summary>Look up an RDF format from a media type.</summary>
    public static RdfFormat? FromMediaType(string mediaType)
    {
        var mime = mediaType.Split(';')[0].Trim().ToLowerInvariant();
        return mime switch
        {
            "text/n3" => RdfFormat.N3,
            "application/n-quads" => RdfFormat.NQuads,
            "application/n-triples" => RdfFormat.NTriples,
            "application/rdf+xml" or "application/xml" => RdfFormat.RdfXml,
            "application/trig" => RdfFormat.TriG,
            "text/turtle" or "application/x-turtle" => RdfFormat.Turtle,
            "application/ld+json" or "application/json" => RdfFormat.JsonLd,
            _ => null,
        };
    }

    /// <summary>Get the format canonical IRI for a query results format.</summary>
    public static string Iri(this QueryResultsFormat format) => format switch
    {
        QueryResultsFormat.Xml => "http://www.w3.org/ns/formats/SPARQL_Results_XML",
        QueryResultsFormat.Json => "http://www.w3.org/ns/formats/SPARQL_Results_JSON",
        QueryResultsFormat.Csv => "http://www.w3.org/ns/formats/SPARQL_Results_CSV",
        QueryResultsFormat.Tsv => "http://www.w3.org/ns/formats/SPARQL_Results_TSV",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the display name for a query results format.</summary>
    public static string Name(this QueryResultsFormat format) => format switch
    {
        QueryResultsFormat.Xml => "SPARQL Results in XML",
        QueryResultsFormat.Json => "SPARQL Results in JSON",
        QueryResultsFormat.Csv => "SPARQL Results in CSV",
        QueryResultsFormat.Tsv => "SPARQL Results in TSV",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the IANA media type for a query results format.</summary>
    public static string MediaType(this QueryResultsFormat format) => format switch
    {
        QueryResultsFormat.Xml => "application/sparql-results+xml",
        QueryResultsFormat.Json => "application/sparql-results+json",
        QueryResultsFormat.Csv => "text/csv",
        QueryResultsFormat.Tsv => "text/tab-separated-values",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Get the file extension for a query results format.</summary>
    public static string FileExtension(this QueryResultsFormat format) => format switch
    {
        QueryResultsFormat.Xml => "srx",
        QueryResultsFormat.Json => "srj",
        QueryResultsFormat.Csv => "csv",
        QueryResultsFormat.Tsv => "tsv",
        _ => throw new ArgumentException($"Unknown format: {format}"),
    };

    /// <summary>Look up a query results format from a file extension.</summary>
    public static QueryResultsFormat? QueryFromExtension(string extension) => extension.ToLowerInvariant() switch
    {
        "srx" or "xml" => QueryResultsFormat.Xml,
        "srj" or "json" => QueryResultsFormat.Json,
        "csv" => QueryResultsFormat.Csv,
        "tsv" => QueryResultsFormat.Tsv,
        _ => null,
    };

    /// <summary>Look up a query results format from a media type.</summary>
    public static QueryResultsFormat? QueryFromMediaType(string mediaType)
    {
        var mime = mediaType.Split(';')[0].Trim().ToLowerInvariant();
        return mime switch
        {
            "application/sparql-results+xml" or "application/xml" => QueryResultsFormat.Xml,
            "application/sparql-results+json" => QueryResultsFormat.Json,
            "text/csv" => QueryResultsFormat.Csv,
            "text/tab-separated-values" => QueryResultsFormat.Tsv,
            _ => null,
        };
    }
}