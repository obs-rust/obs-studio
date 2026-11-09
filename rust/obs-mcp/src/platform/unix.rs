//! Unix sockets and descriptor relay; no host code escapes this module.
#![allow(
    unsafe_code,
    reason = "Unix owner checks and stdin polling require libc"
)]
use super::{EndpointOptions, endpoint};
use crate::server::Server;
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
const MAX_FRAME: usize = 16 * 1024 * 1024;
pub struct Connection {
    pub reader: Box<dyn BufRead + Send>,
    pub writer: Box<dyn Write + Send>,
}
fn owned(meta: &fs::Metadata) -> io::Result<()> {
    // SAFETY: geteuid has no preconditions.
    if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "endpoint must be owned by this user and private",
        ));
    }
    Ok(())
}
fn validate(path: &std::path::Path) -> io::Result<fs::Metadata> {
    let meta = fs::symlink_metadata(path)?;
    owned(&meta)?;
    if !meta.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "endpoint is not a socket",
        ));
    }
    Ok(meta)
}
/// Connect without starting another OBS instance.
/// # Errors
/// Rejects unsafe endpoints and connection failures.
pub fn connect(options: &EndpointOptions) -> io::Result<Connection> {
    let path = endpoint(options)?;
    validate(&path)?;
    let stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(310)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(Connection {
        reader: Box::new(BufReader::new(stream.try_clone()?)),
        writer: Box::new(stream),
    })
}
pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
}
impl SocketServer {
    /// Bind a private socket, refusing active endpoints and non-sockets.
    /// # Errors
    /// Fails for unsafe permissions, active endpoints or filesystem errors.
    pub fn bind(options: &EndpointOptions) -> io::Result<Self> {
        let path = endpoint(options)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("socket needs a parent"))?;
        match fs::DirBuilder::new().mode(0o700).create(parent) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
        let meta = fs::symlink_metadata(parent)?;
        owned(&meta)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(io::Error::other("socket parent is not a directory"));
        }
        if fs::symlink_metadata(&path).is_ok() {
            validate(&path)?;
            match UnixStream::connect(&path) {
                Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => fs::remove_file(&path)?,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "control socket already active",
                    ));
                }
            }
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let meta = fs::symlink_metadata(&path)?;
        Ok(Self {
            listener,
            path,
            identity: (meta.dev(), meta.ino()),
        })
    }
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    /// Multiplex clients on the owning backend thread. Rendering stays on OBS threads.
    /// # Errors
    /// Returns listener failures; disconnected clients are isolated.
    pub fn run(
        self,
        stop: &Arc<AtomicBool>,
        mut factory: impl FnMut() -> Server,
    ) -> io::Result<()> {
        struct Peer {
            stream: UnixStream,
            buffer: Vec<u8>,
            server: Server,
        }
        let mut peers: Vec<Peer> = Vec::new();
        while !stop.load(Ordering::Acquire) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if peers.len() < 32 {
                        stream.set_nonblocking(true)?;
                        peers.push(Peer {
                            stream,
                            buffer: Vec::new(),
                            server: factory(),
                        });
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (),
                Err(e) => return Err(e),
            }
            peers.retain_mut(|peer| {
                let mut bytes = [0u8; 8192];
                let mut eof = false;
                loop {
                    match peer.stream.read(&mut bytes) {
                        Ok(0) => {
                            eof = true;
                            break;
                        }
                        Ok(n) => {
                            peer.buffer.extend_from_slice(&bytes[..n]);
                            if peer.buffer.len() > MAX_FRAME {
                                return false;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => return false,
                    }
                }
                while let Some(n) = peer
                    .buffer
                    .iter()
                    .position(|b| *b == b'\n')
                    .map(|n| n + 1)
                    .or_else(|| (eof && !peer.buffer.is_empty()).then_some(peer.buffer.len()))
                {
                    let line: Vec<_> = peer.buffer.drain(..n).collect();
                    let mut response = Vec::new();
                    if peer.server.serve(line.as_slice(), &mut response).is_err() {
                        return false;
                    }
                    if peer.stream.set_nonblocking(false).is_err() {
                        return false;
                    }
                    let result = peer
                        .stream
                        .set_write_timeout(Some(Duration::from_secs(1)))
                        .and_then(|()| peer.stream.write_all(&response));
                    if result.is_err() || peer.stream.set_nonblocking(true).is_err() {
                        return false;
                    }
                }
                !eof
            });
            std::thread::park_timeout(Duration::from_millis(2));
        }
        Ok(())
    }
}
impl Drop for SocketServer {
    fn drop(&mut self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path)
            && (meta.dev(), meta.ino()) == self.identity
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
/// Relay stdio bytes to an existing instance, preserving newline framing.
/// # Errors
/// Returns connect or relay failures.
pub fn proxy(options: &EndpointOptions) -> io::Result<()> {
    let path = endpoint(options)?;
    validate(&path)?;
    let mut stream = UnixStream::connect(path)?;
    let mut stdin_open = true;
    let mut stdout = io::stdout().lock();
    loop {
        let mut fds = [
            libc::pollfd {
                fd: if stdin_open { 0 } else { -1 },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stream.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: fds is initialized and lives across poll.
        if unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) } < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        let mut bytes = [0u8; 8192];
        if fds[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            // SAFETY: bytes has the supplied writable capacity; fd 0 is stdin.
            let n = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            if n == 0 {
                stdin_open = false;
                stream.shutdown(std::net::Shutdown::Write)?;
            } else {
                stream.write_all(&bytes[..usize::try_from(n).map_err(io::Error::other)?])?;
            }
        }
        if fds[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let n = stream.read(&mut bytes)?;
            if n == 0 {
                return Ok(());
            }
            stdout.write_all(&bytes[..n])?;
            stdout.flush()?;
        }
    }
}
#[must_use]
pub fn default_graphics_module() -> String {
    "libobs-opengl".to_owned()
}
