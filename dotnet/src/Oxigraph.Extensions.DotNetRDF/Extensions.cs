using VDS.RDF;

namespace Oxigraph.Extensions.DotNetRDF;

/// <summary>Conversion methods between Oxigraph and dotNetRDF types.</summary>
public static class Extensions
{
    /// <summary>Convert a dotNetRDF INode to an Oxigraph ITerm.</summary>
    public static ITerm ToOxigraphTerm(this INode node) => node switch
    {
        IUriNode u => new NamedNode(u.Uri.AbsoluteUri),
        IBlankNode b => new BlankNode(b.InternalID),
        ILiteralNode l => l.Language != null
            ? new Literal(l.Value, l.Language)
            : l.DataType != null
                ? new Literal(l.Value, Datatype: new NamedNode(l.DataType.AbsoluteUri))
                : new Literal(l.Value),
        _ => throw new ArgumentException($"Unknown node type: {node.GetType()}"),
    };

    /// <summary>Convert a dotNetRDF Triple to an Oxigraph Quad in the default graph.</summary>
    public static Quad ToOxigraphQuad(this VDS.RDF.Triple triple) =>
        new(
            (INamedOrBlankNode)triple.Subject.ToOxigraphTerm(),
            new NamedNode(((IUriNode)triple.Predicate).Uri.AbsoluteUri),
            triple.Object.ToOxigraphTerm(),
            new DefaultGraph());

    /// <summary>Load all triples from a dotNetRDF IGraph into an Oxigraph Store.</summary>
    public static void LoadFromGraph(this Store store, IGraph graph)
    {
        foreach (var t in graph.Triples)
            store.Add(t.ToOxigraphQuad());
    }
}