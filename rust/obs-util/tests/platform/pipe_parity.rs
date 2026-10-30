//! Tier 3: the pipe C ABI against the original C, compiled as an oracle.
//!
//! Excluded inputs (C would crash; the shim is null-safe instead):
//! - A null command to `create`, null `args` to `create2`/`add_arg`/
//!   `get_argc`/`get_argv`, and a null `arg`. Covered for the Rust side
//!   only in `c_abi_nulls_are_safe`.
//!
//! Other notes, not exclusions:
//! - `argv` with an interior NUL passed to `create2`. Both sides truncate
//!   the argument at the NUL (`bstrdup`), but the kernel then execs a
//!   different (truncated) program path, so spawn success can differ for
//!   reasons outside the port. Args-list parity (with NULs) is covered in
//!   `args_match_oracle`; spawn parity uses NUL-free argv.
//! - A signaled child. C `destroy` falls through to returning the `waitpid`
//!   pid, and the core returns the same pid (`Child::id`). The two sides
//!   spawn distinct children, so cross-side equality is unassertable;
//!   `signaled_child_destroys_to_pid` pins each side to a positive pid,
//!   and Tier 1 pins the exact pid on the Rust side.
//! - A write to a dead pipe. In a C-main process (libobs) both sides die
//!   on SIGPIPE identically: disposition is a process property, and Rust
//!   `std` only ignores SIGPIPE in processes whose `main` is Rust.
//!   Neither side touches the child's signal mask (`posix_spawn` with no
//!   attributes and `Command` both inherit it). `dead_pipe_write_matches`
//!   pins the short-count path, which is what both sides report wherever
//!   SIGPIPE is ignored (as in this test binary).
//! - Chunk boundaries of partial reads. Both sides return one short read
//!   per call, but scheduling decides the sizes, so only the concatenation
//!   and the return values are compared, never the chunking.
//! - `stderr` of a write-mode pipe. Draining it to EOF would block until
//!   the child exits, but the child waits on stdin, which only closes at
//!   destroy: both sides would deadlock identically, so like the cmocka
//!   test, write mode is compared on write counts and exit codes only.
//! - `add_argf` formatting itself. Both production paths format with
//!   `vsnprintf` (the oracle in C, the Rust side through
//!   `util/pipe-variadic.c`), so the differential test pins the arg-list
//!   result for a table of formats with real varargs on the oracle side.
//! - I/O temp dirs (testing-policy: separate dirs per side) do not apply:
//!   no test touches the filesystem except `/dev/null`.
//!
//! Both `os_process_args` and `os_process_pipe` are opaque in `pipe.h`, so
//! this port adds no `#[repr(C)]` struct and needs no layout test.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;
use std::ffi::{CStr, CString};

use obs_c_oracle as _;
use obs_c_oracle::pipe as oracle;
use obs_util::ffi::pipe as shim;
use obs_util::pipe::Args;
use proptest::prelude::*;

/// Pipe-spawning cases first re-exec this same binary in isolation, so other
/// categories cannot fork during the C oracle's pipe-to-CLOEXEC window.
/// Spawners within each isolated test also hold this lock (#192).
///
/// A child spawned on one thread can keep another test's pipe open:
/// `pipe-posix.c` creates its pipes with `pipe()` and only then sets
/// `FD_CLOEXEC`, so a spawn on another thread in between inherits them for
/// the child's whole life; and any child holds copies of every fd until it
/// execs. Either can keep a dead child's stdin open, and the dead-pipe
/// write then goes through. Serializing spawns keeps the oracle the
/// original C.
#[cfg(unix)]
static SPAWN: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
fn serial() -> std::sync::MutexGuard<'static, ()> {
    SPAWN.lock().unwrap_or_else(|e| e.into_inner())
}

fn cstring(bytes: &[u8]) -> CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).expect("truncated at NUL")
}

/// Build the same argv on both sides; returns `(oracle_args, shim_args)`.
/// `items[0]` is the executable, like `os_process_args_create`.
fn make_args(items: &[Vec<u8>]) -> (*mut oracle::OracleArgs, *mut Args) {
    assert!(!items.is_empty());
    let first = cstring(&items[0]);
    // SAFETY: formatting a live C string; pointers stay alive for the call.
    let oargs = unsafe { oracle::oracle_os_process_args_create(first.as_ptr()) };
    assert!(!oargs.is_null());
    let sargs = unsafe { shim::os_process_args_create(first.as_ptr()) };
    assert!(!sargs.is_null());
    for item in &items[1..] {
        let arg = cstring(item);
        unsafe {
            oracle::oracle_os_process_args_add_arg(oargs, arg.as_ptr());
            shim::os_process_args_add_arg(sargs, arg.as_ptr());
        }
    }
    (oargs, sargs)
}

fn free_args(oargs: *mut oracle::OracleArgs, sargs: *mut Args) {
    // SAFETY: both came from their create function.
    unsafe {
        oracle::oracle_os_process_args_destroy(oargs);
        shim::os_process_args_destroy(sargs);
    }
}

