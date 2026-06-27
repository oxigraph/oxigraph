using System.Runtime.InteropServices;
using System.Text.Json;
using Oxigraph.Interop;

namespace Oxigraph;

/// <summary>Custom SPARQL functions via FFI callbacks from Rust to C#.</summary>
public static class CustomFunctions
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate IntPtr BridgeDelegate(IntPtr argsJson);

    // Pinned delegate to prevent GC from collecting the callback
    private static readonly BridgeDelegate _bridge = BridgeImpl;
    private static readonly GCHandle _gcHandle = GCHandle.Alloc(_bridge);
    private static readonly IntPtr _bridgePtr = Marshal.GetFunctionPointerForDelegate(_bridge);
    private static readonly Dictionary<string, Func<ITerm[], ITerm?>> _functions = [];

    private static IntPtr BridgeImpl(IntPtr argsJsonPtr)
    {
        try
        {
            if (argsJsonPtr == IntPtr.Zero) return IntPtr.Zero;
            var json = Marshal.PtrToStringUTF8(argsJsonPtr);
            if (string.IsNullOrEmpty(json)) return IntPtr.Zero;

            using var doc = JsonDocument.Parse(json);
            var arr = doc.RootElement;
            if (arr.ValueKind != JsonValueKind.Array || arr.GetArrayLength() < 2)
                return IntPtr.Zero;

            var name = arr[0].GetString()!;
            var opts = new JsonSerializerOptions { Converters = { new TermConverter() } };
            var terms = new ITerm[arr.GetArrayLength() - 1];
            for (int i = 1; i < arr.GetArrayLength(); i++)
                terms[i - 1] = JsonSerializer.Deserialize<ITerm>(arr[i].GetRawText(), opts)!;

            if (_functions.TryGetValue(name, out var func))
            {
                var result = func(terms);
                if (result == null) return IntPtr.Zero;
                var resultJson = JsonSerializer.Serialize<ITerm>(result, opts);
                return Marshal.StringToHGlobalAnsi(resultJson);
            }
            return IntPtr.Zero;
        }
        catch
        {
            return IntPtr.Zero;
        }
    }

    /// <summary>Register a custom SPARQL function for use in queries.</summary>
    /// <param name="name">The full IRI of the function (e.g., "http://example.com/myFunc")</param>
    /// <param name="func">The function implementation taking terms and returning a term or null</param>
    public static void Register(string name, Func<ITerm[], ITerm?> func)
    {
        _functions[name] = func;
        FFIHelper.CallVoid(() =>
            OxigraphNative.register_custom_function(name, _bridgePtr));
    }

    /// <summary>Unregister a previously registered custom function.</summary>
    public static void Unregister(string name)
    {
        _functions.Remove(name);
        FFIHelper.CallVoid(() =>
            OxigraphNative.unregister_custom_function(name));
    }

    // ─── Custom aggregate functions ──────────────────

    /// <summary>Interface for custom SPARQL aggregate accumulator.</summary>
    public interface IAggregateAccumulator
    {
        /// <summary>Add a term to the accumulator.</summary>
        void Accumulate(ITerm term);
        /// <summary>Return the final aggregated value.</summary>
        ITerm? Finish();
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate IntPtr AggregateNewDelegate();

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void AggregateAccDelegate(IntPtr ctx, IntPtr termJson);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate IntPtr AggregateFinishDelegate(IntPtr ctx);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void AggregateFreeDelegate(IntPtr ctx);

    private static readonly Dictionary<IntPtr, IAggregateAccumulator> _aggregateInstances = [];

    // Shared pinned delegate instances (one set of callbacks for all aggregate functions)
    private static readonly AggregateNewDelegate _aggNew = AggNewImpl;
    private static readonly AggregateAccDelegate _aggAcc = AggAccImpl;
    private static readonly AggregateFinishDelegate _aggFinish = AggFinishImpl;
    private static readonly AggregateFreeDelegate _aggFree = AggFreeImpl;
    private static readonly GCHandle _aggNewHandle = GCHandle.Alloc(_aggNew);
    private static readonly GCHandle _aggAccHandle = GCHandle.Alloc(_aggAcc);
    private static readonly GCHandle _aggFinishHandle = GCHandle.Alloc(_aggFinish);
    private static readonly GCHandle _aggFreeHandle = GCHandle.Alloc(_aggFree);
    private static readonly IntPtr _aggNewPtr = Marshal.GetFunctionPointerForDelegate(_aggNew);
    private static readonly IntPtr _aggAccPtr = Marshal.GetFunctionPointerForDelegate(_aggAcc);
    private static readonly IntPtr _aggFinishPtr = Marshal.GetFunctionPointerForDelegate(_aggFinish);
    private static readonly IntPtr _aggFreePtr = Marshal.GetFunctionPointerForDelegate(_aggFree);

    private static readonly Dictionary<string, Func<IAggregateAccumulator>> _aggregateFactories = [];
    private static readonly object _aggLock = new();

    private static IntPtr AggNewImpl()
    {
        return IntPtr.Zero; // context is allocated by the factory registration
    }

    private static void AggAccImpl(IntPtr ctx, IntPtr termJsonPtr)
    {
        try
        {
            if (ctx == IntPtr.Zero) return;
            lock (_aggLock)
            {
                if (_aggregateInstances.TryGetValue(ctx, out var acc))
                {
                    var json = Marshal.PtrToStringUTF8(termJsonPtr);
                    if (string.IsNullOrEmpty(json)) return;
                    var term = JsonSerializer.Deserialize<ITerm>(json,
                        new JsonSerializerOptions { Converters = { new TermConverter() } });
                    if (term != null) acc.Accumulate(term);
                }
            }
        }
        catch { }
    }

    private static IntPtr AggFinishImpl(IntPtr ctx)
    {
        try
        {
            if (ctx == IntPtr.Zero) return IntPtr.Zero;
            lock (_aggLock)
            {
                if (_aggregateInstances.TryGetValue(ctx, out var acc))
                {
                    var result = acc.Finish();
                    if (result == null) return IntPtr.Zero;
                    var json = JsonSerializer.Serialize<ITerm>(result,
                        new JsonSerializerOptions { Converters = { new TermConverter() } });
                    return Marshal.StringToHGlobalAnsi(json);
                }
            }
            return IntPtr.Zero;
        }
        catch { return IntPtr.Zero; }
    }

    private static void AggFreeImpl(IntPtr ctx)
    {
        if (ctx == IntPtr.Zero) return;
        lock (_aggLock)
        {
            _aggregateInstances.Remove(ctx);
        }
    }

    // Override to create context from factory
    private static readonly AggregateNewDelegate _aggNewWithFactory = AggNewFactoryImpl;
    private static readonly GCHandle _aggNewFactoryHandle = GCHandle.Alloc(_aggNewWithFactory);
    private static readonly IntPtr _aggNewFactoryPtr = Marshal.GetFunctionPointerForDelegate(_aggNewWithFactory);
    private static string? _pendingAggName;

    private static IntPtr AggNewFactoryImpl()
    {
        try
        {
            if (_pendingAggName != null && _aggregateFactories.TryGetValue(_pendingAggName, out var factory))
            {
                var instance = factory();
                var handle = GCHandle.Alloc(instance);
                var ctx = (IntPtr)handle;
                lock (_aggLock) { _aggregateInstances[ctx] = instance; }
                return ctx;
            }
            return IntPtr.Zero;
        }
        catch { return IntPtr.Zero; }
    }

    /// <summary>
    /// Register a custom aggregate SPARQL function.
    /// </summary>
    /// <param name="name">The full IRI of the function (e.g., "http://example.com/myAgg")</param>
    /// <param name="factory">Factory function that creates a new accumulator instance for each group</param>
    public static void RegisterAggregate(string name, Func<IAggregateAccumulator> factory)
    {
        _aggregateFactories[name] = factory;
        _pendingAggName = name;
        FFIHelper.CallVoid(() =>
            OxigraphNative.register_aggregate_function(name, _aggNewFactoryPtr, _aggAccPtr, _aggFinishPtr, _aggFreePtr));
    }

    /// <summary>Unregister a previously registered aggregate function.</summary>
    public static void UnregisterAggregate(string name)
    {
        _aggregateFactories.Remove(name);
        FFIHelper.CallVoid(() =>
            OxigraphNative.unregister_aggregate_function(name));
    }
}