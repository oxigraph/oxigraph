namespace Oxigraph;

/// <summary>The RDF Default Graph name.</summary>
public readonly record struct DefaultGraph : IGraphName
{
    public override string ToString() => "DEFAULT";
}