/// `(argc, argv bytes, argv[argc] is null)`.
fn snapshot_args(oargs: *mut oracle::OracleArgs, sargs: *mut Args) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    // SAFETY: both are live; argv stays valid (no mutation during read).
    unsafe {
        let oargv = oracle::oracle_os_process_args_get_argv(oargs);
        let sargv = shim::os_process_args_get_argv(sargs);
        assert!(!oargv.is_null() && !sargv.is_null());
        let oargc = oracle::oracle_os_process_args_get_argc(oargs);
        let sargc = shim::os_process_args_get_argc(sargs);
        assert_eq!(oargc, sargc);
        let mut ovec = Vec::with_capacity(oargc);
        let mut svec = Vec::with_capacity(sargc);
        for i in 0..oargc {
            let op = *oargv.add(i);
            let sp = *sargv.add(i);
            assert!(!op.is_null() && !sp.is_null());
            ovec.push(CStr::from_ptr(op).to_bytes().to_vec());
            svec.push(CStr::from_ptr(sp).to_bytes().to_vec());
        }
        assert!((*oargv.add(oargc)).is_null());
        assert!((*sargv.add(sargc)).is_null());
        (ovec, svec)
    }
}

proptest! {
    /// Random argv (including NULs, which both sides truncate like
    /// `bstrdup`) produce identical arg lists and NULL terminators.
    #[test]
    fn args_match_oracle(argv in proptest::collection::vec(
        proptest::collection::vec(any::<u8>(), 0..24),
        1..8,
    )) {
        let (oargs, sargs) = make_args(&argv);
        let (oargsnap, sargsnap) = snapshot_args(oargs, sargs);
        free_args(oargs, sargs);
        let expected: Vec<Vec<u8>> = argv
            .iter()
            .map(|a| {
                let end = a.iter().position(|&b| b == 0).unwrap_or(a.len());
                a[..end].to_vec()
            })
            .collect();
        prop_assert_eq!(oargsnap, expected.clone());
        prop_assert_eq!(sargsnap, expected);
    }
}

/// `add_argf` with real varargs on the oracle side; the Rust side stores the
/// same formatted bytes (production formats both with `vsnprintf`).
#[test]
fn add_argf_matches_oracle() {
    // (int, string, expected formatted bytes)
    let cases: &[(c_int, &[u8], &[u8])] = &[
        (42, b"abc", b"42-abc"),
        (0, b"", b"0-"),
        (-7, b"x", b"-7-x"),
        (i32::MAX, b"end", b"2147483647-end"),
    ];
    let fmt = c"%d-%s";
    for (num, text, expected) in cases {
        let ctext = cstring(text);
        let oargs = unsafe { oracle::oracle_os_process_args_create(c"/bin/echo".as_ptr()) };
        let sargs = unsafe { shim::os_process_args_create(c"/bin/echo".as_ptr()) };
        assert!(!oargs.is_null() && !sargs.is_null());
        unsafe {
            oracle::oracle_os_process_args_add_argf(oargs, fmt.as_ptr(), *num, ctext.as_ptr());
        }
        unsafe {
            shim::os_process_args_add_arg(sargs, cstring(expected).as_ptr());
        }
        // The core half of add_argf stores pre-formatted bytes; assert it
        // lands in the same argv slot the oracle formatted into.
        let mut fargs = Args::new(b"/bin/echo");
        fargs.add_formatted(expected);
        assert_eq!(fargs.argc(), 2);
        assert_eq!(fargs.arg(1), Some(*expected));
        let (oargsnap, sargsnap) = snapshot_args(oargs, sargs);
        free_args(oargs, sargs);
        assert_eq!(oargsnap.len(), 2);
        assert_eq!(sargsnap, oargsnap);
        assert_eq!(sargsnap[1].as_slice(), *expected);
    }
}

/// Null handles are safe on the Rust side. Dereferencing null `args` or a
/// null command is C UB and excluded; the remaining null cases are
/// null-checked in C too, so they run against the oracle as well.
#[test]
fn c_abi_nulls_are_safe() {
    use core::ptr;
    unsafe {
        assert!(shim::os_process_args_create(ptr::null()).is_null());
        shim::os_process_args_add_arg(ptr::null_mut(), c"x".as_ptr());
        let live = shim::os_process_args_create(c"p".as_ptr());
        assert!(!live.is_null());
        shim::os_process_args_add_arg(live, ptr::null());
        shim::os_process_args_destroy(live);
        assert_eq!(shim::os_process_args_get_argc(ptr::null()), 0);
        assert!(shim::os_process_args_get_argv(ptr::null()).is_null());
        shim::os_process_args_destroy(ptr::null_mut());
        #[cfg(unix)]
        {
            use obs_util::pipe::Pipe;
            let mut scratch = [0u8; 4];
            assert!(shim::os_process_pipe_create(ptr::null(), c"r".as_ptr()).is_null());
            assert!(shim::os_process_pipe_create(c"exit 0".as_ptr(), ptr::null()).is_null());
            assert!(shim::os_process_pipe_create2(ptr::null(), c"r".as_ptr()).is_null());
            assert_eq!(shim::os_process_pipe_destroy(ptr::null_mut()), 0);
            assert_eq!(
                shim::os_process_pipe_read(ptr::null_mut(), scratch.as_mut_ptr(), 4),
                0
            );
            assert_eq!(
                shim::os_process_pipe_read_err(ptr::null_mut(), scratch.as_mut_ptr(), 4),
                0
            );
            assert_eq!(
                shim::os_process_pipe_write(ptr::null_mut(), scratch.as_ptr(), 4),
                0
            );
            assert_eq!(
                oracle::oracle_os_process_pipe_read(ptr::null_mut(), scratch.as_mut_ptr(), 4),
                0
            );
            assert_eq!(
                oracle::oracle_os_process_pipe_read_err(ptr::null_mut(), scratch.as_mut_ptr(), 4),
                0
            );
            assert_eq!(
                oracle::oracle_os_process_pipe_write(ptr::null_mut(), scratch.as_ptr(), 4),
                0
            );
            assert_eq!(oracle::oracle_os_process_pipe_destroy(ptr::null_mut()), 0);
            // A null `data` pointer yields 0 instead of crashing in
            // `fread`/`fwrite`.
            let _serial = serial();
            let mut pipe = Pipe::create(Some(b"exit 0"), b"r").expect("spawn sh");
            assert_eq!(pipe.read(&mut []), 0);
            assert_eq!(pipe.destroy(), 0);
            assert!(Pipe::create(None, b"r").is_none());
        }
    }
}

