//! Linux: signals through kill(2), liveness from `/proc`, and an inotify
//! `IN_OPEN` watch on the device node on its own thread (never the UI or
//! render thread).

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The user OBS runs as.
pub fn current_uid() -> Option<u32> {
    // SAFETY: getuid(2) cannot fail and touches no memory.
    Some(unsafe { libc::getuid() })
}

/// The owner of a `/proc/<pid>` entry, which is the process's user.
pub fn owner_uid(path: &Path) -> Option<u32> {
    std::fs::metadata(path).ok().map(|m| m.uid())
}

/// The real processes, under `/proc`.
#[derive(Debug, Default)]
pub struct ProcessSignals;

fn send(pid: u32, signal: libc::c_int) -> bool {
    // PID 0 and negative PIDs signal process groups: never.
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill(2) takes plain integers.
    unsafe { libc::kill(pid, signal) == 0 }
}

impl crate::Signals for ProcessSignals {
    fn terminate(&mut self, pid: u32) -> bool {
        send(pid, libc::SIGTERM)
    }

    fn kill(&mut self, pid: u32) -> bool {
        send(pid, libc::SIGKILL)
    }

    fn alive(&mut self, pid: u32) -> bool {
        crate::is_alive(Path::new("/proc"), pid)
    }

    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

/// An inotify `IN_OPEN` watch on one path. Dropping it stops and joins its
/// thread.
#[derive(Debug)]
pub struct Watcher {
    stop: Option<OwnedFd>,
    thread: Option<JoinHandle<()>>,
}

fn last_error<T>() -> io::Result<T> {
    Err(io::Error::last_os_error())
}

impl Watcher {
    /// Calls `on_open` on the watcher thread `settle_ms` after the first of
    /// a burst of opens of `device`.
    pub fn start(
        device: &Path,
        settle_ms: u64,
        mut on_open: Box<dyn FnMut() + Send>,
    ) -> io::Result<Self> {
        let path = CString::new(device.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;

        // SAFETY: inotify_init1(2) takes flags only.
        let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if raw < 0 {
            return last_error();
        }
        // SAFETY: a new descriptor that nothing else owns.
        let inotify = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: a valid descriptor and a NUL-terminated path.
        if unsafe { libc::inotify_add_watch(inotify.as_raw_fd(), path.as_ptr(), libc::IN_OPEN) } < 0
        {
            return last_error();
        }

        let mut fds = [0 as libc::c_int; 2];
        // SAFETY: fds has room for the two descriptors pipe2(2) writes.
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
            return last_error();
        }
        // SAFETY: two new descriptors that nothing else owns.
        let (stop_read, stop_write) =
            unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };

        let settle = Duration::from_millis(settle_ms);
        let thread = std::thread::Builder::new()
            .name("camera-busy-watch".into())
            .spawn(move || run(&inotify, &stop_read, settle, &mut *on_open))?;
        Ok(Self {
            stop: Some(stop_write),
            thread: Some(thread),
        })
    }
}

/// The watcher thread: wait for opens or the stop pipe; report a burst of
/// opens once, `settle` after its first.
fn run(inotify: &OwnedFd, stop: &OwnedFd, settle: Duration, on_open: &mut dyn FnMut()) {
    let mut due: Option<Instant> = None;
    loop {
        let timeout = due.map_or(-1, |at| {
            let left = at.saturating_duration_since(Instant::now()).as_millis();
            libc::c_int::try_from(left).unwrap_or(libc::c_int::MAX)
        });
        let mut fds = [
            libc::pollfd {
                fd: inotify.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stop.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two pollfd entries that live across the call.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) };
        if n < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        // The stop pipe: readable (a byte) or hung up (the write end closed).
        if fds[1].revents != 0 {
            return;
        }
        if fds[0].revents & libc::POLLIN != 0 {
            drain(inotify);
            due.get_or_insert_with(|| Instant::now() + settle);
        } else if fds[0].revents != 0 {
            return;
        }
        if due.is_some_and(|at| Instant::now() >= at) {
            due = None;
            on_open();
        }
    }
}

/// Reads every queued inotify event; only their arrival matters.
fn drain(inotify: &OwnedFd) {
    let mut buf = [0u8; 4096];
    loop {
        // SAFETY: buf is writable for its length.
        let n = unsafe { libc::read(inotify.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n <= 0 {
            return;
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // Closing the write end hangs up the pipe, which ends the thread.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
