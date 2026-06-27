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

    // Memory
    [LibraryImport(LibName, EntryPoint = "oxigraph_free_string", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void free_string(IntPtr ptr);
}