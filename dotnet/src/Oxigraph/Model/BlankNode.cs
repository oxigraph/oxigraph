namespace Oxigraph;

/// <summary>An RDF Blank Node.</summary>
/// <param name="Value">The blank node identifier. Auto-generated if not provided.</param>
public sealed record BlankNode(string Value) : INamedOrBlankNode, IGraphName
{
    private static long _counter;
    private static readonly string _uniqueId = Guid.NewGuid().ToString("N")[..8];

    /// <summary>Create a blank node with an auto-generated unique identifier.</summary>
    public BlankNode() : this($"b{Interlocked.Increment(ref _counter)}_{_uniqueId}") { }
}