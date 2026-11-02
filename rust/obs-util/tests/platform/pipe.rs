//! Tier 1: safe-core tests for `libobs/util/pipe.c`, the POSIX half
//! `libobs/util/pipe-posix.c` and the Windows half `pipe-windows.c`.
//!
//! Each test names the cmocka case in `test/cmocka/test_pipe.c` it mirrors.
//! Raw-pointer C-ABI properties (the argv NULL terminator, null-handle
//! calls) live in `tests/pipe_parity.rs`, like the file-serializer port:
//! Tier 1 only touches the safe API. `os_process_args_destroy(NULL)` has no
//! assertion and nothing to mirror: dropping `Args` is implicit.

use obs_c_oracle as _;
#[cfg(unix)]
use obs_util::pipe::Pipe;
use obs_util::pipe::{Args, windows_command_line};

/// Mirrors `test_args_create` in `test/cmocka/test_pipe.c`.
#[test]
fn args_create_mirrors_test_args_create() {
    let args = Args::new(b"prog");
    assert_eq!(args.argc(), 1);
    assert_eq!(args.arg(0), Some(b"prog".as_slice()));
    assert_eq!(args.arg(1), None);
}

/// Mirrors `test_args_add_arg_order`.
#[test]
fn args_add_arg_order_mirrors_test_args_add_arg_order() {
    let mut args = Args::new(b"prog");
    args.add_arg(b"one");
    args.add_arg(b"two");
    args.add_arg(b"three");

    assert_eq!(args.argc(), 4);
    assert_eq!(args.arg(0), Some(b"prog".as_slice()));
    assert_eq!(args.arg(1), Some(b"one".as_slice()));
    assert_eq!(args.arg(2), Some(b"two".as_slice()));
    assert_eq!(args.arg(3), Some(b"three".as_slice()));
    assert_eq!(args.arg(4), None);
}

/// Mirrors `test_args_add_argf`: the core stores the already-formatted
/// bytes (`"42-abc"`); `vsnprintf` itself stays in `util/pipe-variadic.c`.
#[test]
fn args_add_formatted_mirrors_test_args_add_argf() {
    let mut args = Args::new(b"prog");
    args.add_formatted(b"42-abc");
    args.add_arg(b"after");

    assert_eq!(args.argc(), 3);
    assert_eq!(args.arg(1), Some(b"42-abc".as_slice()));
    assert_eq!(args.arg(2), Some(b"after".as_slice()));
    assert_eq!(args.arg(3), None);
}

/// Mirrors `test_args_verbatim`: spacing, quotes and empty args pass through.
#[test]
fn args_verbatim_mirrors_test_args_verbatim() {
    let mut args = Args::new(b"prog");
    args.add_arg(b"has space");
    args.add_arg(b"say \"hi\" 'there'");
    args.add_arg(b"");

    assert_eq!(args.argc(), 4);
    assert_eq!(args.arg(1), Some(b"has space".as_slice()));
    assert_eq!(args.arg(2), Some(b"say \"hi\" 'there'".as_slice()));
    assert_eq!(args.arg(3), Some(b"".as_slice()));
    assert_eq!(args.arg(4), None);
}

/// `bstrdup` copies with `strlen` semantics, so an embedded NUL truncates.
#[test]
fn args_truncates_at_nul_like_bstrdup() {
    let mut args = Args::new(b"pro\0g");
    args.add_arg(b"a\0b");

    assert_eq!(args.argc(), 2);
    assert_eq!(args.arg(0), Some(b"pro".as_slice()));
    assert_eq!(args.arg(1), Some(b"a".as_slice()));
}

#[cfg(unix)]
mod process {
    use super::*;

