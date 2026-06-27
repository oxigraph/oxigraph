using System.Text.Json;

namespace Oxigraph.Tests;

public class ModelTests
{
    private static readonly NamedNode XsdString = new("http://www.w3.org/2001/XMLSchema#string");
    private static readonly NamedNode XsdInteger = new("http://www.w3.org/2001/XMLSchema#integer");
    private static readonly NamedNode XsdDouble = new("http://www.w3.org/2001/XMLSchema#double");
    private static readonly NamedNode XsdBoolean = new("http://www.w3.org/2001/XMLSchema#boolean");
    private static readonly NamedNode RdfLangString = new("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString");

    // ═══════════════════════════════════════════════════
    // NamedNode
    // ═══════════════════════════════════════════════════

    [Fact]
    public void NamedNode_Constructor()
    {
        var node = new NamedNode("http://foo");
        Assert.Equal("http://foo", node.Value);
        Assert.Contains("http://foo", node.ToString());
    }

    [Fact]
    public void NamedNode_Equality()
    {
        Assert.Equal(new NamedNode("http://foo"), new NamedNode("http://foo"));
        Assert.NotEqual(new NamedNode("http://foo"), new NamedNode("http://bar"));
    }

    [Fact]
    public void NamedNode_Implements_Interfaces()
    {
        var node = new NamedNode("http://foo");
        Assert.IsAssignableFrom<INamedOrBlankNode>(node);
        Assert.IsAssignableFrom<IGraphName>(node);
        Assert.IsAssignableFrom<ITerm>(node);
    }

