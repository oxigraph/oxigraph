namespace Oxigraph;

/// <summary>
/// Base exception for all Oxigraph errors.
/// </summary>
public class OxigraphException : Exception
{
    public OxigraphException(string message) : base(message) { }
    public OxigraphException(string message, Exception inner) : base(message, inner) { }
}

public class StoreException : OxigraphException
{
    public StoreException(string message) : base(message) { }
}

public class ParseException : OxigraphException
{
    public string? FilePath { get; init; }
    public int? Line { get; init; }
    public ParseException(string message) : base(message) { }
}

public class SparqlSyntaxException : OxigraphException
{
    public SparqlSyntaxException(string message) : base(message) { }
}

public class SparqlEvaluationException : OxigraphException
{
    public SparqlEvaluationException(string message) : base(message) { }
}