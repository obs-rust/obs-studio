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
    // actions. Like the C, it closes nothing else: the child inherits every
    // other fd of the process that lacks FD_CLOEXEC, so callers must open
    // what they hold with O_CLOEXEC (obsproject/obs-studio#14054).
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