    [Fact]
    public void NamedNode_Json_Roundtrip()
    {
        var node = new NamedNode("http://foo");
        var json = JsonSerializer.Serialize<ITerm>(node, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserialized = JsonSerializer.Deserialize<ITerm>(json, new JsonSerializerOptions { Converters = { new TermConverter() } });
        Assert.IsType<NamedNode>(deserialized);
        Assert.Equal(node.Value, ((NamedNode)deserialized!).Value);
    }

    // ═══════════════════════════════════════════════════
    // BlankNode
    // ═══════════════════════════════════════════════════

    [Fact]
    public void BlankNode_Constructor()
    {
        var node = new BlankNode("foo");
        Assert.Equal("foo", node.Value);
        Assert.Contains("foo", node.ToString());
    }

    [Fact]
    public void BlankNode_AutoGenerate()
    {
        var a = new BlankNode();
        var b = new BlankNode();
        Assert.NotNull(a.Value);
        Assert.NotNull(b.Value);
        Assert.NotEqual(a.Value, b.Value);
        Assert.StartsWith("b", a.Value);
    }

    [Fact]
    public void BlankNode_Equality()
    {
        Assert.Equal(new BlankNode("foo"), new BlankNode("foo"));
        Assert.NotEqual(new BlankNode("foo"), new BlankNode("bar"));
        Assert.NotEqual<ITerm>(new BlankNode("foo"), new NamedNode("http://foo"));
    }

    [Fact]
    public void BlankNode_Implements_Interfaces()
    {
        var node = new BlankNode("g");
        Assert.IsAssignableFrom<INamedOrBlankNode>(node);
        Assert.IsAssignableFrom<IGraphName>(node);
        Assert.IsAssignableFrom<ITerm>(node);
    }

    [Fact]
    public void BlankNode_Json_Roundtrip()
    {
        var node = new BlankNode("b1");
        var json = JsonSerializer.Serialize<ITerm>(node, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserialized = JsonSerializer.Deserialize<ITerm>(json, new JsonSerializerOptions { Converters = { new TermConverter() } });
        Assert.IsType<BlankNode>(deserialized);
        Assert.Equal(node.Value, ((BlankNode)deserialized!).Value);
    }

    // ═══════════════════════════════════════════════════
    // Literal
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Literal_Simple()
    {
        var lit = new Literal("foo");
        Assert.Equal("foo", lit.Value);
        Assert.Null(lit.Language);
        // A simple literal with no explicit datatype serializes with xsd:string
        Assert.Contains("foo", lit.ToString());
    }

    [Fact]
    public void Literal_LanguageTagged()
    {
        var lit = new Literal("foo", Language: "en");
        Assert.Equal("foo", lit.Value);
        Assert.Equal("en", lit.Language);
        // When a language tag is set, datatype is inferred as rdf:langString by the Rust layer
        // on serialization; on C# side the Datatype is null until round-tripped
        Assert.Contains("foo", lit.ToString());
        Assert.Contains("en", lit.ToString());
    }

    [Fact]
    public void Literal_LanguageTagged_WithDirection()
    {
        var lit = new Literal("foo", Language: "en", Direction: BaseDirection.Ltr);
        Assert.Equal("foo", lit.Value);
        Assert.Equal("en", lit.Language);
        Assert.Equal(BaseDirection.Ltr, lit.Direction);
    }

    [Fact]
    public void Literal_Typed()
    {
        var lit = new Literal("42", Datatype: XsdInteger);
        Assert.Equal("42", lit.Value);
        Assert.Null(lit.Language);
        Assert.Equal(XsdInteger, lit.Datatype);
        Assert.Contains("42", lit.ToString());
    }

    [Fact]
    public void Literal_FromInt()
    {
        var lit = Literal.FromInt(0);
        Assert.Equal("0", lit.Value);
        Assert.Equal(XsdInteger, lit.Datatype);

        var lit2 = Literal.FromInt(42);
        Assert.Equal("42", lit2.Value);
    }

    [Fact]
    public void Literal_FromDouble()
    {
        var lit = Literal.FromDouble(3.14);
        Assert.Equal("3.14", lit.Value);
        Assert.Equal(XsdDouble, lit.Datatype);
    }

    [Fact]
    public void Literal_FromBool()
    {
        Assert.Equal("true", Literal.FromBool(true).Value);
        Assert.Equal("false", Literal.FromBool(false).Value);
        Assert.Equal(XsdBoolean, Literal.FromBool(true).Datatype);
    }

    [Fact]
    public void Literal_Implicit_Conversions()
    {
        Literal fromInt = 42;
        Assert.Equal("42", fromInt.Value);

        Literal fromDouble = 3.14;
        Assert.Equal("3.14", fromDouble.Value);

        Literal fromBool = true;
        Assert.Equal("true", fromBool.Value);
    }

    [Fact]
    public void Literal_Equality()
    {
        Assert.Equal(new Literal("foo"), new Literal("foo"));
        Assert.Equal(new Literal("foo", Language: "en"), new Literal("foo", Language: "en"));
        Assert.NotEqual(new Literal("foo"), new Literal("bar"));
        Assert.NotEqual<ITerm>(new Literal("foo"), new NamedNode("http://foo"));
        Assert.NotEqual<ITerm>(new Literal("foo"), new BlankNode("foo"));
    }

    [Fact]
    public void Literal_Json_Roundtrip()
    {
        var lit = new Literal("hello", Language: "en", Direction: BaseDirection.Ltr);
        var json = JsonSerializer.Serialize<ITerm>(lit, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserialized = JsonSerializer.Deserialize<ITerm>(json, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserLit = Assert.IsType<Literal>(deserialized);
        Assert.Equal("hello", deserLit.Value);
        Assert.Equal("en", deserLit.Language);
        Assert.Equal(BaseDirection.Ltr, deserLit.Direction);
    }

    [Fact]
    public void Literal_Json_Simple()
    {
        var lit = new Literal("plain");
        var json = JsonSerializer.Serialize<ITerm>(lit, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserialized = JsonSerializer.Deserialize<ITerm>(json, new JsonSerializerOptions { Converters = { new TermConverter() } });
        var deserLit = Assert.IsType<Literal>(deserialized);
        Assert.Equal("plain", deserLit.Value);
        Assert.Null(deserLit.Language);
        // Datatype is null when not explicitly set in JSON (xsd:string is the RDF default)
        Assert.Null(deserLit.Datatype);
    }

    // ═══════════════════════════════════════════════════
    // BaseDirection
    // ═══════════════════════════════════════════════════

    [Fact]
    public void BaseDirection_Values()
    {
        Assert.NotEqual(BaseDirection.Ltr, BaseDirection.Rtl);
    }

    [Fact]
    public void BaseDirection_Equality()
    {
        Assert.Equal(BaseDirection.Ltr, BaseDirection.Ltr);
        Assert.NotEqual(BaseDirection.Ltr, BaseDirection.Rtl);
    }

    // ═══════════════════════════════════════════════════
    // DefaultGraph
    // ═══════════════════════════════════════════════════

    [Fact]
    public void DefaultGraph_Singleton()
    {
        var dg = new DefaultGraph();
        Assert.Equal("DEFAULT", dg.ToString());
        Assert.Equal(new DefaultGraph(), new DefaultGraph());
    }

    [Fact]
    public void DefaultGraph_NotEqual_To_Other_Types()
    {
        Assert.NotEqual<IGraphName>(new DefaultGraph(), new NamedNode("http://bar"));
        Assert.NotEqual<IGraphName>(new DefaultGraph(), new BlankNode("b"));
    }

    [Fact]
    public void DefaultGraph_Json_Roundtrip()
    {
        var graph = new DefaultGraph();
        var json = JsonSerializer.Serialize<IGraphName>(graph, new JsonSerializerOptions { Converters = { new GraphNameConverter() } });
        var deserialized = JsonSerializer.Deserialize<IGraphName>(json, new JsonSerializerOptions { Converters = { new GraphNameConverter() } });
        Assert.IsType<DefaultGraph>(deserialized);
    }

    // ═══════════════════════════════════════════════════
    // Triple
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Triple_Constructor()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"));
        Assert.Equal(new NamedNode("http://example.com/s"), t.Subject);
        Assert.Equal(new NamedNode("http://example.com/p"), t.Predicate);
        Assert.Equal(new NamedNode("http://example.com/o"), t.Object);
    }

    [Fact]
    public void Triple_RdfStar()
    {
        // RDF-star: triple as object
        var inner = new Triple(
            new NamedNode("http://example.com/os"),
            new NamedNode("http://example.com/op"),
            new NamedNode("http://example.com/oo"));
        var outer = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            inner);
        Assert.IsType<Triple>(outer.Object);
        Assert.Equal(inner, outer.Object);
    }

    [Fact]
    public void Triple_RdfStar_AsSubject()
    {
        // Triple can be used as subject (INamedOrBlankNode)
        var inner = new Triple(
            new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p1"),
            new NamedNode("http://example.com/o1"));
        var t = new Triple(
            inner, // Triple as subject
            new NamedNode("http://example.com/p2"),
            new NamedNode("http://example.com/o2"));
        Assert.IsType<Triple>(t.Subject);
    }

    [Fact]
    public void Triple_Implements_ITerm()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"));
        Assert.IsAssignableFrom<ITerm>(t);
        Assert.IsAssignableFrom<INamedOrBlankNode>(t);
    }

    [Fact]
    public void Triple_ToString()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"));
        Assert.Contains("http://example.com/s", t.ToString());
        Assert.Contains("http://example.com/p", t.ToString());
        Assert.Contains("http://example.com/o", t.ToString());
        Assert.EndsWith(".", t.ToString());
    }

    [Fact]
    public void Triple_Equality()
    {
        var t1 = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"));
        var t2 = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"));
        var t3 = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/s"));

        Assert.Equal(t1, t2);
        Assert.NotEqual(t1, t3);
        Assert.NotEqual<ITerm>(t1, new NamedNode("http://foo"));
        Assert.NotEqual<ITerm>(t1, new BlankNode("foo"));
        Assert.NotEqual<ITerm>(t1, new Literal("foo"));
    }

    [Fact]
    public void Triple_Json_Roundtrip()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"));
        var json = JsonSerializer.Serialize(t);
        var deserialized = JsonSerializer.Deserialize<Triple>(json);
        Assert.NotNull(deserialized);
        Assert.Equal(t.Subject, deserialized.Subject);
        Assert.Equal(t.Predicate, deserialized.Predicate);
        Assert.Equal("hello", ((Literal)deserialized.Object).Value);
    }

