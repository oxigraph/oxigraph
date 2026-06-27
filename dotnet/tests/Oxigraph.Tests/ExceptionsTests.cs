namespace Oxigraph.Tests;

public class ExceptionsTests
{
    [Fact]
    public void OxigraphException_Constructor_Message()
    {
        var ex = new OxigraphException("test message");
        Assert.Equal("test message", ex.Message);
    }

    [Fact]
    public void OxigraphException_Constructor_MessageAndInner()
    {
        var inner = new InvalidOperationException("inner");
        var ex = new OxigraphException("test message", inner);
        Assert.Equal("test message", ex.Message);
        Assert.Same(inner, ex.InnerException);
    }

    [Fact]
    public void StoreException_IsOxigraphException()
    {
        var ex = new StoreException("store error");
        Assert.IsAssignableFrom<OxigraphException>(ex);
        Assert.Equal("store error", ex.Message);
    }

    [Fact]
    public void ParseException_IsOxigraphException()
    {
        var ex = new ParseException("parse error");
        Assert.IsAssignableFrom<OxigraphException>(ex);
        Assert.Equal("parse error", ex.Message);
    }

    [Fact]
    public void ParseException_FilePathAndLine()
    {
        var ex = new ParseException("parse error")
        {
            FilePath = "/path/to/file.ttl",
            Line = 42
        };
        Assert.Equal("/path/to/file.ttl", ex.FilePath);
        Assert.Equal(42, ex.Line);
    }

    [Fact]
    public void ParseException_FilePathAndLine_Null()
    {
        var ex = new ParseException("parse error");
        Assert.Null(ex.FilePath);
        Assert.Null(ex.Line);
    }

    [Fact]
    public void SparqlSyntaxException_IsOxigraphException()
    {
        var ex = new SparqlSyntaxException("syntax error");
        Assert.IsAssignableFrom<OxigraphException>(ex);
        Assert.Equal("syntax error", ex.Message);
    }

    [Fact]
    public void SparqlEvaluationException_IsOxigraphException()
    {
        var ex = new SparqlEvaluationException("eval error");
        Assert.IsAssignableFrom<OxigraphException>(ex);
        Assert.Equal("eval error", ex.Message);
    }
}