namespace Oxigraph;

/// <summary>A SPARQL query variable.</summary>
public sealed record Variable(string Value)
{
    public override string ToString() => $"?{Value}";
}