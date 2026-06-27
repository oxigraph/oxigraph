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
}