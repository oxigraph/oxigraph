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
}