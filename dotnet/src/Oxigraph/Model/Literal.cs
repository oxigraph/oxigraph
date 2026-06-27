namespace Oxigraph;

/// <summary>An RDF Literal.</summary>
public sealed record Literal(string Value, string? Language = null, NamedNode? Datatype = null) : ITerm;