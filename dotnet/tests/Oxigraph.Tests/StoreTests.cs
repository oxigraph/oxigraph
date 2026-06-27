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

    [Fact]
    public void FilePersistence_Roundtrip()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), "oxigraph-test-" + Guid.NewGuid());
        try
        {
            // Create and populate
            using (var store = new Store(tempDir))
            {
                store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
                Assert.Equal(1UL, store.Count);
            }

            // Reopen and verify data persisted
            using (var store = new Store(tempDir))
            {
                Assert.Equal(1UL, store.Count);
                Assert.True(store.Contains(
                    Q("http://example.com/s", "http://example.com/p", "test")));
            }
        }
        finally
        {
            if (Directory.Exists(tempDir))
                Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Flush_And_Backup()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), "oxigraph-test-" + Guid.NewGuid());
        var backupDir = Path.Combine(Path.GetTempPath(), "oxigraph-backup-" + Guid.NewGuid());
        try
        {
            using (var store = new Store(tempDir))
            {
                store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
                store.Flush();
                store.Backup(backupDir);
            }

            // Verify backup
            using (var store = new Store(backupDir))
            {
                Assert.Equal(1UL, store.Count);
            }
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
            if (Directory.Exists(backupDir)) Directory.Delete(backupDir, true);
        }
    }

    [Fact]
    public void Dataset_Crud()
    {
        using var ds = new Dataset();
        var q = Q("http://example.com/s", "http://example.com/p", "test");
        ds.Add(q);
        Assert.True(ds.Contains(q));
        Assert.Equal(1, ds.Count);

        ds.Remove(q);
        Assert.False(ds.Contains(q));
        Assert.Equal(0, ds.Count);
    }

    [Fact]
    public void Dataset_Init_From_Quads()
    {
        var quads = new[] {
            Q("http://example.com/s1", "http://example.com/p", "a"),
            Q("http://example.com/s2", "http://example.com/p", "b")
        };
        using var ds = new Dataset(quads);
        Assert.Equal(2, ds.Count);
    }

    [Fact]
    public void Dataset_ToString()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var s = ds.ToString();
        Assert.Contains("http://example.com/s", s);
    }

    [Fact]
    public void Store_EnumerateAllQuads()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        store.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var all = store.ToList();
        Assert.Equal(2, all.Count);
    }

    private static Quad Q(string s, string p, string o) =>
        new(new NamedNode(s), new NamedNode(p), new Literal(o), new DefaultGraph());
}