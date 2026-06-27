using System.Reflection;

namespace Oxigraph;

/// <summary>Provides Oxigraph library version information.</summary>
public static class OxigraphVersion
{
    /// <summary>The version of the Oxigraph .NET bindings, matching the Cargo package version.</summary>
    public static string Version { get; } =
        typeof(OxigraphVersion).Assembly
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()
            ?.InformationalVersion
        ?? "0.0.0";
}