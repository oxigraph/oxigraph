using System.Text.Json.Serialization;

namespace Oxigraph;

/// <summary>An RDF Triple (subject, predicate, object). Implements ITerm for RDF-star support.</summary>
public sealed record Triple(
    [property: JsonPropertyName("subject")]
    [property: JsonConverter(typeof(NamedOrBlankNodeConverter))]
    INamedOrBlankNode Subject,
    [property: JsonPropertyName("predicate")]
    [property: JsonConverter(typeof(NamedNodeConverter))]
    NamedNode Predicate,
    [property: JsonPropertyName("object")]
    [property: JsonConverter(typeof(TermConverter))]
    ITerm Object) : ITerm, INamedOrBlankNode
{
    public override string ToString()
        => $"{Subject} {Predicate} {Object} .";
}