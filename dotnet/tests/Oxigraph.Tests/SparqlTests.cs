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

        var solutions = Assert.IsAssignableFrom<QuerySolutions>(results);
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
        var boolean = Assert.IsAssignableFrom<QueryBoolean>(results);
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

        var triples = Assert.IsAssignableFrom<QueryTriples>(results);
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
            var solutions = Assert.IsAssignableFrom<QuerySolutions>(results);
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
            var boolean = Assert.IsAssignableFrom<QueryBoolean>(result);
            Assert.True(boolean.Value);

            boolean.SerializeToFile(tempFile, QueryResultsFormat.Json);
            var content = File.ReadAllText(tempFile);
            Assert.Contains("true", content);

            // Parse back
            var parsed = IO.ParseQueryResults(content, QueryResultsFormat.Json);
            var parsedBool = Assert.IsAssignableFrom<QueryBoolean>(parsed);
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
            var solutions = Assert.IsAssignableFrom<QuerySolutions>(result);

            solutions.SerializeToFile(tempFile, QueryResultsFormat.Json);
            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);

            // Parse back
            var parsed = IO.ParseQueryResults(content, QueryResultsFormat.Json);
            var parsedSols = Assert.IsAssignableFrom<QuerySolutions>(parsed);
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
            var triples = Assert.IsAssignableFrom<QueryTriples>(result);

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

        var solutions = Assert.IsAssignableFrom<QuerySolutions>(results);
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

            var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
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
            var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
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
        var boolean = Assert.IsAssignableFrom<QueryBoolean>(result);
        Assert.True(boolean.Value);

        var json = boolean.Serialize(QueryResultsFormat.Json);
        Assert.Contains("true", json);

        // Parse back
        var parsed = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var parsedBool = Assert.IsAssignableFrom<QueryBoolean>(parsed);
        Assert.True(parsedBool.Value);
    }

    [Fact]
    public void QueryBoolean_SerializeToStream()
    {
        using var store = new Store();
        var result = store.Query("ASK { FILTER(false) }");
        var boolean = Assert.IsAssignableFrom<QueryBoolean>(result);
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
        var solutions = Assert.IsAssignableFrom<QuerySolutions>(result);

        var json = solutions.Serialize(QueryResultsFormat.Json);
        Assert.Contains("http://example.com/s", json);

        // Parse back
        var parsed = IO.ParseQueryResults(json, QueryResultsFormat.Json);
        var parsedSols = Assert.IsAssignableFrom<QuerySolutions>(parsed);
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
        var solutions = Assert.IsAssignableFrom<QuerySolutions>(result);

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
        var triples = Assert.IsAssignableFrom<QueryTriples>(result);

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
        var triples = Assert.IsAssignableFrom<QueryTriples>(result);

        using var stream = new MemoryStream();
        triples.SerializeToStream(stream, RdfFormat.NTriples);

        stream.Position = 0;
        var content = new StreamReader(stream).ReadToEnd();
        Assert.Contains("http://example.com/s", content);
    }

    // ─── RDF-star ─────────────────────────────────────

    [Fact]
    public void RdfStar_Insert_And_Query()
    {
        using var store = new Store();
        store.Update("PREFIX : <http://www.example.org/> " +
            "INSERT DATA { :alice :claims << :bob :age 23 >> }");
        var results = store.Query(
            "PREFIX : <http://www.example.org/> SELECT ?p ?a WHERE { ?p :claims << :bob :age ?a >> }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Single(sols);
    }

    // ─── Query: use_default_graph_as_union ─────────────

    [Fact]
    public void Select_Union_Default_Graph()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), g));
        // Without union, query on default graph yields nothing
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Empty(sols);
        // With union, it searches all graphs
        var results2 = store.Query("SELECT ?s WHERE { ?s ?p ?o }",
            new QueryOptions { UseDefaultGraphAsUnion = true });
        var sols2 = Assert.IsAssignableFrom<QuerySolutions>(results2);
        Assert.Single(sols2);
    }

    // ─── Query: default_graph / named_graph restriction ──

    [Fact]
    public void Select_With_DefaultGraph_Restriction()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"), g));
        // Restrict to named graph g only
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }",
            new QueryOptions { DefaultGraphs = [g] });
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var list = sols.ToList();
        Assert.Single(list);
        Assert.NotNull(list[0]["s"]);
    }

    [Fact]
    public void Select_With_NamedGraph_Restriction()
    {
        using var store = new Store();
        var g = new NamedNode("http://example.com/g");
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), g));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"),
            new NamedNode("http://example.com/g2")));
        // Only g is available in GRAPH clause
        var results = store.Query("SELECT ?s WHERE { GRAPH ?g { ?s ?p ?o } }",
            new QueryOptions { NamedGraphs = [g] });
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var list = sols.ToList();
        Assert.Single(list);
    }

    // ─── Query: base_iri and prefixes ─────────────────

    [Fact]
    public void Ask_With_BaseIri_And_Prefixes()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://foo"),
            new NamedNode("http://bar"),
            new NamedNode("http://baz"),
            new DefaultGraph()));
        var results = store.Query("ASK { <> bar: baz: }",
            new QueryOptions
            {
                BaseIri = "http://foo",
                Prefixes = new Dictionary<string, string>
                {
                    ["bar"] = "http://bar",
                    ["baz"] = "http://baz",
                }
            });
        var b = Assert.IsAssignableFrom<QueryBoolean>(results);
        Assert.True(b.Value);
    }

    // ─── QueryResults CSV/Turtle serialization ─────────

    [Fact]
    public void QuerySolutions_Serialize_CSV()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var csv = sols.Serialize(QueryResultsFormat.Csv);
        Assert.Contains("s", csv);
        Assert.Contains("http://example.com/s", csv);
    }

    [Fact]
    public void QueryBoolean_Serialize_CSV()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("ASK { ?s ?p ?o }");
        var b = Assert.IsAssignableFrom<QueryBoolean>(results);
        var csv = b.Serialize(QueryResultsFormat.Csv);
        Assert.Equal("true", csv);
    }

    [Fact]
    public void QueryTriples_Serialize_Turtle()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
        var triples = Assert.IsAssignableFrom<QueryTriples>(results);
        var turtle = triples.Serialize(RdfFormat.Turtle);
        Assert.Contains("http://example.com/s", turtle);
    }

    // ─── Update: DELETE DATA / LOAD ─────────────────────

    [Fact]
    public void Delete_Data_Update()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        store.Update("DELETE DATA { <http://example.com/s> <http://example.com/p> \"test\" }");
        Assert.Equal(0UL, store.Count);
    }

    // ─── QueryResults lazy iteration ──────────────────

    [Fact]
    public void QueryResults_LazyIteration()
    {
        using var store = new Store();
        for (int i = 0; i < 50; i++)
            store.Add(new Quad(new NamedNode($"http://example.com/s{i}"),
                new NamedNode("http://example.com/p"), new Literal($"value{i}"), new DefaultGraph()));

        var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        int count = 0;
        foreach (var sol in sols)
        {
            count++;
            Assert.NotNull(sol["s"]);
            Assert.NotNull(sol["o"]);
        }
        Assert.Equal(50, count);
    }

    // ─── DESCRIBE Query ──────────────────────────────────

    [Fact]
    public void Describe_Query()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("DESCRIBE <http://example.com/s>");
        var triples = Assert.IsAssignableFrom<QueryTriples>(results);
        Assert.Single(triples);
    }

    // ─── Update with base_iri and prefixes ──────────────

    [Fact]
    public void Update_WithBaseIri_And_Prefixes()
    {
        using var store = new Store();
        store.Update("INSERT DATA { <> bar: \"test\" }",
            new UpdateOptions
            {
                BaseIri = "http://example.com/s",
                Prefixes = new Dictionary<string, string> { ["bar"] = "http://example.com/p" }
            });
        Assert.Equal(1UL, store.Count);
    }

    // ─── Exception types ────────────────────────────────

    [Fact]
    public void Sparql_Error_OnBadQuery()
    {
        using var store = new Store();
        // FFI layer wraps all errors as ArgumentException
        Assert.Throws<ArgumentException>(() => store.Query("THIS IS NOT SPARQL"));
    }

    [Fact]
    public void Sparql_Error_OnBadUpdate()
    {
        using var store = new Store();
        Assert.Throws<ArgumentException>(() => store.Update("NOT AN UPDATE"));
    }

    // ─── QuerySolutions XML serialization ────────────────

    [Fact]
    public void QuerySolutions_Serialize_Xml()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var xml = sols.Serialize(QueryResultsFormat.Xml);
        Assert.Contains("http://example.com/s", xml);

        // Parse back
        var parsed = IO.ParseQueryResults(xml, QueryResultsFormat.Xml);
        var parsedSols = Assert.IsAssignableFrom<QuerySolutions>(parsed);
        Assert.Single(parsedSols);
    }

    // ─── QuerySolutions TSV serialization ────────────────

    [Fact]
    public void QuerySolutions_Serialize_Tsv()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var tsv = sols.Serialize(QueryResultsFormat.Tsv);
        Assert.Contains("http://example.com/s", tsv);

        // Parse back
        var parsed = IO.ParseQueryResults(tsv, QueryResultsFormat.Tsv);
        var parsedSols = Assert.IsAssignableFrom<QuerySolutions>(parsed);
        Assert.Single(parsedSols);
    }

    // ─── QuerySolutions serialize to file TSV ───────────

    [Fact]
    public void QuerySolutions_SerializeToFile_Tsv()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
            var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
            sols.SerializeToFile(tempFile, QueryResultsFormat.Tsv);

            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);

            var parsed = IO.ParseQueryResults(content, QueryResultsFormat.Tsv);
            var parsedSols = Assert.IsAssignableFrom<QuerySolutions>(parsed);
            Assert.Single(parsedSols);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── QueryBoolean serialize to file XML ───────────────

    [Fact]
    public void QueryBoolean_SerializeToFile_Xml()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var result = store.Query("ASK { ?s ?p ?o }");
            var boolean = Assert.IsAssignableFrom<QueryBoolean>(result);
            boolean.SerializeToFile(tempFile, QueryResultsFormat.Xml);

            var content = File.ReadAllText(tempFile);
            Assert.Contains("true", content);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── QueryTriples serialize to file RDF/XML ───────────

    [Fact]
    public void QueryTriples_SerializeToFile_RdfXml()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s"),
                new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
            var result = store.Query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
            var triples = Assert.IsAssignableFrom<QueryTriples>(result);
            triples.SerializeToFile(tempFile, RdfFormat.RdfXml);

            var content = File.ReadAllText(tempFile);
            Assert.Contains("http://example.com/s", content);
        }
        finally { File.Delete(tempFile); }
    }

    // ─── Custom aggregate with GROUP BY ───────────────────

    [Fact]
    public void Custom_Aggregate_WithGroupBy()
    {
        var factory = () => new CountAggregate();
        CustomFunctions.RegisterAggregate("http://example.com/myCount2",
            factory);

        try
        {
            using var store = new Store();
            store.Add(new Quad(new NamedNode("http://example.com/s1"),
                new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
            store.Add(new Quad(new NamedNode("http://example.com/s1"),
                new NamedNode("http://example.com/q"), new Literal("b"), new DefaultGraph()));
            store.Add(new Quad(new NamedNode("http://example.com/s2"),
                new NamedNode("http://example.com/p"), new Literal("c"), new DefaultGraph()));

            var results = store.Query(@"
                PREFIX my: <http://example.com/>
                SELECT ?s (my:myCount2(?p) AS ?cnt) WHERE { ?s ?p ?o }
                GROUP BY ?s");
            var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
            var list = sols.ToList();
            Assert.Equal(2, list.Count);
        }
        finally
        {
            CustomFunctions.UnregisterAggregate("http://example.com/myCount2");
        }
    }

    // ─── QueryOptions: CustomAggregateFunctions ───────────

    [Fact]
    public void QueryOptions_CustomAggregateFunctions()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        store.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"), new DefaultGraph()));

        var results = store.Query(@"
            PREFIX my: <http://example.com/>
            SELECT (my:total(?o) AS ?cnt) WHERE { ?s ?p ?o }",
            new QueryOptions
            {
                CustomAggregateFunctions = new()
                {
                    ["http://example.com/total"] = () => new CountAggregate()
                }
            });
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Single(sols);
        Assert.Equal("2", ((Literal)sols.First()["cnt"]!).Value);
    }

    // ─── UpdateOptions with CustomAggregateFunctions ───────

    [Fact]
    public void UpdateOptions_CustomAggregateFunctions()
    {
        using var store = new Store();
        store.Update("INSERT DATA { <http://example.com/s> <http://example.com/p> \"test\" }",
            new UpdateOptions
            {
                CustomAggregateFunctions = new()
                {
                    ["http://example.com/dummy"] = () => new CountAggregate()
                }
            });
        Assert.Equal(1UL, store.Count);
    }

    // ─── QueryResults Dispose ──────────────────────────

    [Fact]
    public void QueryResults_Dispose()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        results.Dispose();
        // Should not throw
    }

    // ─── Select with empty result ──────────────────────

    [Fact]
    public void Select_EmptyResult()
    {
        using var store = new Store();
        var results = store.Query("SELECT ?s WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        Assert.Empty(sols);
        Assert.Equal(0, sols.Count);
    }

    }