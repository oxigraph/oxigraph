using System.Text.Json;

namespace Oxigraph.Tests;

public class StoreTests
{
    private static readonly JsonSerializerOptions _opts = new()
    {
        Converters = { new NamedOrBlankNodeConverter(), new GraphNameConverter() }
    };

    [Fact]
    public void NewStore_IsEmpty()
    {
        using var store = new Store();
        Assert.Equal(0UL, store.Count);
        Assert.True(store.IsEmpty);
    }

    [Fact]
    public void Add_And_Contains()
    {
        using var store = new Store();
        var quad = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph());
        store.Add(quad);
        Assert.Equal(1UL, store.Count);
        Assert.True(store.Contains(quad));
    }

    [Fact]
    public void Contains_NotFound()
    {
        using var store = new Store();
        var quad = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph());
        Assert.False(store.Contains(quad));
    }

    [Fact]
    public void Add_Remove_Contains()
    {
        using var store = new Store();
        var quad = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph());
        store.Add(quad);
        Assert.True(store.Contains(quad));
        store.Remove(quad);
        Assert.False(store.Contains(quad));
        Assert.Equal(0UL, store.Count);
    }

    [Fact]
    public void Add_Multiple_Quads()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        store.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        Assert.Equal(2UL, store.Count);
    }

    [Fact]
    public void Clear_All()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        Assert.Equal(1UL, store.Count);
        store.Clear();
        Assert.Equal(0UL, store.Count);
    }

    [Fact]
    public void Extend_Bulk_Insert()
    {
        using var store = new Store();
        var quads = new[]
        {
            Q("http://example.com/s1", "http://example.com/p", "a"),
            Q("http://example.com/s2", "http://example.com/p", "b"),
            Q("http://example.com/s3", "http://example.com/p", "c"),
        };
        store.Extend(quads);
        Assert.Equal(3UL, store.Count);
    }

    [Fact]
    public void NamedGraph_Add_And_Contains()
    {
        using var store = new Store();
        var graph = new NamedNode("http://example.com/g");
        store.AddGraph(graph);
        Assert.True(store.ContainsNamedGraph(graph));
    }

    [Fact]
    public void NamedGraph_Remove()
    {
        using var store = new Store();
        var graph = new NamedNode("http://example.com/g");
        store.AddGraph(graph);
        Assert.NotEmpty(store.NamedGraphs);
        store.RemoveGraph(graph);
        Assert.False(store.ContainsNamedGraph(graph));
    }

    [Fact]
    public void ClearGraph_KeepsGraphName()
    {
        using var store = new Store();
        var graph = new NamedNode("http://example.com/g");
        store.AddGraph(graph);
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("test"),
            graph));
        Assert.Equal(1UL, store.Count);

        store.ClearGraph(graph);
        Assert.Equal(0UL, store.Count);
        Assert.True(store.ContainsNamedGraph(graph)); // graph still exists
    }

    [Fact]
    public void Match_Returns_All_Quads()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        store.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var results = store.Match();
        Assert.Equal(2, results.Count);
    }

    private static Quad Q(string s, string p, string o) =>
        new(new NamedNode(s), new NamedNode(p), new Literal(o), new DefaultGraph());
}