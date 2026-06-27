using System.Text.Json.Serialization;

namespace Oxigraph;

/// <summary>An RDF Triple (subject, predicate, object).</summary>
public sealed record Triple(
    [property: JsonPropertyName("subject")]
    [property: JsonConverter(typeof(NamedOrBlankNodeConverter))]
    INamedOrBlankNode Subject,
    [property: JsonPropertyName("predicate")]
    [property: JsonConverter(typeof(NamedNodeConverter))]
    NamedNode Predicate,
    [property: JsonPropertyName("object")]
    [property: JsonConverter(typeof(TermConverter))]
    ITerm Object)
{
    public override string ToString()
        => $"{Subject} {Predicate} {Object} .";
}