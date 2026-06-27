using System.Text.Json.Serialization;

namespace Oxigraph;

/// <summary>An RDF Quad with JSON converters matching Rust serde format.</summary>
public sealed record Quad(
    [property: JsonPropertyName("subject")]
    [property: JsonConverter(typeof(NamedOrBlankNodeConverter))]
    INamedOrBlankNode Subject,
    [property: JsonPropertyName("predicate")]
    [property: JsonConverter(typeof(NamedNodeConverter))]
    NamedNode Predicate,
    [property: JsonPropertyName("object")]
    [property: JsonConverter(typeof(TermConverter))]
    ITerm Object,
    [property: JsonPropertyName("graph")]
    [property: JsonConverter(typeof(GraphNameConverter))]
    IGraphName Graph)
{
    /// <summary>The underlying Triple (same S/P/O, no graph).</summary>
    [JsonIgnore]
    public Triple Triple => new(Subject, Predicate, Object);

    public override string ToString()
        => $"{Subject} {Predicate} {Object} {Graph} .";
}