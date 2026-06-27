namespace Oxigraph;

/// <summary>An RDF Literal.</summary>
/// <param name="Value">The literal lexical form.</param>
/// <param name="Language">The optional language tag (RFC 5646 / BCP 47).</param>
/// <param name="Datatype">The optional datatype IRI. Defaults to xsd:string for simple literals.</param>
/// <param name="Direction">The optional base direction (RDF 1.2: ltr or rtl). Only valid with a language tag.</param>
public sealed record Literal(string Value, string? Language = null, NamedNode? Datatype = null, BaseDirection? Direction = null) : ITerm
{
    // ─── XSD vocabulary ──────────────────────────────

    /// <summary>XSD string datatype IRI.</summary>
    public static readonly NamedNode XsdString = new("http://www.w3.org/2001/XMLSchema#string");
    /// <summary>XSD integer datatype IRI.</summary>
    public static readonly NamedNode XsdInteger = new("http://www.w3.org/2001/XMLSchema#integer");
    /// <summary>XSD double datatype IRI.</summary>
    public static readonly NamedNode XsdDouble = new("http://www.w3.org/2001/XMLSchema#double");
    /// <summary>XSD boolean datatype IRI.</summary>
    public static readonly NamedNode XsdBoolean = new("http://www.w3.org/2001/XMLSchema#boolean");

    // ─── Factory methods ─────────────────────────────

    /// <summary>Create a typed literal from an integer value (xsd:integer).</summary>
    public static Literal FromInt(int value) =>
        new(value.ToString(), Datatype: XsdInteger);

    /// <summary>Create a typed literal from a double value (xsd:double).
    /// Uses the "R" format to match Python's round-trippable output.</summary>
    public static Literal FromDouble(double value) =>
        new(value.ToString("R"), Datatype: XsdDouble);

    /// <summary>Create a typed literal from a boolean value (xsd:boolean).</summary>
    public static Literal FromBool(bool value) =>
        new(value ? "true" : "false", Datatype: XsdBoolean);

    // ─── Implicit conversions ────────────────────────

    /// <summary>Implicitly convert an int to an xsd:integer literal.</summary>
    public static implicit operator Literal(int value) => FromInt(value);

    /// <summary>Implicitly convert a double to an xsd:double literal.</summary>
    public static implicit operator Literal(double value) => FromDouble(value);

    /// <summary>Implicitly convert a bool to an xsd:boolean literal.</summary>
    public static implicit operator Literal(bool value) => FromBool(value);
}