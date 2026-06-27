using System.Runtime.InteropServices;

namespace Oxigraph.Interop;

internal static partial class OxigraphNative
{
    private const string LibName = "oxigraph_dotnet";

    // Store lifecycle — EntryPoint matches Rust exported symbol names
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_new", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_new();

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_destroy")]
    internal static partial void store_destroy(IntPtr handle);

    // CRUD
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_add", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_add(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_remove", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_remove(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_contains", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_contains(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_count")]
    internal static partial IntPtr store_count(IntPtr handle);

    // SPARQL
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_query", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_query(IntPtr handle, string queryJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_update", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_update(IntPtr handle, string updateJson);

    // Pattern matching
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_match", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_match(IntPtr handle, string patternJson);

    // Named graphs
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_named_graphs")]
    internal static partial IntPtr store_named_graphs(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_contains_named_graph", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_contains_named_graph(IntPtr handle, string graphJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_insert_named_graph", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_insert_named_graph(IntPtr handle, string graphJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_clear_graph", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_clear_graph(IntPtr handle, string graphJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_remove_named_graph", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_remove_named_graph(IntPtr handle, string graphJson);

    // Bulk & management
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_clear")]
    internal static partial IntPtr store_clear(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_extend", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_extend(IntPtr handle, string quadsJson);

    // Memory
    [LibraryImport(LibName, EntryPoint = "oxigraph_free_string", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void free_string(IntPtr ptr);
}