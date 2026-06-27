namespace Oxigraph.Tests;

public class SparqlTests
{
    [Fact]
    public void Select_All_Quads()
    {
        using var store = new Store();
        var q1 = new Quad(
            new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph());
        var q2 = new Quad(
            new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"),
            new Literal("world"),
            new DefaultGraph());
        store.Add(q1);
        store.Add(q2);

        var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");

        var solutions = Assert.IsType<QuerySolutions>(results);
        Assert.Equal(2, solutions.Count);
        Assert.Contains(solutions.Variables, v => v.Value == "s");
        Assert.Contains(solutions.Variables, v => v.Value == "o");
    }

    [Fact]
    public void Insert_Data_Update()
    {
        using var store = new Store();
        store.Update(
            "INSERT DATA { <http://example.com/s> <http://example.com/p> \"test\" }");

        Assert.Equal(1UL, store.Count);
    }

    [Fact]
    public void Ask_Query()
    {
        using var store = new Store();
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("test"),
            new DefaultGraph());
        store.Add(q);

        var results = store.Query("ASK { ?s ?p ?o }");
        var boolean = Assert.IsType<QueryBoolean>(results);
        Assert.True(boolean.Value);
    }

    [Fact]
    public void Construct_Query()
    {
        using var store = new Store();
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("test"),
            new DefaultGraph());
        store.Add(q);

        var results = store.Query(
            "CONSTRUCT { ?s <http://example.com/new> ?o } WHERE { ?s ?p ?o }");

        var triples = Assert.IsType<QueryTriples>(results);
        Assert.Single(triples);
        Assert.Equal("http://example.com/new", triples.First().Predicate.Value);
    }

    [Fact]
    public void Delete_Where_Update()
    {
        using var store = new Store();
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("test"),
            new DefaultGraph());
        store.Add(q);

        store.Update(
            "DELETE WHERE { <http://example.com/s> ?p ?o }");

        Assert.Equal(0UL, store.Count);
    }

    [Fact]
    public void Custom_Function_Register_Only()
    {
        // Just verify registration doesn't crash
        CustomFunctions.Register("http://example.com/suffix", args =>
            new Literal(((Literal)args[0]).Value + "_suffix"));
        CustomFunctions.Unregister("http://example.com/suffix");
    }

    [Fact]
    public void Custom_Function()
    {
        CustomFunctions.Register("http://example.com/suffix", args =>
            new Literal(((Literal)args[0]).Value + "_suffix"));

        try
        {
            using var store = new Store();
            store.Add(new Quad(
                new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"),
                new Literal("hello"),
                new DefaultGraph()));

            var results = store.Query(@"
                PREFIX my: <http://example.com/>
                SELECT ?result WHERE {
                    ?s ?p ?o .
                    BIND(my:suffix(?o) AS ?result)
                }");
            var solutions = Assert.IsType<QuerySolutions>(results);
            Assert.Single(solutions);
            var val = solutions.First()["result"];
            Assert.Equal("hello_suffix", ((Literal)val!).Value);
        }
        finally
        {
            CustomFunctions.Unregister("http://example.com/suffix");
        }
    }

    // ─── Query Results Serialization ─────────────────

    [Fact]
    public void QueryBoolean_SerializeToFile_Roundtrip()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var result = store.Query("ASK { ?s ?p ?o }");
            var boolean = Assert.IsType<QueryBoolean>(result);
            Assert.True(boolean.Value);

            boolean.SerializeToFile(tempFile, QueryResultsFormat.Json);
            var content = File.ReadAllText(tempFile);
            Assert.Contains("true", content);

            // Parse back
            var parsed = IO.ParseQueryResults(content, QueryResultsFormat.Json);
            var parsedBool = Assert.IsType<QueryBoolean>(parsed);
            Assert.True(parsedBool.Value);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void QuerySolutions_SerializeToFile_Roundtrip()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var result = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
            var solutions = Assert.IsType<QuerySolutions>(result);

            solutions.SerializeToFile(tempFile, QueryResultsFormat.Json);
            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);

            // Parse back
            var parsed = IO.ParseQueryResults(content, QueryResultsFormat.Json);
            var parsedSols = Assert.IsType<QuerySolutions>(parsed);
            Assert.Single(parsedSols);
        }
        finally { File.Delete(tempFile); }
    }

    [Fact]
    public void QueryTriples_SerializeToFile_Roundtrip()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var result = store.Query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
            var triples = Assert.IsType<QueryTriples>(result);

            triples.SerializeToFile(tempFile, RdfFormat.NTriples);
            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);

            // Parse back
            var parsed = IO.ParseFromFile(tempFile, RdfFormat.NTriples);
            Assert.Single(parsed);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── SPARQL Substitutions ────────────────────────

    [Fact]
    public void Substitutions_FilterByVariable()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"),
            new Literal("a"),
            new DefaultGraph()));
        store.Add(new Quad(
            new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"),
            new Literal("b"),
            new DefaultGraph()));

        // Substitute ?s with s1 — should only return the first quad's object
        var results = store.Query(
            "SELECT ?s ?o WHERE { ?s ?p ?o }",
            new QueryOptions
            {
                Substitutions = new Dictionary<string, ITerm>
                {
                    ["s"] = new NamedNode("http://example.com/s1")
                }
            });

        var solutions = Assert.IsType<QuerySolutions>(results);
        Assert.Single(solutions);
        Assert.Equal("a", ((Literal)solutions.First()["o"]!).Value);
    }

    // ─── Custom Aggregate ────────────────────────────

    [Fact]
    public void Custom_Aggregate_CountValues()
    {
        // Simple accumulator that counts values
        var acc = new CountAggregate();
        CustomFunctions.RegisterAggregate("http://example.com/myCount",
            () => new CountAggregate());

        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s1"),
                new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
            store.Add(new Quad(new NamedNode("http://example.com/s2"),
                new NamedNode("http://example.com/p"), new Literal("b"), new DefaultGraph()));
            store.Add(new Quad(new NamedNode("http://example.com/s3"),
                new NamedNode("http://example.com/p"), new Literal("c"), new DefaultGraph()));

            var results = store.Query(@"
                PREFIX my: <http://example.com/>
                SELECT (my:myCount(?o) AS ?cnt) WHERE { ?s ?p ?o }");

            var sols = Assert.IsType<QuerySolutions>(results);
            Assert.Single(sols);
            var cnt = ((Literal)sols.First()["cnt"]!).Value;
            Assert.Equal("3", cnt);
        }
        finally
        {
            CustomFunctions.UnregisterAggregate("http://example.com/myCount");
        }
    }

    private sealed class CountAggregate : CustomFunctions.IAggregateAccumulator
    {
        private int _count;
        public void Accumulate(ITerm term) => _count++;
        public ITerm? Finish() => new Literal(_count.ToString());
    }

    // ─── Custom Functions in UPDATE ──────────────────

    [Fact]
    public void Custom_Function_In_Update()
    {
        CustomFunctions.Register("http://example.com/upper",
            args => new Literal(((Literal)args[0]).Value.ToUpperInvariant()));

        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"),
                new Literal("hello"), new DefaultGraph()));

            // Use custom function in INSERT to transform values
            store.Update(@"
                PREFIX my: <http://example.com/>
                INSERT { ?s <http://example.com/new> ?upper }
                WHERE  { ?s ?p ?o . BIND(my:upper(?o) AS ?upper) }",
                new UpdateOptions
                {
                    CustomFunctions = new() {
                        ["http://example.com/upper"] = args =>
                            new Literal(((Literal)args[0]).Value.ToUpperInvariant())
                    }
                });

            Assert.Equal(2UL, store.Count);
            // Verify the transformed value exists
            var results = store.Query(
                "SELECT ?v WHERE { ?s <http://example.com/new> ?v }");
            var sols = Assert.IsType<QuerySolutions>(results);
            Assert.Single(sols);
            Assert.Equal("HELLO", ((Literal)sols.First()["v"]!).Value);
        }
        finally
        {
            CustomFunctions.Unregister("http://example.com/upper");
        }
    }

    // ─── Query Results Serialize to Buffer/Stream ──────

    [Fact]
    public void QueryBoolean_Serialize_ToBuffer()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var result = store.Query("ASK { ?s ?p ?o }");
        var boolean = Assert.IsType<QueryBoolean>(result);
        Assert.True(boolean.Value);

        var json = boolean.Serialize(QueryResultsFormat.Json);
        Assert.Contains("true", json);

        // Parse back
        var parsed = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var parsedBool = Assert.IsType<QueryBoolean>(parsed);
        Assert.True(parsedBool.Value);
    }

    [Fact]
    public void QueryBoolean_SerializeToStream()
    {
        using var store = new Store();
        var result = store.Query("ASK { FILTER(false) }");
        var boolean = Assert.IsType<QueryBoolean>(result);
        Assert.False(boolean.Value);

        using var stream = new MemoryStream();
        boolean.SerializeToStream(stream, QueryResultsFormat.Json);

        stream.Position = 0;
        var reader = new StreamReader(stream);
        var content = reader.ReadToEnd();
        Assert.Contains("false", content);
    }

    [Fact]
    public void QuerySolutions_Serialize_ToBuffer_Roundtrip()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var result = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        var solutions = Assert.IsType<QuerySolutions>(result);

        var json = solutions.Serialize(QueryResultsFormat.Json);
        Assert.Contains("http://example.com/s", json);

        // Parse back
        var parsed = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var parsedSols = Assert.IsType<QuerySolutions>(parsed);
        Assert.Single(parsedSols);
        Assert.Equal("http://example.com/s", ((NamedNode)parsedSols.First()["s"]!).Value);
    }

    [Fact]
    public void QuerySolutions_SerializeToStream_Roundtrip()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var result = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        var solutions = Assert.IsType<QuerySolutions>(result);

        using var stream = new MemoryStream();
        solutions.SerializeToStream(stream, QueryResultsFormat.Json);

        stream.Position = 0;
        var reader = new StreamReader(stream);
        var content = reader.ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    [Fact]
    public void QueryTriples_Serialize_ToBuffer_Roundtrip()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var result = store.Query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
        var triples = Assert.IsType<QueryTriples>(result);

        var ntriples = triples.Serialize(RdfFormat.NTriples);
        Assert.Contains("http://example.com/s", ntriples);

        // Parse back
        var parsed = IO.Parse(ntriples, RdfFormat.NTriples);
        Assert.Single(parsed);
    }

    [Fact]
    public void QueryTriples_SerializeToStream()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var result = store.Query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
        var triples = Assert.IsType<QueryTriples>(result);

        using var stream = new MemoryStream();
        triples.SerializeToStream(stream, RdfFormat.NTriples);

        stream.Position = 0;
        var content = new StreamReader(stream).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }
}