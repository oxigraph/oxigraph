use std::io::{self, Read, Write};
use std::os::raw::c_void;

/// Callback from C#: read a chunk of data.
/// Returns: bytes read (>0), 0 = EOF, -1 = error.
pub type ReadFn = unsafe extern "C" fn(context: *mut c_void, buf: *mut u8, buf_size: i32) -> i32;

/// Callback from C#: write a chunk of data.
/// Returns: bytes written, -1 = error.
pub type WriteFn = unsafe extern "C" fn(context: *mut c_void, buf: *const u8, buf_size: i32) -> i32;

/// A `Read` implementation that delegates to a C# callback.
pub struct CallbackReader {
    context: *mut c_void,
    callback: ReadFn,
}

impl CallbackReader {
    pub fn new(context: *mut c_void, callback: ReadFn) -> Self {
        Self { context, callback }
    }
}

impl Read for CallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let len = (buf.len().min(i32::MAX as usize)) as i32;
        let result = unsafe { (self.callback)(self.context, buf.as_mut_ptr(), len) };
        match result {
            n if n > 0 => Ok(n as usize),
            0 => Ok(0), // EOF
            -1 => Err(io::Error::new(
                io::ErrorKind::Other,
                "Read callback returned an error",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Read callback returned unexpected value: {result}"),
            )),
        }
    }
}

/// A `Write` implementation that delegates to a C# callback.
pub struct CallbackWriter {
    context: *mut c_void,
    callback: WriteFn,
}

impl CallbackWriter {
    pub fn new(context: *mut c_void, callback: WriteFn) -> Self {
        Self { context, callback }
    }
}

impl Write for CallbackWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let len = (buf.len().min(i32::MAX as usize)) as i32;
        let result = unsafe { (self.callback)(self.context, buf.as_ptr(), len) };
        match result {
            n if n > 0 => Ok(n as usize),
            0 => Ok(0),
            -1 => Err(io::Error::new(
                io::ErrorKind::Other,
                "Write callback returned an error",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Write callback returned unexpected value: {result}"),
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // No-op: .NET Streams handle their own buffering.
        Ok(())
    }
}