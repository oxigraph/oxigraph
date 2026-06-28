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

    // ─── Bulk Extend ──────────────────────────────────

    [Fact]
    public void BulkExtend_LargeDataset()
    {
        using var store = new Store();
        var quads = Enumerable.Range(0, 100).Select(i =>
            Q($"http://example.com/s{i}", "http://example.com/p", $"value{i}"));
        store.BulkExtend(quads);
        Assert.Equal(100UL, store.Count);
    }

    // ─── Pattern Matching ─────────────────────────────

    [Fact]
    public void Match_BySubject()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        store.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var results = store.Match(subject: new NamedNode("http://example.com/s1"));
        Assert.Single(results);
        Assert.Equal("http://example.com/s1", ((NamedNode)results[0].Subject).Value);
    }

    [Fact]
    public void Match_ByPredicate()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p1", "a"));
        store.Add(Q("http://example.com/s2", "http://example.com/p2", "b"));
        var results = store.Match(predicate: new NamedNode("http://example.com/p1"));
        Assert.Single(results);
    }

    [Fact]
    public void Match_ByObject()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"), new DefaultGraph()));
        var results = store.Match(@object: new Literal("a"));
        Assert.Single(results);
        Assert.Equal("a", ((Literal)results[0].Object).Value);
    }

    [Fact]
    public void Match_ByGraph()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("a"), g));
        store.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var results = store.Match(graph: g);
        Assert.Single(results);
        Assert.IsType<NamedNode>(results[0].Graph);
    }

    [Fact]
    public void Match_ByDefaultGraph()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"),
            new NamedNode("http://example.com/g")));
        var results = store.Match(graph: new DefaultGraph());
        Assert.Single(results);
        Assert.IsType<DefaultGraph>(results[0].Graph);
    }

    // ─── Named Graph with Blank Node ──────────────────

    [Fact]
    public void NamedGraph_BlankNode()
    {
        using var store = new Store();
        var g = new BlankNode("g");
        store.AddGraph(g);
        Assert.True(store.ContainsNamedGraph(g));
        var graphs = store.NamedGraphs;
        Assert.Contains(g, graphs);
    }

    [Fact]
    public void NamedGraph_Contains_DefaultGraph()
    {
        using var store = new Store();
        // Default graph always "exists"
        Assert.True(store.ContainsNamedGraph(new DefaultGraph()));
    }

    // ─── Read-Only Store ──────────────────────────────

    [Fact]
    public void ReadOnly_Store()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), "oxigraph-readonly-" + Guid.NewGuid());
        try
        {
            using (var writeStore = new Store(tempDir))
            {
                writeStore.Add(Q("http://example.com/s", "http://example.com/p", "test"));
            }

            using var readStore = Store.OpenReadOnly(tempDir);
            Assert.Equal(1UL, readStore.Count);
            Assert.True(readStore.Contains(Q("http://example.com/s", "http://example.com/p", "test")));
        }
        finally { if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true); }
    }

    // ─── Store ToString ───────────────────────────────

    [Fact]
    public void Store_ToString()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var s = store.ToString();
        Assert.Contains("http://example.com/s", s);
        Assert.Contains("http://example.com/p", s);
    }

    // ─── Optimize ─────────────────────────────────────

    [Fact]
    public void Optimize_AfterInsert()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        store.Optimize(); // Should not throw
        Assert.Equal(1UL, store.Count);
    }

    // ─── Dataset Extend ───────────────────────────────

    [Fact]
    public void Dataset_Extend()
    {
        using var ds = new Dataset();
        ds.Extend([
            Q("http://example.com/s1", "http://example.com/p", "a"),
            Q("http://example.com/s2", "http://example.com/p", "b"),
        ]);
        Assert.Equal(2, ds.Count);
    }

    // ─── Dataset: ToList enumeration ──────────────────

    [Fact]
    public void Dataset_ToList_Enumeration()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        ds.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var list = ds.ToList();
        Assert.Equal(2, list.Count);
    }

    // ─── Store: Match combined parameters ─────────────

    [Fact]
    public void Match_Combined_SubjectAndPredicate()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s1", "http://example.com/p1", "a"));
        store.Add(Q("http://example.com/s1", "http://example.com/p2", "b"));
        store.Add(Q("http://example.com/s2", "http://example.com/p1", "c"));
        var results = store.Match(
            subject: new NamedNode("http://example.com/s1"),
            predicate: new NamedNode("http://example.com/p1"));
        Assert.Single(results);
    }

    [Fact]
    public void Match_Combined_AllParams()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), g));
        store.Add(Q("http://example.com/s", "http://example.com/p", "other"));
        var results = store.Match(
            subject: new NamedNode("http://example.com/s"),
            predicate: new NamedNode("http://example.com/p"),
            @object: new Literal("test"),
            graph: g);
        Assert.Single(results);
    }

    [Fact]
    public void Match_NoMatch_ReturnsEmpty()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var results = store.Match(subject: new NamedNode("http://example.com/nonexistent"));
        Assert.Empty(results);
    }

    // ─── BulkExtend: edge cases ──────────────────────

    [Fact]
    public void BulkExtend_Empty()
    {
        using var store = new Store();
        store.BulkExtend([]);
        Assert.Equal(0UL, store.Count);
    }

    [Fact]
    public void BulkExtend_SmallBatch()
    {
        using var store = new Store();
        store.BulkExtend([Q("http://example.com/s", "http://example.com/p", "test")]);
        Assert.Equal(1UL, store.Count);
    }

    // ─── Store: BulkLoad ─────────────────────────────

    [Fact]
    public void BulkLoad_Works()
    {
        using var store = new Store();
        store.BulkLoad("@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .",
            RdfFormat.Turtle, new LoadOptions { BaseIri = "http://example.com/" });
        Assert.Equal(1UL, store.Count);
    }

    // ─── Store: BulkLoadFromFile with options ─────────

    [Fact]
    public void BulkLoadFromFile_WithLenient()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<not a valid iri> <http://example.com/p> \"hello\" .");
            using var store = new Store();
            store.BulkLoadFromFile(tempFile, RdfFormat.NTriples,
                new LoadOptions { Lenient = true });
            Assert.Equal(1UL, store.Count);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Store: Dispose safety ───────────────────────

    [Fact]
    public void Dispose_CalledMultipleTimes_NoException()
    {
        var store = new Store();
        store.Dispose();
        // Double dispose should be safe (SafeHandle handles this)
        store.Dispose();
    }

    // ─── Store: empty named graphs ───────────────────

    [Fact]
    public void NamedGraphs_Empty()
    {
        using var store = new Store();
        Assert.Empty(store.NamedGraphs);
    }

    [Fact]
    public void RemoveGraph_NonExistent()
    {
        using var store = new Store();
        // Removing a graph that doesn't exist should not throw
        store.RemoveGraph(new NamedNode("http://example.com/nonexistent"));
    }

    // ─── Store: Extend with empty list ────────────────

    [Fact]
    public void Extend_Empty()
    {
        using var store = new Store();
        store.Extend([]);
        Assert.Equal(0UL, store.Count);
    }

    // ─── Store: Dump with base_iri ───────────────────

    [Fact]
    public void Dump_WithBaseIri()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var dumped = store.Dump(RdfFormat.NTriples, new DumpOptions
        {
            FromGraph = new DefaultGraph(),
            BaseIri = "http://example.com/"
        });
        // N-Triples always uses full URIs; Turtle uses @base for relative URIs
        Assert.Contains("http://example.com/s", dumped);
    }

    // ─── Store: DumpToStream ─────────────────────────

    [Fact]
    public void DumpToStream_Roundtrip()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        using var stream = new MemoryStream();
        store.DumpToStream(stream, RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });
        stream.Position = 0;
        var content = new StreamReader(stream).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    // ─── Store: Load with lenient ────────────────────

    [Fact]
    public void Load_Lenient_InvalidIri()
    {
        using var store = new Store();
        store.Load("<not valid> <http://example.com/p> \"hello\" .",
            RdfFormat.NTriples, new LoadOptions { Lenient = true });
        Assert.Equal(1UL, store.Count);
    }

    // ─── Store: Load with rename_blank_nodes ───────────

    [Fact]
    public void Load_RenameBlankNodes()
    {
        using var store = new Store();
        store.Load("_:s <http://example.com/p> \"o\" .", RdfFormat.NTriples,
            new LoadOptions { RenameBlankNodes = true });
        Assert.Equal(1UL, store.Count);
    }

    // ─── BulkLoad with bytes ──────────────────────────

    [Fact]
    public void BulkLoad_Bytes()
    {
        using var store = new Store();
        store.BulkLoad(
            "<http://foo> <http://bar> <http://baz> .",
            RdfFormat.NTriples);
        Assert.Equal(1UL, store.Count);
    }

    // ─── BulkLoad with to_graph ───────────────────────

    [Fact]
    public void BulkLoad_ToGraph()
    {
        using var store = new Store();
        var targetGraph = new NamedNode("http://example.com/g");
        store.BulkLoad(
            "<http://foo> <http://bar> <http://baz> .",
            RdfFormat.NTriples,
            new LoadOptions { ToGraph = targetGraph });
        Assert.Equal(1UL, store.Count);
        var quads = store.Match();
        Assert.Equal("http://example.com/g", ((NamedNode)quads[0].Graph).Value);
    }

    [Fact]
    public void BulkLoad_ToGraph_BlankNode()
    {
        using var store = new Store();
        var targetGraph = new BlankNode("g");
        store.BulkLoad(
            "<http://foo> <http://bar> <http://baz> .",
            RdfFormat.NTriples,
            new LoadOptions { ToGraph = targetGraph });
        Assert.Equal(1UL, store.Count);
        Assert.IsType<BlankNode>(store.Match()[0].Graph);
    }

    // ─── Load TriG with base_iri ─────────────────────

    [Fact]
    public void Load_TriG_WithBaseIri()
    {
        using var store = new Store();
        store.Load(
            "<http://graph> { <http://foo> <http://bar> <> . }",
            RdfFormat.TriG,
            new LoadOptions { BaseIri = "http://baz" });
        Assert.Equal(1UL, store.Count);
    }

    // ─── Dump TriG format ────────────────────────────

    [Fact]
    public void Dump_TriG()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://foo"), new NamedNode("http://bar"),
            new Literal("baz"), g));
        store.Add(Q("http://foo", "http://bar", "baz"));
        var dumped = store.Dump(RdfFormat.TriG);
        Assert.Contains("http://foo", dumped);
    }

    // ─── Write-in-read pattern ──────────────────────

    [Fact]
    public void WriteInRead_ModifyWhileIterating()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/a", "http://example.com/p", "v1"));
        store.Add(Q("http://example.com/b", "http://example.com/p", "v2"));
        var snapshot = store.ToList(); // materialize snapshot
        Assert.Equal(2, snapshot.Count);
        foreach (var q in snapshot)
        {
            store.Add(new Quad(
                q.Subject,
                q.Predicate,
                new Literal(((Literal)q.Object).Value + "_extra"),
                q.Graph));
        }
        Assert.Equal(4UL, store.Count);
    }

    // ─── DefaultGraph restriction with BlankNode ──────

    [Fact]
    public void Select_With_DefaultGraph_Bnode_Restriction()
    {
        using var store = new Store();
        var g = new BlankNode("g");
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"), g));
        // Restrict to BlankNode graph
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }",
            new QueryOptions { DefaultGraphs = [g] });
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Single(sols);
    }

    // ─── NamedGraph restriction with BlankNode ────────

    [Fact]
    public void Select_With_NamedGraph_Bnode_Restriction()
    {
        using var store = new Store();
        var g = new BlankNode("g");
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), g));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"),
            new NamedNode("http://example.com/g2")));
        var results = store.Query("SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o } }",
            new QueryOptions { NamedGraphs = [g] });
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Single(sols);
    }

    // ─── LOAD URL in SPARQL update ────────────────────

    [Fact]
    public void Update_Load_FromUrl()
    {
        using var store = new Store();
        store.Update("LOAD <https://www.w3.org/1999/02/22-rdf-syntax-ns>");
        Assert.True(store.Count > 0);
    }

    // ─── Load with IO error (write-only stream) ──────

    [Fact]
    public void Load_WriteOnlyStream_Throws()
    {
        using var store = new Store();
        // Create a write-only memory stream that can't be read
        var writeOnlyStream = new MemoryStream();
        writeOnlyStream.Close(); // Cannot read from closed stream
        Assert.ThrowsAny<Exception>(() =>
            store.LoadFromStream(writeOnlyStream, RdfFormat.NTriples));
    }

    // ─── Dump to file system path ────────────────────

    [Fact]
    public void Dump_ToFile()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
            store.DumpToFile(tempFile, RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });
            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);
        }
        finally { File.Delete(tempFile); }
    }

    // ═══════════════════════════════════════════════════
    // Async API tests
    // ═══════════════════════════════════════════════════

    [Fact]
    public async Task Async_Query_ReturnsResults()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var results = await store.QueryAsync("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Single(sols);
    }

    [Fact]
    public async Task Async_Update_InsertsData()
    {
        using var store = new Store();
        await store.UpdateAsync("INSERT DATA { <http://example.com/s> <http://example.com/p> \"test\" }");
        Assert.Equal(1UL, store.Count);
    }

    [Fact]
    public async Task Async_Query_WithCancellationToken()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        var results = await store.QueryAsync("SELECT ?s WHERE { ?s ?p ?o }", ct: cts.Token);
        Assert.IsAssignableFrom<QuerySolutions>(results);
    }

    [Fact]
    public async Task Async_LoadFromFile()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<http://example.com/s> <http://example.com/p> \"hello\" .");
            using var store = new Store();
            await store.LoadFromFileAsync(tempFile, RdfFormat.NTriples);
            Assert.Equal(1UL, store.Count);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public async Task Async_BulkLoadFromFile()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            var lines = string.Join("\n", Enumerable.Range(0, 20).Select(i =>
                $"<http://example.com/s{i}> <http://example.com/p> \"value{i}\" ."));
            File.WriteAllText(tempFile, lines);
            using var store = new Store();
            await store.BulkLoadFromFileAsync(tempFile, RdfFormat.NTriples);
            Assert.Equal(20UL, store.Count);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public async Task Async_DumpToFile()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
            await store.DumpToFileAsync(tempFile, RdfFormat.NTriples,
                new DumpOptions { FromGraph = new DefaultGraph() });
            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public async Task Async_LoadFromStream()
    {
        var data = "<http://example.com/s> <http://example.com/p> \"stream\" .";
        using var stream = new MemoryStream(System.Text.Encoding.UTF8.GetBytes(data));
        using var store = new Store();
        await store.LoadFromStreamAsync(stream, RdfFormat.NTriples);
        Assert.Equal(1UL, store.Count);
    }

    [Fact]
    public async Task Async_DumpToStream()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        using var output = new MemoryStream();
        await store.DumpToStreamAsync(output, RdfFormat.NTriples,
            new DumpOptions { FromGraph = new DefaultGraph() });
        output.Position = 0;
        var content = new StreamReader(output).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    [Fact]
    public async Task Async_Backup()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), "oxigraph-test-" + Guid.NewGuid());
        var backupDir = Path.Combine(Path.GetTempPath(), "oxigraph-backup-" + Guid.NewGuid());
        try
        {
            using (var store = new Store(tempDir))
            {
                store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
                await store.BackupAsync(backupDir);
            }
            using var restored = new Store(backupDir);
            Assert.Equal(1UL, restored.Count);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
            if (Directory.Exists(backupDir)) Directory.Delete(backupDir, true);
        }
    }

    [Fact]
    public async Task Async_Update_WithCancellationToken()
    {
        using var store = new Store();
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        await store.UpdateAsync("INSERT DATA { <http://example.com/s> <http://example.com/p> \"x\" }",
            ct: cts.Token);
        Assert.Equal(1UL, store.Count);
    }

    [Fact]
    public async Task Async_Query_CanceledToken_Throws()
    {
        using var store = new Store();
        store.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        using var cts = new CancellationTokenSource();
        cts.Cancel();
        await Assert.ThrowsAsync<TaskCanceledException>(() =>
            store.QueryAsync("SELECT ?s WHERE { ?s ?p ?o }", ct: cts.Token));
    }

    private static Quad Q(string s, string p, string o) =>
        new(new NamedNode(s), new NamedNode(p), new Literal(o), new DefaultGraph());
}