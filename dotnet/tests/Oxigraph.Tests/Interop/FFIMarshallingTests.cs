using System.Text.Json;

namespace Oxigraph.Tests.Interop;

public class FFIMarshallingTests
{
    [Fact]
    public void Quad_Roundtrip_Json()
    {
        var quad = new Quad(
            new NamedNode("http://example.com/s"),
            new NamedNode("http://example.com/p"),
            new Literal("hello"),
            new DefaultGraph());

        var json = JsonSerializer.Serialize(quad);
        var deserialized = JsonSerializer.Deserialize<Quad>(json);

        Assert.NotNull(deserialized);
        Assert.Equal("http://example.com/s", ((NamedNode)deserialized.Subject).Value);
        Assert.Equal("http://example.com/p", deserialized.Predicate.Value);
        Assert.Equal("hello", ((Literal)deserialized.Object).Value);
        Assert.IsType<DefaultGraph>(deserialized.Graph);
    }

    [Fact]
    public void CreateManyStores_NoException()
    {
        for (int i = 0; i < 100; i++)
        {
            using var store = new Store();
        }

        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
    }
}