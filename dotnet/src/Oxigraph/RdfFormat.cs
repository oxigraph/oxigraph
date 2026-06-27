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
}

/// <summary>Options for loading RDF data.</summary>
public sealed record LoadOptions(
    string? BaseIri = null,
    IGraphName? ToGraph = null);

/// <summary>Options for dumping/serializing RDF data.</summary>
public sealed record DumpOptions(
    IGraphName? FromGraph = null,
    string? BaseIri = null);