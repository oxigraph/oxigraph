namespace Oxigraph.Tests;

public class StoreTests
{
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
        var q1 = new Quad(
            new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"),
            new Literal("a"),
            new DefaultGraph());
        var q2 = new Quad(
            new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"),
            new Literal("b"),
            new DefaultGraph());

        store.Add(q1);
        store.Add(q2);

        Assert.Equal(2UL, store.Count);
        Assert.True(store.Contains(q1));
        Assert.True(store.Contains(q2));
    }
}