    /// Mirrors the `read_all` helper in `test/cmocka/test_pipe.c`: read
    /// until EOF (read returns 0); returns total bytes.
    fn read_all(pipe: &mut Pipe, mut reader: impl FnMut(&mut Pipe, &mut [u8]) -> usize) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 64];
        loop {
            let n = reader(pipe, &mut chunk);
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    /// Mirrors `test_pipe_create_read`.
    #[test]
    fn pipe_create_read_mirrors_test_pipe_create_read() {
        let mut pipe = Pipe::create(Some(b"printf 'hello'"), b"r").expect("spawn sh");
        let out = read_all(&mut pipe, |p, buf| p.read(buf));
        assert_eq!(out, b"hello");
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_exit_code`.
    #[test]
    fn pipe_exit_code_mirrors_test_pipe_exit_code() {
        let mut pipe = Pipe::create(Some(b"exit 3"), b"r").expect("spawn sh");
        let out = read_all(&mut pipe, |p, buf| p.read(buf));
        assert!(out.is_empty());
        assert_eq!(pipe.destroy(), 3);
    }

    /// Mirrors `test_pipe_exit_code_signed_char`: the status is cast through
    /// the platform `char`, so 255 becomes -1 where `char` is signed.
    #[test]
    fn pipe_exit_code_signed_char_mirrors_test_pipe_exit_code_signed_char() {
        let mut pipe = Pipe::create(Some(b"exit 255"), b"r").expect("spawn sh");
        let out = read_all(&mut pipe, |p, buf| p.read(buf));
        assert!(out.is_empty());
        assert_eq!(
            pipe.destroy(),
            255u8 as core::ffi::c_char as core::ffi::c_int
        );
    }

    /// Mirrors `test_pipe_create2_stdout_stderr`.
    #[test]
    fn pipe_create2_stdout_stderr_mirrors_test_pipe_create2_stdout_stderr() {
        let mut args = Args::new(b"/bin/sh");
        args.add_arg(b"-c");
        args.add_arg(b"printf abc; printf err 1>&2");

        let mut pipe = Pipe::create2(Some(&args), b"r").expect("spawn sh");
        let out = read_all(&mut pipe, |p, buf| p.read(buf));
        assert_eq!(out, b"abc");
        let err = read_all(&mut pipe, |p, buf| p.read_err(buf));
        assert_eq!(err, b"err");
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_create2_missing_binary`: a bad path is `None`.
    #[test]
    fn pipe_create2_missing_binary_mirrors_test_pipe_create2_missing_binary() {
        let args = Args::new(b"/nonexistent/obs-test-binary");
        assert!(Pipe::create2(Some(&args), b"r").is_none());
    }

    /// Mirrors `test_pipe_create2` with null args: C dereferences the null;
    /// the core returns `None` instead (listed in `pipe_parity.rs`).
    #[test]
    fn pipe_create2_null_args_is_none() {
        assert!(Pipe::create2(None, b"r").is_none());
    }

    /// Mirrors `test_pipe_write_mode`: full write, reads yield nothing.
    #[test]
    fn pipe_write_mode_mirrors_test_pipe_write_mode() {
        let mut pipe = Pipe::create(Some(b"cat > /dev/null"), b"w").expect("spawn sh");
        assert_eq!(pipe.write(b"some data"), b"some data".len());
        assert_eq!(pipe.read(&mut [0u8; 4]), 0);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_write_on_read_pipe`.
    #[test]
    fn pipe_write_on_read_pipe_mirrors_test_pipe_write_on_read_pipe() {
        let mut pipe = Pipe::create(Some(b"exit 0"), b"r").expect("spawn sh");
        assert_eq!(pipe.write(b"x"), 0);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_type_handling`: only a leading `r` reads.
    #[test]
    fn pipe_type_handling_mirrors_test_pipe_type_handling() {
        let mut pipe = Pipe::create(Some(b"cat > /dev/null"), b"x").expect("spawn sh");
        assert_eq!(pipe.write(b"ab"), 2);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_null_arguments`: a null command is `None`.
    #[test]
    fn pipe_null_command_is_none_mirrors_test_pipe_null_arguments() {
        assert!(Pipe::create(None, b"r").is_none());
    }

    /// A signaled child destroys to its pid, mirroring the `waitpid`
    /// fallthrough in `os_process_pipe_destroy`.
    #[test]
    fn pipe_signaled_child_destroys_to_pid() {
        let pipe = Pipe::create(Some(b"sleep 30"), b"r").expect("spawn sh");
        let pid = pipe.pid();
        // SAFETY: `pid` is our own child, signaled exactly once.
        unsafe {
            assert_eq!(libc::kill(pid as libc::pid_t, libc::SIGKILL), 0);
        }
        assert_eq!(pipe.destroy(), pid as core::ffi::c_int);
    }
}

/// Mirrors `test_pipe_create2_quoting` without spawning: the command line
/// `os_process_pipe_create2` builds on Windows, checked on every OS. The
/// spawned form is in `windows_process` below.
#[test]
fn windows_command_line_mirrors_test_pipe_create2_quoting() {
    let mut args = Args::new(b"cmd");
    for arg in QUOTING_ARGS {
        args.add_arg(arg);
    }
    let mut want = b"cmd /c echo ".to_vec();
    want.extend_from_slice(QUOTING_ECHO);
    assert_eq!(windows_command_line(&args), want);
}

/// The arguments of `test_pipe_create2_quoting` after `cmd`.
const QUOTING_ARGS: [&[u8]; 12] = [
    b"/c",
    b"echo",
    b"plain",
    b"a b",
    b"",
    b"t\tb",
    b"x\"y",
    b"q \"z",
    b"b\\\"c",
    b"dir\\",
    b"s p\\",
    b"C:\\a\\b",
];

/// What `cmd /c echo` prints for them (before its CRLF).
const QUOTING_ECHO: &[u8] =
    b"plain \"a b\" \"\" \"t\tb\" x\\\"y \"q \\\"z\" b\\\\\\\"c dir\\ \"s p\\\\\" C:\\a\\b";

/// Edges of the quoting: lone and empty programs, runs of backslashes
/// before a quote and at the end, and characters that do not quote.
#[test]
fn windows_command_line_edges() {
    let line = |items: &[&[u8]]| {
        let mut args = Args::new(items[0]);
        for item in &items[1..] {
            args.add_arg(item);
        }
        windows_command_line(&args)
    };
    assert_eq!(line(&[b"prog"]), b"prog");
    assert_eq!(line(&[b""]), b"\"\"");
    assert_eq!(line(&[b"p", b""]), b"p \"\"");
    // a\\"b: both backslashes doubled, then the quote escaped
    assert_eq!(line(&[b"p", b"a\\\\\"b"]), b"p a\\\\\\\\\\\"b");
    // trailing backslashes are doubled only before a closing quote
    assert_eq!(line(&[b"p", b"x y\\\\"]), b"p \"x y\\\\\\\\\"");
    assert_eq!(line(&[b"p", b"xy\\\\"]), b"p xy\\\\");
    // backslashes before anything else are kept
    assert_eq!(line(&[b"p", b"a\\\\b c"]), b"p \"a\\\\b c\"");
    // only a space or a tab quotes
    assert_eq!(line(&[b"p", b"a\nb"]), b"p a\nb");
}

/// The Windows process cases of `test/cmocka/test_pipe.c`, with cmd.exe
/// children.
#[cfg(windows)]
mod windows_process {
    use super::*;
    use obs_util::pipe::Pipe;
    use std::sync::{Mutex, MutexGuard};

    /// The cmocka cases run one at a time; so do these. Children inherit
    /// only their standard handles (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, as
    /// in C), so two tests spawning at once no longer hand each other's pipe
    /// ends to their children.
    static SPAWN: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        SPAWN.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn read_all(pipe: &mut Pipe, mut reader: impl FnMut(&mut Pipe, &mut [u8]) -> usize) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 64];
        loop {
            let n = reader(pipe, &mut chunk);
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    fn spawn(cmd: &[u8], type_: &[u8]) -> Pipe {
        Pipe::create(Some(cmd), type_).expect("spawn cmd")
    }

    /// Mirrors `test_pipe_create_read`.
    #[test]
    fn pipe_create_read_mirrors_test_pipe_create_read() {
        let _serial = serial();
        let mut pipe = spawn(b"cmd /c echo hello", b"r");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read(buf)), b"hello\r\n");
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_exit_code`: `(int)DWORD`, no char cast.
    #[test]
    fn pipe_exit_code_mirrors_test_pipe_exit_code() {
        let _serial = serial();
        for (cmd, code) in [
            (&b"cmd /c exit 3"[..], 3),
            (b"cmd /c exit 255", 255),
            (b"cmd /c exit 256", 256),
            (b"cmd /c exit -1", -1),
        ] {
            let mut pipe = spawn(cmd, b"r");
            assert!(read_all(&mut pipe, |p, buf| p.read(buf)).is_empty());
            assert_eq!(pipe.destroy(), code, "{cmd:?}");
        }
    }

    /// Mirrors `test_pipe_stderr`.
    #[test]
    fn pipe_stderr_mirrors_test_pipe_stderr() {
        let _serial = serial();
        let mut pipe = spawn(b"cmd /c (echo out) & (1>&2 echo err) & exit 5", b"r");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read(buf)), b"out\r\n");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read_err(buf)), b"err\r\n");
        assert_eq!(pipe.destroy(), 5);
    }

    /// Mirrors `test_pipe_create2_quoting`.
    #[test]
    fn pipe_create2_quoting_mirrors_test_pipe_create2_quoting() {
        let _serial = serial();
        let mut args = Args::new(b"cmd");
        for arg in QUOTING_ARGS {
            args.add_arg(arg);
        }
        let mut pipe = Pipe::create2(Some(&args), b"r").expect("spawn cmd");
        let mut want = QUOTING_ECHO.to_vec();
        want.extend_from_slice(b"\r\n");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read(buf)), want);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_create2_missing_binary`, plus the error codes C
    /// logs: `ERROR_FILE_NOT_FOUND` and, for an empty command line,
    /// `ERROR_INVALID_PARAMETER`.
    #[test]
    fn pipe_create2_missing_binary_mirrors_test_pipe_create2_missing_binary() {
        let _serial = serial();
        let args = Args::new(b"obs-test-binary-that-does-not-exist");
        assert!(Pipe::create2(Some(&args), b"r").is_none());
        assert!(Pipe::create(Some(b""), b"r").is_none());

        let mut logged = None;
        assert!(Pipe::create2_logged(Some(&args), b"r", |code| logged = Some(code)).is_none());
        assert_eq!(logged, Some(2));
        let mut logged = None;
        assert!(Pipe::create_logged(Some(b""), b"r", |code| logged = Some(code)).is_none());
        assert_eq!(logged, Some(87));
    }

    /// C dereferences null args; the core returns `None` (listed in
    /// `pipe_parity.rs`).
    #[test]
    fn pipe_create2_null_args_is_none() {
        assert!(Pipe::create2(None, b"r").is_none());
    }

    /// Mirrors `test_pipe_write_mode`.
    #[test]
    fn pipe_write_mode_mirrors_test_pipe_write_mode() {
        let _serial = serial();
        let mut pipe = spawn(b"cmd /c more > NUL", b"w");
        assert_eq!(pipe.write(b"some data\r\n"), 11);
        assert_eq!(pipe.read(&mut [0u8; 4]), 0);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_write_on_read_pipe`.
    #[test]
    fn pipe_write_on_read_pipe_mirrors_test_pipe_write_on_read_pipe() {
        let _serial = serial();
        let mut pipe = spawn(b"cmd /c exit 0", b"r");
        assert_eq!(pipe.write(b"x"), 0);
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_type_handling`: only a leading `r` or `w`.
    #[test]
    fn pipe_type_handling_mirrors_test_pipe_type_handling() {
        let _serial = serial();
        assert!(Pipe::create(Some(b"cmd /c exit 0"), b"x").is_none());
        assert!(Pipe::create(Some(b"cmd /c exit 0"), b"").is_none());

        let mut pipe = spawn(b"cmd /c more > NUL", b"wx");
        assert_eq!(pipe.write(b"ab"), 2);
        assert_eq!(pipe.destroy(), 0);

        let mut pipe = spawn(b"cmd /c echo r", b"rw");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read(buf)), b"r\r\n");
        assert_eq!(pipe.destroy(), 0);
    }

    /// Mirrors `test_pipe_null_arguments`: a null command is `None`.
    #[test]
    fn pipe_null_command_is_none_mirrors_test_pipe_null_arguments() {
        assert!(Pipe::create(None, b"r").is_none());
    }

    /// A UTF-8 BOM before the command is dropped, as `utf8_to_wchar` does
    /// on Windows.
    #[test]
    fn pipe_command_line_bom_is_dropped() {
        let _serial = serial();
        let mut pipe = spawn(b"\xef\xbb\xbfcmd /c echo bom", b"r");
        assert_eq!(read_all(&mut pipe, |p, buf| p.read(buf)), b"bom\r\n");
        assert_eq!(pipe.destroy(), 0);
    }
}
