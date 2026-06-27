namespace Oxigraph;

/// <summary>An RDF Literal.</summary>
/// <param name="Value">The literal lexical form.</param>
/// <param name="Language">The optional language tag (RFC 5646 / BCP 47).</param>
/// <param name="Datatype">The optional datatype IRI. Defaults to xsd:string for simple literals.</param>
/// <param name="Direction">The optional base direction (RDF 1.2: ltr or rtl). Only valid with a language tag.</param>
public sealed record Literal(string Value, string? Language = null, NamedNode? Datatype = null, BaseDirection? Direction = null) : ITerm;