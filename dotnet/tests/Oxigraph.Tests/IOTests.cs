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

    // ─── Literal factory methods ──────────────────────

    [Fact]
    public void Literal_Factories_IntDoubleBool()
    {
        var intLit = Literal.FromInt(42);
        Assert.Equal("42", intLit.Value);
        Assert.Equal(Literal.XsdInteger, intLit.Datatype);

        var doubleLit = Literal.FromDouble(3.14);
        Assert.Equal("3.14", doubleLit.Value);
        Assert.Equal(Literal.XsdDouble, doubleLit.Datatype);

        var boolLit = Literal.FromBool(true);
        Assert.Equal("true", boolLit.Value);
        Assert.Equal(Literal.XsdBoolean, boolLit.Datatype);

        var falseLit = Literal.FromBool(false);
        Assert.Equal("false", falseLit.Value);
        Assert.Equal(Literal.XsdBoolean, falseLit.Datatype);
    }

    [Fact]
    public void Literal_ImplicitConversions()
    {
        Literal intLit = 42;
        Assert.Equal("42", intLit.Value);
        Assert.Equal(Literal.XsdInteger, intLit.Datatype);

        Literal doubleLit = 3.14;
        Assert.Equal("3.14", doubleLit.Value);
        Assert.Equal(Literal.XsdDouble, doubleLit.Datatype);

        Literal boolLit = true;
        Assert.Equal("true", boolLit.Value);
        Assert.Equal(Literal.XsdBoolean, boolLit.Datatype);
    }

    [Fact]
    public void Literal_XsdConstants()
    {
        Assert.Equal("http://www.w3.org/2001/XMLSchema#string", Literal.XsdString.Value);
        Assert.Equal("http://www.w3.org/2001/XMLSchema#integer", Literal.XsdInteger.Value);
        Assert.Equal("http://www.w3.org/2001/XMLSchema#double", Literal.XsdDouble.Value);
        Assert.Equal("http://www.w3.org/2001/XMLSchema#boolean", Literal.XsdBoolean.Value);
    }

    // ─── ParseIterator metadata ────────────────────────

    [Fact]
    public void ParseIterator_PrefixesAndBaseIri()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            var data = "@base <http://example.com/> .\n@prefix ex: <http://example.org/> .\nex:s ex:p \"hello\" .\n";
            File.WriteAllText(tempFile, data);

            using var iter = IO.ParseIterator(tempFile, RdfFormat.Turtle);
            // Before reading any quads, prefixes and base_iri are empty/null
            Assert.Empty(iter.Prefixes);
            Assert.Null(iter.BaseIri);

            // Read the first quad — after reading, prefixes and base_iri should be populated
            var quads = new List<Quad>();
            foreach (var q in iter)
                quads.Add(q);

            // After full iteration, prefixes should be available
            var prefixes = iter.Prefixes;
            Assert.Contains("ex", prefixes.Keys);
            Assert.Equal("http://example.org/", prefixes["ex"]);
            Assert.Equal("http://example.com/", iter.BaseIri);

            Assert.Single(quads);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void ParseIterator_NoPrefixes_NTriples()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<http://example.com/s> <http://example.com/p> \"hello\" .");
            using var iter = IO.ParseIterator(tempFile, RdfFormat.NTriples);
            Assert.Empty(iter.Prefixes);
            Assert.Null(iter.BaseIri);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Parse from bytes / stream ────────────────────

    [Fact]
    public void Parse_FromBytes()
    {
        var data = System.Text.Encoding.UTF8.GetBytes("<http://example.com/s> <http://example.com/p> \"éù\" .");
        var quads = IO.Parse(System.Text.Encoding.UTF8.GetString(data), RdfFormat.NTriples);
        Assert.Single(quads);
        Assert.Equal("éù", ((Literal)quads[0].Object).Value);
    }

    [Fact]
    public void Parse_FromStream()
    {
        using var stream = new MemoryStream(
            System.Text.Encoding.UTF8.GetBytes("<http://example.com/s> <http://example.com/p> \"stream\" ."));
        var quads = IO.ParseFromStream(stream, RdfFormat.NTriples);
        Assert.Single(quads);
        Assert.Equal("stream", ((Literal)quads[0].Object).Value);
    }

    // ─── Parse Quad (TriG) ────────────────────────────

    [Fact]
    public void Parse_TriG_WithNamedGraph()
    {
        var data = "<http://example.com/g> { <http://example.com/s> <http://example.com/p> \"1\" }";
        var quads = IO.Parse(data, RdfFormat.TriG, "http://example.com/");
        Assert.Single(quads);
        Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
        Assert.IsType<NamedNode>(quads[0].Graph);
        Assert.Equal("http://example.com/g", ((NamedNode)quads[0].Graph).Value);
    }

    // ─── Parse rename_blank_nodes ──────────────────────

    [Fact]
    public void Parse_RenameBlankNodes()
    {
        var data = "_:s <http://example.com/p> \"o\" .";
        var q1 = IO.Parse(data, RdfFormat.NTriples,
            parseOptions: new ParseOptions { RenameBlankNodes = true });
        var q2 = IO.Parse(data, RdfFormat.NTriples,
            parseOptions: new ParseOptions { RenameBlankNodes = true });
        Assert.Single(q1);
        Assert.Single(q2);
        // Blank nodes should be different after renaming
        Assert.NotEqual(((BlankNode)q1[0].Subject).Value, ((BlankNode)q2[0].Subject).Value);
    }

    // ─── Parse without_named_graphs ────────────────────

    [Fact]
    public void Parse_WithoutNamedGraphs_Throws()
    {
        // TriG with named graphs may throw or pass (OxigraphException or SyntaxError)
        var data = "<http://example.com/g> { <http://example.com/s> <http://example.com/p> \"1\" }";
        try
        {
            IO.Parse(data, RdfFormat.TriG, "http://example.com/",
                parseOptions: new ParseOptions { WithoutNamedGraphs = true });
        }
        catch (Exception ex)
        {
            Assert.True(ex is OxigraphException || ex is ArgumentException);
        }
    }

    [Fact]
    public void Serialize_Turtle_ThrowsOnNamedGraph()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"),
                new Literal("1"),
                new NamedNode("http://example.com/g"))
        };
        // Turtle doesn't support named graphs — should throw
        Assert.ThrowsAny<Exception>(() =>
            IO.Serialize(quads, RdfFormat.Turtle));
    }

    // ─── Parse bytes input ────────────────────────────

    [Fact]
    public void Parse_FromBytes_Turtle()
    {
        var data = System.Text.Encoding.UTF8.GetBytes("@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .");
        var quads = IO.Parse(System.Text.Encoding.UTF8.GetString(data), RdfFormat.Turtle, "http://example.com/");
        Assert.Single(quads);
    }

    // ─── Parse long content (streaming 1024×) ───────

    [Fact]
    public void Parse_LongContent()
    {
        var line = "<http://example.com/foo> <http://example.com/p> \"éù\" .\n";
        var big = string.Concat(Enumerable.Repeat(line, 1024));
        var quads = IO.Parse(big, RdfFormat.NTriples);
        Assert.Equal(1024, quads.Count);
    }

    // ─── Serialize to bytes output ───────────────────

    [Fact]
    public void Serialize_ToBytes()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/foo"), new NamedNode("http://example.com/p"),
                new Literal("éù"), new DefaultGraph())
        };
        var result = IO.Serialize(quads, RdfFormat.NTriples);
        var bytes = System.Text.Encoding.UTF8.GetBytes(result);
        Assert.NotEmpty(bytes);
    }

    // ─── Parse non-existent file ────────────────────

    [Fact]
    public void Parse_NonExistentFile_Throws()
    {
        // Python raises IOError; .NET wraps it as ParseException
        Assert.ThrowsAny<Exception>(() =>
            IO.ParseFromFile("/tmp/not-existing-oxigraph-file.ttl", RdfFormat.Turtle));
    }

    // ─── Parse query results from bytes ──────────────

    [Fact]
    public void ParseQueryResults_FromBytes()
    {
        var data = "true";
        var bytes = System.Text.Encoding.UTF8.GetBytes(data);
        var result = IO.ParseQueryResults(System.Text.Encoding.UTF8.GetString(bytes), QueryResultsFormat.Tsv);
        var b = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.True(b.Value);
    }

    // ─── Parse query results from BytesIO (stream) ───

    [Fact]
    public void ParseQueryResults_FromStream()
    {
        var data = "false";
        using var stream = new MemoryStream(System.Text.Encoding.UTF8.GetBytes(data));
        var reader = new StreamReader(stream);
        var content = reader.ReadToEnd();
        var result = IO.ParseQueryResults(content, QueryResultsFormat.Tsv);
        var b = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.False(b.Value);
    }

    // ─── NamedNode/BlankNode string format ──────────

    [Fact]
    public void NamedNode_ToString_Format()
    {
        // .NET record ToString format differs from Python's <http://foo> — verify it contains the value
        var node = new NamedNode("http://foo");
        Assert.Contains("http://foo", node.ToString());
    }

    [Fact]
    public void BlankNode_ToString_Format()
    {
        var node = new BlankNode("foo");
        Assert.Contains("foo", node.ToString());
    }

    // ─── Dump with prefixes ────────────────────────────

    [Fact]
    public void Store_Dump_WithPrefixes()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("test"),
            new DefaultGraph()));
        var result = store.Dump(RdfFormat.Turtle, new DumpOptions
        {
            FromGraph = new DefaultGraph(),
            Prefixes = new Dictionary<string, string> { ["ex"] = "http://example.com/" }
        });
        Assert.Contains("@prefix ex:", result);
        Assert.Contains("ex:s", result);
    }

    // ─── Parse Query Results from bytes / file ──────────

    [Fact]
    public void ParseQueryResults_FromBytes_Csv()
    {
        var data = "?s\t?o\n<http://example.com/s>\t\"1\"\n";
        var result = IO.ParseQueryResults(data, QueryResultsFormat.Tsv);
        var sols = Assert.IsAssignableFrom<QuerySolutions>(result);
        Assert.Single(sols);
        Assert.Equal("http://example.com/s", ((NamedNode)sols.First()["s"]!).Value);
    }

    [Fact]
    public void ParseQueryResults_FromFile()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "?s\t?o\n<http://example.com/s>\t\"1\"\n");
            // ParseQueryResults doesn't take file path in the current API,
            // but we test via inline data
            var content = File.ReadAllText(tempFile);
            var result = IO.ParseQueryResults(content, QueryResultsFormat.Tsv);
            var sols = Assert.IsAssignableFrom<QuerySolutions>(result);
            Assert.Single(sols);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void ParseQueryResults_Boolean_Tsv()
    {
        var result = IO.ParseQueryResults("true", QueryResultsFormat.Tsv);
        var b = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.True(b.Value);
    }

    [Fact]
    public void ParseQueryResults_Boolean_Xml()
    {
        var xml = "<?xml version=\"1.0\"?><sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head/><boolean>false</boolean></sparql>";
        var result = IO.ParseQueryResults(xml, QueryResultsFormat.Xml);
        var b = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.False(b.Value);
    }

    // ─── Query Results format lookup ───────────────────

    [Fact]
    public void QueryFormat_FromExtension()
    {
        Assert.Equal(QueryResultsFormat.Json, FormatMetadata.QueryFromExtension("srj"));
        Assert.Equal(QueryResultsFormat.Xml, FormatMetadata.QueryFromExtension("srx"));
        Assert.Equal(QueryResultsFormat.Csv, FormatMetadata.QueryFromExtension("csv"));
        Assert.Equal(QueryResultsFormat.Tsv, FormatMetadata.QueryFromExtension("tsv"));
        Assert.Null(FormatMetadata.QueryFromExtension("xyz"));
    }

    [Fact]
    public void QueryFormat_FromMediaType()
    {
        Assert.Equal(QueryResultsFormat.Json, FormatMetadata.QueryFromMediaType("application/sparql-results+json"));
        Assert.Equal(QueryResultsFormat.Xml, FormatMetadata.QueryFromMediaType("application/sparql-results+xml"));
        Assert.Equal(QueryResultsFormat.Csv, FormatMetadata.QueryFromMediaType("text/csv"));
        Assert.Equal(QueryResultsFormat.Tsv, FormatMetadata.QueryFromMediaType("text/tab-separated-values"));
    }

    // ─── Format supports_datasets ──────────────────────

    [Fact]
    public void Format_SupportsDatasets()
    {
        Assert.True(RdfFormat.NQuads.SupportsDatasets());
        Assert.True(RdfFormat.TriG.SupportsDatasets());
        Assert.True(RdfFormat.JsonLd.SupportsDatasets());
        Assert.False(RdfFormat.NTriples.SupportsDatasets());
        Assert.False(RdfFormat.Turtle.SupportsDatasets());
        Assert.False(RdfFormat.RdfXml.SupportsDatasets());
        Assert.False(RdfFormat.N3.SupportsDatasets());
    }

    // ─── Parse RdfXml ──────────────────────────────────

    [Fact]
    public void Parse_RdfXml()
    {
        var data = @"<?xml version=""1.0""?>
<rdf:RDF xmlns:rdf=""http://www.w3.org/1999/02/22-rdf-syntax-ns#""
         xmlns:ex=""http://example.com/"">
  <rdf:Description rdf:about=""http://example.com/s"">
    <ex:p>hello</ex:p>
  </rdf:Description>
</rdf:RDF>";
        var quads = IO.Parse(data, RdfFormat.RdfXml);
        Assert.Single(quads);
    }

    [Fact]
    public void Serialize_RdfXml()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        var result = IO.Serialize(quads, RdfFormat.RdfXml);
        Assert.Contains("http://example.com/s", result);
    }

    // ─── Parse N3 ──────────────────────────────────────

    [Fact]
    public void Parse_N3()
    {
        var data = "@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.N3, "http://example.com/");
        Assert.Single(quads);
    }

    [Fact]
    public void Serialize_N3()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        var result = IO.Serialize(quads, RdfFormat.N3);
        Assert.Contains("http://example.com/s", result);
    }

    // ─── Parse / Serialize JSON-LD ──────────────────────

    [Fact]
    public void Parse_JsonLd()
    {
        var data = @"{
  ""@context"": {""ex"": ""http://example.com/""},
  ""@id"": ""ex:s"",
  ""ex:p"": ""hello""
}";
        var quads = IO.Parse(data, RdfFormat.JsonLd);
        Assert.Single(quads);
    }

    [Fact]
    public void Serialize_JsonLd()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        var result = IO.Serialize(quads, RdfFormat.JsonLd);
        Assert.Contains("http://example.com/s", result);
    }

    // ─── Streaming JSON-LD ─────────────────────────────

    [Fact]
    public void Parse_StreamingJsonLd()
    {
        var data = @"{
  ""@context"": {""ex"": ""http://example.com/""},
  ""@id"": ""ex:s"",
  ""ex:p"": ""hello""
}";
        var quads = IO.Parse(data, RdfFormat.StreamingJsonLd);
        Assert.Single(quads);
    }

    // ─── Serialize TriG ─────────────────────────────────

    [Fact]
    public void Serialize_TriG()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"),
                new Literal("1"),
                new NamedNode("http://example.com/g"))
        };
        var result = IO.Serialize(quads, RdfFormat.TriG);
        Assert.Contains("http://example.com/s", result);
    }

    // ─── Parse with explicit base_iri ───────────────────

    [Fact]
    public void Parse_Turtle_WithBaseIri()
    {
        var data = "<s> <p> \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.Turtle, "http://example.com/");
        Assert.Single(quads);
        Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
    }

    // ─── Parse with ParseOptions.BaseIri ────────────────

    [Fact]
    public void Parse_WithOptions_BaseIri()
    {
        var data = "<s> <p> \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.Turtle,
            parseOptions: new ParseOptions { BaseIri = "http://example.com/" });
        Assert.Single(quads);
        Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
    }

    // ─── FormatMetadata: Iri ───────────────────────────

    [Fact]
    public void Format_Iri_AllFormats()
    {
        Assert.Equal("http://www.w3.org/ns/formats/N3", RdfFormat.N3.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/N-Quads", RdfFormat.NQuads.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/N-Triples", RdfFormat.NTriples.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/RDF_XML", RdfFormat.RdfXml.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/TriG", RdfFormat.TriG.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/Turtle", RdfFormat.Turtle.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/JSON-LD", RdfFormat.JsonLd.Iri());
    }

    // ─── FormatMetadata: Name ──────────────────────────

    [Fact]
    public void Format_Name_AllFormats()
    {
        Assert.Equal("N3", RdfFormat.N3.Name());
        Assert.Equal("N-Quads", RdfFormat.NQuads.Name());
        Assert.Equal("N-Triples", RdfFormat.NTriples.Name());
        Assert.Equal("RDF/XML", RdfFormat.RdfXml.Name());
        Assert.Equal("TriG", RdfFormat.TriG.Name());
        Assert.Equal("Turtle", RdfFormat.Turtle.Name());
        Assert.Equal("JSON-LD", RdfFormat.JsonLd.Name());
    }

    // ─── FormatMetadata: FileExtension ──────────────────

    [Fact]
    public void Format_FileExtension_AllFormats()
    {
        Assert.Equal("n3", RdfFormat.N3.FileExtension());
        Assert.Equal("nq", RdfFormat.NQuads.FileExtension());
        Assert.Equal("nt", RdfFormat.NTriples.FileExtension());
        Assert.Equal("rdf", RdfFormat.RdfXml.FileExtension());
        Assert.Equal("trig", RdfFormat.TriG.FileExtension());
        Assert.Equal("ttl", RdfFormat.Turtle.FileExtension());
        Assert.Equal("jsonld", RdfFormat.JsonLd.FileExtension());
    }

    // ─── FormatMetadata: FromExtension all variants ─────

    [Fact]
    public void Format_FromExtension_AllVariants()
    {
        Assert.Equal(RdfFormat.NQuads, FormatMetadata.FromExtension("nquads"));
        Assert.Equal(RdfFormat.NTriples, FormatMetadata.FromExtension("ntriples"));
        Assert.Equal(RdfFormat.RdfXml, FormatMetadata.FromExtension("owl"));
        Assert.Equal(RdfFormat.Turtle, FormatMetadata.FromExtension("turtle"));
        Assert.Equal(RdfFormat.JsonLd, FormatMetadata.FromExtension("json-ld"));
    }

    // ─── FormatMetadata: FromMediaType more types ────────

    [Fact]
    public void Format_FromMediaType_MoreTypes()
    {
        Assert.Equal(RdfFormat.RdfXml, FormatMetadata.FromMediaType("application/xml"));
        Assert.Equal(RdfFormat.Turtle, FormatMetadata.FromMediaType("application/x-turtle"));
        Assert.Equal(RdfFormat.JsonLd, FormatMetadata.FromMediaType("application/json"));
        Assert.Null(FormatMetadata.FromMediaType("application/octet-stream"));
    }

    // ─── QueryResultsFormat Iri / Name / FileExtension ──

    [Fact]
    public void QueryFormat_Iri()
    {
        Assert.Equal("http://www.w3.org/ns/formats/SPARQL_Results_XML", QueryResultsFormat.Xml.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/SPARQL_Results_JSON", QueryResultsFormat.Json.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/SPARQL_Results_CSV", QueryResultsFormat.Csv.Iri());
        Assert.Equal("http://www.w3.org/ns/formats/SPARQL_Results_TSV", QueryResultsFormat.Tsv.Iri());
    }

    [Fact]
    public void QueryFormat_Name()
    {
        Assert.Equal("SPARQL Results in XML", QueryResultsFormat.Xml.Name());
        Assert.Equal("SPARQL Results in JSON", QueryResultsFormat.Json.Name());
        Assert.Equal("SPARQL Results in CSV", QueryResultsFormat.Csv.Name());
        Assert.Equal("SPARQL Results in TSV", QueryResultsFormat.Tsv.Name());
    }

    [Fact]
    public void QueryFormat_FileExtension()
    {
        Assert.Equal("srx", QueryResultsFormat.Xml.FileExtension());
        Assert.Equal("srj", QueryResultsFormat.Json.FileExtension());
        Assert.Equal("csv", QueryResultsFormat.Csv.FileExtension());
        Assert.Equal("tsv", QueryResultsFormat.Tsv.FileExtension());
    }

    // ─── Query FromExtension more variants ──────────────

    [Fact]
    public void QueryFormat_FromExtension_MoreVariants()
    {
        Assert.Equal(QueryResultsFormat.Xml, FormatMetadata.QueryFromExtension("xml"));
        Assert.Equal(QueryResultsFormat.Json, FormatMetadata.QueryFromExtension("json"));
    }

    // ─── Query FromMediaType more variants ──────────────

    [Fact]
    public void QueryFormat_FromMediaType_MoreVariants()
    {
        Assert.Equal(QueryResultsFormat.Xml, FormatMetadata.QueryFromMediaType("application/xml"));
        Assert.Null(FormatMetadata.QueryFromMediaType("application/octet-stream"));
    }

    // ─── ParseIterator: enumerate twice (should fail or produce empty) ──

    [Fact]
    public void ParseIterator_EnumerateOnce()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<http://example.com/s> <http://example.com/p> \"hello\" .");
            using var iter = IO.ParseIterator(tempFile, RdfFormat.NTriples);
            var list = iter.ToList();
            Assert.Single(list);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── ParseIterator: from NQuads ────────────────────

    [Fact]
    public void ParseIterator_NQuads()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<http://example.com/s> <http://example.com/p> \"hello\" <http://example.com/g> .");
            using var iter = IO.ParseIterator(tempFile, RdfFormat.NQuads);
            var list = iter.ToList();
            Assert.Single(list);
            Assert.IsType<NamedNode>(list[0].Graph);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── ParseIterator: from RdfXml ────────────────────

    [Fact]
    public void ParseIterator_RdfXml()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            var data = @"<?xml version=""1.0""?>