#[cfg(unix)]
mod process {
    use super::*;
    use obs_c_oracle::pipe::OraclePipe;
    use obs_util::pipe::Pipe as ShimPipe;

    /// Drain one stream to EOF; returns the concatenated bytes.
    fn drain_oracle(
        pp: *mut OraclePipe,
        reader: unsafe extern "C" fn(*mut OraclePipe, *mut u8, usize) -> usize,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 32];
        loop {
            // SAFETY: `pp` is live; `chunk` covers its length.
            let n = unsafe { reader(pp, chunk.as_mut_ptr(), chunk.len()) };
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    fn drain_shim(pipe: &mut ShimPipe, reader: fn(&mut ShimPipe, &mut [u8]) -> usize) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 32];
        loop {
            let n = reader(pipe, &mut chunk);
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    fn spawn_oracle(cmd: &CStr, type_: &CStr) -> *mut OraclePipe {
        // SAFETY: live C strings.
        unsafe { oracle::oracle_os_process_pipe_create(cmd.as_ptr(), type_.as_ptr()) }
    }

    fn spawn_shim(cmd: &[u8], type_: &[u8]) -> Option<ShimPipe> {
        ShimPipe::create(Some(cmd), type_)
    }

    /// Same shell command on both sides; stdout, stderr and the destroy
    /// code must match. Chunk boundaries are deliberately not compared
    /// (scheduling decides them); concatenations are.
    ///
    /// Deadlock rule: `stderr` is drained to EOF only when the child exits
    /// on its own (read mode). A write-mode child waits on stdin, so
    /// draining its `stderr` first would block forever on both sides; like
    /// the cmocka test, write mode writes a payload instead and never
    /// touches `stderr`.
    fn check_command(cmd: &[u8], type_: &[u8]) {
        let cc = cstring(cmd);
        let ct = cstring(type_);
        let read_mode = type_.first().copied() == Some(b'r');
        let opp = spawn_oracle(&cc, &ct);
        assert!(!opp.is_null(), "oracle failed to spawn {cmd:?}");
        let mut spp = spawn_shim(cmd, type_).expect("shim failed to spawn");
        if read_mode {
            let o_out = drain_oracle(opp, oracle::oracle_os_process_pipe_read);
            let s_out = drain_shim(&mut spp, ShimPipe::read);
            let o_err = drain_oracle(opp, oracle::oracle_os_process_pipe_read_err);
            let s_err = drain_shim(&mut spp, ShimPipe::read_err);
            // SAFETY: both pipes are live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            assert_eq!(s_out, o_out, "stdout of {cmd:?}");
            assert_eq!(s_err, o_err, "stderr of {cmd:?}");
            assert_eq!(s_code, o_code, "exit code of {cmd:?}");
        } else {
            let payload = b"parity payload";
            // SAFETY: `opp` is live; `payload` covers its length.
            let o_wrote = unsafe {
                oracle::oracle_os_process_pipe_write(opp, payload.as_ptr(), payload.len())
            };
            let s_wrote = spp.write(payload);
            // SAFETY: both pipes are live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            assert_eq!(s_wrote, o_wrote, "write count of {cmd:?}");
            assert_eq!(s_wrote, payload.len());
            assert_eq!(s_code, o_code, "exit code of {cmd:?}");
        }
    }

    #[test]
    fn process_commands_match_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::process_commands_match_oracle")) {
            return;
        }
        let _serial = serial();
        for (cmd, type_) in [
            (&b"printf 'hello'"[..], &b"r"[..]),
            (b"exit 3", b"r"),
            (b"exit 255", b"r"),
            (b"exit 0", b"r"),
            (b"printf out; printf err 1>&2; exit 5", b"r"),
            (b"cat > /dev/null", b"w"),
            (b"cat > /dev/null", b"x"),
            (b"printf ''", b"r"),
        ] {
            check_command(cmd, type_);
        }
    }

    /// `create2` with a NUL-free argv on both sides.
    #[test]
    fn process_create2_matches_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::process_create2_matches_oracle")) {
            return;
        }
        let _serial = serial();
        let items: &[&[u8]] = &[b"/bin/sh", b"-c", b"printf abc; printf err 1>&2"];
        let owned: Vec<Vec<u8>> = items.iter().map(|s| s.to_vec()).collect();
        let (oargs, sargs) = make_args(&owned);
        // SAFETY: live args and C strings.
        let opp = unsafe { oracle::oracle_os_process_pipe_create2(oargs, c"r".as_ptr()) };
        assert!(!opp.is_null());
        // SAFETY: `sargs` is a live shim args.
        let spp = unsafe { shim::os_process_pipe_create2(sargs, c"r".as_ptr()) };
        assert!(!spp.is_null());
        let o_out = drain_oracle(opp, oracle::oracle_os_process_pipe_read);
        // SAFETY: `spp` is a live shim pipe.
        let (s_out, s_err, s_code) = unsafe {
            let mut buf = [0u8; 64];
            let mut out = Vec::new();
            loop {
                let n = shim::os_process_pipe_read(spp, buf.as_mut_ptr(), buf.len());
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }
            let mut err = Vec::new();
            loop {
                let n = shim::os_process_pipe_read_err(spp, buf.as_mut_ptr(), buf.len());
                if n == 0 {
                    break;
                }
                err.extend_from_slice(&buf[..n]);
            }
            let code = shim::os_process_pipe_destroy(spp);
            (out, err, code)
        };
        let o_err = drain_oracle(opp, oracle::oracle_os_process_pipe_read_err);
        // SAFETY: `opp` is live.
        let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
        free_args(oargs, sargs);
        assert_eq!(s_out, o_out);
        assert_eq!(s_out, b"abc");
        assert_eq!(s_err, o_err);
        assert_eq!(s_err, b"err");
        assert_eq!(s_code, o_code);
    }

    /// A signaled child destroys to its own pid on both sides (the two
    /// children have distinct pids, so cross-side equality is unassertable;
    /// Tier 1 pins the exact pid on the Rust side).
    #[test]
    fn signaled_child_destroys_to_pid() {
        if !crate::common::isolated(concat!(module_path!(), "::signaled_child_destroys_to_pid")) {
            return;
        }
        let _serial = serial();
        for _ in 0..4 {
            let opp = spawn_oracle(&cstring(b"kill -KILL $$"), &cstring(b"r"));
            assert!(!opp.is_null());
            let spp = spawn_shim(b"kill -KILL $$", b"r").expect("spawn");
            // SAFETY: both pipes are live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            assert!(o_code > 0, "oracle signaled code");
            assert!(s_code > 0, "shim signaled code");
        }
    }

    /// obs-rust/obs-studio#192: other threads keep spawning while the
    /// dead-pipe scenario runs, as the other tests in this binary do. Every
    /// spawn holds [`serial`], so none can land between creating the dead
    /// child's pipe and writing to it. Without the lock, 2 to 6 of 3000
    /// writes went through on Linux (see `serial` for why).
    #[test]
    fn dead_pipe_write_survives_concurrent_spawns() {
        if !crate::common::isolated(concat!(
            module_path!(),
            "::dead_pipe_write_survives_concurrent_spawns"
        )) {
            return;
        }
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let stop = Arc::new(AtomicBool::new(false));
        let spawners: Vec<_> = (0..16)
            .map(|_| {
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    let cmd = cstring(b"sleep 0.05");
                    let r = cstring(b"r");
                    while !stop.load(Ordering::Relaxed) {
                        let p = {
                            let _serial = serial();
                            spawn_oracle(&cmd, &r)
                        };
                        if !p.is_null() {
                            // SAFETY: live pipe from spawn_oracle.
                            unsafe { oracle::oracle_os_process_pipe_destroy(p) };
                        }
                    }
                })
            })
            .collect();
        let cmd = cstring(b"echo closed 1>&2; exec 0<&-");
        let w = cstring(b"w");
        let big = vec![0xABu8; 1 << 20];
        let mut leaked = 0;
        for _ in 0..3000 {
            let _serial = serial();
            let opp = spawn_oracle(&cmd, &w);
            assert!(!opp.is_null());
            let err = drain_oracle(opp, oracle::oracle_os_process_pipe_read_err);
            assert_eq!(err, b"closed\n");
            // SAFETY: live pipe; `big` covers its length.
            let wrote =
                unsafe { oracle::oracle_os_process_pipe_write(opp, big.as_ptr(), big.len()) };
            if wrote != 0 {
                leaked += 1;
            }
            // SAFETY: live pipe.
            unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
        }
        stop.store(true, Ordering::Relaxed);
        for t in spawners {
            t.join().unwrap();
        }
        assert_eq!(
            leaked, 0,
            "{leaked} of 3000 writes reached a dead child through a leaked fd"
        );
    }

    /// A write to a dead pipe reports a short count on both sides (this
    /// binary ignores SIGPIPE, like any Rust-main process; in a C-main
    /// process both sides die on SIGPIPE identically). Sync is via `stderr`
    /// EOF, which also proves the child is gone, so the EPIPE is
    /// deterministic with no sleeps.
    #[test]
    fn dead_pipe_write_matches_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::dead_pipe_write_matches_oracle")) {
            return;
        }
        let _serial = serial();
        let cmd = b"echo closed 1>&2; exec 0<&-";
        let opp = spawn_oracle(&cstring(cmd), &cstring(b"w"));
        assert!(!opp.is_null());
        let mut spp = spawn_shim(cmd, b"w").expect("spawn");
        let o_err = drain_oracle(opp, oracle::oracle_os_process_pipe_read_err);
        let s_err = drain_shim(&mut spp, ShimPipe::read_err);
        assert_eq!(o_err, b"closed\n");
        assert_eq!(s_err, b"closed\n");
        let big = vec![0xABu8; 1 << 20];
        // SAFETY: `opp` is live; `big` covers its length.
        let o_wrote = unsafe { oracle::oracle_os_process_pipe_write(opp, big.as_ptr(), big.len()) };
        let s_wrote = spp.write(&big);
        // SAFETY: both pipes are live.
        let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
        let s_code = spp.destroy();
        assert_eq!(s_wrote, o_wrote);
        assert_eq!(s_wrote, 0);
        assert_eq!(s_code, o_code);
    }

    proptest! {
        /// Random payloads through `cat > /dev/null` in write mode: full
        /// short counts and exit codes match.
        fn write_matches_oracle_cases(payload in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let _serial = serial();
            let opp = spawn_oracle(&cstring(b"cat > /dev/null"), &cstring(b"w"));
            prop_assert!(!opp.is_null());
            let mut spp = spawn_shim(b"cat > /dev/null", b"w").expect("spawn");
            // SAFETY: `opp` is live; `payload` covers its length.
            let o_wrote = unsafe {
                oracle::oracle_os_process_pipe_write(opp, payload.as_ptr(), payload.len())
            };
            let s_wrote = spp.write(&payload);
            prop_assert_eq!(s_wrote, o_wrote);
            prop_assert_eq!(s_wrote, payload.len());
            // SAFETY: `opp` is live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            prop_assert_eq!(s_code, o_code);
        }

        /// Random read chunk sizes over a fixed output: concatenations and
        /// codes match regardless of chunking.
        fn read_chunked_matches_oracle_cases(chunk in 1usize..64) {
            let _serial = serial();
            let opp = spawn_oracle(&cstring(b"printf '0123456789abcdef'"), &cstring(b"r"));
            prop_assert!(!opp.is_null());
            let mut spp = spawn_shim(b"printf '0123456789abcdef'", b"r").expect("spawn");
            let mut o_out = Vec::new();
            let mut s_out = Vec::new();
            let mut o_buf = vec![0u8; chunk];
            let mut s_buf = vec![0u8; chunk];
            loop {
                // SAFETY: `opp` is live; buffers cover their lengths.
                let n = unsafe {
                    oracle::oracle_os_process_pipe_read(
                        opp,
                        o_buf.as_mut_ptr(),
                        o_buf.len(),
                    )
                };
                if n == 0 {
                    break;
                }
                o_out.extend_from_slice(&o_buf[..n]);
            }
            loop {
                let n = spp.read(&mut s_buf);
                if n == 0 {
                    break;
                }
                s_out.extend_from_slice(&s_buf[..n]);
            }
            // SAFETY: `opp` is live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            prop_assert_eq!(s_out.as_slice(), o_out.as_slice());
            prop_assert_eq!(s_out, b"0123456789abcdef");
            prop_assert_eq!(s_code, o_code);
        }
    }
    #[test]
    fn write_matches_oracle() {
        if crate::common::isolated(concat!(module_path!(), "::write_matches_oracle")) {
            write_matches_oracle_cases();
        }
    }

    #[test]
    fn read_chunked_matches_oracle() {
        if crate::common::isolated(concat!(module_path!(), "::read_chunked_matches_oracle")) {
            read_chunked_matches_oracle_cases();
        }
    }
}

