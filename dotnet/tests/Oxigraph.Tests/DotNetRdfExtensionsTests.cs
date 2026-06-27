using Oxigraph.Extensions.DotNetRDF;
using VDS.RDF;

namespace Oxigraph.Tests;

public class DotNetRdfExtensionsTests
{
    [Fact]
    public void ToOxigraphTerm_UriNode()
    {
        var g = new Graph();
        var uriNode = g.CreateUriNode(new Uri("http://example.com/s"));
        var term = uriNode.ToOxigraphTerm();
        var named = Assert.IsType<NamedNode>(term);
        Assert.Equal("http://example.com/s", named.Value);
    }

    [Fact]
    public void ToOxigraphTerm_BlankNode()
    {
        var g = new Graph();
        var blankNode = g.CreateBlankNode();
        var term = blankNode.ToOxigraphTerm();
        var bn = Assert.IsType<BlankNode>(term);
        Assert.NotNull(bn.Value);
    }

    [Fact]
    public void ToOxigraphTerm_LiteralNode_Plain()
    {
        var g = new Graph();
        var litNode = g.CreateLiteralNode("hello");
        var term = litNode.ToOxigraphTerm();
        var lit = Assert.IsType<Literal>(term);
        Assert.Equal("hello", lit.Value);
    }

    [Fact]
    public void ToOxigraphTerm_LiteralNode_LanguageTagged()
    {
        var g = new Graph();
        var litNode = g.CreateLiteralNode("bonjour", "fr");
        var term = litNode.ToOxigraphTerm();
        var lit = Assert.IsType<Literal>(term);
        Assert.Equal("bonjour", lit.Value);
        Assert.Equal("fr", lit.Language);
    }

    [Fact]
    public void ToOxigraphTerm_LiteralNode_Typed()
    {
        var g = new Graph();
        var dt = new Uri("http://www.w3.org/2001/XMLSchema#integer");
        var litNode = g.CreateLiteralNode("42", dt);
        var term = litNode.ToOxigraphTerm();
        var lit = Assert.IsType<Literal>(term);
        Assert.Equal("42", lit.Value);
        Assert.NotNull(lit.Datatype);
        Assert.Equal("http://www.w3.org/2001/XMLSchema#integer", lit.Datatype!.Value);
        Assert.Null(lit.Language);
    }

    [Fact]
    public void ToOxigraphTerm_LiteralNode_Typed_StringType()
    {
        var g = new Graph();
        var litNode = g.CreateLiteralNode("hello", new Uri("http://www.w3.org/2001/XMLSchema#string"));
        var term = litNode.ToOxigraphTerm();
        var lit = Assert.IsType<Literal>(term);
        Assert.Equal("hello", lit.Value);
        Assert.NotNull(lit.Datatype);
        Assert.Null(lit.Language);
    }

    [Fact]
    public void ToOxigraphQuad_ConvertsToDefaultGraph()
    {
        var g = new Graph();
        var s = g.CreateUriNode(new Uri("http://example.com/s"));
        var p = g.CreateUriNode(new Uri("http://example.com/p"));
        var o = g.CreateLiteralNode("test");
        var triple = new VDS.RDF.Triple(s, p, o);

        var quad = triple.ToOxigraphQuad();
        Assert.Equal("http://example.com/s", ((NamedNode)quad.Subject).Value);
        Assert.Equal("http://example.com/p", quad.Predicate.Value);
        Assert.Equal("test", ((Literal)quad.Object).Value);
        Assert.IsType<DefaultGraph>(quad.Graph);
    }

    [Fact]
    public void LoadFromGraph_AddsAllTriples()
    {
        var g = new Graph();
        var s1 = g.CreateUriNode(new Uri("http://example.com/s1"));
        var p = g.CreateUriNode(new Uri("http://example.com/p"));
        // dotNetRDF returns Language="" for plain literals as well,
        // so use language-tagged literals to work around this
        var o1 = g.CreateLiteralNode("a", "en");
        var s2 = g.CreateUriNode(new Uri("http://example.com/s2"));
        var o2 = g.CreateLiteralNode("b", "en");

        g.Assert(new VDS.RDF.Triple(s1, p, o1));
        g.Assert(new VDS.RDF.Triple(s2, p, o2));

        using var store = new Store();
        store.LoadFromGraph(g);
        Assert.Equal(2UL, store.Count);
    }
}