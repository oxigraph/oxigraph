namespace Oxigraph.Tests;

public class IOTests
{
    [Fact]
    public void Parse_Turtle()
    {
        var data = "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.Turtle, "http://example.com/");
        Assert.Single(quads);
        Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
        Assert.Equal("http://example.com/p", quads[0].Predicate.Value);
        Assert.Equal("hello", ((Literal)quads[0].Object).Value);
    }

    [Fact]
    public void Parse_NTriples()
    {
        var data = "<http://example.com/s> <http://example.com/p> \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.NTriples);
        Assert.Single(quads);
    }

    [Fact]
    public void Parse_NQuads()
    {
        var data = "<http://example.com/s> <http://example.com/p> \"hello\" <http://example.com/g> .";
        var quads = IO.Parse(data, RdfFormat.NQuads);
        Assert.Single(quads);
        Assert.IsType<NamedNode>(quads[0].Graph);
    }

    [Fact]
    public void Load_And_Dump_Roundtrip()
    {
        using var store = new Store();
        var data = "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .";
        store.Load(data, RdfFormat.Turtle, new LoadOptions { BaseIri = "http://example.com/" });
        Assert.Equal(1UL, store.Count);

        var dumped = store.Dump(RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });
        Assert.Contains("http://example.com/s", dumped);
        Assert.Contains("http://example.com/p", dumped);
    }

    [Fact]
    public void Dump_Turtle()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var result = store.Dump(RdfFormat.Turtle, new DumpOptions { FromGraph = new DefaultGraph() });
        // Turtle may or may not include @prefix; verify the data is present
        Assert.Contains("http://example.com/s", result);
        Assert.Contains("http://example.com/p", result);
    }

    [Fact]
    public void Dump_NQuads()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var result = store.Dump(RdfFormat.NQuads);
        Assert.Contains("<http://example.com/s>", result);
    }
}