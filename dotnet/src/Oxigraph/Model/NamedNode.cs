namespace Oxigraph;

/// <summary>An RDF Named Node (IRI reference).</summary>
public sealed record NamedNode(string Value) : INamedOrBlankNode, IGraphName;