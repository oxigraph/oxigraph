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

/// <summary>
/// SafeHandle wrapping a native Dataset pointer.
/// </summary>
internal sealed class DatasetSafeHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public DatasetSafeHandle() : base(true) { }

    public DatasetSafeHandle(IntPtr handle) : base(true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        OxigraphNative.dataset_destroy(handle);
        return true;
    }
}

/// <summary>SafeHandle for lazy quad iterators from parse.</summary>
internal sealed class QuadIterSafeHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public QuadIterSafeHandle() : base(true) { }

    public QuadIterSafeHandle(IntPtr handle) : base(true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        OxigraphNative.parse_iter_destroy(handle);
        return true;
    }
}