/// `pipe-windows.c` against the Rust Windows half, with cmd.exe children.
///
/// Notes for this half, not exclusions:
/// - Spawning is serialized (`windows_process::serial`). Children
///   inherit every inheritable handle (`bInheritHandles`, as in C), so
///   concurrent tests could hand each other's pipe ends to their children
///   and keep them open; the cmocka cases also run one at a time. Within a
///   test the two sides spawn one after the other, which is safe: our ends
///   are made non-inheritable before the next spawn.
/// - The handles C leaks when creating a pipe fails half-way are closed on
///   the Rust side. Only the handle count can tell, so it is not compared.
/// - A child's output is in the console code page, the same for both
///   sides, so bytes are compared as they come.
#[cfg(windows)]
mod windows_process {
    use super::*;
    use core::ptr;
    use obs_util::pipe::Pipe as ShimPipe;
    use std::sync::{Mutex, MutexGuard};

    static SPAWN: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        SPAWN.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Drain one stream to EOF through a C ABI reader.
    fn drain<P>(
        pp: *mut P,
        reader: unsafe extern "C" fn(*mut P, *mut u8, usize) -> usize,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 32];
        loop {
            // SAFETY: `pp` is live; `chunk` covers its length.
            let n = unsafe { reader(pp, chunk.as_mut_ptr(), chunk.len()) };
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    /// What one side did with a pipe: `None` if it did not spawn, else
    /// stdout (read mode) or the write count (write mode), stderr, and the
    /// exit code.
    type Outcome = Option<(Vec<u8>, usize, Vec<u8>, c_int)>;

    /// Runs one pipe through the C ABI of either side. Write mode writes a
    /// payload and leaves stderr alone, as on POSIX: draining it first
    /// would wait for a child that waits for stdin.
    unsafe fn run<P>(
        pp: *mut P,
        read_mode: bool,
        read: unsafe extern "C" fn(*mut P, *mut u8, usize) -> usize,
        read_err: unsafe extern "C" fn(*mut P, *mut u8, usize) -> usize,
        write: unsafe extern "C" fn(*mut P, *const u8, usize) -> usize,
        destroy: unsafe extern "C" fn(*mut P) -> c_int,
    ) -> Outcome {
        if pp.is_null() {
            return None;
        }
        let payload = b"parity payload\r\n";
        let (out, wrote, err) = if read_mode {
            (drain(pp, read), 0, drain(pp, read_err))
        } else {
            // SAFETY: `pp` is live; `payload` covers its length.
            let wrote = unsafe { write(pp, payload.as_ptr(), payload.len()) };
            (Vec::new(), wrote, Vec::new())
        };
        // SAFETY: `pp` is live and destroyed once.
        let code = unsafe { destroy(pp) };
        Some((out, wrote, err, code))
    }

    fn oracle_command(cmd: &CStr, type_: &CStr) -> Outcome {
        let read_mode = type_.to_bytes().first() == Some(&b'r');
        // SAFETY: live C strings; the pipe is used and destroyed by `run`.
        unsafe {
            let pp = oracle::oracle_os_process_pipe_create(cmd.as_ptr(), type_.as_ptr());
            run(
                pp,
                read_mode,
                oracle::oracle_os_process_pipe_read,
                oracle::oracle_os_process_pipe_read_err,
                oracle::oracle_os_process_pipe_write,
                oracle::oracle_os_process_pipe_destroy,
            )
        }
    }

    fn shim_command(cmd: &CStr, type_: &CStr) -> Outcome {
        let read_mode = type_.to_bytes().first() == Some(&b'r');
        // SAFETY: live C strings; the pipe is used and destroyed by `run`.
        unsafe {
            let pp = shim::os_process_pipe_create(cmd.as_ptr(), type_.as_ptr());
            run(
                pp,
                read_mode,
                shim::os_process_pipe_read,
                shim::os_process_pipe_read_err,
                shim::os_process_pipe_write,
                shim::os_process_pipe_destroy,
            )
        }
    }

    fn check_command(cmd: &[u8], type_: &[u8]) -> Outcome {
        let (cmd, type_) = (cstring(cmd), cstring(type_));
        let want = oracle_command(&cmd, &type_);
        let got = shim_command(&cmd, &type_);
        assert_eq!(got, want, "{cmd:?} {type_:?}");
        got
    }

    #[test]
    fn process_commands_match_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::process_commands_match_oracle")) {
            return;
        }
        let _serial = serial();
        for (cmd, type_, spawns) in [
            (&b"cmd /c echo hello"[..], &b"r"[..], true),
            (b"cmd /c exit 3", b"r", true),
            (b"cmd /c exit 255", b"r", true),
            (b"cmd /c exit -1", b"r", true),
            (b"cmd /c (echo out) & (1>&2 echo err) & exit 5", b"r", true),
            (b"cmd /c echo", b"r", true),
            (b"cmd /c more > NUL", b"w", true),
            (b"cmd /c more > NUL", b"wx", true),
            (b"cmd /c (1>&2 echo skipped) & more > NUL", b"w", true),
            (b"cmd /c exit 0", b"rw", true),
            (b"\xef\xbb\xbfcmd /c echo bom", b"r", true),
            (b"cmd /c echo caf\xc3\xa9 \xff\xfe end", b"r", true),
            (b"cmd /c exit 0", b"x", false),
            (b"cmd /c exit 0", b"", false),
            (b"", b"r", false),
            (b"\xef\xbb\xbf", b"r", false),
            (b"obs-test-binary-that-does-not-exist", b"r", false),
        ] {
            assert_eq!(check_command(cmd, type_).is_some(), spawns, "{cmd:?}");
        }
    }

    /// `create2` through both C ABIs with the same argv; `None` if either
    /// did not spawn.
    fn create2_outcomes(items: &[Vec<u8>], type_: &CStr) -> (Outcome, Outcome) {
        let (oargs, sargs) = make_args(items);
        let read_mode = type_.to_bytes().first() == Some(&b'r');
        // SAFETY: live args and C strings; each pipe is used and destroyed
        // by `run`.
        let outcomes = unsafe {
            let opp = oracle::oracle_os_process_pipe_create2(oargs, type_.as_ptr());
            let want = run(
                opp,
                read_mode,
                oracle::oracle_os_process_pipe_read,
                oracle::oracle_os_process_pipe_read_err,
                oracle::oracle_os_process_pipe_write,
                oracle::oracle_os_process_pipe_destroy,
            );
            let spp = shim::os_process_pipe_create2(sargs, type_.as_ptr());
            let got = run(
                spp,
                read_mode,
                shim::os_process_pipe_read,
                shim::os_process_pipe_read_err,
                shim::os_process_pipe_write,
                shim::os_process_pipe_destroy,
            );
            (want, got)
        };
        free_args(oargs, sargs);
        outcomes
    }

    #[test]
    fn process_create2_matches_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::process_create2_matches_oracle")) {
            return;
        }
        let _serial = serial();
        let cases: &[(&[&[u8]], bool)] = &[
            (
                &[b"cmd", b"/c", b"echo", b"a b", b"", b"x\"y", b"s p\\"],
                true,
            ),
            (&[b"cmd", b"/c", b"exit", b"9"], true),
            (&[b"cmd", b"/c", b"(1>&2 echo e)"], true),
            (&[b"obs-test-binary-that-does-not-exist", b"x"], false),
            (&[b""], false),
        ];
        for (items, spawns) in cases {
            let owned: Vec<Vec<u8>> = items.iter().map(|s| s.to_vec()).collect();
            let (want, got) = create2_outcomes(&owned, c"r");
            assert_eq!(got, want, "{items:?}");
            assert_eq!(got.is_some(), *spawns, "{items:?}");
        }
    }

    /// Characters that cmd.exe passes through `echo` unchanged, plus the
    /// ones the quoting is about.
    fn echo_arg() -> impl Strategy<Value = Vec<u8>> {
        proptest::collection::vec(
            prop_oneof![
                3 => proptest::sample::select(b"abcXYZ019.-_,:".to_vec()),
                2 => Just(b' '),
                1 => Just(b'\t'),
                2 => Just(b'"'),
                3 => Just(b'\\'),
            ],
            0..10,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(48)))]

        /// Random arguments to `cmd /c echo`: echo prints the command line
        /// create2 built, so both sides must print the same bytes.
        fn create2_quoting_matches_oracle_cases(args in proptest::collection::vec(echo_arg(), 1..5)) {
            let _serial = serial();
            let mut items = vec![b"cmd".to_vec(), b"/c".to_vec(), b"echo".to_vec(), b"=".to_vec()];
            items.extend(args);
            let (want, got) = create2_outcomes(&items, c"r");
            prop_assert!(want.is_some());
            prop_assert_eq!(got, want);
        }

        /// Random payloads through `more > NUL` in write mode.
        fn write_matches_oracle_cases(payload in proptest::collection::vec(0x20u8..0x7f, 0..1024)) {
            let _serial = serial();
            let opp = unsafe {
                oracle::oracle_os_process_pipe_create(c"cmd /c more > NUL".as_ptr(), c"w".as_ptr())
            };
            prop_assert!(!opp.is_null());
            let mut spp = ShimPipe::create(Some(b"cmd /c more > NUL"), b"w").expect("spawn");
            // SAFETY: `opp` is live; `payload` covers its length.
            let o_wrote = unsafe {
                oracle::oracle_os_process_pipe_write(opp, payload.as_ptr(), payload.len())
            };
            let s_wrote = spp.write(&payload);
            // SAFETY: `opp` is live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            let s_code = spp.destroy();
            prop_assert_eq!(s_wrote, o_wrote);
            prop_assert_eq!(s_code, o_code);
        }

        /// Random read sizes over a fixed output: concatenations and codes
        /// match whatever the chunking.
        fn read_chunked_matches_oracle_cases(chunk in 1usize..64) {
            let _serial = serial();
            let cmd = c"cmd /c echo 0123456789abcdef";
            let opp = unsafe { oracle::oracle_os_process_pipe_create(cmd.as_ptr(), c"r".as_ptr()) };
            prop_assert!(!opp.is_null());
            let mut spp = ShimPipe::create(Some(cmd.to_bytes()), b"r").expect("spawn");
            let (mut o_out, mut s_out) = (Vec::new(), Vec::new());
            let mut buf = vec![0u8; chunk];
            loop {
                // SAFETY: `opp` is live; `buf` covers its length.
                let n = unsafe { oracle::oracle_os_process_pipe_read(opp, buf.as_mut_ptr(), buf.len()) };
                if n == 0 {
                    break;
                }
                o_out.extend_from_slice(&buf[..n]);
            }
            loop {
                let n = spp.read(&mut buf);
                if n == 0 {
                    break;
                }
                s_out.extend_from_slice(&buf[..n]);
            }
            // SAFETY: `opp` is live.
            let o_code = unsafe { oracle::oracle_os_process_pipe_destroy(opp) };
            prop_assert_eq!(&s_out, &o_out);
            prop_assert_eq!(s_out, b"0123456789abcdef\r\n");
            prop_assert_eq!(spp.destroy(), o_code);
        }
    }

    /// Writes until the pipe reports broken (0) or ten seconds pass, and
    /// returns the last count. Stderr EOF only proves the child closed its
    /// stderr: Windows closes a dying process's handles one by one, so the
    /// stdin read end can outlive that EOF by a moment and a write in the
    /// gap lands in the pipe buffer and reports its full length (#192).
    /// Spawns are all under `serial()`, so no other process holds a copy
    /// of the read end; waiting out the teardown is enough.
    fn write_until_broken(mut write: impl FnMut() -> usize) -> usize {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let wrote = write();
            if wrote == 0 || std::time::Instant::now() >= deadline {
                return wrote;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// A write after the child exited fails on both sides (the broken pipe
    /// is `ERROR_NO_DATA`). Sync is via stderr EOF, then the write is
    /// retried until the child's stdin is really gone (see
    /// [`write_until_broken`]).
    #[test]
    fn dead_pipe_write_matches_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::dead_pipe_write_matches_oracle")) {
            return;
        }
        let _serial = serial();
        let cmd = c"cmd /c (1>&2 echo closed) & exit 7";
        // SAFETY: live C strings; each pipe is used and destroyed once.
        unsafe {
            let opp = oracle::oracle_os_process_pipe_create(cmd.as_ptr(), c"w".as_ptr());
            assert!(!opp.is_null());
            assert_eq!(
                drain(opp, oracle::oracle_os_process_pipe_read_err),
                b"closed\r\n"
            );
            let spp = shim::os_process_pipe_create(cmd.as_ptr(), c"w".as_ptr());
            assert!(!spp.is_null());
            assert_eq!(drain(spp, shim::os_process_pipe_read_err), b"closed\r\n");

            let data = [0xabu8; 64];
            let o_wrote = write_until_broken(|| {
                oracle::oracle_os_process_pipe_write(opp, data.as_ptr(), data.len())
            });
            let s_wrote =
                write_until_broken(|| shim::os_process_pipe_write(spp, data.as_ptr(), data.len()));
            assert_eq!(s_wrote, o_wrote);
            assert_eq!(s_wrote, 0);
            let o_code = oracle::oracle_os_process_pipe_destroy(opp);
            assert_eq!(shim::os_process_pipe_destroy(spp), o_code);
            assert_eq!(o_code, 7);
        }
    }

    /// The null cases that C checks too, on both sides, and a null `data`
    /// on the Rust side (C passes it to `ReadFile`/`WriteFile`, which fail).
    #[test]
    fn nulls_match_oracle() {
        if !crate::common::isolated(concat!(module_path!(), "::nulls_match_oracle")) {
            return;
        }
        let mut scratch = [0u8; 4];
        // SAFETY: only null handles and live C strings are passed.
        unsafe {
            assert!(shim::os_process_pipe_create(ptr::null(), c"r".as_ptr()).is_null());
            assert!(oracle::oracle_os_process_pipe_create(ptr::null(), c"r".as_ptr()).is_null());
            assert!(shim::os_process_pipe_create(c"cmd /c exit 0".as_ptr(), ptr::null()).is_null());
            assert!(
                oracle::oracle_os_process_pipe_create(c"cmd /c exit 0".as_ptr(), ptr::null())
                    .is_null()
            );
            assert!(shim::os_process_pipe_create2(ptr::null(), c"r".as_ptr()).is_null());
            assert_eq!(shim::os_process_pipe_destroy(ptr::null_mut()), 0);
            assert_eq!(oracle::oracle_os_process_pipe_destroy(ptr::null_mut()), 0);
            for read in [shim::os_process_pipe_read, shim::os_process_pipe_read_err] {
                assert_eq!(read(ptr::null_mut(), scratch.as_mut_ptr(), 4), 0);
            }
            for read in [
                oracle::oracle_os_process_pipe_read,
                oracle::oracle_os_process_pipe_read_err,
            ] {
                assert_eq!(read(ptr::null_mut(), scratch.as_mut_ptr(), 4), 0);
            }
            assert_eq!(
                shim::os_process_pipe_write(ptr::null_mut(), scratch.as_ptr(), 4),
                0
            );
            assert_eq!(
                oracle::oracle_os_process_pipe_write(ptr::null_mut(), scratch.as_ptr(), 4),
                0
            );
        }

        let _serial = serial();
        // SAFETY: live C strings; the pipe is destroyed once.
        unsafe {
            let spp = shim::os_process_pipe_create(c"cmd /c exit 0".as_ptr(), c"r".as_ptr());
            assert!(!spp.is_null());
            assert_eq!(shim::os_process_pipe_read(spp, ptr::null_mut(), 4), 0);
            assert_eq!(shim::os_process_pipe_destroy(spp), 0);
        }
    }
    #[test]
    fn create2_quoting_matches_oracle() {
        if crate::common::isolated(concat!(module_path!(), "::create2_quoting_matches_oracle")) {
            create2_quoting_matches_oracle_cases();
        }
    }

    #[test]
    fn write_matches_oracle() {
        if crate::common::isolated(concat!(module_path!(), "::write_matches_oracle")) {
            write_matches_oracle_cases();
        }
    }

    #[test]
    fn read_chunked_matches_oracle() {
        if crate::common::isolated(concat!(module_path!(), "::read_chunked_matches_oracle")) {
            read_chunked_matches_oracle_cases();
        }
    }
}
