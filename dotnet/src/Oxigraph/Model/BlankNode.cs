namespace Oxigraph;

/// <summary>An RDF Blank Node.</summary>
public sealed record BlankNode(string Value) : INamedOrBlankNode, IGraphName;