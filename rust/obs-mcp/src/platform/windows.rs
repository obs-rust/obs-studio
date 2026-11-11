//! Windows named-pipe transport; modeled on shared/ipc-util pipe-windows.
#![allow(
    unsafe_code,
    reason = "Win32 named-pipe and file handles are raw FFI calls"
)]
use super::EndpointOptions;
use crate::server::Server;
use std::{
    io::{self, BufReader, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const MAX_FRAME: usize = 16 * 1024 * 1024;
const PIPE_MSG: usize = 1024 * 1024;
const PIPE_MSG32: u32 = 1024 * 1024;
const CREATE_MODE: u32 = 0x40000003u32;
const CREATE_FLAGS: u32 = 0x7u32;
// Upstream pipe-windows.c.h passes 1: the creator handle is not counted against
// the instance cap, so exactly one client may attach while the server handle is
// open.
const MAX_OPEN: u32 = 1u32;
const ACCESS_RW: u32 = 0x40000000u32 | 0x80000000u32;
const OPEN_EXISTING: u32 = 1u32;
unsafe extern "C" {
    fn CreateNamedPipeA(
        path: *const u8,
        mode: u32,
        openFlags: u32,
        reservedSize: u32,
        maxPacketLen: u32,
        inputBufSize: u32,
        outputBufSize: u32,
        sa: usize,
    ) -> usize;
    fn CreateFileA(
        path: *const u8,
        access: u32,
        reserved: u32,
        sa: usize,
        openFlags: u32,
        mode: usize,
        volInfo: usize,
    ) -> usize;
    fn ReadFile(handle: usize, buffer: *mut u8, size: u32, done: *mut u32, ov: usize) -> i32;
    fn WriteFile(handle: usize, buffer: *const u8, size: u32, done: *mut u32, ov: usize) -> i32;
    fn CloseHandle(handle: usize) -> i32;
    fn GetLastError() -> u32;
}
/// NUL-terminated ASCII bytes for a Win32 ANSI entry point.
fn cstr(path: &PathBuf) -> io::Result<Vec<u8>> {
    let bytes = path.to_string_lossy();
    let mut out = bytes.as_bytes().to_vec();
    out.push(0);
    Ok(out)
}
fn os_failure() -> io::Error {
    io::Error::last_os_error()
}
/// Attach to an existing pipe instance; absent names fail immediately.
fn open_existing(name: &[u8]) -> usize {
    unsafe {
        CreateFileA(
            name.as_ptr(),
            ACCESS_RW,
            0u32,
            0usize,
            OPEN_EXISTING,
            0usize,
            0usize,
        )
    }
}
/// Resolve an explicit endpoint or the fixed Windows control pipe.
pub fn endpoint(options: &EndpointOptions) -> io::Result<PathBuf> {
    Ok(options
        .socket
        .clone()
        .unwrap_or(PathBuf::from(r"\\.\pipe\obs-rust-control")))
}
/// A pipe handle read as whole messages; `buf` holds the undrained tail.
struct PipeRead {
    handle: usize,
    buf: Vec<u8>,
    start: usize,
    end: usize,
}
impl PipeRead {
    /// Blocking ReadFile of one message; 0 means the writer closed the pipe.
    fn refill(&mut self) -> io::Result<usize> {
        let mut bytes = Vec::new();
        bytes.resize(PIPE_MSG, 0);
        let mut done = [0u32; 1];
        let ok = unsafe {
            ReadFile(
                self.handle,
                bytes.as_mut_ptr(),
                PIPE_MSG32,
                done.as_mut_ptr(),
                0usize,
            )
        };
        if ok == 0 {
            self.start = 0;
            self.end = 0;
            return Ok(0);
        }
        self.buf = bytes;
        self.start = 0;
        self.end = usize::try_from(done[0]).unwrap_or(0);
        Ok(self.end)
    }
}
impl io::Read for PipeRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.start == self.end && self.refill()? == 0 {
            return Ok(0);
        }
        let avail = self.end - self.start;
        let n = avail.min(buffer.len());
        for i in 0..n {
            buffer[i] = self.buf[self.start + i]
        }
        self.start = self.start + n;
        Ok(n)
    }
}
impl Drop for PipeRead {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
/// A pipe handle written as whole messages; each `write` is one message.
struct PipeWrite {
    handle: usize,
}
impl io::Write for PipeWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut done = [0u32; 1];
        let ok = unsafe {
            WriteFile(
                self.handle,
                buffer.as_ptr(),
                u32::try_from(buffer.len()).unwrap_or(0),
                done.as_mut_ptr(),
                0usize,
            )
        };
        if ok == 0 {
            return Err(os_failure());
        }
        Ok(usize::try_from(done[0]).unwrap_or(0))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for PipeWrite {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
pub struct Connection {
    pub reader: Box<dyn io::BufRead + Send>,
    pub writer: Box<dyn io::Write + Send>,
}
/// Attach to the server's pipe instance; unknown names fail immediately.
pub fn connect(options: &EndpointOptions) -> io::Result<Connection> {
    let path = endpoint(options)?;
    let name = cstr(&path)?;
    let handle = open_existing(&name);
    if handle as i64 == -1i64 {
        return Err(os_failure());
    }
    Ok(Connection {
        reader: Box::new(BufReader::new(PipeRead {
            handle,
            buf: Vec::new(),
            start: 0,
            end: 0,
        })),
        writer: Box::new(PipeWrite { handle }),
    })
}
pub struct SocketServer {
    pub handle: usize,
    pub path: PathBuf,
}
impl SocketServer {
    /// Create a private control pipe, refusing names already in use.
    /// # Errors
    /// Fails when the name exists or CreateNamedPipeA fails.
    pub fn bind(options: &EndpointOptions) -> io::Result<Self> {
        let path = endpoint(options)?;
        let name = cstr(&path)?;
        // No pre-probe: an open Existing handle consumes a pipe instance that
        // CloseHandle will not give back, and CreateNamedPipeA refuses a name
        // while instances are attached (231) on its own, so a second bind
        // fails without help.
        let handle = unsafe {
            CreateNamedPipeA(
                name.as_ptr(),
                CREATE_MODE,
                CREATE_FLAGS,
                MAX_OPEN,
                PIPE_MSG32,
                PIPE_MSG32,
                0u32,
                0usize,
            )
        };
        if handle as i64 == -1i64 {
            return Err(os_failure());
        }
        Ok(Self { handle, path })
    }
    /// Serve one client over the pipe until it disconnects or `stop` is set.
    /// # Errors
    /// Returns pipe I/O failures.
    pub fn run(
        self,
        stop: &Arc<AtomicBool>,
        mut factory: impl FnMut() -> Server,
    ) -> io::Result<()> {
        let mut message = Vec::new();
        while !stop.load(Ordering::Acquire) {
            let mut bytes = Vec::new();
            bytes.resize(PIPE_MSG, 0);
            let mut done = [0u32; 1];
            let ok = unsafe {
                ReadFile(
                    self.handle,
                    bytes.as_mut_ptr(),
                    PIPE_MSG32,
                    done.as_mut_ptr(),
                    0usize,
                )
            };
            if ok == 0 {
                // Closing the creator handle removes the pipe name, so later
                // attaches fail deterministically once the client disconnects.
                unsafe {
                    CloseHandle(self.handle);
                }
                return Ok(());
            }
            message.extend_from_slice(&bytes[..usize::try_from(done[0]).unwrap_or(0)]);
            if message.len() > MAX_FRAME {
                return Err(io::Error::other("request exceeds frame limit"));
            }
            while let Some(n) = message.iter().position(|b| *b == b'\n').map(|n| n + 1) {
                let line: Vec<_> = message.drain(..n).collect();
                let mut response = Vec::new();
                if factory().serve(line.as_slice(), &mut response).is_err() {
                    return Err(io::Error::other("server rejected request"));
                }
                let mut sent = [0u32; 1];
                let ok = unsafe {
                    WriteFile(
                        self.handle,
                        response.as_ptr(),
                        u32::try_from(response.len()).unwrap_or(0),
                        sent.as_mut_ptr(),
                        0usize,
                    )
                };
                if ok == 0 {
                    return Err(os_failure());
                }
            }
        }
        Ok(())
    }
}
impl Drop for SocketServer {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
/// Relay one stdin line to the pipe and echo the reply to stdout.
/// # Errors
/// Returns connect or relay failures.
pub fn proxy(options: &EndpointOptions) -> io::Result<()> {
    let path = endpoint(options)?;
    let name = cstr(&path)?;
    let handle = open_existing(&name);
    if handle as i64 == -1i64 {
        return Err(os_failure());
    }
    let mut stdin: Box<dyn io::BufRead + Send> = Box::new(io::BufReader::new(io::stdin()));
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let request = line.as_bytes().to_vec();
        let mut done = [0u32; 1];
        let ok = unsafe {
            WriteFile(
                handle,
                request.as_ptr(),
                u32::try_from(request.len()).unwrap_or(0),
                done.as_mut_ptr(),
                0usize,
            )
        };
        if ok == 0 {
            break;
        }
        let mut reply = Vec::new();
        reply.resize(PIPE_MSG, 0);
        let mut read = [0u32; 1];
        let ok = unsafe {
            ReadFile(
                handle,
                reply.as_mut_ptr(),
                PIPE_MSG32,
                read.as_mut_ptr(),
                0usize,
            )
        };
        if ok == 0 {
            break;
        }
        io::stdout().write_all(&reply[..usize::try_from(read[0]).unwrap_or(0)])?;
    }
    unsafe {
        CloseHandle(handle);
    }
    Ok(())
}
#[must_use]
pub fn default_graphics_module() -> String {
    "libobs-d3d11".to_owned()
}
