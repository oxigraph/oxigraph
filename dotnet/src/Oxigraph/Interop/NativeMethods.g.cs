using System.Runtime.InteropServices;

namespace Oxigraph.Interop;

internal static partial class OxigraphNative
{
    private const string LibName = "oxigraph_dotnet";

    // Store lifecycle
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_new", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_new();

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_open", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_open(string path);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_open_read_only", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_open_read_only(string path);

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

    // Management
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_flush")]
    internal static partial IntPtr store_flush(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_optimize")]
    internal static partial IntPtr store_optimize(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_backup", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_backup(IntPtr handle, string targetPath);

    // Bulk & management
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_clear")]
    internal static partial IntPtr store_clear(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_extend", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_extend(IntPtr handle, string quadsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_extend", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_bulk_extend(IntPtr handle, string quadsJson);

    // I/O
    [LibraryImport(LibName, EntryPoint = "oxigraph_parse", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr parse(string inputJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_serialize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr serialize(string inputJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_query_results", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr parse_query_results(string inputJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_load", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_load(IntPtr handle, string loadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_dump", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_dump(IntPtr handle, string dumpJson);

    // Custom functions
    [LibraryImport(LibName, EntryPoint = "oxigraph_register_custom_function", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr register_custom_function(string name, IntPtr callback);

    [LibraryImport(LibName, EntryPoint = "oxigraph_unregister_custom_function", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr unregister_custom_function(string name);

    // Custom aggregate functions
    [LibraryImport(LibName, EntryPoint = "oxigraph_register_aggregate_function", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr register_aggregate_function(string name, IntPtr newFn, IntPtr accFn, IntPtr finishFn, IntPtr freeFn);

    [LibraryImport(LibName, EntryPoint = "oxigraph_unregister_aggregate_function", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr unregister_aggregate_function(string name);

    // File-based I/O
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_load_from_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_load_from_file(IntPtr handle, string path, string format, string? baseIri, string? toGraphJson, string? optionsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_load_from_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_bulk_load_from_file(IntPtr handle, string path, string format, string? baseIri, string? toGraphJson, string? optionsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_dump_to_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_dump_to_file(IntPtr handle, string path, string format, string? baseIri, string? fromGraphJson, string? prefixesJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_from_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr parse_from_file(string path, string format, string? baseIri, string? optionsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_serialize_to_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr serialize_to_file(string path, string quadsJson, string format, string? baseIri, string? prefixesJson);

    // Stream callback I/O
    [LibraryImport(LibName, EntryPoint = "oxigraph_store_load_from_callback", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_load_from_callback(IntPtr handle, IntPtr callback, IntPtr context, string format, string? baseIri, string? toGraphJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_dump_to_callback", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_dump_to_callback(IntPtr handle, IntPtr callback, IntPtr context, string format, string? baseIri, string? fromGraphJson, string? prefixesJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_from_callback", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr parse_from_callback(IntPtr callback, IntPtr context, string format, string? baseIri);

    [LibraryImport(LibName, EntryPoint = "oxigraph_serialize_to_callback", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr serialize_to_callback(IntPtr callback, IntPtr context, string quadsJson, string format, string? baseIri);

    // Iterator
    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_iter_from_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr parse_iter_from_file(string path, string format, string? baseIri);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_iter_next")]
    internal static partial IntPtr parse_iter_next(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_iter_prefixes")]
    internal static partial IntPtr parse_iter_prefixes(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_iter_base_iri")]
    internal static partial IntPtr parse_iter_base_iri(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_parse_iter_destroy")]
    internal static partial void parse_iter_destroy(IntPtr handle);

    // Query results serialization
    [LibraryImport(LibName, EntryPoint = "oxigraph_query_solutions_serialize_to_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_solutions_serialize_to_file(string path, string format, string variablesJson, string solutionsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_boolean_serialize_to_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_boolean_serialize_to_file(string path, string format, [MarshalAs(UnmanagedType.I1)] bool value);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_triples_serialize_to_file", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_triples_serialize_to_file(string path, string rdfFormat, string triplesJson);

    // Query results serialization to buffer
    [LibraryImport(LibName, EntryPoint = "oxigraph_query_solutions_serialize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_solutions_serialize(string format, string variablesJson, string solutionsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_boolean_serialize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_boolean_serialize(string format, [MarshalAs(UnmanagedType.I1)] bool value);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_triples_serialize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr query_triples_serialize(string rdfFormat, string triplesJson);

    // Canonicalization
    [LibraryImport(LibName, EntryPoint = "oxigraph_canonicalize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr canonicalize(string quadsJson, string algorithm);

    // Dataset (in-memory)
    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_new")]
    internal static partial IntPtr dataset_new();

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_from_quads", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr dataset_from_quads(string quadsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_insert", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr dataset_insert(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_remove", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr dataset_remove(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_contains", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr dataset_contains(IntPtr handle, string quadJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_count")]
    internal static partial IntPtr dataset_count(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_clear")]
    internal static partial IntPtr dataset_clear(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_iter")]
    internal static partial IntPtr dataset_iter(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_canonicalize", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr dataset_canonicalize(IntPtr handle, string algorithm);

    [LibraryImport(LibName, EntryPoint = "oxigraph_dataset_destroy")]
    internal static partial void dataset_destroy(IntPtr handle);

    // Memory
    [LibraryImport(LibName, EntryPoint = "oxigraph_free_string", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial void free_string(IntPtr ptr);

    // ─── Lazy query results iterator ──────────────────

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_query_iter", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_query_iter(IntPtr handle, string queryJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_get_type")]
    internal static partial IntPtr query_iter_get_type(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_boolean_value")]
    internal static partial IntPtr query_iter_boolean_value(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_variables")]
    internal static partial IntPtr query_iter_variables(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_next_solution")]
    internal static partial IntPtr query_iter_next_solution(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_next_triple")]
    internal static partial IntPtr query_iter_next_triple(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_query_iter_destroy")]
    internal static partial void query_iter_destroy(IntPtr handle);

    // ─── Chunked bulk extend ──────────────────────────

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_extend_begin")]
    internal static partial IntPtr store_bulk_extend_begin(IntPtr handle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_extend_add_chunk", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr store_bulk_extend_add_chunk(IntPtr bulkHandle, string quadsJson);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_extend_commit")]
    internal static partial IntPtr store_bulk_extend_commit(IntPtr bulkHandle);

    [LibraryImport(LibName, EntryPoint = "oxigraph_store_bulk_extend_cancel")]
    internal static partial void store_bulk_extend_cancel(IntPtr bulkHandle);
}