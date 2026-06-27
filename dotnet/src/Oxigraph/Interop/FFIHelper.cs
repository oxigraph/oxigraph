using System.Runtime.InteropServices;
using System.Text.Json;

namespace Oxigraph.Interop;

/// <summary>
/// Unified JSON-based FFI call helper.
/// All Rust FFI functions return JSON: {"ok": <result>} or {"error": {"kind": "...", ...}}.
/// </summary>
internal static class FFIHelper
{
    /// <summary>
    /// Call an FFI function that returns a JSON string, deserialize the "ok" field.
    /// Throws OxigraphException on error.
    /// </summary>
    internal static T Call<T>(Func<IntPtr> ffiCall) where T : class
    {
        IntPtr jsonPtr = ffiCall();
        return ProcessResponse<T>(jsonPtr);
    }

    /// <summary>
    /// Call an FFI function that returns a JSON string, expecting a struct result.
    /// </summary>
    internal static T CallValue<T>(Func<IntPtr> ffiCall) where T : struct
    {
        IntPtr jsonPtr = ffiCall();
        return ProcessValueResponse<T>(jsonPtr);
    }

    /// <summary>
    /// Call an FFI function and check for errors, but return void (ignore ok value).
    /// </summary>
    internal static void CallVoid(Func<IntPtr> ffiCall)
    {
        IntPtr jsonPtr = ffiCall();
        string json = ReadAndFree(jsonPtr);
        ThrowIfError(json);
    }

    /// <summary>
    /// Call an FFI function that doesn't return anything (e.g., store_destroy).
    /// </summary>
    internal static void CallVoid(Action ffiCall)
    {
        ffiCall();
    }

    private static T ProcessResponse<T>(IntPtr jsonPtr) where T : class
    {
        string json = ReadAndFree(jsonPtr);
        ThrowIfError(json);
        using var doc = JsonDocument.Parse(json);
        return JsonSerializer.Deserialize<T>(
            doc.RootElement.GetProperty("ok").GetRawText())!;
    }

    private static T ProcessValueResponse<T>(IntPtr jsonPtr) where T : struct
    {
        string json = ReadAndFree(jsonPtr);
        ThrowIfError(json);
        using var doc = JsonDocument.Parse(json);
        return JsonSerializer.Deserialize<T>(
            doc.RootElement.GetProperty("ok").GetRawText());
    }

    private static string ReadAndFree(IntPtr jsonPtr)
    {
        if (jsonPtr == IntPtr.Zero)
            throw new OxigraphException("FFI returned null pointer");

        string json = Marshal.PtrToStringUTF8(jsonPtr)
            ?? throw new OxigraphException("FFI returned invalid UTF-8");
        OxigraphNative.free_string(jsonPtr);
        return json;
    }

    internal static void ThrowIfError(string json)
    {
        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.TryGetProperty("error", out var error))
        {
            var kind = error.GetProperty("kind").GetString();
            var message = error.GetProperty("message").GetString() ?? "Unknown error";
            throw MapError(kind!, message);
        }
    }

    private static Exception MapError(string kind, string message)
    {
        return kind switch
        {
            "store" => new StoreException(message),
            "parse" => new ParseException(message),
            "invalid_argument" => new ArgumentException(message),
            _ => new OxigraphException($"[{kind}] {message}"),
        };
    }
}