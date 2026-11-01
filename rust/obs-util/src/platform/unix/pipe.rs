//! POSIX half of the pipe port: `libobs/util/pipe-posix.c`. The
//! platform-neutral parts (`Args`, quoting) are in [`crate::pipe`].

use core::ffi::{c_char, c_int};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};

use crate::pipe::{Args, truncated};

/// A spawned child with its stdio pipes. Mirrors `struct os_process_pipe`
/// on POSIX: `read_mode` selects `stdout` vs `stdin`, and `stderr` is always
/// captured.
pub struct Pipe {
    child: std::process::Child,
    /// `Some` in read mode (`stdout`); `None` in write mode. The presence
    /// of the streams encodes the C `read_pipe` flag.
    reader: Option<std::process::ChildStdout>,
    writer: Option<std::process::ChildStdin>,
    err: Option<std::process::ChildStderr>,
}

fn c_string(bytes: &[u8]) -> Option<CString> {
    CString::new(truncated(bytes)).ok()
}

fn spawn(program: &CString, argv0: &CString, rest: &[&CString], read_mode: bool) -> Option<Pipe> {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(std::ffi::OsStr::from_bytes(program.as_bytes()));
    cmd.arg0(std::ffi::OsStr::from_bytes(argv0.as_bytes()));
    for arg in rest {
        cmd.arg(std::ffi::OsStr::from_bytes(arg.as_bytes()));
    }
    // Rust sets CLOEXEC on the pipe fds it creates and closes the unused
    // ends in the child, matching the C `fcntl` + `posix_spawn` file
    // actions. Every other fd of the process is closed in the child too,
    // as in C, so it inherits none that lacks FD_CLOEXEC
    // (obsproject/obs-studio#4596).
    keep_only_stdio(&mut cmd);
    if read_mode {
        cmd.stdout(Stdio::piped());
    } else {
        cmd.stdin(Stdio::piped());
    }
    // The C always captures stderr, in both modes.
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let (reader, writer) = if read_mode {
        (child.stdout.take(), None)
    } else {
        (None, child.stdin.take())
    };
    let err = child.stderr.take();
    Some(Pipe {
        child,
        reader,
        writer,
        err,
    })
}

/// Gives the child only its standard streams, matching C's
/// `limit_inherited_fds`: an fd the parent opened without CLOEXEC (a socket,
/// the log file) would otherwise stay open in the child while it runs, so a
/// socket the parent closed stays bound. Marking every fd above stderr
/// CLOEXEC, rather than closing it, keeps std's own exec-error pipe working.
#[allow(unsafe_code)] // pre_exec: the hook runs between fork and exec
fn keep_only_stdio(cmd: &mut std::process::Command) {
    let limit = fd_limit();
    // SAFETY: the hook only makes async-signal-safe calls (close_range,
    // open, getdents64, fcntl, close) and allocates nothing.
    unsafe {
        cmd.pre_exec(move || {
            #[cfg(target_os = "linux")]
            if libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) == 0
                || cloexec_proc_fds()
            {
                return Ok(());
            }
            // Linux without close_range (before 5.11) or /proc, and macOS.
            for fd in 3..limit {
                set_cloexec(fd);
            }
            Ok(())
        });
    }
}

/// One past the highest fd that can be open: the kernel refuses an fd at or
/// above the soft `RLIMIT_NOFILE` when it hands one out. Computed before the
/// fork, where these calls need not be async-signal-safe.
#[allow(unsafe_code)]
fn fd_limit() -> c_int {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `lim` is a valid out pointer.
    let soft = if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) } == 0
        && lim.rlim_cur != libc::RLIM_INFINITY
    {
        libc::c_long::try_from(lim.rlim_cur).unwrap_or(libc::c_long::MAX)
    } else {
        0
    };
    // SAFETY: plain sysconf query.
    let open_max = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    match soft.max(open_max) {
        n if n <= 0 => 1024,
        n => c_int::try_from(n).unwrap_or(c_int::MAX),
    }
}

