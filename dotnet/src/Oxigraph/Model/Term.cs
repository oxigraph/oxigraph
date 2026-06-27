using System.Text.Json;
using System.Text.Json.Serialization;

namespace Oxigraph;

/// <summary>RDF Term types, matching Rust serde format.</summary>
public interface ITerm { }

/// <summary>Subject types.</summary>
public interface INamedOrBlankNode : ITerm { }

/// <summary>Graph name types.</summary>
public interface IGraphName { }

/// <summary>
/// JSON converters that handle the Rust serde tagged-enum format:
/// {"type": "uri"|"bnode"|"literal"|"default", "value": "...", ...}
/// </summary>
public class TermConverter : JsonConverter<ITerm>
{
    public override ITerm? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        var kind = root.GetProperty("type").GetString();
        var value = root.GetProperty("value").GetString() ?? "";

        return kind switch
        {
            "uri" => new NamedNode(value),
            "bnode" => new BlankNode(value),
            "literal" => new Literal(
                value,
                root.TryGetProperty("language", out var lang) && lang.ValueKind != JsonValueKind.Null ? lang.GetString() : null,
                root.TryGetProperty("datatype", out var dt) && dt.ValueKind != JsonValueKind.Null ? new NamedNode(dt.GetString()!) : null),
            _ => throw new JsonException($"Unknown term type: {kind}")
        };
    }

    public override void Write(Utf8JsonWriter writer, ITerm value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case NamedNode nn:
                writer.WriteString("type", "uri");
                writer.WriteString("value", nn.Value);
                break;
            case BlankNode bn:
                writer.WriteString("type", "bnode");
                writer.WriteString("value", bn.Value);
                break;
            case Literal lit:
                writer.WriteString("type", "literal");
                writer.WriteString("value", lit.Value);
                if (lit.Language != null)
                    writer.WriteString("language", lit.Language);
                if (lit.Datatype != null)
                {
                    writer.WritePropertyName("datatype");
                    JsonSerializer.Serialize(writer, lit.Datatype, options);
                }
                break;
            default:
                throw new JsonException($"Unknown term type: {value.GetType()}");
        }
        writer.WriteEndObject();
    }
}

public class NamedOrBlankNodeConverter : JsonConverter<INamedOrBlankNode>
{
    public override INamedOrBlankNode? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        var kind = root.GetProperty("type").GetString();
        var value = root.GetProperty("value").GetString() ?? "";
        return kind switch
        {
            "uri" => new NamedNode(value),
            "bnode" => new BlankNode(value),
            _ => throw new JsonException($"Unknown subject type: {kind}")
        };
    }

    public override void Write(Utf8JsonWriter writer, INamedOrBlankNode value, JsonSerializerOptions options)
    {
        var converter = new TermConverter();
        converter.Write(writer, value, options);
    }
}

public class GraphNameConverter : JsonConverter<IGraphName>
{
    public override IGraphName? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        var kind = root.GetProperty("type").GetString();
        return kind switch
        {
            "uri" => new NamedNode(root.GetProperty("value").GetString()!),
            "bnode" => new BlankNode(root.GetProperty("value").GetString()!),
            "default" => new DefaultGraph(),
            _ => throw new JsonException($"Unknown graph name type: {kind}")
        };
    }

    public override void Write(Utf8JsonWriter writer, IGraphName value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case NamedNode nn:
                writer.WriteString("type", "uri");
                writer.WriteString("value", nn.Value);
                break;
            case BlankNode bn:
                writer.WriteString("type", "bnode");
                writer.WriteString("value", bn.Value);
                break;
            case DefaultGraph:
                writer.WriteString("type", "default");
                break;
            default:
                throw new JsonException($"Unknown graph name type: {value.GetType()}");
        }
        writer.WriteEndObject();
    }
}

/// <summary>
/// Custom converter for NamedNode as predicate — Rust serde uses
/// the same {"type":"uri","value":"..."} format.
/// </summary>
public class NamedNodeConverter : JsonConverter<NamedNode>
{
    public override NamedNode? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        if (root.TryGetProperty("value", out var val))
            return new NamedNode(val.GetString()!);
        return new NamedNode(root.GetString()!);
    }

    public override void Write(Utf8JsonWriter writer, NamedNode value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteString("type", "uri");
        writer.WriteString("value", value.Value);
        writer.WriteEndObject();
    }
}