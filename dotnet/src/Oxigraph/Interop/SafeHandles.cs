using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Oxigraph.Interop;

/// <summary>
/// SafeHandle wrapping a native Store pointer.
/// Ensures store_destroy is called even on abnormal termination.
/// </summary>
internal sealed class StoreSafeHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public StoreSafeHandle() : base(true) { }

    public StoreSafeHandle(IntPtr handle) : base(true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        OxigraphNative.store_destroy(handle);
        return true;
    }
}