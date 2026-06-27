using System.Runtime.InteropServices;

namespace Oxigraph.Interop;

/// <summary>
/// Callback delegates and context wrappers for streaming data across the FFI boundary.
/// Pattern: same as CustomFunctions — GCHandle-pinned delegates with context pointers.
/// </summary>

/// <summary>Callback: read up to bufferSize bytes into buffer. Returns bytes read, 0=EOF, -1=error.</summary>
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
internal delegate int ReadCallback(IntPtr context, IntPtr buffer, int bufferSize);

/// <summary>Callback: write bufferSize bytes from buffer. Returns bytes written, -1=error.</summary>
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
internal delegate int WriteCallback(IntPtr context, IntPtr buffer, int bufferSize);

/// <summary>
/// Context for a read callback — holds a .NET Stream and a pinned delegate.
/// Passed as IntPtr to Rust where it's used as the callback's context parameter.
/// </summary>
internal sealed class ReadContext : IDisposable
{
    private readonly Stream _stream;
    private readonly byte[] _buffer;

    // Pinned to prevent GC collection while Rust holds a reference
    private readonly GCHandle _gcHandle;

    public readonly IntPtr ContextPtr;
    public readonly ReadCallback Callback;

    public ReadContext(Stream stream, int bufferSize = 8192)
    {
        _stream = stream;
        _buffer = new byte[bufferSize];
        Callback = ReadImpl;

        // Pin the delegate so it stays alive across FFI calls
        _gcHandle = GCHandle.Alloc(Callback);
        ContextPtr = (IntPtr)_gcHandle; // pass handle as context
    }

    private int ReadImpl(IntPtr context, IntPtr buffer, int bufferSize)
    {
        try
        {
            int totalRead = 0;
            int remaining = Math.Min(bufferSize, _buffer.Length);

            while (remaining > 0)
            {
                int read = _stream.Read(_buffer, 0, remaining);
                if (read == 0) break;
                Marshal.Copy(_buffer, 0, buffer + totalRead, read);
                totalRead += read;
                remaining -= read;
            }
            return totalRead;
        }
        catch
        {
            return -1;
        }
    }

    public void Dispose()
    {
        if (_gcHandle.IsAllocated)
            _gcHandle.Free();
    }
}

/// <summary>
/// Context for a write callback — holds a .NET Stream and a pinned delegate.
/// </summary>
internal sealed class WriteContext : IDisposable
{
    private readonly Stream _stream;
    private readonly byte[] _buffer = new byte[8192];

    private readonly GCHandle _gcHandle;

    public readonly IntPtr ContextPtr;
    public readonly WriteCallback Callback;

    public WriteContext(Stream stream)
    {
        _stream = stream;
        Callback = WriteImpl;

        _gcHandle = GCHandle.Alloc(Callback);
        ContextPtr = (IntPtr)_gcHandle;
    }

    private int WriteImpl(IntPtr context, IntPtr buffer, int bufferSize)
    {
        try
        {
            int remaining = bufferSize;
            int offset = 0;

            while (remaining > 0)
            {
                int chunk = Math.Min(remaining, _buffer.Length);
                Marshal.Copy(buffer + offset, _buffer, 0, chunk);
                _stream.Write(_buffer, 0, chunk);
                offset += chunk;
                remaining -= chunk;
            }
            return bufferSize; // all bytes written
        }
        catch
        {
            return -1;
        }
    }

    public void Dispose()
    {
        if (_gcHandle.IsAllocated)
            _gcHandle.Free();
    }
}