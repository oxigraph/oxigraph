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

    // ─── File-based I/O tests ───────────────────────

    [Fact]
    public void ParseFromFile_Turtle()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .");
            var quads = IO.ParseFromFile(tempFile, RdfFormat.Turtle, "http://example.com/");
            Assert.Single(quads);
            Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void SerializeToFile_Roundtrip()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        var tempFile = Path.GetTempFileName();
        try
        {
            IO.SerializeToFile(tempFile, quads, RdfFormat.NTriples);
            var result = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", result);

            // Parse back
            var parsed = IO.ParseFromFile(tempFile, RdfFormat.NTriples);
            Assert.Single(parsed);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void Store_LoadFromFile_DumpToFile_Roundtrip()
    {
        var tempFile = Path.GetTempFileName();
        var dumpFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .");
            using var store = new Store();
            store.LoadFromFile(tempFile, RdfFormat.Turtle, new LoadOptions { BaseIri = "http://example.com/" });
            Assert.Equal(1UL, store.Count);

            store.DumpToFile(dumpFile, RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });
            var dumped = File.ReadAllText(dumpFile);
            Assert.Contains("http://example.com/s", dumped);
            Assert.Contains("http://example.com/p", dumped);
        }
        finally { File.Delete(tempFile); File.Delete(dumpFile); }
    }

    [Fact]
    public void Store_BulkLoadFromFile_LoadsData()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            // Write a larger file with multiple quads
            var lines = string.Join("\n", Enumerable.Range(0, 10).Select(i =>
                $"<http://example.com/s{i}> <http://example.com/p> \"value{i}\" ."));
            File.WriteAllText(tempFile, lines);

            using var store = new Store();
            store.BulkLoadFromFile(tempFile, RdfFormat.NTriples);
            Assert.Equal(10UL, store.Count);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void Store_LoadFromFile_WithGraph()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<http://example.com/s> <http://example.com/p> \"hello\" .");
            using var store = new Store();
            var targetGraph = new NamedNode("http://example.com/g");
            store.LoadFromFile(tempFile, RdfFormat.NTriples,
                new LoadOptions { ToGraph = targetGraph });

            var results = store.Match();
            Assert.Single(results);
            Assert.IsType<NamedNode>(results[0].Graph);
            Assert.Equal("http://example.com/g", ((NamedNode)results[0].Graph).Value);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Stream-based I/O tests ─────────────────────

    [Fact]
    public void ParseFromStream_MemoryStream()
    {
        var data = "<http://example.com/s> <http://example.com/p> \"hello\" .";
        using var stream = new MemoryStream(System.Text.Encoding.UTF8.GetBytes(data));
        var quads = IO.ParseFromStream(stream, RdfFormat.NTriples);
        Assert.Single(quads);
        Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
    }

    [Fact]
    public void SerializeToStream_Roundtrip()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        using var stream = new MemoryStream();
        IO.SerializeToStream(stream, quads, RdfFormat.NTriples);

        stream.Position = 0;
        var parsed = IO.ParseFromStream(stream, RdfFormat.NTriples);
        Assert.Single(parsed);
    }

    [Fact]
    public void Store_LoadFromStream_DumpToStream_Roundtrip()
    {
        var data = "<http://example.com/s1> <http://example.com/p> \"a\" .\n<http://example.com/s2> <http://example.com/p> \"b\" .\n";
        using var input = new MemoryStream(System.Text.Encoding.UTF8.GetBytes(data));

        using var store = new Store();
        store.LoadFromStream(input, RdfFormat.NTriples);
        Assert.Equal(2UL, store.Count);

        using var output = new MemoryStream();
        store.DumpToStream(output, RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });

        output.Position = 0;
        var reader = new StreamReader(output);
        var dumped = reader.ReadToEnd();
        Assert.Contains("http://example.com/s1", dumped);
        Assert.Contains("http://example.com/s2", dumped);
    }

    [Fact]
    public void Store_LoadFromStream_EmptyStream_NoQuads()
    {
        using var stream = new MemoryStream([]);
        using var store = new Store();
        store.LoadFromStream(stream, RdfFormat.NTriples);
        Assert.Equal(0UL, store.Count);
    }

    // ─── Iterator & Query Results tests ──────────────

    [Fact]
    public void ParseIterator_LazyFileParsing()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            var lines = string.Join("\n", Enumerable.Range(0, 10).Select(i =>
                $"<http://example.com/s{i}> <http://example.com/p> \"value{i}\" ."));
            File.WriteAllText(tempFile, lines);

            using var iter = IO.ParseIterator(tempFile, RdfFormat.NTriples);
            int count = 0;
            foreach (var q in iter)
                count++;
            Assert.Equal(10, count);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void Format_Metadata()
    {
        Assert.Equal("text/turtle", RdfFormat.Turtle.MediaType());
        Assert.Equal("ttl", RdfFormat.Turtle.FileExtension());
        Assert.Equal("application/n-triples", RdfFormat.NTriples.MediaType());
        Assert.Equal("application/ld+json", RdfFormat.JsonLd.MediaType());
        Assert.Equal("application/sparql-results+json", QueryResultsFormat.Json.MediaType());
        Assert.Equal("application/sparql-results+xml", QueryResultsFormat.Xml.MediaType());
    }

    [Fact]
    public void Format_FromExtension()
    {
        Assert.Equal(RdfFormat.Turtle, FormatMetadata.FromExtension("ttl"));
        Assert.Equal(RdfFormat.NTriples, FormatMetadata.FromExtension("nt"));
        Assert.Equal(RdfFormat.NQuads, FormatMetadata.FromExtension("nq"));
        Assert.Null(FormatMetadata.FromExtension("xyz"));
    }

    [Fact]
    public void Format_FromMediaType()
    {
        Assert.Equal(RdfFormat.Turtle, FormatMetadata.FromMediaType("text/turtle"));
        Assert.Equal(RdfFormat.JsonLd, FormatMetadata.FromMediaType("application/ld+json"));
        Assert.Equal(QueryResultsFormat.Json, FormatMetadata.QueryFromMediaType("application/sparql-results+json"));
    }

    [Fact]
    public void Parse_Lenient_InvalidIri()
    {
        // Lenient mode should allow technically invalid IRIs
        var data = "<not a valid iri> <http://example.com/p> \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.NTriples,
            parseOptions: new ParseOptions { Lenient = true });
        Assert.Single(quads);
    }
}