    [Fact]
    public void Triple_Json_RdfStar()
    {
        // Serialize and deserialize a Triple using its own JSON schema (not ITerm wrapper)
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"));
        var json = JsonSerializer.Serialize(t);
        var deserialized = JsonSerializer.Deserialize<Triple>(json);
        Assert.NotNull(deserialized);
        Assert.Equal("http://example.com/s", ((NamedNode)deserialized.Subject).Value);
        Assert.Equal("http://example.com/p", deserialized.Predicate.Value);
        Assert.Equal("hello", ((Literal)deserialized.Object).Value);
    }

    // ═══════════════════════════════════════════════════
    // Quad
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Quad_Constructor()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new NamedNode("http://example.com/g"));
        Assert.Equal(new NamedNode("http://example.com/s"), q.Subject);
        Assert.Equal(new NamedNode("http://example.com/p"), q.Predicate);
        Assert.Equal(new NamedNode("http://example.com/o"), q.Object);
        Assert.Equal(new NamedNode("http://example.com/g"), q.Graph);
    }

    [Fact]
    public void Quad_DefaultGraph()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new DefaultGraph());
        Assert.IsType<DefaultGraph>(q.Graph);
    }

    [Fact]
    public void Quad_Triple_Property()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new NamedNode("http://example.com/g"));
        var t = q.Triple;
        Assert.Equal(q.Subject, t.Subject);
        Assert.Equal(q.Predicate, t.Predicate);
        Assert.Equal(q.Object, t.Object);
    }

    [Fact]
    public void Quad_ToString()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("1"),
            new NamedNode("http://example.com/g"));
        Assert.Contains("http://example.com/s", q.ToString());
        Assert.Contains("http://example.com/p", q.ToString());
        Assert.Contains("http://example.com/g", q.ToString());
        Assert.EndsWith(".", q.ToString());
    }

    [Fact]
    public void Quad_DefaultGraph_ToString()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("1"),
            new DefaultGraph());
        Assert.Contains("http://example.com/s", q.ToString());
        Assert.Contains("http://example.com/p", q.ToString());
        Assert.EndsWith(".", q.ToString());
    }

    [Fact]
    public void Quad_Equality()
    {
        var q1 = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new NamedNode("http://example.com/g"));
        var q2 = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new NamedNode("http://example.com/g"));
        var q3 = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new NamedNode("http://example.com/o"),
            new DefaultGraph());

        Assert.Equal(q1, q2);
        Assert.NotEqual(q1, q3);
    }

    [Fact]
    public void Quad_Json_Roundtrip()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("1"),
            new NamedNode("http://example.com/g"));
        var json = JsonSerializer.Serialize(q);
        var deserialized = JsonSerializer.Deserialize<Quad>(json);
        Assert.NotNull(deserialized);
        Assert.Equal("http://example.com/s", ((NamedNode)deserialized.Subject).Value);
        Assert.Equal("http://example.com/p", deserialized.Predicate.Value);
        Assert.Equal("1", ((Literal)deserialized.Object).Value);
        Assert.IsType<NamedNode>(deserialized.Graph);
        Assert.Equal("http://example.com/g", ((NamedNode)deserialized.Graph).Value);
    }

    // ═══════════════════════════════════════════════════
    // Variable
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Variable_Constructor()
    {
        var v = new Variable("foo");
        Assert.Equal("foo", v.Value);
        Assert.Equal("?foo", v.ToString());
    }

    [Fact]
    public void Variable_Equality()
    {
        Assert.Equal(new Variable("foo"), new Variable("foo"));
        Assert.NotEqual(new Variable("foo"), new Variable("bar"));
    }

    // ═══════════════════════════════════════════════════
    // QuerySolution
    // ═══════════════════════════════════════════════════

    [Fact]
    public void QuerySolution_Accessors()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var results = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var sol = sols.First();

        // Access by string
        Assert.NotNull(sol["s"]);
        Assert.IsType<NamedNode>(sol["s"]);
        // Access by Variable
        Assert.NotNull(sol[new Variable("p")]);
        Assert.IsType<NamedNode>(sol[new Variable("p")]);
        // Access by int index (all 3 columns exist)
        Assert.NotNull(sol[0]);
        Assert.NotNull(sol[1]);
        Assert.NotNull(sol[2]);
        // The third column ('o') is the object literal
        Assert.NotNull(sol["o"]);
        Assert.IsType<Literal>(sol["o"]);
        // TryGetValue
        Assert.True(sol.TryGetValue("o", out var o));
        Assert.Equal("hello", ((Literal)o!).Value);
        Assert.False(sol.TryGetValue("nonexistent", out _));
    }

    [Fact]
    public void QuerySolution_Count()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var results = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var sol = sols.First();
        Assert.Equal(3, sol.Count);
    }

    [Fact]
    public void QuerySolution_Deconstruct_Two()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var (s, o) = sols.First();
        Assert.NotNull(s);
        Assert.NotNull(o);
    }

    [Fact]
    public void QuerySolution_Deconstruct_Three()
    {
        using var store = new Store();
        store.Add(new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph()));
        var results = store.Query("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var (s, p, o) = sols.First();
        Assert.NotNull(s);
        Assert.NotNull(p);
        Assert.NotNull(o);
    }

    // ═══════════════════════════════════════════════════
    // Dataset
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Dataset_Discard()
    {
        using var ds = new Dataset();
        var q = Q("http://example.com/s", "http://example.com/p", "test");
        ds.Add(q);
        ds.Discard(q);
        Assert.False(ds.Contains(q));
        Assert.Equal(0, ds.Count);
        // Discard on non-existent quad should not throw
        ds.Discard(q);
    }

    [Fact]
    public void Dataset_Clear()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        ds.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        ds.Clear();
        Assert.Equal(0, ds.Count);
        Assert.True(ds.IsEmpty);
    }

    [Fact]
    public void Dataset_Remove_Throws_WhenNotFound()
    {
        using var ds = new Dataset();
        var q = Q("http://example.com/s", "http://example.com/p", "test");
        Assert.Throws<KeyNotFoundException>(() => ds.Remove(q));
    }

    [Fact]
    public void Dataset_Canonicalize()
    {
        using var ds1 = new Dataset();
        ds1.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));
        using var ds2 = new Dataset();
        ds2.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));

        Assert.NotEqual(ds1.ToString(), ds2.ToString()); // Different blank node IDs

        ds1.Canonicalize(CanonicalizationAlgorithm.Unstable);
        ds2.Canonicalize(CanonicalizationAlgorithm.Unstable);
        Assert.Equal(ds1.ToString(), ds2.ToString());
    }

    [Fact]
    public void Dataset_Load()
    {
        using var ds = new Dataset();
        ds.Load("@prefix ex: <http://example.com/> . ex:s ex:p \"hello\" .", RdfFormat.Turtle,
            new LoadOptions { BaseIri = "http://example.com/" });
        Assert.Equal(1, ds.Count);
    }

    [Fact]
    public void Dataset_Dump()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s", "http://example.com/p", "test"));
        var dumped = ds.Dump(RdfFormat.NTriples, new DumpOptions { FromGraph = new DefaultGraph() });
        Assert.Contains("http://example.com/s", dumped);
    }

    // ═══════════════════════════════════════════════════
    // OxigraphVersion
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Version_IsNotDefault()
    {
        var v = OxigraphVersion.Version;
        Assert.NotNull(v);
        Assert.NotEmpty(v);
        Assert.NotEqual("0.0.0", v);
    }

    // ═══════════════════════════════════════════════════
    // NamedNode — edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void NamedNode_SpecialCharacters()
    {
        var node = new NamedNode("http://example.com/path?query=value&foo=bar#frag");
        Assert.Equal("http://example.com/path?query=value&foo=bar#frag", node.Value);
    }

    [Fact]
    public void NamedNode_Blank()
    {
        // NamedNode must be an absolute IRI; relative or blank should still work for storage
        var node = new NamedNode("");
        Assert.Equal("", node.Value);
    }

    // ═══════════════════════════════════════════════════
    // BlankNode — edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void BlankNode_ToString()
    {
        var node = new BlankNode("test");
        Assert.Contains("test", node.ToString());
    }

    [Fact]
    public void BlankNode_DefaultGraph_Equality()
    {
        // BlankNode implements IGraphName; ensure it works as a graph name
        IGraphName g1 = new BlankNode("g1");
        IGraphName g2 = new BlankNode("g1");
        Assert.Equal(g1, g2);
    }

    // ═══════════════════════════════════════════════════
    // Literal — edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Literal_Rtl_Direction()
    {
        var lit = new Literal("שלום", Language: "he", Direction: BaseDirection.Rtl);
        Assert.Equal("שלום", lit.Value);
        Assert.Equal("he", lit.Language);
        Assert.Equal(BaseDirection.Rtl, lit.Direction);
    }

    [Fact]
    public void Literal_RdfLangString()
    {
        // RDF langString IRI constant
        Assert.Equal("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString", RdfLangString.Value);
    }

    [Fact]
    public void Literal_FromDouble_Zero_And_Negative()
    {
        var zero = Literal.FromDouble(0.0);
        Assert.Equal("0", zero.Value);
        Assert.Equal(XsdDouble, zero.Datatype);

        var neg = Literal.FromDouble(-1.5);
        Assert.Equal("-1.5", neg.Value);
        Assert.Equal(XsdDouble, neg.Datatype);
    }

    [Fact]
    public void Literal_FromInt_Negative()
    {
        var neg = Literal.FromInt(-42);
        Assert.Equal("-42", neg.Value);
        Assert.Equal(XsdInteger, neg.Datatype);
    }

    [Fact]
    public void Literal_Typed_CustomDatatype()
    {
        var dt = new NamedNode("http://example.com/myType");
        var lit = new Literal("custom", Datatype: dt);
        Assert.Equal("custom", lit.Value);
        Assert.Equal(dt, lit.Datatype);
        Assert.Null(lit.Language);
    }

    [Fact]
    public void Literal_ToString_Simple()
    {
        var lit = new Literal("hello");
        Assert.Contains("hello", lit.ToString());
    }

    [Fact]
    public void Literal_ToString_LanguageTagged()
    {
        var lit = new Literal("bonjour", Language: "fr");
        var s = lit.ToString();
        Assert.Contains("bonjour", s);
        Assert.Contains("fr", s);
    }

    [Fact]
    public void Literal_ToString_Typed()
    {
        var lit = new Literal("42", Datatype: XsdInteger);
        Assert.Contains("42", lit.ToString());
    }

    // ═══════════════════════════════════════════════════
    // Variable — more tests
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Variable_ToString_QuestionMark()
    {
        var v = new Variable("x");
        Assert.Equal("?x", v.ToString());
    }

    [Fact]
    public void Variable_Json_Roundtrip()
    {
        var v = new Variable("myVar");
        var json = JsonSerializer.Serialize(v);
        var deserialized = JsonSerializer.Deserialize<Variable>(json);
        Assert.NotNull(deserialized);
        Assert.Equal("myVar", deserialized.Value);
    }

    // ═══════════════════════════════════════════════════
    // Triple — more edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Triple_WithBlankNodeSubject()
    {
        var t = new Triple(
            new BlankNode("b1"),
            new NamedNode("http://example.com/p"),
            new Literal("test"));
        Assert.IsType<BlankNode>(t.Subject);
    }

    [Fact]
    public void Triple_WithLiteralObject()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello", Language: "en"));
        var obj = Assert.IsType<Literal>(t.Object);
        Assert.Equal("en", obj.Language);
    }

    [Fact]
    public void Triple_RdfStar_EqualityDeep()
    {
        var inner1 = new Triple(new NamedNode("http://x"), new NamedNode("http://y"), new NamedNode("http://z"));
        var inner2 = new Triple(new NamedNode("http://x"), new NamedNode("http://y"), new NamedNode("http://z"));
        Assert.Equal(inner1, inner2);
    }

    [Fact]
    public void Triple_Deconstruction()
    {
        var t = new Triple(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("o"));
        var (s, p, o) = t;
        Assert.Equal(new NamedNode("http://example.com/s"), s);
        Assert.Equal(new NamedNode("http://example.com/p"), p);
        Assert.Equal(new Literal("o"), o);
    }

    // ═══════════════════════════════════════════════════
    // Quad — more edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Quad_WithBlankNodeGraph()
    {
        var q = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("1"),
            new BlankNode("g"));
        Assert.IsType<BlankNode>(q.Graph);
        Assert.Equal("g", ((BlankNode)q.Graph).Value);
    }

    [Fact]
    public void Quad_WithTripleAsSubject()
    {
        // RDF-star: quad with triple as subject
        var t = new Triple(new NamedNode("http://x"), new NamedNode("http://y"), new NamedNode("http://z"));
        var q = new Quad(t, new NamedNode("http://p"), new Literal("o"), new DefaultGraph());
        Assert.IsType<Triple>(q.Subject);
        Assert.Equal(t, q.Subject);
    }

    [Fact]
    public void Quad_Deconstruction()
    {
        var q = new Quad(
            new NamedNode("http://s"),
            new NamedNode("http://p"),
            new Literal("o"),
            new DefaultGraph());
        var (s, p, o, g) = q;
        Assert.Equal(new NamedNode("http://s"), s);
        Assert.Equal(new NamedNode("http://p"), p);
        Assert.Equal(new Literal("o"), o);
        Assert.Equal(new DefaultGraph(), g);
    }

    // ═══════════════════════════════════════════════════
    // DefaultGraph — more tests
    // ═══════════════════════════════════════════════════

    [Fact]
    public void DefaultGraph_Equality_WithSelf()
    {
        var dg = new DefaultGraph();
        Assert.True(dg.Equals(new DefaultGraph()));
    }

    [Fact]
    public void DefaultGraph_Implements_IGraphName()
    {
        var dg = new DefaultGraph();
        Assert.IsAssignableFrom<IGraphName>(dg);
    }

    // ═══════════════════════════════════════════════════
    // BaseDirection — more tests
    // ═══════════════════════════════════════════════════

    [Fact]
    public void BaseDirection_AllValues_Different()
    {
        Assert.NotEqual((int)BaseDirection.Ltr, (int)BaseDirection.Rtl);
    }

    // ═══════════════════════════════════════════════════
    // Dataset — pattern matching methods
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Dataset_QuadsForSubject()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s1", "http://example.com/p", "a"));
        ds.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var results = ds.QuadsForSubject(new NamedNode("http://example.com/s1"));
        Assert.Single(results);
    }

    [Fact]
    public void Dataset_QuadsForPredicate()
    {
        using var ds = new Dataset();
        ds.Add(Q("http://example.com/s1", "http://example.com/p1", "a"));
        ds.Add(Q("http://example.com/s2", "http://example.com/p2", "b"));
        var results = ds.QuadsForPredicate(new NamedNode("http://example.com/p1"));
        Assert.Single(results);
    }

    [Fact]
    public void Dataset_QuadsForObject()
    {
        using var ds = new Dataset();
        ds.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), new DefaultGraph()));
        ds.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p"), new Literal("b"), new DefaultGraph()));
        var results = ds.QuadsForObject(new Literal("a"));
        Assert.Single(results);
    }

    [Fact]
    public void Dataset_QuadsForGraphName()
    {
        using var ds = new Dataset();
        var g = new NamedNode("http://example.com/g");
        ds.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p"), new Literal("a"), g));
        ds.Add(Q("http://example.com/s2", "http://example.com/p", "b"));
        var results = ds.QuadsForGraphName(g);
        Assert.Single(results);
    }

    [Fact]
    public void Dataset_Match_Combined()
    {
        using var ds = new Dataset();
        ds.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p1"), new Literal("a"), new NamedNode("http://example.com/g")));
        ds.Add(new Quad(new NamedNode("http://example.com/s1"),
            new NamedNode("http://example.com/p2"), new Literal("b"), new DefaultGraph()));
        ds.Add(new Quad(new NamedNode("http://example.com/s2"),
            new NamedNode("http://example.com/p1"), new Literal("a"), new DefaultGraph()));

        var results = ds.Match(
            subject: new NamedNode("http://example.com/s1"),
            predicate: new NamedNode("http://example.com/p1"));
        Assert.Single(results);
    }

    // ═══════════════════════════════════════════════════
    // CanonicalizationAlgorithm — all variants
    // ═══════════════════════════════════════════════════

    [Fact]
    public void Canonicalize_Rdfc10Sha256()
    {
        using var ds1 = new Dataset();
        ds1.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));
        using var ds2 = new Dataset();
        ds2.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));

        ds1.Canonicalize(CanonicalizationAlgorithm.Rdfc10Sha256);
        ds2.Canonicalize(CanonicalizationAlgorithm.Rdfc10Sha256);
        Assert.Equal(ds1.ToString(), ds2.ToString());
    }

    [Fact]
    public void Canonicalize_Rdfc10Sha384()
    {
        using var ds1 = new Dataset();
        ds1.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));
        using var ds2 = new Dataset();
        ds2.Add(new Quad(new BlankNode(), new NamedNode("http://example.com/p"), new BlankNode(), new DefaultGraph()));

        ds1.Canonicalize(CanonicalizationAlgorithm.Rdfc10Sha384);
        ds2.Canonicalize(CanonicalizationAlgorithm.Rdfc10Sha384);
        Assert.Equal(ds1.ToString(), ds2.ToString());
    }

    // ═══════════════════════════════════════════════════
    // QuerySolution — edge cases
    // ═══════════════════════════════════════════════════

    [Fact]
    public void QuerySolution_IndexOutOfRange()
    {
        using var store = new Store();
        store.Add(new Quad(new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"), new Literal("test"), new DefaultGraph()));
        var results = store.Query("SELECT ?s ?o WHERE { ?s ?p ?o }");
        var sols = Assert.IsAssignableFrom<QuerySolutions>(results);
        var sol = sols.First();
        Assert.Throws<ArgumentOutOfRangeException>(() => sol[99]);
        Assert.Throws<ArgumentOutOfRangeException>(() => sol[-1]);
    }

    // ═══════════════════════════════════════════════════
    // Helpers
    // ═══════════════════════════════════════════════════

    private static Quad Q(string s, string p, string o) =>
        new(new NamedNode(s), new NamedNode(p), new Literal(o), new DefaultGraph());
}