/// Marks `fd` CLOEXEC if it is open without it. Async-signal-safe.
#[allow(unsafe_code)]
fn set_cloexec(fd: c_int) {
    // SAFETY: fcntl on any fd number is defined (EBADF when not open).
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags != -1 && flags & libc::FD_CLOEXEC == 0 {
            libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
        }
    }
}

/// Marks every open fd above stderr CLOEXEC, listing them from
/// `/proc/self/fd` with raw `getdents64` into a stack buffer so the forked
/// child allocates nothing. `false` when `/proc` cannot be read.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn cloexec_proc_fds() -> bool {
    // SAFETY: a NUL-terminated path; the fd is closed below.
    let dir = unsafe {
        libc::open(
            c"/proc/self/fd".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if dir < 0 {
        return false;
    }
    // `struct linux_dirent64`: d_ino (8), d_off (8), d_reclen (2), d_type
    // (1), then the NUL-terminated name.
    let mut buf = [0u8; 1024];
    let ok = loop {
        // SAFETY: `buf` is writable for its length.
        let n = unsafe { libc::syscall(libc::SYS_getdents64, dir, buf.as_mut_ptr(), buf.len()) };
        if n == 0 {
            break true;
        }
        let Ok(n) = usize::try_from(n) else {
            break false;
        };
        let mut off = 0;
        while off + 19 <= n {
            let reclen = usize::from(u16::from_ne_bytes([buf[off + 16], buf[off + 17]]));
            if reclen == 0 {
                break;
            }
            let name = &buf[off + 19..(off + reclen).min(n)];
            let mut fd: c_int = 0;
            let mut digits = 0;
            for &b in name.iter().take_while(|&&b| b != 0) {
                if !b.is_ascii_digit() {
                    digits = 0;
                    break;
                }
                fd = fd.saturating_mul(10).saturating_add(c_int::from(b - b'0'));
                digits += 1;
            }
            if digits > 0 && fd > 2 && fd != dir {
                set_cloexec(fd);
            }
            off += reclen;
        }
    };
    // SAFETY: `dir` was opened above and is closed once.
    unsafe {
        libc::close(dir);
    }
    ok
}

impl Pipe {
    /// Only a leading `r` selects read mode; anything else (including empty)
    /// is write mode, exactly like `*type == 'r'`.
    fn read_mode(type_: &[u8]) -> bool {
        type_.first().copied() == Some(b'r')
    }

    /// Mirrors `os_process_pipe_create`: runs `/bin/sh -c cmd_line`.
    /// A null command is `None`.
    pub fn create(cmd_line: Option<&[u8]>, type_: &[u8]) -> Option<Self> {
        let cmd = c_string(cmd_line?)?;
        let shell = c_string(b"/bin/sh")?;
        let sh_argv0 = c_string(b"sh")?;
        let dash_c = c_string(b"-c")?;
        spawn(&shell, &sh_argv0, &[&dash_c, &cmd], Self::read_mode(type_))
    }

    /// Mirrors `os_process_pipe_create2`: spawns `args` directly, with
    /// `argv[0]` as the program. A null `args` is `None`; C would
    /// dereference it.
    pub fn create2(args: Option<&Args>, type_: &[u8]) -> Option<Self> {
        let args = args?;
        if args.args.is_empty() {
            return None;
        }
        // C passes argv[0] to `posix_spawn` as a path, so a name without a
        // slash names a file in the current directory. `Command` would
        // search PATH for it instead; `./` keeps the C lookup. argv[0]
        // itself is passed through unchanged. One difference remains for
        // every path: `Command` spawns with `posix_spawnp`, which on macOS
        // runs an executable without a `#!` line through /bin/sh, where C's
        // `posix_spawn` fails with ENOEXEC.
        let argv0 = &args.args[0];
        let prefixed;
        let program = if argv0.as_bytes().contains(&b'/') {
            argv0
        } else {
            prefixed = c_string(&[b"./", argv0.as_bytes()].concat())?;
            &prefixed
        };
        spawn(
            program,
            argv0,
            &args.args[1..].iter().collect::<Vec<_>>(),
            Self::read_mode(type_),
        )
    }

    /// Closes the streams, reaps the child, and returns its exit status with
    /// the C `(int)(char)` cast applied, mirroring
    /// `os_process_pipe_destroy`. A signaled child has no status code; C
    /// falls through to returning the `waitpid` pid, so this returns the
    /// child's pid too.
    pub fn destroy(self) -> c_int {
        let Pipe {
            mut child,
            reader,
            writer,
            err,
        } = self;
        let pid = child.id();
        // fclose order in C is `file` then `err_file`, both before waitpid.
        drop(reader);
        drop(writer);
        drop(err);
        match child.wait() {
            Ok(status) => match status.code() {
                Some(code) => code as c_char as c_int,
                None => {
                    debug_assert!(status.signal().is_some());
                    pid as c_int
                }
            },
            Err(_) => -1,
        }
    }

    /// The child's pid, for tests that signal it. Mirrors the `pid` field
    /// of `struct os_process_pipe`.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Read until the buffer is full, EOF, or an error, matching `fread` in
    /// `os_process_pipe_read` across short kernel reads.
    /// Zero for a write-mode pipe.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let Some(reader) = self.reader.as_mut() else {
            return 0;
        };
        read_stream(reader, buf)
    }

    /// One read from the child's stderr, mirroring
    /// `os_process_pipe_read_err`. Unlike [`Pipe::read`] this works on a
    /// write-mode pipe too.
    pub fn read_err(&mut self, buf: &mut [u8]) -> usize {
        let Some(err) = self.err.as_mut() else {
            return 0;
        };
        read_stream(err, buf)
    }

    /// Write until `buf` is consumed or the pipe errors, mirroring the one
    /// `fwrite` in `os_process_pipe_write`, which also writes until done or
    /// an error and returns the bytes the kernel took. Zero for a read-mode
    /// pipe.
    /// A write to a closed reader surfaces as a short count here; in a
    /// C-main process (libobs) both sides die on SIGPIPE identically, since
    /// signal disposition is a process property (Rust `std` only ignores
    /// SIGPIPE in processes whose `main` is Rust). Neither side touches the
    /// child's signal mask: `posix_spawn` with no attributes and
    /// `Command` both inherit it.
    pub fn write(&mut self, buf: &[u8]) -> usize {
        use std::io::Write;
        let Some(writer) = self.writer.as_mut() else {
            return 0;
        };
        let mut done = 0;
        while done < buf.len() {
            match writer.write(&buf[done..]) {
                Ok(0) => break,
                Ok(n) => done += n,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        done
    }
}

/// `fread(..., 1, len, ...)` retries short reads until EOF or failure.
fn read_stream(reader: &mut impl std::io::Read, buf: &mut [u8]) -> usize {
    let mut done = 0;
    while done < buf.len() {
        match reader.read(&mut buf[done..]) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use super::Pipe;

    #[test]
    fn read_fills_requested_buffer_across_pipe_chunks() {
        let mut pipe = Pipe::create(Some(b"head -c 131072 /dev/zero"), b"r").unwrap();
        let mut buf = vec![1; 131072];
        let got = pipe.read(&mut buf);
        let _ = pipe.destroy();
        assert_eq!(got, buf.len());
        assert!(buf.iter().all(|&byte| byte == 0));
    }

    #[test]
    fn read_err_fills_requested_buffer_across_pipe_chunks() {
        let mut pipe = Pipe::create(Some(b"head -c 131072 /dev/zero >&2"), b"r").unwrap();
        let mut buf = vec![1; 131072];
        let got = pipe.read_err(&mut buf);
        let _ = pipe.destroy();
        assert_eq!(got, buf.len());
        assert!(buf.iter().all(|&byte| byte == 0));
    }
}