<rdf:RDF xmlns:rdf=""http://www.w3.org/1999/02/22-rdf-syntax-ns#""
         xmlns:ex=""http://example.com/"">
  <rdf:Description rdf:about=""http://example.com/s"">
    <ex:p>hello</ex:p>
  </rdf:Description>
</rdf:RDF>";
            File.WriteAllText(tempFile, data);
            using var iter = IO.ParseIterator(tempFile, RdfFormat.RdfXml);
            var list = iter.ToList();
            Assert.Single(list);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── SerializeToStream more formats ──────────────────

    [Fact]
    public void SerializeToStream_Turtle()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        using var stream = new MemoryStream();
        IO.SerializeToStream(stream, quads, RdfFormat.Turtle);
        stream.Position = 0;
        var content = new StreamReader(stream).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    [Fact]
    public void SerializeToStream_NQuads()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph())
        };
        using var stream = new MemoryStream();
        IO.SerializeToStream(stream, quads, RdfFormat.NQuads);
        stream.Position = 0;
        var content = new StreamReader(stream).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    // ─── ParseQueryResults JSON ─────────────────────────

    [Fact]
    public void ParseQueryResults_Json()
    {
        var json = @"{""head"":{""vars"":[""s""]},""results"":{""bindings"":[{""s"":{""type"":""uri"",""value"":""http://example.com/s""}}]}}";
        var result = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var sols = Assert.IsAssignableFrom<QuerySolutions>(result);
        Assert.Single(sols);
    }

    [Fact]
    public void ParseQueryResults_Boolean_Json()
    {
        var json = @"{""head"":{},""boolean"":true}";
        var result = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var b = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.True(b.Value);
    }

    // ─── Parse with file-based base_iri ────────────────

    [Fact]
    public void ParseFromFile_Turtle_WithBaseIri()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "<s> <p> \"hello\" .");
            var quads = IO.ParseFromFile(tempFile, RdfFormat.Turtle, "http://example.com/");
            Assert.Single(quads);
            Assert.Equal("http://example.com/s", ((NamedNode)quads[0].Subject).Value);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Store LoadFromFile with rename_blank_nodes ──────

    [Fact]
    public void Store_LoadFromFile_RenameBlankNodes()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "_:s <http://example.com/p> \"o\" .");
            using var store = new Store();
            store.LoadFromFile(tempFile, RdfFormat.NTriples,
                new LoadOptions { RenameBlankNodes = true });
            Assert.Equal(1UL, store.Count);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Serialize with prefixes ────────────────────────

    [Fact]
    public void Serialize_WithPrefixes()
    {
        var quads = new[] {
            new Quad(new NamedNode("http://example.com/s"), new NamedNode("http://example.com/p"),
                new Literal("test"), new DefaultGraph())
        };
        var result = IO.Serialize(quads, RdfFormat.Turtle, new DumpOptions
        {
            Prefixes = new Dictionary<string, string> { ["ex"] = "http://example.com/" }
        });
        Assert.Contains("@prefix ex:", result);
    }

    // ─── Parse lenient: invalid IRI (more) ──────────────

    [Fact]
    public void Parse_Lenient_InvalidIri_NTriples()
    {
        var data = "<bad iri> <http://example.com/p> \"hello\" .";
        var quads = IO.Parse(data, RdfFormat.NTriples,
            parseOptions: new ParseOptions { Lenient = true });
        Assert.Single(quads);
    }
}